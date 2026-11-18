//! Narrow host composition ports; ownership and concrete business implementations stay in their domains.
use crate::{app_state::ApiState, error_response::ApiError, generation_dispatch::GenerationLease};
use std::sync::Arc;
pub trait CurrentGenerationSource: Send + Sync + 'static {
    fn current_generation(&self) -> Option<GenerationLease>;
}
#[async_trait::async_trait]
pub trait ApplicationLogExchangePort: Send + Sync + 'static {
    async fn execute(
        &self,
        state: &Arc<ApiState>,
        actor: &domain::ActorContext,
        request: extension_contracts::ApplicationLogsRequest,
    ) -> Result<extension_contracts::ApplicationLogsResponse, ApiError>;
}
