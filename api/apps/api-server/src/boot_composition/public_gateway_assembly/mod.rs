use crate::{app_state::ApiState, error_response::ApiError, routes::mcp_protocol::virtual_ui};
use std::sync::Arc;
pub(crate) struct PublicRuntimeInvokerAdapter;
#[async_trait::async_trait]
impl api_host_kernel::generation_dispatch::public_runtime_invoker::PublicRuntimeInvokerPort
    for PublicRuntimeInvokerAdapter
{
    async fn for_actor(
        &self,
        state: &Arc<ApiState>,
        actor: &control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    ) -> Result<
        Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
        ApiError,
    > {
        Ok(Arc::new(
            virtual_ui::ApiMcpRuntimeToolInvoker::new(
                crate::runtime_internal_tool_invoker_factory(state, &actor.actor).await?,
                axum::http::HeaderMap::new(),
                actor.actor.clone(),
                Vec::new(),
            )
            .await?,
        ))
    }
}
