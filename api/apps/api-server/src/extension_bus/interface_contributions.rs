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
        );
    let console_applications = crate::routes::applications::interface::applications_port(
        state.store.clone(),
        state.bootstrap_workspace_id,
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
            "api-server.console-application-orchestration",
            &[
                "applications.orchestration.get",
                "applications.orchestration.draft.save",
                "applications.orchestration.version.restore",
                "applications.orchestration.version.update",
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
            "api-server.native-runs",
            &["application.native.runs.create"],
            &["api-server.application-public-api"],
            crate::routes::application_public_api::native::compile_native_interface_registry(
                Arc::clone(state) as Arc<dyn crate::routes::application_public_api::native_interface::ApplicationNativeRunPort>,
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
