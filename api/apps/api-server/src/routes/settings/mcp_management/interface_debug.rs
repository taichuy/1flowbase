use std::sync::Arc;

use async_trait::async_trait;
use control_plane::mcp_management::McpManagementService;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;

use super::{
    debug_execute::{self, McpDebugDispatchError, McpDebugExecuteBody, McpServerBoundInputs},
    interface_catalog::{bindable_mcp_interface_with, McpInterfaceCatalogDependencies},
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
    const CONTRACT_ID: &'static str = "console-mcp-debug-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum McpDebugOutput {
    Json(Value),
    Target(CallableDispatchHttpResponse),
}

impl InterfaceContract for McpDebugOutput {
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
