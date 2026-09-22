use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTrajectoryView {
    #[default]
    Semantic,
    Protocol,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderTrajectoryStep {
    pub event_id: Uuid,
    pub event_sequence: i64,
    pub event_type: String,
    pub created_at: String,
    /// Narrow, persisted metadata; never contains protocol body.
    pub metadata: serde_json::Value,
    pub links: Vec<WorkflowTrajectoryLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTrajectoryLink {
    pub relation: String,
    pub flow_run_id: Uuid,
    pub request_id: Uuid,
    pub response_id: Option<String>,
}

#[derive(Debug)]
pub struct TrajectoryTargetNotFound;
impl std::fmt::Display for TrajectoryTargetNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("trajectory target not found")
    }
}
impl std::error::Error for TrajectoryTargetNotFound {}

/// Internal index selection; target_id is the native event or client step identity.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrajectorySelection {
    pub request_id: Option<Uuid>,
    pub target_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowEventSection {
    pub kind: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderTrajectoryPage {
    pub items: Vec<ProviderTrajectoryStep>,
    pub next_cursor: Option<i64>,
    pub observation_count: i64,
    pub persist_failed_count: i64,
    pub integrity: String,
    pub protocol_integrity: String,
    pub protocol_persist_failed_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderTrajectoryEvidence {
    pub event_id: Uuid,
    pub sequence: i64,
    pub body: String,
    pub encoding: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderTrajectoryBody {
    pub event_id: Uuid,
    pub source: String,
    pub evidence_scope: String,
    pub sections: Vec<WorkflowEventSection>,
    pub items: Vec<ProviderTrajectoryEvidence>,
    pub next_cursor: Option<i64>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationRunPayloadSection {
    InputPayload,
    OutputPayload,
}

#[async_trait]
pub trait ProviderTrajectoryRepository: Send + Sync {
    async fn provider_trajectory_filtered_page(
        &self,
        flow_run_id: Uuid,
        node_run_id: Option<Uuid>,
        cursor: Option<i64>,
        limit: i64,
        selection: TrajectorySelection,
    ) -> anyhow::Result<ProviderTrajectoryPage>;
    async fn client_trajectory_filtered_page(
        &self,
        flow_run_id: Uuid,
        node_run_id: Option<Uuid>,
        cursor: Option<i64>,
        limit: i64,
        selection: TrajectorySelection,
    ) -> anyhow::Result<super::ClientTrajectoryPage>;

    async fn provider_run_trajectory_page(
        &self,
        flow_run_id: Uuid,
        cursor: Option<i64>,
        limit: i64,
    ) -> anyhow::Result<ProviderTrajectoryPage>;
    async fn application_run_payload(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        section: ApplicationRunPayloadSection,
    ) -> anyhow::Result<Option<serde_json::Value>>;
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
        cursor: Option<i64>,
        limit: i64,
        view: ProviderTrajectoryView,
    ) -> anyhow::Result<Option<ProviderTrajectoryBody>>;
}
