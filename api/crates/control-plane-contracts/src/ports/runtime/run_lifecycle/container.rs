use super::*;

/// Adapter input for the single atomic container state owner. The opaque state
/// belongs to orchestration-runtime; the adapter only stores its current value.
#[derive(Debug, Clone)]
pub struct CommitContainerTransitionInput {
    pub flow_run_id: Uuid,
    pub resume_claim_owner: Option<RenewResumeClaimInput>,
    pub execution_identity: domain::NodeExecutionIdentity,
    pub checkpoint: serde_json::Value,
    pub node: Option<ContainerNodeFact>,
    pub rounds: Vec<ContainerRoundFact>,
}

#[derive(Debug, Clone)]
pub enum ContainerNodeFact {
    Started {
        node_type: String,
        node_alias: String,
        input_payload: serde_json::Value,
    },
    Updated {
        status: domain::NodeRunStatus,
        output_payload: serde_json::Value,
        error_payload: Option<serde_json::Value>,
        metrics_payload: serde_json::Value,
        debug_payload: serde_json::Value,
        terminal: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ContainerRoundFact {
    pub execution_id: Uuid,
    pub container_execution_id: Uuid,
    pub round_index: u64,
    pub deadline_unix_ms: i64,
    pub status: String,
    pub error_payload: Option<serde_json::Value>,
}
