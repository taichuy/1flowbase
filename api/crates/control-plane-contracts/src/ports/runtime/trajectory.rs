use async_trait::async_trait;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderTrajectoryStep {
    pub event_id: Uuid,
    pub event_sequence: i64,
    pub event_type: String,
    pub created_at: String,
    /// Narrow, persisted metadata; never contains protocol body.
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderTrajectoryPage {
    pub items: Vec<ProviderTrajectoryStep>,
    pub next_cursor: Option<i64>,
    pub observation_count: i64,
    pub persist_failed_count: i64,
    pub integrity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderTrajectoryBody {
    pub event_id: Uuid,
    pub body: String,
    pub encoding: String,
}

#[async_trait]
pub trait ProviderTrajectoryRepository: Send + Sync {
    async fn provider_trajectory_page(
        &self,
        flow_run_id: Uuid,
        node_run_id: Uuid,
        cursor: Option<i64>,
        limit: i64,
    ) -> anyhow::Result<ProviderTrajectoryPage>;
    async fn provider_trajectory_body(
        &self,
        flow_run_id: Uuid,
        node_run_id: Uuid,
        event_id: Uuid,
    ) -> anyhow::Result<Option<ProviderTrajectoryBody>>;
}
