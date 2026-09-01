use std::{collections::BTreeSet, sync::Arc};

use interface_runtime::{
    AuthorizationOperation, CompiledInterfaceRegistry, GraphFingerprint, InterfaceId,
    InterfaceOwner, RegistryCompilationError, RegistryCompiler,
};

#[derive(Clone)]
pub(crate) struct InterfaceRegistryContribution {
    contribution_id: &'static str,
    authorization_operations: &'static [&'static str],
    owners: &'static [&'static str],
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
            contribution_id,
            authorization_operations,
            owners,
            registry,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_test_contribution_id(mut self, contribution_id: &'static str) -> Self {
        self.contribution_id = contribution_id;
        self
    }
}

struct PublishedInterfaceContribution {
    registry: Arc<CompiledInterfaceRegistry>,
    interface_id: InterfaceId,
    authorization_operation: AuthorizationOperation,
    owner: InterfaceOwner,
}

pub(crate) struct InterfaceContributionCollector {
    graph_fingerprint: GraphFingerprint,
    published: Vec<PublishedInterfaceContribution>,
    contributions: Vec<InterfaceRegistryContribution>,
    contribution_ids: BTreeSet<&'static str>,
}

impl InterfaceContributionCollector {
    pub(crate) fn new(graph_fingerprint: GraphFingerprint) -> Self {
        Self {
            graph_fingerprint,
            published: Vec::new(),
            contributions: Vec::new(),
            contribution_ids: BTreeSet::new(),
        }
    }

    pub(crate) fn absorb_published_interface(
        &mut self,
        registry: Arc<CompiledInterfaceRegistry>,
        interface_id: InterfaceId,
        authorization_operation: AuthorizationOperation,
        owner: InterfaceOwner,
    ) {
        self.published.push(PublishedInterfaceContribution {
            registry,
            interface_id,
            authorization_operation,
            owner,
        });
    }

    pub(crate) fn add(
        &mut self,
        contribution: InterfaceRegistryContribution,
    ) -> anyhow::Result<()> {
        if !self.contribution_ids.insert(contribution.contribution_id) {
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
        for published in &self.published {
            operations.insert(published.authorization_operation.clone());
            owners.insert(published.owner.clone());
        }
        for contribution in &self.contributions {
            for operation in contribution.authorization_operations {
                operations.insert(AuthorizationOperation::new(operation)?);
            }
            for owner in contribution.owners {
                owners.insert(InterfaceOwner::new(owner)?);
            }
        }

        let mut compiler = RegistryCompiler::new(self.graph_fingerprint, operations, owners);
        for published in self.published {
            compiler.absorb_interface(published.registry.as_ref(), &published.interface_id)?;
        }
        for contribution in self.contributions {
            compiler.absorb_snapshot(contribution.registry.as_ref())?;
        }
        Ok(compiler.compile()?)
    }
}

pub(crate) fn production_interface_contributions(
    state: &Arc<crate::app_state::ApiState>,
) -> Result<Vec<InterfaceRegistryContribution>, RegistryCompilationError> {
    let public_login_instances = crate::routes::auth::public_login_instances_port(
        state.store.clone(),
        Arc::clone(&state.authenticator_registry),
        state.bootstrap_workspace_id,
    );
    let public_sign_in = crate::routes::sign_in_interface::public_sign_in_port(
        state.store.clone(),
        Arc::clone(&state.session_store),
        state.session_ttl_days,
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
    let console_role_access = crate::routes::role_access_interface::role_access_port(
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

    Ok(vec![
        InterfaceRegistryContribution::new(
            "api-server.public-login-instances",
            &["public.auth.login-instances.read"],
            &["api-server.public-auth"],
            crate::routes::auth::compile_public_login_instances_registry(public_login_instances)?,
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
                "auth_center.authenticators.create",
                "auth_center.authenticators.order",
                "auth_center.authenticators.enable",
                "auth_center.authenticators.copy",
                "auth_center.authenticators.update.config",
                "auth_center.authenticators.update.public-ui-block",
                "auth_center.authenticators.delete",
            ],
            &["api-server.console-auth-center"],
            crate::routes::auth_center_interface::compile_registry(console_auth_center)?,
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
            &["files.upload", "files.content.download"],
            &["api-server.console-business-files"],
            crate::routes::files::compile_registry(
                state.store.clone(),
                state.file_storage_registry.clone(),
                state.runtime_engine.clone(),
            )?,
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
        InterfaceRegistryContribution::new(
            "api-server.console-provider-catalog-logs",
            &[
                "model_providers.catalog.view",
                "model_providers.request_logs.view",
                "model_providers.request_logs.delete",
                "model_providers.request_logs.clear",
            ],
            &["api-server.console-provider-catalog-logs"],
            crate::routes::model_providers::catalog_logs_interface::compile_registry(
                crate::routes::model_providers::catalog_logs_interface::ProviderCatalogLogsDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                    install_root: state.provider_install_root.clone(),
                    cache_store: state.infrastructure.cache_store(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-provider-instance-lifecycle",
            &[
                "model_providers.instances.view",
                "model_providers.instances.create",
                "model_providers.instances.update",
                "model_providers.instances.validate",
                "model_providers.instances.delete",
            ],
            &["api-server.console-provider-instance-lifecycle"],
            crate::routes::model_providers::instance_lifecycle_interface::compile_registry(
                crate::routes::model_providers::instance_lifecycle_interface::ProviderInstanceLifecycleDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                    install_root: state.provider_install_root.clone(),
                    cache_store: state.infrastructure.cache_store(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-provider-instance-operations",
            &[
                "model_providers.instances.authenticate",
                "model_providers.instances.usage.view",
                "model_providers.instances.reset_credits.view",
                "model_providers.instances.reset_credits.consume",
                "model_providers.balance.view",
                "model_providers.preview.view",
                "model_providers.instances.secrets.reveal",
            ],
            &["api-server.console-provider-instance-operations"],
            crate::routes::model_providers::instance_operations_interface::compile_registry(
                crate::routes::model_providers::instance_operations_interface::ProviderInstanceOperationsDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                    install_root: state.provider_install_root.clone(),
                    cache_store: state.infrastructure.cache_store(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-provider-routing",
            &["model_providers.main_instance.view", "model_providers.main_instance.update"],
            &["api-server.console-provider-routing"],
            crate::routes::model_providers::routing_interface::compile_registry(
                crate::routes::model_providers::routing_interface::ProviderRoutingDependencies {
                    store: state.store.clone(), provider_runtime: state.provider_runtime.clone(), secret_key: state.provider_secret_master_key.clone(), api_node_id: state.api_node_id.clone(), install_root: state.provider_install_root.clone(), cache_store: state.infrastructure.cache_store(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-provider-discovery",
            &["model_providers.instances.models.view", "model_providers.instances.models.refresh", "model_providers.options.view", "model_providers.settings_options.view"],
            &["api-server.console-provider-discovery"],
            crate::routes::model_providers::discovery_interface::compile_registry(crate::routes::model_providers::discovery_interface::ProviderDiscoveryDependencies { store: state.store.clone(), provider_runtime: state.provider_runtime.clone(), secret_key: state.provider_secret_master_key.clone(), api_node_id: state.api_node_id.clone(), install_root: state.provider_install_root.clone(), cache_store: state.infrastructure.cache_store() })?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.native-runs",
            &["application.native.runs.create"],
            &["api-server.application-public-api"],
            crate::routes::application_public_api::native::compile_native_interface_registry(
                Arc::clone(state) as Arc<dyn crate::routes::application_public_api::native_interface::ApplicationNativeRunPort>,
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
        InterfaceRegistryContribution::new(
            "api-server.console-network-pools",
            &[
                "network_egress_pools.list",
                "network_egress_proxies.create",
                "network_egress_pool_members.test_connection",
                "network_egress_pools.create",
                "network_egress_pools.update",
                "network_egress_pools.delete",
                "network_egress_pool_members.create",
                "network_egress_pool_members.create_static_http",
                "network_egress_pool_members.add_provider_egresses",
                "network_egress_pool_members.update",
                "network_egress_pool_members.delete",
            ],
            &["api-server.console-network-pools"],
            crate::routes::network_center::pools_interface::compile_registry(
                crate::routes::network_center::pools_interface::NetworkPoolsDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-network-center",
            &[
                "network_egress_providers.list",
                "network_egress_proxy_types.list",
                "network_egress_providers.create",
                "network_egress_providers.lifecycle.update",
                "network_egress_providers.sync",
                "network_egress_routes.list",
                "network_egress_routes.create",
                "network_egress_routes.update",
                "network_egress_routes.delete",
            ],
            &["api-server.console-network-center"],
            crate::routes::network_center::core_interface::compile_registry(
                crate::routes::network_center::core_interface::NetworkCenterDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-network-plugins",
            &[
                "network_egress_plugins.official_catalog.view",
                "network_egress_plugins.families.view",
                "network_egress_plugins.families.switch",
                "network_egress_plugins.families.uninstall",
                "network_egress_plugins.install.official",
                "network_egress_plugins.install.upload",
            ],
            &["api-server.console-network-plugins"],
            crate::routes::network_center::plugins::plugins_interface::compile_registry(
                crate::routes::network_center::plugins::plugins_interface::NetworkPluginDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    official_plugin_source: state.official_plugin_source.clone(),
                    official_catalog_source: state.official_extension_catalog_source.clone(),
                    cache_store: state.infrastructure.cache_store(),
                    provider_install_root: state.provider_install_root.clone(),
                    provider_secret_master_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                    bootstrap_workspace_id: state.bootstrap_workspace_id,
                    allow_uploaded_host_extensions: state.allow_uploaded_host_extensions,
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-plugins",
            &[
                "plugins.catalog.view",
                "plugins.families.view",
                "plugins.official_catalog.view",
                "plugins.install",
                "plugins.install.upload",
                "plugins.install.official",
                "plugins.catalog_projection.refresh",
                "plugins.artifact.refresh",
                "plugins.artifact.install",
                "plugins.families.upgrade",
                "plugins.families.switch",
                "plugins.families.delete",
                "plugins.enable",
                "plugins.assign",
                "plugins.tasks.view",
                "model_provider_plugins.families.view",
                "model_provider_plugins.official_catalog.view",
                "model_provider_plugins.install.official",
                "model_provider_plugins.install.upload",
                "model_provider_plugins.artifact.refresh",
                "model_provider_plugins.artifact.install",
                "model_provider_plugins.families.upgrade",
                "model_provider_plugins.families.switch",
                "model_provider_plugins.families.delete",
                "model_provider_plugins.tasks.view",
            ],
            &["api-server.console-plugins"],
            crate::routes::plugins::interface::compile_registry(
                crate::routes::plugins::interface::PluginInterfaceDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    official_plugin_source: state.official_plugin_source.clone(),
                    official_catalog_source: state.official_extension_catalog_source.clone(),
                    cache_store: state.infrastructure.cache_store(),
                    provider_install_root: state.provider_install_root.clone(),
                    api_node_id: state.api_node_id.clone(),
                    bootstrap_workspace_id: state.bootstrap_workspace_id,
                    allow_uploaded_host_extensions: state.allow_uploaded_host_extensions,
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-extension-center",
            &[
                "extension_center.installed.view",
                "extension_center.installed.select",
                "extension_center.installed.enable",
                "extension_center.installed.disable",
                "extension_center.installed.delete",
                "extension_center.catalog.view",
                "extension_center.catalog.detail",
                "extension_center.update_check",
                "extension_center.install",
                "extension_center.update",
                "extension_center.install.upload",
            ],
            &["api-server.console-extension-center"],
            crate::routes::plugins::extension_center::interface::compile_registry(
                crate::routes::plugins::extension_center::ExtensionCenterDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    official_plugin_source: state.official_plugin_source.clone(),
                    official_mcp_bundle_source: state.official_mcp_bundle_source.clone(),
                    official_extension_catalog_source: state.official_extension_catalog_source.clone(),
                    cache_store: state.infrastructure.cache_store(),
                    provider_install_root: state.provider_install_root.clone(),
                    api_node_id: state.api_node_id.clone(),
                    allow_uploaded_host_extensions: state.allow_uploaded_host_extensions,
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-mcp-core",
            &[
                "mcp.client_credential.reveal",
                "mcp.client_credential.save",
                "mcp.client_credential.delete",
                "mcp.instances.view",
                "mcp.instances.create",
                "mcp.instances.copy",
                "mcp.instances.update",
                "mcp.instances.delete",
                "mcp.groups.upsert",
                "mcp.groups.move",
                "mcp.groups.delete",
                "mcp.tool_bindings.create",
                "mcp.tool_bindings.update",
                "mcp.tool_bindings.delete",
                "mcp.discovery_policy.view",
                "mcp.discovery_policy.update",
            ],
            &["api-server.console-mcp-core"],
            crate::routes::mcp_management::interface_core::compile_registry(
                crate::routes::mcp_management::interface_core::McpCoreDependencies {
                    store: state.store.clone(),
                    provider_secret_master_key: state.provider_secret_master_key.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-mcp-catalog",
            &["mcp.catalog.view", "mcp.catalog.export"],
            &["api-server.console-mcp-catalog"],
            crate::routes::mcp_management::interface_catalog_routes::compile_registry(
                crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
                    store: state.store.clone(),
                    openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                        store: state.store.clone(),
                        console_operations: state.console_operation_registry.inventory().clone(),
                        interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()),
                        api_docs: Arc::clone(&state.api_docs),
                        template_catalog: state.runtime_engine.template_catalog().clone(),
                    },
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-mcp-tools",
            &[
                "mcp.tools.view",
                "mcp.tools.create",
                "mcp.tools.update",
                "mcp.tools.delete",
                "mcp.tools.description.refresh",
                "mcp.tools.description.check",
            ],
            &["api-server.console-mcp-tools"],
            crate::routes::mcp_management::interface_tools::compile_registry(
                crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
                    store: state.store.clone(),
                    openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                        store: state.store.clone(),
                        console_operations: state.console_operation_registry.inventory().clone(),
                        interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()),
                        api_docs: Arc::clone(&state.api_docs),
                        template_catalog: state.runtime_engine.template_catalog().clone(),
                    },
                },
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
            ],
            &["api-server.console-frontstage-pages"],
            crate::routes::frontstage::interface_pages::compile_registry(
                crate::routes::frontstage::interface_pages::FrontstagePagesDependencies {
                    store: state.store.clone(),
                    bootstrap_workspace_id: state.bootstrap_workspace_id,
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
                    store: state.store.clone(),
                    api_node_id: state.api_node_id.clone(),
                },
            )?,
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
                    crate::routes::application_public_api::native::api_provider_runtime(state),
                    Arc::clone(&state.runtime_engine),
                    state.provider_secret_master_key.clone(),
                    state.api_node_id.clone(),
                    state.provider_install_root.clone(),
                    Arc::clone(&state.file_storage_registry),
                    state.infrastructure.cache_store(),
                    state.infrastructure.task_queue(),
                    state.infrastructure.provider_transport_store(),
                    Arc::clone(&state.runtime_event_stream),
                    Arc::clone(&state.runtime_activity),
                    crate::routes::application_public_api::native::native_runtime_invoker_factory(
                        Arc::clone(state),
                    ),
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
                state.clone(),
                crate::routes::application_public_api::compatibility_interface::compatibility_models_port(
                    state.store.clone(),
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.mcp",
            &["mcp.tools.invoke"],
            &["api-server.mcp-protocol"],
            crate::routes::mcp_protocol::compile_mcp_interface_registry(state.clone())?,
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
                    state.api_node_id.clone(),
                    state.provider_install_root.clone(),
                    Arc::clone(&state.file_storage_registry),
                    state.infrastructure.cache_store(),
                    state.infrastructure.task_queue(),
                    Arc::clone(&state.runtime_event_stream),
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
    ])
}
