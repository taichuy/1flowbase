use std::sync::Arc;

use async_trait::async_trait;
use control_plane::mcp_management::McpManagementService;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;

use super::{
    debug_execute::{self, McpDebugDispatchError, McpDebugExecuteBody, McpServerBoundInputs},
    interface_catalog::{McpInterfaceCatalogDependencies, bindable_mcp_interface_with},
};
use crate::{
    error_response::ApiError,
    openapi_interface::{
        CallableDispatchForwarding, CallableDispatchHttpResponse, CallableDispatchPort,
    },
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

#[async_trait]
pub(crate) trait McpDebugActivatedOperationPort: Send + Sync {
    async fn providers_view(&self, principal: &UserPrincipal) -> Result<Value, ApiError>;
}

#[derive(Clone)]
pub(crate) struct McpDebugDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) catalog: McpInterfaceCatalogDependencies,
    pub(crate) dispatcher: Arc<dyn CallableDispatchPort>,
    pub(crate) activated_operations: Arc<dyn McpDebugActivatedOperationPort>,
}

pub(crate) struct McpDebugInput {
    pub(crate) body: McpDebugExecuteBody,
    pub(crate) forwarding: CallableDispatchForwarding,
}

impl InterfaceContract for McpDebugInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "body",
            mp::object_schema(&[
                ("interface_id", mp::text_schema()),
                (
                    "debug_response_mode",
                    mp::union_schema(vec![
                        mp::object_schema(&[("variant", mp::tag_schema("ToolResult"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("DebugDetails"))]),
                    ]),
                ),
                ("mcp_arguments", mp::json_summary_schema()),
                ("input_mapping", mp::json_summary_schema()),
                ("output_mapping", mp::json_summary_schema()),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[("body",mp::object_value(&[("interface_id",mp::text(&(&(self).body).interface_id)?), ("debug_response_mode",match &(&(self).body).debug_response_mode {crate::routes::settings_group::mcp_management::debug_execute::McpDebugResponseMode::ToolResult => mp::object_value(&[("variant",serde_json::Value::String("ToolResult".to_owned()))]), crate::routes::settings_group::mcp_management::debug_execute::McpDebugResponseMode::DebugDetails => mp::object_value(&[("variant",serde_json::Value::String("DebugDetails".to_owned()))])}), ("mcp_arguments",mp::json_summary(&(&(self).body).mcp_arguments)), ("input_mapping",mp::json_summary(&(&(self).body).input_mapping)), ("output_mapping",mp::json_summary(&(&(self).body).output_mapping))]))]))
    }

    const CONTRACT_ID: &'static str = "console-mcp-debug-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum McpDebugOutput {
    Json(Value),
    Target(CallableDispatchHttpResponse),
}

impl InterfaceContract for McpDebugOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Json")),
                ("0", mp::json_summary_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Target")),
                (
                    "0",
                    mp::object_schema(&[
                        ("status", serde_json::json!({"type":"integer"})),
                        (
                            "body",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Json(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Json".to_owned())),
                ("0", mp::json_summary(_field_0)),
            ]),
            Self::Target(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Target".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("status", serde_json::json!(*(&(_field_0).status))),
                        (
                            "body",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).body).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-mcp-debug-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct McpDebugAdapter(McpDebugDependencies);

pub(crate) fn port(
    dependencies: McpDebugDependencies,
) -> Arc<dyn ConsoleInterfacePort<McpDebugInput, McpDebugOutput>> {
    Arc::new(McpDebugAdapter(dependencies))
}

impl McpDebugAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: McpDebugInput,
    ) -> Result<McpDebugOutput, ApiError> {
        let actor = principal.actor();
        McpManagementService::new(self.0.store.clone())
            .authorize_debug_execute(actor.user_id)
            .await?;
        let interface_entry =
            bindable_mcp_interface_with(&self.0.catalog, actor, &input.body.interface_id).await?;
        let activated_interface_response = if interface_entry.interface_id
            == crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
        {
            Some(self.0.activated_operations.providers_view(principal).await?)
        } else {
            None
        };
        match debug_execute::execute_with_dispatch_port(
            self.0.dispatcher.as_ref(),
            input.forwarding,
            interface_entry,
            input.body,
            McpServerBoundInputs {
                workspace_id: actor.current_workspace_id,
            },
            activated_interface_response,
        )
        .await
        {
            Ok(value) => Ok(McpDebugOutput::Json(value)),
            Err(McpDebugDispatchError::Api(error)) => Err(ApiError::from(error)),
            Err(McpDebugDispatchError::Target(response)) => Ok(McpDebugOutput::Target(response)),
        }
    }
}

impl ConsoleInterfacePort<McpDebugInput, McpDebugOutput> for McpDebugAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: McpDebugInput,
    ) -> ConsoleInterfaceFuture<'a, McpDebugOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "mcp.debug.execute",
    binding_id: "http.console.mcp.debug.execute.v1",
    method: "POST",
    path: "/api/console/mcp/debug/execute",
    mutating: true,
}];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<McpDebugInput, McpDebugOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-mcp-debug",
        "graph:console-mcp-debug-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableMcpDebugPort;

#[cfg(test)]
impl ConsoleInterfacePort<McpDebugInput, McpDebugOutput> for UnavailableMcpDebugPort {
    fn execute<'a>(
        &'a self,
        _: &'a UserPrincipal,
        _: McpDebugInput,
    ) -> ConsoleInterfaceFuture<'a, McpDebugOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("MCP debug fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f11c_registry_freezes_mcp_debug_binding() {
        let registry = compile_registry(Arc::new(UnavailableMcpDebugPort)).unwrap();
        let declaration = DECLARATIONS.first().unwrap();
        let binding = registry
            .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
            .expect("declared MCP debug binding must be frozen");
        let route = binding.projection().http_route().unwrap();
        assert_eq!(route.method(), declaration.method);
        assert_eq!(route.path(), declaration.path);
    }
}
