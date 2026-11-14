use crate::{app_state::ApiState, error_response::ApiError};
use std::sync::Arc;
#[async_trait::async_trait]
pub trait PublicRuntimeInvokerPort: Send + Sync {
    async fn for_actor(
        &self,
        state: &Arc<ApiState>,
        actor: &control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    ) -> Result<
        Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
        ApiError,
    >;
}
