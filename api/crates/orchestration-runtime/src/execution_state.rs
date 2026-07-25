use anyhow::{anyhow, bail, Result};
use plugin_framework::provider_contract::{
    ProviderCompactProfile, ProviderCompactResult, ProviderCountTokensResult, ProviderStreamEvent,
    ProviderWireOperation,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

const COMPACT_OPERATION_TERMINAL_KIND: &str = "compact";
const COUNT_TOKENS_TERMINAL_KIND: &str = "count_tokens";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountTokensReceipt {
    result: ProviderCountTokensResult,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeOperationTerminal {
    CountTokens(CountTokensReceipt),
    Compact(CompactOperationReceipt),
}

impl NativeOperationTerminal {
    pub fn from_payload(payload: &Value) -> Result<Option<Self>> {
        match payload.get("semantic_terminal").and_then(Value::as_str) {
            Some(COUNT_TOKENS_TERMINAL_KIND) => CountTokensReceipt::from_payload(payload)
                .map(Self::CountTokens)
                .map(Some),
            Some(COMPACT_OPERATION_TERMINAL_KIND) => CompactOperationReceipt::from_payload(payload)
                .map(Self::Compact)
                .map(Some),
            _ => Ok(None),
        }
    }

    pub fn as_payload(&self) -> Result<Value> {
        match self {
            Self::CountTokens(receipt) => receipt.as_payload(),
            Self::Compact(receipt) => receipt.as_payload(),
        }
    }
}

impl Serialize for NativeOperationTerminal {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.as_payload()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NativeOperationTerminal {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let payload = Value::deserialize(deserializer)?;
        Self::from_payload(&payload)
            .map_err(serde::de::Error::custom)?
            .ok_or_else(|| serde::de::Error::custom("payload is not a Native operation terminal"))
    }
}

impl CountTokensReceipt {
    pub fn new(result: ProviderCountTokensResult) -> Result<Self> {
        if result.operation != ProviderWireOperation::CountTokens {
            bail!("CountTokens receipt requires a typed CountTokens provider result");
        }
        Ok(Self { result })
    }

    pub fn input_tokens(&self) -> u64 {
        self.result.input_tokens
    }

    pub fn as_payload(&self) -> Result<Value> {
        Ok(json!({
            "semantic_terminal": COUNT_TOKENS_TERMINAL_KIND,
            "result": serde_json::to_value(&self.result)
                .map_err(|error| anyhow!("could not serialize CountTokens receipt: {error}"))?,
        }))
    }

    pub fn from_payload(payload: &Value) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Payload {
            semantic_terminal: String,
            result: ProviderCountTokensResult,
        }

        let payload: Payload = serde_json::from_value(payload.clone())
            .map_err(|error| anyhow!("invalid CountTokens receipt payload: {error}"))?;
        if payload.semantic_terminal != COUNT_TOKENS_TERMINAL_KIND {
            bail!("invalid CountTokens semantic terminal");
        }
        Self::new(payload.result)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompactOperationReceipt {
    result: ProviderCompactResult,
}

impl CompactOperationReceipt {
    pub fn from_provider_result(result: ProviderCompactResult) -> Result<Self> {
        if !result.satisfies_profile(result.profile()) {
            bail!("Compact receipt requires a typed provider Compact result");
        }
        Ok(Self { result })
    }

    pub fn profile(&self) -> ProviderCompactProfile {
        self.result.profile()
    }

    pub fn result(&self) -> &ProviderCompactResult {
        &self.result
    }

    pub fn as_payload(&self) -> Result<Value> {
        Ok(json!({
            "semantic_terminal": COMPACT_OPERATION_TERMINAL_KIND,
            "result": serde_json::to_value(&self.result)
                .map_err(|error| anyhow!("could not serialize Compact receipt: {error}"))?,
        }))
    }

    pub fn from_payload(payload: &Value) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Payload {
            semantic_terminal: String,
            result: ProviderCompactResult,
        }

        let payload: Payload = serde_json::from_value(payload.clone())
            .map_err(|error| anyhow!("invalid Compact receipt payload: {error}"))?;
        if payload.semantic_terminal != COMPACT_OPERATION_TERMINAL_KIND {
            bail!("invalid Compact semantic terminal");
        }
        Self::from_provider_result(payload.result)
    }
}

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
