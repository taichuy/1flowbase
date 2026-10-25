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
pub(super) struct ScopedTools {
    pub inner: Arc<dyn RuntimeInternalToolInvoker>,
    pub scope: CommunicationHistoryScope,
}
#[async_trait]
impl RuntimeInternalToolInvoker for ScopedTools {
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
