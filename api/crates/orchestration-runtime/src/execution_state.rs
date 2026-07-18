use plugin_framework::provider_contract::ProviderStreamEvent;
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct PendingHumanInput {
    pub node_id: String,
    pub node_alias: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingCallbackTask {
    pub node_id: String,
    pub node_alias: String,
    pub callback_kind: String,
    pub request_payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionStopReason {
    Completed,
    Incomplete(ExecutionIncompleteReason),
    WaitingHuman(PendingHumanInput),
    WaitingCallback(PendingCallbackTask),
    Failed(NodeExecutionFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionIncompleteReason {
    OutputLimit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointSnapshot {
    pub next_node_index: usize,
    pub variable_pool: Map<String, Value>,
    pub active_node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeExecutionTrace {
    pub node_id: String,
    pub node_type: String,
    pub node_alias: String,
    pub input_payload: Value,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub metrics_payload: Value,
    pub debug_payload: Value,
    pub provider_events: Vec<ProviderStreamEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowDebugExecutionOutcome {
    pub stop_reason: ExecutionStopReason,
    pub variable_pool: Map<String, Value>,
    pub checkpoint_snapshot: Option<CheckpointSnapshot>,
    pub node_traces: Vec<NodeExecutionTrace>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeExecutionFailure {
    pub node_id: String,
    pub node_alias: String,
    pub error_payload: Value,
}
