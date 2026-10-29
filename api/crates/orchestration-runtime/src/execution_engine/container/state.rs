use crate::execution_state::NodeExecutionTrace;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Logical execution identity survives recovery. The persistence owner supplies
/// and validates its resume claim/generation independently of this logical key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedExecutionIdentity {
    pub execution_id: Uuid,
    pub node_id: String,
    pub parent_execution_id: Option<Uuid>,
    pub round_path: Vec<ContainerRoundPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRoundPath {
    pub container_execution_id: Uuid,
    pub container_node_id: String,
    pub round_index: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerCheckpoint {
    pub version: u32,
    pub root_next_node_index: usize,
    pub root_variable_pool: Map<String, Value>,
    pub root_active_node_ids: Vec<String>,
    pub frames: BTreeMap<Uuid, ContainerFrame>,
    pub execution_budget: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerFrame {
    pub identity: ScopedExecutionIdentity,
    pub frozen_limit: u64,
    pub started_rounds: u64,
    pub frozen_items: Option<Vec<Value>>,
    pub variables: Map<String, Value>,
    pub rounds: BTreeMap<u64, ContainerRound>,
    pub results: BTreeMap<u64, Value>,
    pub closed: bool,
    pub global_failure: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerRound {
    pub execution_id: Uuid,
    pub round_index: u64,
    /// Absolute Unix milliseconds; never renewed on resume.
    pub deadline_unix_ms: i64,
    pub deadline_owner_execution_id: Uuid,
    pub status: ContainerRoundStatus,
    pub next_node_index: usize,
    pub variable_pool: Map<String, Value>,
    pub active_node_ids: Vec<String>,
    pub nodes: BTreeMap<String, ScopedNodeState>,
    pub waiting: Option<ContainerWait>,
    pub error_payload: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerRoundStatus {
    Running,
    Waiting,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopedNodeState {
    pub identity: ScopedExecutionIdentity,
    pub input_payload: Value,
    /// Presence is the durable terminal fact; absence means the external result
    /// may be uncertain and must not be automatically replayed.
    pub terminal_trace: Option<NodeExecutionTrace>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContainerWait {
    Human {
        node_id: String,
        node_alias: String,
        prompt: String,
    },
    Callback {
        node_id: String,
        node_alias: String,
        callback_kind: String,
        request_payload: Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerTransitionKind {
    FrameEntered,
    RoundStarted,
    NodeStarted,
    NodeCompleted,
    ScopeAdvanced,
    ScopeSuspended,
    RoundCommitted,
    FrameClosed,
    Resumed,
    InternalToolStarted,
    InternalToolFinished,
}

/// One owner serializes these calls. A durable consumer must atomically persist
/// the checkpoint and associated node/round facts, rejecting stale generations.
/// References avoid cloning the complete checkpoint at every transition.
#[derive(Debug)]
pub struct ContainerTransition<'a> {
    pub kind: ContainerTransitionKind,
    pub identity: &'a ScopedExecutionIdentity,
    pub checkpoint: &'a ContainerCheckpoint,
    pub input_payload: Option<&'a Value>,
    pub trace: Option<&'a NodeExecutionTrace>,
    pub detail: Option<&'a Value>,
}
