use super::trajectory::{ProviderTrajectoryStep, WorkflowEventSection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEventCategory {
    #[default]
    All,
    Nodes,
    Requests,
    Tools,
    Rounds,
    Agents,
}
impl WorkflowEventCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Nodes => "nodes",
            Self::Requests => "requests",
            Self::Tools => "tools",
            Self::Rounds => "rounds",
            Self::Agents => "agents",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkflowTrajectoryQuery {
    #[serde(default)]
    pub category: WorkflowEventCategory,
    pub node_run_id: Option<Uuid>,
    pub request_id: Option<Uuid>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowTrajectoryNode {
    pub flow_run_id: Uuid,
    pub node_run_id: Uuid,
    pub node_id: String,
    pub node_alias: String,
    pub node_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowTrajectoryEvent {
    pub event_id: String,
    /// Sequence within the persisted source/run, never a global causal order.
    pub event_sequence: Option<i64>,
    pub event_type: String,
    pub created_at: String,
    pub category: String,
    pub flow_run_id: Uuid,
    pub task_run_id: Option<Uuid>,
    pub parent_task_run_id: Option<Uuid>,
    pub node_run_id: Option<Uuid>,
    pub node_id: Option<String>,
    pub node_alias: Option<String>,
    pub node_type: Option<String>,
    pub status: Option<String>,
    pub preview: String,
    pub native_step: Option<ProviderTrajectoryStep>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowTrajectoryPage {
    pub items: Vec<WorkflowTrajectoryEvent>,
    pub next_cursor: Option<String>,
    pub nodes: Vec<WorkflowTrajectoryNode>,
    pub time_start: Option<String>,
    pub time_end: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowTrajectoryBody {
    pub event_id: String,
    pub sections: Vec<WorkflowEventSection>,
}

#[derive(Debug)]
pub struct InvalidWorkflowTrajectoryQuery(pub String);
impl std::fmt::Display for InvalidWorkflowTrajectoryQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for InvalidWorkflowTrajectoryQuery {}
