use crate::{app_state::ApiState, error_response::ApiError};
use axum::http::HeaderMap;
use domain::{ActorContext, CommunicationBindingRecord};
use orchestration_runtime::execution_engine::RuntimeInternalToolInvoker;
use std::sync::Arc;
use storage_durable_postgres::MainDurableStore;

#[async_trait::async_trait]
pub trait RuntimeToolInvokerFactoryHandle: Send + Sync {
    async fn create_for_assistant(&self, headers: HeaderMap, actor: ActorContext, instance_ids: Vec<String>, client: Option<Arc<crate::routes::assistant::AssistantClientToolBridge>>) -> Result<Arc<dyn RuntimeInternalToolInvoker>, ApiError>;
    async fn create_server_delegated(
        &self,
        headers: HeaderMap,
        actor: ActorContext,
        instance_ids: Vec<String>,
    ) -> Result<Arc<dyn RuntimeInternalToolInvoker>, ApiError>;
    async fn create_workflow_credential_host(
        &self,
        actor: ActorContext,
    ) -> Result<Arc<dyn RuntimeInternalToolInvoker>, ApiError>;
}
#[async_trait::async_trait]
pub trait RuntimeDebugMcpPort: Send + Sync {
    async fn for_actor(
        &self,
        state: &Arc<ApiState>,
        actor: &ActorContext,
    ) -> Result<Arc<dyn RuntimeToolInvokerFactoryHandle>, ApiError>;
}
#[async_trait::async_trait]
pub trait CommunicationBindingValidator: Send + Sync {
    async fn validate_binding(
        &self,
        store: &MainDurableStore,
        binding: &CommunicationBindingRecord,
    ) -> anyhow::Result<ActorContext>;
}
#[async_trait::async_trait]
pub trait TraceProjectionRebuildPort: Send + Sync {
    async fn rebuild(
        &self,
        store: &MainDurableStore,
        application_id: uuid::Uuid,
        run_id: uuid::Uuid,
    ) -> anyhow::Result<()>;
}
