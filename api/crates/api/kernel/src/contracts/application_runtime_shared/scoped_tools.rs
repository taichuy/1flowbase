use anyhow::Result;
use async_trait::async_trait;
use domain::CommunicationHistoryScope;
use orchestration_runtime::{
    compiled_plan::CompiledNode,
    execution_engine::{
        RuntimeInternalToolInvoker, RuntimeInternalToolOutput, RuntimeInternalToolRegistration,
    },
};
use serde_json::Value;
use std::sync::Arc;

/// Scope is reapplied at each invocation, including tools executed in spawned runtime tasks.
/// Tokio task-local state itself is not inherited by tokio::spawn.
pub(crate) struct ScopedTools {
    pub inner: Arc<dyn RuntimeInternalToolInvoker>,
    pub scope: CommunicationHistoryScope,
}
#[async_trait]
impl RuntimeInternalToolInvoker for ScopedTools {
    async fn prepare_workflow_credentials(
        &self,
        flow_run_id: uuid::Uuid,
    ) -> Result<orchestration_runtime::execution_engine::WorkflowCredentialReferences> {
        crate::routes::communications::scope::with_scope(
            self.scope.clone(),
            self.inner.prepare_workflow_credentials(flow_run_id),
        )
        .await
    }

    async fn invoke_workflow_internal_http(
        &self,
        flow_run_id: uuid::Uuid,
        credential_reference: &str,
        request: orchestration_runtime::execution_engine::WorkflowInternalHttpRequest,
    ) -> Result<orchestration_runtime::execution_engine::WorkflowInternalHttpResponse> {
        crate::routes::communications::scope::with_scope(
            self.scope.clone(),
            self.inner
                .invoke_workflow_internal_http(flow_run_id, credential_reference, request),
        )
        .await
    }

    fn registrations_for_node(&self, node: &CompiledNode) -> Vec<RuntimeInternalToolRegistration> {
        self.inner.registrations_for_node(node)
    }
    async fn invoke_runtime_internal_tool(
        &self,
        node: &CompiledNode,
        registration: &RuntimeInternalToolRegistration,
        arguments: Value,
    ) -> Result<RuntimeInternalToolOutput> {
        crate::routes::communications::scope::with_scope(
            self.scope.clone(),
            self.inner
                .invoke_runtime_internal_tool(node, registration, arguments),
        )
        .await
    }
}
