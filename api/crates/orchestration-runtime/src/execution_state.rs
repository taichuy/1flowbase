use anyhow::{bail, Result};
pub use extension_contracts::semantic_terminal::{
    CompactOperationReceipt, CountTokensReceipt, NativeOperationTerminal,
};
use extension_contracts::{
    provider_contract::ProviderStreamEvent,
    semantic_terminal::{COMPACT_OPERATION_TERMINAL_KIND, COUNT_TOKENS_TERMINAL_KIND},
};
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

/// Decode the runtime-owned Compact receipt from a semantic terminal trace.
/// Non-Compact traces deliberately return `None` so terminal owners
/// cannot mistake an Answer or arbitrary node payload for a Compact result.
pub fn compact_operation_receipt_from_trace(
    trace: &NodeExecutionTrace,
) -> Result<Option<CompactOperationReceipt>> {
    if trace
        .output_payload
        .get("semantic_terminal")
        .and_then(Value::as_str)
        != Some(COMPACT_OPERATION_TERMINAL_KIND)
    {
        return Ok(None);
    }

    CompactOperationReceipt::from_payload(&trace.output_payload).map(Some)
}

pub fn compact_operation_receipt_from_traces(
    traces: &[NodeExecutionTrace],
) -> Result<CompactOperationReceipt> {
    let receipts = traces
        .iter()
        .map(compact_operation_receipt_from_trace)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    match receipts.as_slice() {
        [receipt] => Ok(receipt.clone()),
        [] => bail!("Compact workflow completed without a typed Compact terminal"),
        _ => bail!(
            "Compact workflow completed with {} typed Compact terminals; expected exactly one",
            receipts.len()
        ),
    }
}

pub fn count_tokens_receipt_from_trace(
    trace: &NodeExecutionTrace,
) -> Result<Option<CountTokensReceipt>> {
    if trace
        .output_payload
        .get("semantic_terminal")
        .and_then(Value::as_str)
        != Some(COUNT_TOKENS_TERMINAL_KIND)
    {
        return Ok(None);
    }

    CountTokensReceipt::from_payload(&trace.output_payload).map(Some)
}

pub fn count_tokens_receipt_from_traces(
    traces: &[NodeExecutionTrace],
) -> Result<CountTokensReceipt> {
    let receipts = traces
        .iter()
        .map(count_tokens_receipt_from_trace)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    match receipts.as_slice() {
        [receipt] => Ok(receipt.clone()),
        [] => bail!("CountTokens workflow completed without a typed token-count terminal"),
        _ => bail!(
            "CountTokens workflow completed with {} typed token-count terminals; expected exactly one",
            receipts.len()
        ),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowDebugExecutionOutcome {
    pub stop_reason: ExecutionStopReason,
    pub variable_pool: Map<String, Value>,
    pub checkpoint_snapshot: Option<CheckpointSnapshot>,
    pub operation_terminal: Option<NativeOperationTerminal>,
    pub node_traces: Vec<NodeExecutionTrace>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeExecutionFailure {
    pub node_id: String,
    pub node_alias: String,
    pub error_payload: Value,
}
