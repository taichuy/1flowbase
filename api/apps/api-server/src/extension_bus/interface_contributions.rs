mod provider_operations;
mod network;
mod plugins;
mod mcp;
mod system;

use std::{collections::BTreeSet, sync::Arc};

use interface_runtime::{
    AuthorizationOperation, CompiledInterfaceRegistry, GraphFingerprint, InterfaceId,
    InterfaceOwner, RegistryCompilationError, RegistryCompiler,
};

use crate::console_operation_compilation::{
    compile_console_operation_snapshot, migration_contributions_from_plan,
    policy_contributions_from_inventory, CompiledConsoleOperationSnapshot, ConsoleBindingOwnerKind,
    ConsoleBindingOwnershipContribution,
};

#[derive(Clone)]
pub(crate) struct InterfaceRegistryContribution {
    contribution_id: String,
    authorization_operations: Vec<String>,
    owners: Vec<String>,
    registry: Arc<CompiledInterfaceRegistry>,
}

impl InterfaceRegistryContribution {
    pub(crate) fn new(
        contribution_id: &'static str,
        authorization_operations: &'static [&'static str],
        owners: &'static [&'static str],
        registry: Arc<CompiledInterfaceRegistry>,
    ) -> Self {
        Self {
            contribution_id: contribution_id.into(),
            authorization_operations: authorization_operations
                .iter()
                .map(|s| s.to_string())
                .collect(),
            owners: owners.iter().map(|s| s.to_string()).collect(),
            registry,
        }
    }

    pub(crate) fn managed(
        owner: String,
        operations: Vec<String>,
        registry: Arc<CompiledInterfaceRegistry>,
    ) -> Self {
        Self {
            contribution_id: format!("managed-service.{owner}"),
            authorization_operations: operations,
            owners: vec![owner],
            registry,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_test_contribution_id(mut self, contribution_id: &'static str) -> Self {
        self.contribution_id = contribution_id.into();
        self
    }

    fn console_binding_contributions(&self) -> Vec<ConsoleBindingOwnershipContribution> {
        console_bindings_from_registry(
            &self.contribution_id,
            ConsoleBindingOwnerKind::Family,
            self.registry.as_ref(),
        )
    }
}

pub(crate) struct InterfaceContributionCollector {
    managed_factory: Option<Arc<dyn interface_runtime::ManagedInterfaceInvocationFactory>>,
    graph_fingerprint: GraphFingerprint,
    contributions: Vec<InterfaceRegistryContribution>,
    contribution_ids: BTreeSet<String>,
}

impl InterfaceContributionCollector {
    pub(crate) fn new(graph_fingerprint: GraphFingerprint) -> Self {
        Self {
            managed_factory: None,
            graph_fingerprint,
            contributions: Vec::new(),
            contribution_ids: BTreeSet::new(),
        }
    }

    pub(crate) fn add(
        &mut self,
        contribution: InterfaceRegistryContribution,
    ) -> anyhow::Result<()> {
        if !self
            .contribution_ids
            .insert(contribution.contribution_id.clone())
        {
            anyhow::bail!(
                "duplicate interface registry contribution `{}`",
                contribution.contribution_id
            );
        }
        self.contributions.push(contribution);
        Ok(())
    }

    pub(crate) fn compile(self) -> anyhow::Result<Arc<CompiledInterfaceRegistry>> {
        let mut operations = BTreeSet::new();
        let mut owners = BTreeSet::new();
        for contribution in &self.contributions {
            for operation in &contribution.authorization_operations {
                operations.insert(AuthorizationOperation::new(operation)?);
            }
            for owner in &contribution.owners {
                owners.insert(InterfaceOwner::new(owner)?);
            }
        }

        let mut compiler = RegistryCompiler::new(self.graph_fingerprint, operations, owners);
        if let Some(factory) = self.managed_factory {
            compiler = compiler.with_managed_invocations(factory);
        }
        for contribution in self.contributions {
            compiler.absorb_snapshot(contribution.registry.as_ref())?;
        }
        Ok(compiler.compile()?)
    }

    pub(crate) fn compile_complete_console_snapshot(
        self,
        console_inventory: &access_control::ConsoleOperationCompiledInventory,
    ) -> anyhow::Result<(
        Arc<CompiledInterfaceRegistry>,
        CompiledConsoleOperationSnapshot,
    )> {
        let migration =
            crate::console_policy_migration::compile_core_console_policy_migration_plan(
                console_inventory,
            )?;
        let mut binding_contributions = Vec::new();
        let mut known_binding_owners = BTreeSet::new();
        for contribution in &self.contributions {
            binding_contributions.extend(contribution.console_binding_contributions());
            known_binding_owners.extend(
                contribution
                    .registry
                    .definitions()
                    .map(|definition| definition.owner().as_str().to_string()),
            );
        }
        let mut migrations = migration_contributions_from_plan(&migration);
        migrations.extend(console_inventory.operations.iter().filter(|operation| operation.owner.kind == access_control::SettingsFeatureOwnerKind::ManagedService).map(|operation| crate::console_operation_compilation::ConsoleMigrationDispositionContribution {
            operation_id: operation.operation_id.clone(),
            disposition: crate::console_operation_compilation::ConsoleMigrationDisposition::DefaultDisabled { evidence: "New managed service operation requires explicit role policy; no legacy grants".into() },
        }));
        let managed_stream_controls = self
            .contributions
            .iter()
            .filter(|contribution| contribution.contribution_id.starts_with("managed-service."))
            .flat_map(|contribution| {
                contribution
                    .registry
                    .bindings()
                    .filter_map(|binding| {
                        let definition = contribution
                            .registry
                            .definition(binding.interface_identity().interface_id())?;
                        let route = binding.projection().http_route()?;
                        (definition.execution_mode()
                            == interface_runtime::InterfaceExecutionMode::ServerStream
                            && route.method() == "POST")
                            .then(|| access_control::ConsoleRouteBinding {
                                method: "GET".into(),
                                path: route.path().into(),
                            })
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let snapshot = compile_console_operation_snapshot(
            policy_contributions_from_inventory(console_inventory, &managed_stream_controls),
            binding_contributions,
            migrations,
            known_binding_owners,
        )?;
        let registry = self.compile()?;
        Ok((registry, snapshot))
    }
}

fn console_bindings_from_registry(
    contribution_id: &str,
    owner_kind: ConsoleBindingOwnerKind,
    registry: &CompiledInterfaceRegistry,
) -> Vec<ConsoleBindingOwnershipContribution> {
    registry
        .bindings()
        .filter_map(|binding| {
            let route = binding.projection().http_route()?;
            if !route.path().starts_with("/api/console") {
                return None;
            }
            let definition = registry.definition(binding.interface_identity().interface_id())?;
            Some(ConsoleBindingOwnershipContribution {
                contribution_id: contribution_id.to_string(),
                owner_kind,
                owner_id: definition.owner().as_str().to_string(),
                owner_active: true,
                interface_id: definition.interface_id().as_str().to_string(),
                binding_id: binding.binding_id().as_str().to_string(),
                protocol: match binding.projection().protocol() {
                    interface_runtime::InterfaceProtocol::Http => "http",
                    interface_runtime::InterfaceProtocol::Mcp => "mcp",
                    interface_runtime::InterfaceProtocol::Internal => "internal",
                    interface_runtime::InterfaceProtocol::Worker => "worker",
                }
                .to_string(),
                route: access_control::ConsoleRouteBinding {
                    method: route.method().to_string(),
                    path: route.path().to_string(),
                },
            })
        })
        .collect()
}

pub(crate) fn production_interface_contributions(
    state: &Arc<crate::app_state::ApiState>,
) -> Result<Vec<InterfaceRegistryContribution>, RegistryCompilationError> {
    let console_frontstage_callable_dispatch =
        crate::openapi_interface::console_router_callable_dispatch_port(crate::console_router(
            Arc::clone(state),
            true,
        ));
    let public_login_entries = crate::routes::auth::public_login_entries_port(
        state.store.clone(),
        Arc::clone(&state.authenticator_registry),
        state.bootstrap_workspace_id,
    );
    let public_sign_in = crate::routes::sign_in_interface::public_sign_in_port(
        state.store.clone(),
        Arc::clone(&state.session_store),
        state.session_ttl_days,
        Arc::clone(&state.authenticator_registry),
    );
    let public_providers = crate::routes::auth::public_providers_port(
        state.store.clone(),
        state.bootstrap_workspace_id,
    );
    let public_sign_up = crate::routes::auth::public_sign_up_port(
        state.store.clone(),
        Arc::clone(&state.session_store),
        state.session_ttl_days,
    );
    let console_identity = crate::routes::console_identity_interface::console_identity_port(
        state.store.clone(),
        Arc::clone(&state.session_store),
        state.cookie_name.clone(),
    );
    let console_membership =
        crate::routes::membership_interface::membership_port(state.store.clone());
    let console_organization = crate::routes::organization::interface::port(
        state.store.clone(),
        control_plane::navigation_cache::NavigationCache(state.infrastructure.cache_store()),
    );
    let console_navigation = crate::routes::navigation_interface::port(
        crate::routes::navigation_interface::ConsoleNavigationDependencies {
            navigation_cache: control_plane::navigation_cache::NavigationCache(
                state.infrastructure.cache_store(),
            ),
            store: state.store.clone(),
            surfaces: Arc::clone(&state.console_surface_registry),
            settings_features: state.settings_feature_registry.inventory().features.clone(),
        },
    );
    let console_i18n_catalog = crate::routes::i18n_catalog::interface::port(
        crate::routes::i18n_catalog::interface::I18nCatalogDependencies {
            store: state.store.clone(),
            bootstrap_workspace_id: state.bootstrap_workspace_id,
            update_service: Arc::clone(&state.official_i18n_catalog_update_service),
            api_node_id: state.api_node_id.clone(),
            provider_install_root: state.provider_install_root.clone(),
        },
    );
    let console_role_access = crate::routes::role_access_interface::role_access_port(
        control_plane::navigation_cache::NavigationCache(state.infrastructure.cache_store()),
        state.store.clone(),
        state.console_operation_registry.inventory().clone(),
        state.settings_feature_registry.inventory().features.clone(),
        state.bootstrap_workspace_id,
    );
    let console_auth_center = crate::routes::auth_center_interface::auth_center_port(
        state.store.clone(),
        Arc::clone(&state.authenticator_registry),
        state.bootstrap_workspace_id,
    );
    let console_ui_management = crate::routes::ui_management_interface::ui_management_port(
        crate::routes::ui_management_interface::UiManagementDependencies {
            store: state.store.clone(),
            api_node_id: state.api_node_id.clone(),
            surfaces: state.console_surface_registry.clone(),
            network_egress: state.network_egress_http_clients(),
        },
    );
    let console_billing = crate::routes::billing_interface::port(
        crate::routes::billing_interface::BillingDependencies {
            store: state.store.clone(),
            cache_store: state.infrastructure.cache_store(),
            catalog_index_url: state.official_model_pricing_catalog_index_url.clone(),
            network_egress: state.network_egress_http_clients(),
        },
    );
    let console_application_orchestration =
        crate::routes::application_orchestration::interface::port(
            state.store.clone(),
            state.bootstrap_workspace_id,
            state.api_node_id.clone(),
            state.provider_install_root.clone(),
        );
    let console_applications = crate::routes::applications::interface::applications_port(
        state.store.clone(),
        state.bootstrap_workspace_id,
        crate::routes::applications::collectors::CollectorCatalogDependencies {
            source: state.official_extension_catalog_source.clone(),
            install_root: state.provider_install_root.clone(),
            node_id: state.api_node_id.clone(),
        },
    );
    let console_application_api_keys =
        crate::routes::application_api::interface_keys::port(state.store.clone());
    let console_application_publication =
        crate::routes::application_api::interface_publication::port(
            state.store.clone(),
            state.infrastructure.cache_store(),
        );
    let console_workflow_schedule =
        crate::routes::application_api::interface_schedule::port(state.store.clone());
    let console_application_docs = crate::routes::application_api::interface_docs::port(
        state.store.clone(),
        Arc::clone(&state.api_docs),
    );
    let native_interface_registry = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.interface_registry())
        .cloned()
        .expect("native runtime invoker requires a published dynamic interface registry");
    let runtime_tool_invoker_dependencies =
        crate::routes::mcp_protocol::virtual_ui::RuntimeInternalToolInvokerDependencies::new(
            state.store.clone(),
            state.infrastructure.cache_store(),
            state.provider_secret_master_key.clone(),
        )
        .with_managed_services(state.console_surface_registry.managed_services().to_vec());
    let mcp_interface_catalog_dependencies =
        crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
            store: state.store.clone(),
            openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                store: state.store.clone(),
                console_operations: state.console_operation_registry.inventory().clone(),
                interface_registry: None,
                api_docs: Arc::clone(&state.api_docs),
                template_catalog: state.runtime_engine.template_catalog().clone(),
            },
        };
    let mcp_interface_dispatch: Arc<
        dyn crate::routes::mcp_protocol::virtual_ui::McpInterfaceDispatchPort,
    > = Arc::new(
        crate::routes::mcp_protocol::virtual_ui::ConsoleRouterMcpInterfaceDispatchPort::new(
            crate::console_router(Arc::clone(state), true),
        ),
    );
    let native_runtime_invoker_factory =
        crate::routes::application_public_api::native::native_runtime_invoker_factory(
            crate::routes::application_public_api::native::NativeRuntimeInvokerFactoryDependencies {
                runtime_dependencies: runtime_tool_invoker_dependencies.clone(),
                interface_catalog: mcp_interface_catalog_dependencies.clone(),
                interface_registry: Arc::new(
                    crate::routes::mcp_management::interface_catalog::DynamicMcpInterfaceRegistrySnapshotPort::new(
                        Arc::clone(&native_interface_registry),
                    ),
                ),
                interface_dispatch: Arc::clone(&mcp_interface_dispatch),
            },
        );
    let webmcp_port = crate::routes::webmcp::port(
        state.store.clone(),
        crate::routes::mcp_protocol::McpToolCallDependencies {
            runtime_dependencies: runtime_tool_invoker_dependencies.clone(),
            interface_catalog: mcp_interface_catalog_dependencies.clone(),
            interface_registry: Arc::new(
                crate::routes::mcp_management::interface_catalog::DynamicMcpInterfaceRegistrySnapshotPort::new(
                    Arc::clone(&native_interface_registry),
                ),
            ),
            interface_dispatch: Arc::clone(&mcp_interface_dispatch),
        },
    );
    let mcp_tool_call_port = crate::routes::mcp_protocol::mcp_tool_call_port(
        crate::routes::mcp_protocol::McpToolCallDependencies {
            runtime_dependencies: runtime_tool_invoker_dependencies,
            interface_catalog: mcp_interface_catalog_dependencies.clone(),
            interface_registry: Arc::new(
                crate::routes::mcp_management::interface_catalog::DynamicMcpInterfaceRegistrySnapshotPort::new(
                    native_interface_registry,
                ),
            ),
            interface_dispatch: mcp_interface_dispatch,
        },
    );
    let native_run_dependencies =
        crate::routes::application_public_api::native::ApplicationNativeRunDependencies {
            store: state.store.clone(),
            cache_store: state.infrastructure.cache_store(),
            published_plan_cache: state.infrastructure.published_plan_cache(),
            published_publication_cache: state.infrastructure.published_publication_cache(),
            runtime_engine: state.runtime_engine.clone(),
            provider_runtime: state.provider_runtime.clone(),
            network_egress: Arc::new(state.network_egress_http_clients()),
            provider_secret_master_key: state.provider_secret_master_key.clone(),
            model_billing_require_provider_usage: state.model_billing_require_provider_usage,
            api_node_id: state.api_node_id.clone(),
            provider_install_root: state.provider_install_root.clone(),
            file_storage_registry: state.file_storage_registry.clone(),
            notification_sender: state.notification_service.clone(),
            task_queue: state.infrastructure.task_queue(),
            provider_transport_store: state.infrastructure.provider_transport_store(),
            runtime_event_stream: state.runtime_event_stream.clone(),
            runtime_activity: state.runtime_activity.clone(),
            runtime_invoker_factory: Arc::clone(&native_runtime_invoker_factory),
        };
    let native_run_port =
        crate::routes::application_public_api::native::application_native_run_port(
            native_run_dependencies.clone(),
        );
    let compatibility_execution_port = crate::routes::application_public_api::compatibility_interface::compatibility_execution_port(
        crate::routes::application_public_api::compatibility_interface::CompatibilityExecutionDependencies {
            native: native_run_dependencies,
            provider_transport_store: state.infrastructure.provider_transport_store(),
        },
    );

    let mut contributions = vec![
        InterfaceRegistryContribution::new(
            "api-server.client-collectors", &["public.client-collectors.assets.read"], &["api-server.client-collectors"],
            crate::routes::client_collectors::compile_registry(state)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.webmcp",
            &["webmcp.registrations.list", "webmcp.tools.invoke"],
            &[crate::routes::webmcp::interface::OWNER],
            crate::routes::webmcp::interface::compile_registry(webmcp_port)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.public-login-entries",
            &["public.auth.login-entries.read"],
            &["api-server.public-auth"],
            crate::routes::auth::compile_public_login_entries_registry(public_login_entries)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.public-sign-in",
            &["public.auth.sign-in"],
            &["api-server.public-auth"],
            crate::routes::sign_in_interface::compile_registry(public_sign_in)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.public-auth-residual",
            &["public.auth.providers.read", "public.auth.sign-up"],
            &["api-server.public-auth"],
            crate::routes::auth::compile_public_residual_registry(
                public_providers,
                public_sign_up,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-identity",
            &[
                "console.identity.session.get",
                "console.identity.session.delete",
                "console.identity.session.revoke-all",
                "console.identity.session.switch-workspace",
                "console.identity.session.switch-role",
                "console.identity.me.get",
                "console.identity.me.patch",
                "console.identity.me.meta.patch",
                "console.identity.me.change-password",
                "console.identity.user-api-keys.list",
                "console.identity.user-api-keys.create",
                "console.identity.user-api-keys.role-options",
                "console.identity.user-api-keys.revoke",
            ],
            &["api-server.console-identity"],
            crate::routes::console_identity_interface::compile_registry(
                console_identity,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-organization",
            &["console.departments.access","departments.list","departments.create","departments.update","departments.delete","members.departments.replace"],
            &["api-server.console-organization"],
            crate::routes::organization::interface::compile_registry(console_organization)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-notifications",
            &["notifications.channels.list","notifications.channels.read_all","notifications.messages.unread","notifications.types.list","notifications.instances.list","notifications.instances.get","notifications.instances.create","notifications.instances.update","notifications.instances.delete","notifications.instances.send","notifications.instances.authorization","notifications.logs.list","notifications.logs.get","notifications.messages.list","notifications.messages.unread_count","notifications.messages.read","notifications.messages.read_all"],
            &["api-server.console-notifications"],
            crate::routes::notifications::interface::compile_registry(state.notification_service.clone())?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-membership",
            &[
                "members.role_options.list",
                "members.list",
                "members.create",
                "members.update",
                "members.disable",
                "members.enable",
                "members.delete",
                "members.password.reset",
                "members.roles.replace",
                "console.workspace.get",
                "workspace.update",
                "console.workspaces.list",
            ],
            &["api-server.console-membership"],
            crate::routes::membership_interface::compile_registry(console_membership)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-navigation",
            &["console.navigation.view"],
            &["api-server.console-navigation"],
            crate::routes::navigation_interface::compile_registry(console_navigation)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-i18n-catalog",
            &[
                "i18n_catalog.state.get",
                "i18n_catalog.update.check",
                "i18n_catalog.update.activate",
                "i18n_catalog.installed_extension.preview",
                "i18n_catalog.installed_extension.activate",
                "i18n_catalog.entries.list",
                "i18n_catalog.entries.detail",
                "i18n_catalog.overrides.upsert",
                "i18n_catalog.overrides.restore",
                "i18n_catalog.custom_translations.upsert",
                "i18n_catalog.custom_keys.delete",
                "i18n_catalog.overrides.restore_all",
            ],
            &["api-server.console-i18n-catalog"],
            crate::routes::i18n_catalog::interface::compile_registry(console_i18n_catalog)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-role-access",
            &[
                "roles.data_model_options.list",
                "roles.console_policy_catalog.view",
                "roles.console_settings_order.replace",
                "roles.console_policy.view",
                "roles.console_policy.replace",
                "roles.list",
                "roles.create",
                "roles.update",
                "roles.delete",
                "roles.permissions.view",
                "roles.permissions.replace",
                "roles.frontstage_routes.view",
                "roles.frontstage_routes.replace",
                "roles.data_policy.view",
                "roles.data_policy.replace",
                "roles.permission_options.list",
            ],
            &["api-server.console-role-access"],
            crate::routes::role_access_interface::compile_registry(console_role_access)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-assistant-settings",
            &["assistant.settings.get", "assistant.settings.update"],
            &["api-server.console-assistant-settings"],
            crate::routes::assistant::interface::compile_registry(state.store.clone())?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-assistant-conversations",
            &[
                "assistant.runs.activity.get",
                "assistant.conversations.create",
                "assistant.conversations.list",
                "assistant.conversations.messages.get",
                "assistant.legacy-runs.messages.get",
            ],
            &["api-server.console-assistant-conversations"],
            crate::routes::assistant::interface::compile_conversations_registry(
                state.store.clone(),
                Arc::clone(&state.assistant_conversation_events),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-assistant-runs",
            &["assistant.runs.create"],
            &["api-server.console-assistant-runs"],
            crate::routes::assistant::interface::compile_runs_registry(
                crate::routes::assistant::run_dependencies(state.clone()),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-assistant-run-stream",
            &["assistant.runs.stream.create"],
            &["api-server.console-assistant-run-stream"],
            crate::routes::assistant::interface::compile_run_stream_registry(
                crate::routes::assistant::run_dependencies(state.clone()),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-assistant-websocket-ticket",
            &["assistant.runs.websocket-ticket.create"],
            &["api-server.console-assistant-websocket-ticket"],
            crate::routes::assistant::websocket_ticket_interface::compile_registry(
                state.store.clone(),
                state.infrastructure.cache_store(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-assistant-websocket-commands",
            &["assistant.runs.websocket.command"],
            &["api-server.console-assistant-websocket-commands"],
            crate::routes::assistant::websocket_interface::compile_registry(
                crate::routes::assistant::run_dependencies(state.clone()),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-applications",
            &[
                "applications.list",
                "applications.create",
                "applications.get",
                "applications.update",
                "applications.delete",
                "applications.catalog.get",
                "applications.tags.create",
                "applications.environment-variables.list",
                "applications.environment-variables.replace",
                "applications.js-dependencies.list",
                "applications.js-dependencies.replace",
            ],
            &["api-server.console-applications"],
            crate::routes::applications::interface::compile_registry(console_applications)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-api-keys",
            &[
                "applications.api-keys.list",
                "applications.api-keys.create",
                "applications.api-keys.revoke",
            ],
            &["api-server.console-application-api-keys"],
            crate::routes::application_api::interface_keys::compile_registry(
                console_application_api_keys,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-publication",
            &[
                "applications.api-mapping.get",
                "applications.api-mapping.replace",
                "applications.api-publication.get",
                "applications.api-publication.publish",
                "applications.api-publication.unpublish",
                "applications.api-status.update",
            ],
            &["api-server.console-application-publication"],
            crate::routes::application_api::interface_publication::compile_registry(
                console_application_publication,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-workflow-schedule",
            &[
                "applications.workflow-schedule.get",
                "applications.workflow-schedule.replace",
            ],
            &["api-server.console-workflow-schedule"],
            crate::routes::application_api::interface_schedule::compile_registry(
                console_workflow_schedule,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-docs",
            &[
                "applications.api-docs.catalog",
                "applications.api-docs.category-operations",
                "applications.api-docs.category-openapi",
                "applications.api-docs.operation-openapi",
            ],
            &["api-server.console-application-docs"],
            crate::routes::application_api::interface_docs::compile_registry(
                console_application_docs,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-runtime-reads",
            &[
                "applications.runtime.current-context.get",
                "applications.runtime.records.query-fields",
                "applications.runtime.records.query",
                "applications.runtime.conversations.query",
                "applications.runtime.conversation-trajectory.query",
                "applications.runtime.conversation-trajectory.section",
                "applications.runtime.record.trace-tree.get",
                "applications.runtime.record.trace-tree.children.list",
                "applications.runtime.record.client-trajectory.query",
                "applications.runtime.record.get",
                "applications.runtime.record.conversation.messages.list",
                "applications.runtime.record.client-trajectory.list",
                "applications.runtime.record.client-trajectory.section.get",
                "applications.runtime.workflow-trajectory.list",
                "applications.runtime.workflow-trajectory.body.get",
                "applications.runtime.client-trajectory.list",
                "applications.runtime.client-trajectory.section.get",
                "applications.runtime.run.trajectory.list",
                "applications.runtime.run.payload.get",
                "applications.runtime.trajectory.list",
                "applications.runtime.trajectory.body.get",
                "applications.runtime.logs.list",
                "applications.runtime.conversations.messages.list",
                "applications.runtime.run-conversation.messages.list",
                "applications.runtime.run.overview.get",
                "applications.runtime.trace-tree.get",
                "applications.runtime.trace-tree.children.get",
                "applications.runtime.resume-timeline.get",
                "applications.runtime.resume-timeline-summary.get",
                "applications.runtime.run-node-last-run.get",
                "applications.runtime.monitoring.report.get",
                "applications.runtime.monitoring.activity.get",
                "applications.runtime.debug-stream.get",
                "applications.runtime.node-last-run.get",
            ],
            &["api-server.console-application-runtime-reads"],
            crate::routes::application_runtime::interface_runtime_reads::compile_registry(
                state.store.clone(),
                state.infrastructure.cache_store(),
                Arc::clone(&state.runtime_activity),
                state.process_started_at,
                Arc::clone(&state.file_storage_registry),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-runtime-debug-variables",
            &[
                "applications.runtime.debug-variables.snapshot.get",
                "applications.runtime.debug-variables.cache.upsert",
                "applications.runtime.debug-variables.cache.delete",
            ],
            &["api-server.console-application-runtime-debug-variables"],
            crate::routes::application_runtime::interface_debug_variables::compile_registry(
                state.store.clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-host-infrastructure-memory-inspection",
            &[
                "host_infrastructure.memory.overview.get",
                "host_infrastructure.memory.stats-overview.get",
                "host_infrastructure.memory.entries.list",
                "host_infrastructure.memory.stats.get",
                "host_infrastructure.memory.entries.search",
                "host_infrastructure.memory.tree.list",
                "host_infrastructure.memory.entry.reveal",
            ],
            &["api-server.console-host-infrastructure-memory-inspection"],
            crate::routes::host_infrastructure::interface_memory_inspection::compile_registry(
                crate::routes::host_infrastructure::interface_memory_inspection::MemoryInspectionInterfaceDependencies {
                    memory: crate::routes::host_infrastructure::memory_inspection_dependencies(state),
                    audit_policy: Arc::new(
                        crate::routes::host_infrastructure::interface_memory_inspection::MemoryAuditPolicyAdapter::new(
                            state.store.clone(),
                            Arc::clone(&state.console_operation_registry),
                        ),
                    ),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-host-infrastructure-cache-inspection",
            &[
                "host_infrastructure.cache.overview.get",
                "host_infrastructure.cache.entries.list",
                "host_infrastructure.cache.entry.reveal",
                "host_infrastructure.cache.entry.clear",
                "host_infrastructure.cache.domain.clear",
            ],
            &["api-server.console-host-infrastructure-cache-inspection"],
            crate::routes::host_infrastructure::interface_cache_inspection::compile_registry(
                crate::routes::host_infrastructure::interface_cache_inspection::CacheInspectionDependencies {
                    cache: state.infrastructure.cache_store(),
                    provider_code: state
                        .infrastructure
                        .default_provider("cache-store")
                        .map(ToString::to_string),
                    audit_policy: Arc::new(
                        crate::routes::host_infrastructure::interface_cache_inspection::CacheAuditPolicyAdapter::new(
                            state.store.clone(),
                            Arc::clone(&state.console_operation_registry),
                        ),
                    ),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-runtime-debug-artifacts",
            &[
                "applications.runtime.debug-artifact.get",
                "applications.runtime.debug-artifacts.resolve",
                "applications.runtime.debug-snapshot.get",
            ],
            &["api-server.console-application-runtime-debug-artifacts"],
            crate::routes::application_runtime::interface_debug_artifacts::compile_registry(
                state.store.clone(),
                state.file_storage_registry.clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-runtime-debug-commands",
            &[
                "applications.runtime.debug-runs.create",
                "applications.runtime.runs.resume",
                "applications.runtime.runs.cancel",
                "applications.runtime.callback-tasks.complete",
                "applications.runtime.nodes.debug-runs.create",
            ],
            &["api-server.console-application-runtime-debug-commands"],
            crate::routes::application_runtime::interface_debug_commands::compile_registry(
                crate::routes::application_runtime::interface_debug_commands::dependencies(
                    state.clone(),
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-runtime-debug-streams",
            &[
                "applications.runtime.debug-runs.stream.create",
                "applications.runtime.debug-runs.stream.subscribe",
            ],
            &["api-server.console-application-runtime-debug-streams"],
            crate::routes::application_runtime::interface_debug_commands::compile_stream_registry(
                crate::routes::application_runtime::interface_debug_commands::dependencies(
                    state.clone(),
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-runtime-trace-exports",
            &[
                "applications.runtime.trace-export.get",
                "applications.runtime.trace-export.selected-runs",
            ],
            &["api-server.console-application-runtime-trace-exports"],
            crate::routes::application_runtime::interface_trace_exports::compile_registry(
                state.store.clone(),
                state.file_storage_registry.clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-runtime-trace-payloads",
            &[
                "applications.runtime.trace-node.content.get",
                "applications.runtime.trace-node.detail.get",
                "applications.runtime.trace-tool-callback.content.get",
            ],
            &["api-server.console-application-runtime-trace-payloads"],
            crate::routes::application_runtime::interface_trace_payloads::compile_registry(
                state.store.clone(),
                state.file_storage_registry.clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-log-deletion-jobs",
            crate::routes::application_runtime::logs_delete::jobs::OPERATIONS,
            &["api-server.console-application-log-deletion-jobs"],
            crate::routes::application_runtime::logs_delete::jobs::compile_registry(state.store.clone())?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-log-deletion-preview",
            &["applications.logs.delete.preview"],
            &["api-server.console-application-log-deletion-preview"],
            crate::routes::application_runtime::logs_delete::jobs::compile_preview_registry(state.store.clone())?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-log-exchange",
            crate::routes::application_runtime::log_exchange::OPERATIONS,
            &["api-server.console-application-log-exchange"],
            crate::routes::application_runtime::log_exchange::compile_registry(state.clone())?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-logs-delete",
            &["applications.logs.delete"],
            &["api-server.console-application-logs-delete"],
            crate::routes::application_runtime::logs_delete::compile_registry(state.store.clone())?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-runtime-archive",
            &[
                "applications.runtime.archive.run.export",
                "applications.runtime.archive.runs.export",
                "applications.runtime.archive.upload-sessions.create",
                "applications.runtime.archive.upload-chunks.upsert",
                "applications.runtime.archive.upload-sessions.complete",
                "applications.runtime.archive.import-jobs.get",
            ],
            &["api-server.console-application-runtime-archive"],
            crate::routes::application_runtime::archive::interface::compile_registry(
                crate::routes::application_runtime::archive::interface::dependencies(
                    state.store.clone(),
                    state.file_storage_registry.clone(),
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-orchestration",
            &[
                "applications.orchestration.get",
                "applications.orchestration.draft.save",
                "applications.orchestration.version.restore",
                "applications.orchestration.version.update",
                "applications.archive.export",
                "applications.archive.preview",
                "applications.archive.import",
                "applications.archive.installed.preview",
                "applications.archive.installed.import",
            ],
            &["api-server.console-application-orchestration"],
            crate::routes::application_orchestration::interface::compile_registry(
                console_application_orchestration,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-auth-center",
            &[
                "auth_center.overview.view",
                "auth_center.login_entries.create",
                "auth_center.login_entries.order",
                "auth_center.login_entries.enabled.update",
                "auth_center.login_entries.copy",
                "auth_center.login_entries.update.config",
                "auth_center.login_entries.update.public-ui-block",
                "auth_center.login_entries.delete",
            ],
            &["api-server.console-auth-center"],
            crate::routes::auth_center_interface::compile_registry(console_auth_center)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-ui-management",
            &[
                "ui_management.plugin_settings_page.view",
                "ui_management.plugin_settings_module_asset.view",
                "ui_management.templates.list",
                "ui_management.templates.create",
                "ui_management.templates.default.reset",
                "ui_management.templates.update",
                "ui_management.templates.publish",
                "ui_management.templates.default.set",
                "ui_management.templates.delete",
                "ui_management.components.list",
                "ui_management.components.create",
                "ui_management.components.view",
                "ui_management.components.update",
                "ui_management.components.delete",
                "ui_management.catalog.index",
                "ui_management.catalog.page",
                "ui_management.catalog.search",
                "ui_management.catalog.update_status",
                "ui_management.catalog.download",
                "ui_management.catalog.sync_group",
            ],
            &["api-server.console-ui-management"],
            crate::routes::ui_management_interface::compile_registry(console_ui_management)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-billing",
            &[
                "billing.pricing_rules.list",
                "billing.pricing_rules.create",
                "billing.pricing_rules.update",
                "billing.pricing_rules.delete",
                "billing.pricing_catalog.view",
                "billing.pricing_catalog.import",
                "billing.pricing_catalog.sync",
                "billing.credit_accounts.list",
                "billing.credit_accounts.view",
                "billing.credit_ledger.list",
                "billing.credit.grant",
                "billing.credit.charge",
                "billing.credit.adjust",
                "billing.credit.enable",
                "billing.credit.disable",
                "billing.credit.refund",
            ],
            &["api-server.console-billing"],
            crate::routes::billing_interface::compile_registry(console_billing)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-file-storages",
            &[
                "file_storages.list",
                "file_storages.create",
                "file_storages.update",
                "file_storages.delete",
            ],
            &["api-server.console-file-storages"],
            crate::routes::file_storages::compile_registry(
                state.store.clone(),
                state.bootstrap_workspace_id,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-file-tables",
            &[
                "file_tables.list",
                "file_tables.create",
                "file_tables.delete",
                "file_tables.storage.bind",
            ],
            &["api-server.console-file-tables"],
            crate::routes::file_tables::compile_registry(
                state.store.clone(),
                state.bootstrap_workspace_id,
                Arc::new(crate::runtime_registry_sync::ApiRuntimeRegistrySync::new(
                    state.store.clone(),
                    state.runtime_engine.registry().clone(),
                )),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-business-files",
            &["files.upload", "files.import_remote", "files.content.download", "files.access_links.create"],
            &["api-server.console-business-files"],
            crate::routes::files::compile_registry(
                state.store.clone(),
                state.file_storage_registry.clone(),
                state.runtime_engine.clone(),
                state.infrastructure.ephemeral_kv_store(),
                state.network_egress_http_clients(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.temporary-files",
            &["public.files.temporary.read"],
            &["api-server.temporary-files"],
            crate::routes::files::public::compile_registry(state)?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-data-sources",
            &[
                "agent_flow.data_source_options.list",
                "settings_feature.access.system.data-models",
                "data_sources.list",
                "data_sources.create",
                "data_sources.defaults.update",
                "data_sources.validate",
                "data_sources.secret.rotate",
                "data_sources.view",
                "data_sources.discover",
                "data_sources.preview",
                "data_sources.map_to_model",
            ],
            &["api-server.console-data-sources"],
            crate::routes::data_sources::compile_registry(
                state.store.clone(),
                state.provider_runtime.clone(),
                state.provider_secret_master_key.clone(),
                state.api_node_id.clone(),
                state.provider_install_root.clone(),
                state.runtime_engine.clone(),
                Arc::new(crate::runtime_registry_sync::ApiRuntimeRegistrySync::new(
                    state.store.clone(),
                    state.runtime_engine.registry().clone(),
                )),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-model-definitions",
            &[
                "model_definitions.list",
                "model_templates.list",
                "agent_flow.data_model_options.list",
                "model_definitions.create",
                "model_definitions.advisor.view",
                "model_scope_grants.list",
                "model_definitions.update",
                "model_definitions.delete",
                "model_fields.create",
                "model_fields.update",
                "model_fields.delete",
                "model_scope_grants.create",
                "model_scope_grants.update",
            ],
            &["api-server.console-model-definitions"],
            crate::routes::model_definitions::interface::compile_registry(
                crate::routes::model_definitions::interface::dependencies(
                    state.store.clone(),
                    state.bootstrap_workspace_id,
                    state.runtime_engine.clone(),
                    state.provider_runtime.clone(),
                    state.provider_secret_master_key.clone(),
                    state.api_node_id.clone(),
                    state.provider_install_root.clone(),
                ),
                GraphFingerprint::new(
                    state.extension_boot_snapshot.as_ref().ok_or_else(|| {
                        RegistryCompilationError::ExtensionGraphFingerprintMismatch(
                            InterfaceId::new("model_definitions.create").expect("static interface id"),
                        )
                    })?.graph().fingerprint().as_str(),
                ).map_err(|_| RegistryCompilationError::ExtensionGraphFingerprintMismatch(
                    InterfaceId::new("model_definitions.create").expect("static interface id"),
                ))?,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-docs",
            &[
                "docs.catalog.view",
                "docs.categories.operations.list",
                "docs.categories.openapi.view",
                "docs.operations.openapi.view",
            ],
            &["api-server.console-docs"],
            crate::routes::docs::interface::compile_registry(
                crate::routes::docs::interface::port(
                    crate::routes::docs::interface::DocsDependencies {
                        store: state.store.clone(),
                        api_docs: Arc::clone(&state.api_docs),
                        template_catalog: state.runtime_engine.template_catalog().clone(),
                        cookie_name: state.cookie_name.clone(),
                    },
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-data-model-openapi",
            &["model_definitions.openapi.view"],
            &["api-server.console-data-model-openapi"],
            crate::routes::docs::data_model_openapi_interface::compile_registry(
                state.store.clone(),
                state.runtime_engine.clone(),
            )?,
        ),
    ];
    contributions.extend(provider_operations::contributions(state)?);
    contributions.extend(vec![
        InterfaceRegistryContribution::new(
            "api-server.native-runs",
            &["application.native.runs.create"],
            &["api-server.application-public-api"],
            crate::routes::application_public_api::native::compile_native_interface_registry(
                Arc::clone(&native_run_port),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-provider-icons",
            &["model_providers.icons.view"],
            &["api-server.console-provider-icons"],
            crate::routes::model_providers::icons::compile_registry(
                state.store.clone(),
                state.provider_runtime.clone(),
                state.provider_secret_master_key.clone(),
                state.api_node_id.clone(),
                state.provider_install_root.clone(),
            )?,
        ),
    ]);
    contributions.extend(network::contributions(state)?);
    contributions.extend(plugins::contributions(state)?);
    contributions.extend(mcp::contributions(state)?);
    contributions.extend(vec![
        InterfaceRegistryContribution::new(
            "api-server.console-mcp-debug",
            &["mcp.debug.execute"],
            &["api-server.console-mcp-debug"],
            crate::routes::mcp_management::interface_debug::compile_registry(
                crate::routes::mcp_management::interface_debug::port(
                    crate::routes::mcp_management::interface_debug::McpDebugDependencies {
                        store: state.store.clone(),
                        catalog: crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
                            store: state.store.clone(),
                            openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                                store: state.store.clone(), console_operations: state.console_operation_registry.inventory().clone(), interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()), api_docs: Arc::clone(&state.api_docs), template_catalog: state.runtime_engine.template_catalog().clone(),
                            },
                        },
                        dispatcher: Arc::clone(&console_frontstage_callable_dispatch),
                    },
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontend-blocks",
            &["frontend_blocks.view"],
            &["api-server.console-frontend-blocks"],
            crate::routes::frontend_block_catalog::compile_registry(
                crate::routes::frontend_block_catalog::FrontendBlockDependencies {
                    store: state.store.clone(),
                    api_node_id: state.api_node_id.clone(),
                    graph: Arc::clone(
                        state
                            .extension_boot_snapshot
                            .as_ref()
                            .expect("production interface publication requires ExtensionBootSnapshot")
                            .graph_arc(),
                    ),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-js-dependencies",
            &["js_dependencies.view"],
            &["api-server.console-js-dependencies"],
            crate::routes::js_dependencies::compile_registry(
                state.store.clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-node-contributions",
            &["node_contributions.view"],
            &["api-server.console-node-contributions"],
            crate::routes::node_contributions::compile_registry(
                state.store.clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-data-capabilities",
            &["frontstage.data_capabilities.view"],
            &["api-server.console-frontstage-data-capabilities"],
            crate::routes::frontstage::data_capabilities::compile_catalog_registry(
                state.runtime_engine.registry().clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-block-context-contract",
            &["frontstage.block_context.view"],
            &["api-server.console-frontstage-block-context-contract"],
            crate::routes::frontstage::block_context_contract::compile_registry()?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-components",
            &["frontstage.components.view"],
            &["api-server.console-frontstage-components"],
            crate::routes::frontstage::components::compile_registry(
                crate::routes::frontstage::components::FrontstageComponentsDependencies {
                    store: state.store.clone(),
                    api_node_id: state.api_node_id.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-pages",
            &[
                "frontstage.pages.view", "frontstage.pages.create", "frontstage.groups.create",
                "frontstage.pages.update", "frontstage.pages.move", "frontstage.pages.delete",
                "frontstage.tabs.view", "frontstage.tabs.create", "frontstage.tabs.update",
                "frontstage.tabs.delete", "frontstage.tabs.document.save",
                "frontstage.ui_templates.view",
                "frontstage.queries.dispatch", "frontstage.actions.dispatch",
            ],
            &["api-server.console-frontstage-pages"],
            crate::routes::frontstage::interface_pages::compile_registry(
                crate::routes::frontstage::interface_pages::FrontstagePagesDependencies {
                    navigation_cache: control_plane::navigation_cache::NavigationCache(state.infrastructure.cache_store()),
                    store: state.store.clone(),
                    bootstrap_workspace_id: state.bootstrap_workspace_id,
                    api_node_id: state.api_node_id.clone(),
                    runtime_engine: Arc::clone(&state.runtime_engine),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-blocks",
            &[
                "frontstage.blocks.open", "frontstage.blocks.view", "frontstage.blocks.create",
                "frontstage.blocks.search", "frontstage.blocks.update", "frontstage.blocks.delete",
                "frontstage.blocks.move", "frontstage.blocks.code.view",
                "frontstage.blocks.code.update", "frontstage.blocks.runtime.view",
            ],
            &["api-server.console-frontstage-blocks"],
            crate::routes::frontstage::block_tree::interface::compile_registry(
                crate::routes::frontstage::block_tree::interface::FrontstageBlocksDependencies {
                    block_code_cache: control_plane::frontstage::BlockCodeCache(state.infrastructure.cache_store()),
                    store: state.store.clone(),
                    api_node_id: state.api_node_id.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-callable-catalog",
            &[
                "frontstage.callable_interfaces.list",
                "frontstage.callable_interfaces.view",
            ],
            &["api-server.console-frontstage-callable-catalog"],
            crate::routes::frontstage::callable_interface_catalog::compile_registry(
                crate::routes::frontstage::callable_interface_catalog::port(
                    crate::routes::frontstage::callable_interface_catalog::FrontstageCallableCatalogDependencies {
                        openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                            store: state.store.clone(),
                            console_operations: state.console_operation_registry.inventory().clone(),
                            interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()),
                            api_docs: Arc::clone(&state.api_docs),
                            template_catalog: state.runtime_engine.template_catalog().clone(),
                        },
                    },
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-callable-dispatch",
            &["frontstage.callable_interfaces.dispatch"],
            &["api-server.console-frontstage-callable-dispatch"],
            crate::routes::frontstage::callable_interface_dispatch::compile_registry(
                crate::routes::frontstage::callable_interface_dispatch::port(
                    crate::routes::frontstage::callable_interface_dispatch::FrontstageCallableDispatchDependencies {
                        store: state.store.clone(),
                        openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                            store: state.store.clone(),
                            console_operations: state.console_operation_registry.inventory().clone(),
                            interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()),
                            api_docs: Arc::clone(&state.api_docs),
                            template_catalog: state.runtime_engine.template_catalog().clone(),
                        },
                        dispatcher: Arc::clone(&console_frontstage_callable_dispatch),
                    },
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-runtime-i18n",
            &["i18n.catalog.view"],
            &["api-server.console-runtime-i18n"],
            crate::routes::runtime_i18n_catalog::compile_registry(
                state.store.clone(),
                state.bootstrap_workspace_id,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-management",
            &[access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION],
            &["api-server.console-application-management"],
            crate::routes::application_management::compile_registry(state.store.clone())?,
        ),
    ]);
    contributions.extend(system::contributions(state, &mcp_interface_catalog_dependencies)?);
    contributions.extend(vec![
        InterfaceRegistryContribution::new(
            "api-server.application-agent-logs",
            &["application.logs.events.ingest"],
            &["api-server.application-agent-logs"],
            crate::routes::application_public_api::agent_logs::compile_registry(state.store.clone())?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.application-agent-logs-otlp",
            &["application.logs.otlp.traces.ingest"],
            &["api-server.application-agent-logs-otlp"],
            crate::routes::application_public_api::agent_logs::otlp::compile_registry(state.store.clone())?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.native-read",
            &[
                "application.native.models.list",
                "application.native.runs.read",
                "application.native.runs.cancel",
                "application.native.runs.resume",
                "application.native.files.upload",
            ],
            &["api-server.application-public-api"],
            crate::routes::application_public_api::native_read_interface::compile_registry(
                crate::routes::application_public_api::native_read_interface::native_read_port(
                    state.store.clone(),
                    state.infrastructure.cache_store(),
                    Arc::clone(&state.runtime_event_stream),
                ),
                crate::routes::application_public_api::native_read_interface::native_resume_port(
                    state.store.clone(),
                    state.infrastructure.published_plan_cache(),
                    crate::routes::application_public_api::native::api_provider_runtime(state),
                    Arc::clone(&state.runtime_engine),
                    state.provider_secret_master_key.clone(),
                    state.model_billing_require_provider_usage,
                    state.api_node_id.clone(),
                    state.provider_install_root.clone(),
                    Arc::clone(&state.file_storage_registry),
                    state.notification_service.clone(),
                    state.infrastructure.cache_store(),
                    state.infrastructure.task_queue(),
                    state.infrastructure.provider_transport_store(),
                    Arc::clone(&state.runtime_event_stream),
                    Arc::clone(&state.runtime_activity),
                    Arc::clone(&native_runtime_invoker_factory),
                ),
                crate::routes::application_public_api::native_read_interface::native_file_port(
                    state.store.clone(),
                    Arc::clone(&state.file_storage_registry),
                    Arc::clone(&state.runtime_engine),
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.compatibility",
            &[
                "application.native.runs.create",
                "application.compatibility.models.list",
            ],
            &["api-server.application-public-api"],
            crate::routes::application_public_api::compatibility_interface::compile_registry(
                Arc::clone(&compatibility_execution_port),
                crate::routes::application_public_api::compatibility_interface::compatibility_models_port(
                    state.store.clone(),
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.mcp",
            &["mcp.tools.invoke"],
            &["api-server.mcp-protocol"],
            crate::routes::mcp_protocol::compile_mcp_interface_registry(Arc::clone(
                &mcp_tool_call_port,
            ))?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.workflow-extension",
            &["workflow-extension.invoke"],
            &["api-server.workflow-extension"],
            crate::routes::application_public_api::ex::compile_workflow_extension_registry(
                crate::routes::application_public_api::ex::workflow_extension_port(
                    state.store.clone(),
                    Arc::clone(&state.runtime_activity),
                    crate::routes::application_public_api::native::api_provider_runtime(state),
                    Arc::clone(&state.runtime_engine),
                    state.provider_secret_master_key.clone(),
                    state.model_billing_require_provider_usage,
                    state.infrastructure.provider_transport_store(),
                    state.api_node_id.clone(),
                    state.provider_install_root.clone(),
                    Arc::clone(&state.file_storage_registry),
                    state.infrastructure.cache_store(),
                    state.infrastructure.published_plan_cache(),
                    state.infrastructure.task_queue(),
                    Arc::clone(&state.runtime_event_stream),
                    state.notification_service.clone(),
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.runtime-model-operations",
            &["runtime.models.invoke"],
            &["api-server.runtime-models"],
            crate::routes::runtime_models::compile_runtime_model_interface_registry(
                crate::routes::runtime_models::runtime_model_operation_port(
                    state.store.clone(),
                    Arc::clone(&state.runtime_engine),
                    state.infrastructure.cache_store(),
                ),
            )?,
        ),
    ]);
    Ok(contributions)
}

impl InterfaceContributionCollector {
    pub(crate) fn with_managed_invocations(
        mut self,
        factory: Arc<dyn interface_runtime::ManagedInterfaceInvocationFactory>,
    ) -> Self {
        self.managed_factory = Some(factory);
        self
    }
}
