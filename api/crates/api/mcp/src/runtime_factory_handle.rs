use crate::routes::mcp_protocol::virtual_ui::RuntimeInternalToolInvokerFactory;
use api_host_kernel::{error_response::ApiError, runtime_ports::RuntimeToolInvokerFactoryHandle};
use std::sync::Arc;
#[async_trait::async_trait]
impl RuntimeToolInvokerFactoryHandle for RuntimeInternalToolInvokerFactory {
    async fn create_server_delegated(
        &self,
        headers: axum::http::HeaderMap,
        actor: domain::ActorContext,
        instance_ids: Vec<String>,
    ) -> Result<
        Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
        ApiError,
    > {
        Ok(Arc::new(
            RuntimeInternalToolInvokerFactory::create_server_delegated(
                self,
                headers,
                actor,
                instance_ids,
            )
            .await?,
        ))
    }
    async fn create_workflow_credential_host(
        &self,
        actor: domain::ActorContext,
    ) -> Result<
        Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
        ApiError,
    > {
        Ok(Arc::new(
            RuntimeInternalToolInvokerFactory::create_workflow_credential_host(self, actor).await?,
        ))
    }
}
