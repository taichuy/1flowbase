use crate::routes::mcp_management::debug_execute;
use crate::routes::mcp_management::debug_execute::{McpDebugExecuteBody, McpServerBoundInputs};
use async_trait::async_trait;
use axum::http::HeaderMap;
use serde_json::Value;
#[async_trait]
pub trait McpInterfaceDispatchPort: Send + Sync + 'static {
    async fn execute_workflow_http(
        &self,
        _request: orchestration_runtime::execution_engine::WorkflowInternalHttpRequest,
    ) -> anyhow::Result<orchestration_runtime::execution_engine::WorkflowInternalHttpResponse> {
        anyhow::bail!("workflow Console dispatcher is unavailable")
    }

    async fn execute(
        &self,
        headers: HeaderMap,
        interface: domain::McpInterfaceCatalogEntry,
        body: McpDebugExecuteBody,
        server_bound_inputs: McpServerBoundInputs,
    ) -> Result<Value, debug_execute::McpDebugExecuteError>;
}
