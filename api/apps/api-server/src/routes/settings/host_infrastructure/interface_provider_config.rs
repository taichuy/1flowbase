use std::sync::Arc;

use control_plane::host_infrastructure_config::{
    HostInfrastructureConfigService, SaveHostInfrastructureProviderConfigCommand,
};
use interface_runtime::InterfaceContract;
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    SaveHostInfrastructureProviderConfigBody, SaveHostInfrastructureProviderConfigResponse,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) struct HostInfrastructureProviderConfigInput {
    pub(crate) installation_id: String,
    pub(crate) provider_code: String,
    pub(crate) body: SaveHostInfrastructureProviderConfigBody,
}

impl InterfaceContract for HostInfrastructureProviderConfigInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[
            ("installation_id", mp::text_schema()),
            ("provider_code", mp::text_schema()),
            (
                "body",
                mp::object_schema(&[
                    (
                        "enabled_contracts",
                        mp::object_schema(&[("item_count", mp::count_schema())]),
                    ),
                    ("config_json", mp::json_summary_schema()),
                ]),
            ),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[
            ("installation_id", mp::text(&(self).installation_id)?),
            ("provider_code", mp::text(&(self).provider_code)?),
            (
                "body",
                mp::object_value(&[
                    (
                        "enabled_contracts",
                        mp::object_value(&[(
                            "item_count",
                            serde_json::json!((&(&(self).body).enabled_contracts).len()),
                        )]),
                    ),
                    ("config_json", mp::json_summary(&(&(self).body).config_json)),
                ]),
            ),
        ]))
    }

    const CONTRACT_ID: &'static str = "console-host-infrastructure-provider-config-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct HostInfrastructureProviderConfigOutput {
    response: SaveHostInfrastructureProviderConfigResponse,
}

impl HostInfrastructureProviderConfigOutput {
    pub(crate) fn into_response(self) -> SaveHostInfrastructureProviderConfigResponse {
        self.response
    }
}

impl InterfaceContract for HostInfrastructureProviderConfigOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "response",
            mp::object_schema(&[
                ("restart_required", serde_json::json!({"type":"boolean"})),
                (
                    "installation_desired_state",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "provider_config_status",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "response",
            mp::object_value(&[
                (
                    "restart_required",
                    serde_json::Value::Bool(*(&(&(self).response).restart_required)),
                ),
                (
                    "installation_desired_state",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((&(&(self).response).installation_desired_state).len()),
                    )]),
                ),
                (
                    "provider_config_status",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((&(&(self).response).provider_config_status).len()),
                    )]),
                ),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-host-infrastructure-provider-config-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct HostInfrastructureProviderConfigDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) api_node_id: String,
}

struct HostInfrastructureProviderConfigAdapter(HostInfrastructureProviderConfigDependencies);

impl HostInfrastructureProviderConfigAdapter {
    async fn execute_inner(
        &self,
        principal: &interface_runtime::UserPrincipal,
        input: HostInfrastructureProviderConfigInput,
    ) -> Result<HostInfrastructureProviderConfigOutput, ApiError> {
        let installation_id = Uuid::parse_str(&input.installation_id).map_err(|_| {
            control_plane::errors::ControlPlaneError::InvalidInput("installation_id")
        })?;
        let result =
            HostInfrastructureConfigService::new(self.0.store.clone(), self.0.api_node_id.clone())
                .save_provider_config(SaveHostInfrastructureProviderConfigCommand {
                    actor_user_id: principal.actor().user_id,
                    installation_id,
                    provider_code: input.provider_code,
                    enabled_contracts: input.body.enabled_contracts,
                    config_json: input.body.config_json,
                })
                .await?;
        Ok(HostInfrastructureProviderConfigOutput {
            response: SaveHostInfrastructureProviderConfigResponse {
                restart_required: result.restart_required,
                installation_desired_state: result.installation_desired_state,
                provider_config_status: result.provider_config_status,
            },
        })
    }
}

impl
    ConsoleInterfacePort<
        HostInfrastructureProviderConfigInput,
        HostInfrastructureProviderConfigOutput,
    > for HostInfrastructureProviderConfigAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a interface_runtime::UserPrincipal,
        input: HostInfrastructureProviderConfigInput,
    ) -> ConsoleInterfaceFuture<'a, HostInfrastructureProviderConfigOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "host_infrastructure.providers.configure",
    binding_id: "http.console.host-infrastructure.providers.configure.v1",
    method: "PUT",
    path: "/api/console/settings/host-infrastructure/providers/:installation_id/:provider_code/config",
    mutating: true,
}];

pub(crate) fn compile_registry(
    dependencies: HostInfrastructureProviderConfigDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-host-infrastructure-provider-config",
        "graph:console-host-infrastructure-provider-config-v1",
        DECLARATIONS,
        Arc::new(HostInfrastructureProviderConfigAdapter(dependencies)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use interface_runtime::BindingId;

    struct Unavailable;

    impl
        ConsoleInterfacePort<
            HostInfrastructureProviderConfigInput,
            HostInfrastructureProviderConfigOutput,
        > for Unavailable
    {
        fn execute<'a>(
            &'a self,
            _principal: &'a interface_runtime::UserPrincipal,
            _input: HostInfrastructureProviderConfigInput,
        ) -> ConsoleInterfaceFuture<'a, HostInfrastructureProviderConfigOutput> {
            Box::pin(async {
                Err(ConsoleInterfaceTargetError(
                    anyhow::anyhow!("fixture unavailable").into(),
                ))
            })
        }
    }

    #[test]
    fn eil_f11_d3_registry_freezes_provider_config_binding() {
        let registry = console_interface::compile_registry(
            "api-server.console-host-infrastructure-provider-config",
            "graph:console-host-infrastructure-provider-config-v1",
            DECLARATIONS,
            Arc::new(Unavailable),
        )
        .unwrap();
        assert!(
            registry
                .binding(&BindingId::new(DECLARATIONS[0].binding_id).unwrap())
                .is_some()
        );
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
