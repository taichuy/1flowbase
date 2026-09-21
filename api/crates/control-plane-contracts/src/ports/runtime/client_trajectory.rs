//! Captured client Responses facts, independent of provider diagnostics.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientTrajectoryTransport {
    Http,
    Websocket,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientTrajectoryFrameKind {
    Request,
    ResponseJson,
    ResponseSse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientTrajectoryStep {
    pub id: Uuid,
    /// Capture identity, never a fabricated protocol response/turn identifier.
    pub request_id: Uuid,
    pub sequence: i64,
    pub created_at: String,
    pub category: String,
    pub name: String,
    /// Actual Responses namespace; older metadata has no namespace.
    #[serde(default)]
    pub namespace: Option<String>,
    pub preview: String,
    pub parameters_preview: Option<String>,
    pub result_preview: Option<String>,
    pub status: String,
    pub origin: String,
    pub protocol: String,
    pub transport: ClientTrajectoryTransport,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub call_id: Option<String>,
    pub item_id: Option<String>,
    pub response_id: Option<String>,
    pub turn_id: Option<String>,
    /// Only links actual call_id facts within this run. Submitted history is not execution.
    pub related_step_id: Option<Uuid>,
    pub available_sections: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientTrajectoryPage {
    pub items: Vec<ClientTrajectoryStep>,
    pub next_cursor: Option<i64>,
    pub integrity: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientTrajectorySectionItem {
    pub sequence: i64,
    pub value: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientTrajectorySection {
    pub step_id: Uuid,
    pub request_id: Uuid,
    pub evidence_scope: String,
    pub section: String,
    pub items: Vec<ClientTrajectorySectionItem>,
    pub next_cursor: Option<i64>,
}

/// Append-only observation facts. Storage projects metadata separately from sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClientTrajectoryFact {
    /// A Native event identifies an LLM node traversed by this client request.
    /// This association does not claim that client content is provider wire data.
    NodeLink {
        node_run_id: Uuid,
    },
    Integrity {
        status: String,
        dropped_count: u64,
        persist_failed_count: u64,
    },
    Step {
        step: ClientTrajectoryStep,
    },
    Section {
        step_id: Uuid,
        section: String,
        value: Value,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendClientTrajectoryInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub request_id: Uuid,
    pub observed_at: String,
    pub fact: ClientTrajectoryFact,
}
