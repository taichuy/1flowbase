use anyhow::{anyhow, bail, Result};
use plugin_framework::provider_contract::{
    ProviderCompactProfile, ProviderCompactResult, ProviderCountTokensResult, ProviderFinishReason,
    ProviderInvocationResult, ProviderStreamEvent, ProviderWireOperation,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

const COMPACT_RESPONSE_TERMINAL_KIND: &str = "compact_response";
const COUNT_TOKENS_TERMINAL_KIND: &str = "count_tokens";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountTokensReceipt {
    result: ProviderCountTokensResult,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeOperationTerminal {
    CountTokens(CountTokensReceipt),
    Compact(CompactResponseReceipt),
}

impl NativeOperationTerminal {
    pub fn from_payload(payload: &Value) -> Result<Option<Self>> {
        match payload
            .get("semantic_terminal")
            .and_then(Value::as_str)
        {
            Some(COUNT_TOKENS_TERMINAL_KIND) => {
                CountTokensReceipt::from_payload(payload).map(Self::CountTokens).map(Some)
            }
            Some(COMPACT_RESPONSE_TERMINAL_KIND) => CompactResponseReceipt::from_payload(payload)
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

/// The closed set of canonical Compact profiles that may enter an
/// application-flow Compact Response terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactResponseProfile {
    LocalSummary,
    ResponsesCompact,
    ResponsesCompactionV2,
}

/// The execution intent seen by the orchestration runtime. Public ingress is
/// responsible for classifying this before it chooses the application-flow
/// route; the runtime never infers it from prompt text or node configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationFlowExecutionIntent {
    Ordinary,
    Compact(CompactResponseProfile),
}

/// A successful, typed Compact result admitted by the application-flow
/// ingress. Fields are private so callers cannot combine a profile with the
/// wrong result family after classification.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactResponseIngress {
    receipt: CompactResponseReceipt,
}

impl CompactResponseIngress {
    pub fn local_generate(result: ProviderInvocationResult) -> Result<Self> {
        Ok(Self {
            receipt: CompactResponseReceipt::local_generate(result)?,
        })
    }

    pub fn responses_compact(result: ProviderCompactResult) -> Result<Self> {
        Ok(Self {
            receipt: CompactResponseReceipt::responses_compact(result)?,
        })
    }

    pub fn responses_compaction_v2(result: ProviderCompactResult) -> Result<Self> {
        Ok(Self {
            receipt: CompactResponseReceipt::responses_compaction_v2(result)?,
        })
    }

    pub fn intent(&self) -> ApplicationFlowExecutionIntent {
        ApplicationFlowExecutionIntent::Compact(self.receipt.profile())
    }

    pub(crate) fn receipt(&self) -> &CompactResponseReceipt {
        &self.receipt
    }
}

/// The semantic terminal payload emitted by `compact_response`. It is built
/// solely from a typed ingress result, never from an authorable graph value.
#[derive(Debug, Clone, PartialEq)]
pub enum CompactResponseReceipt {
    LocalGenerate(ProviderInvocationResult),
    ResponsesCompact(ProviderCompactResult),
    ResponsesCompactionV2(ProviderCompactResult),
}

impl CompactResponseReceipt {
    fn local_generate(result: ProviderInvocationResult) -> Result<Self> {
        if matches!(
            result.finish_reason.as_ref(),
            Some(ProviderFinishReason::Error)
        ) {
            bail!("local compact ingress cannot use a failed Generate result");
        }
        if !result
            .final_content
            .as_deref()
            .is_some_and(|content| !content.trim().is_empty())
        {
            bail!("local compact ingress requires a non-empty Generate result");
        }

        Ok(Self::LocalGenerate(result))
    }

    fn responses_compact(result: ProviderCompactResult) -> Result<Self> {
        if !result.satisfies_profile(ProviderCompactProfile::ResponsesCompact) {
            bail!("Compact ingress requires a typed responses_compact provider result");
        }

        Ok(Self::ResponsesCompact(result))
    }

    fn responses_compaction_v2(result: ProviderCompactResult) -> Result<Self> {
        if !result.satisfies_profile(ProviderCompactProfile::ResponsesCompactionV2) {
            bail!("Compact ingress requires a real opaque responses_compaction_v2 provider result");
        }

        Ok(Self::ResponsesCompactionV2(result))
    }

    pub fn from_provider_result(result: ProviderCompactResult) -> Result<Self> {
        match result.profile() {
            ProviderCompactProfile::ResponsesCompact => Self::responses_compact(result),
            ProviderCompactProfile::ResponsesCompactionV2 => {
                Self::responses_compaction_v2(result)
            }
        }
    }

    pub fn profile(&self) -> CompactResponseProfile {
        match self {
            Self::LocalGenerate(_) => CompactResponseProfile::LocalSummary,
            Self::ResponsesCompact(_) => CompactResponseProfile::ResponsesCompact,
            Self::ResponsesCompactionV2(_) => CompactResponseProfile::ResponsesCompactionV2,
        }
    }

    pub fn generate_result(&self) -> Option<&ProviderInvocationResult> {
        match self {
            Self::LocalGenerate(result) => Some(result),
            Self::ResponsesCompact(_) | Self::ResponsesCompactionV2(_) => None,
        }
    }

    pub fn compact_result(&self) -> Option<&ProviderCompactResult> {
        match self {
            Self::LocalGenerate(_) => None,
            Self::ResponsesCompact(result) | Self::ResponsesCompactionV2(result) => Some(result),
        }
    }

    pub fn as_payload(&self) -> Result<Value> {
        let (profile, result) = match self {
            Self::LocalGenerate(result) => (
                CompactResponseProfile::LocalSummary,
                serde_json::to_value(result),
            ),
            Self::ResponsesCompact(result) => (
                CompactResponseProfile::ResponsesCompact,
                serde_json::to_value(result),
            ),
            Self::ResponsesCompactionV2(result) => (
                CompactResponseProfile::ResponsesCompactionV2,
                serde_json::to_value(result),
            ),
        };
        let result =
            result.map_err(|error| anyhow!("could not serialize Compact receipt: {error}"))?;

        Ok(json!({
            "semantic_terminal": COMPACT_RESPONSE_TERMINAL_KIND,
            "profile": profile,
            "result": result,
        }))
    }

    pub fn from_payload(payload: &Value) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct CompactResponseReceiptPayload {
            semantic_terminal: String,
            profile: CompactResponseProfile,
            result: Value,
        }

        let payload: CompactResponseReceiptPayload = serde_json::from_value(payload.clone())
            .map_err(|error| anyhow!("invalid Compact Response receipt payload: {error}"))?;
        if payload.semantic_terminal != COMPACT_RESPONSE_TERMINAL_KIND {
            bail!("invalid Compact Response semantic terminal");
        }

        match payload.profile {
            CompactResponseProfile::LocalSummary => {
                let result = serde_json::from_value(payload.result).map_err(|error| {
                    anyhow!("invalid local Generate Compact receipt result: {error}")
                })?;
                Self::local_generate(result)
            }
            CompactResponseProfile::ResponsesCompact => {
                let result = serde_json::from_value(payload.result).map_err(|error| {
                    anyhow!("invalid responses_compact receipt result: {error}")
                })?;
                Self::responses_compact(result)
            }
            CompactResponseProfile::ResponsesCompactionV2 => {
                let result = serde_json::from_value(payload.result).map_err(|error| {
                    anyhow!("invalid responses_compaction_v2 receipt result: {error}")
                })?;
                Self::responses_compaction_v2(result)
            }
        }
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

/// Decode the runtime-owned Compact Response receipt from a semantic terminal
/// trace. Non-Compact traces deliberately return `None` so terminal owners
/// cannot mistake an Answer or arbitrary node payload for a Compact result.
pub fn compact_response_receipt_from_trace(
    trace: &NodeExecutionTrace,
) -> Result<Option<CompactResponseReceipt>> {
    if trace
        .output_payload
        .get("semantic_terminal")
        .and_then(Value::as_str)
        != Some(COMPACT_RESPONSE_TERMINAL_KIND)
    {
        return Ok(None);
    }

    CompactResponseReceipt::from_payload(&trace.output_payload).map(Some)
}

pub fn compact_response_receipt_from_traces(
    traces: &[NodeExecutionTrace],
) -> Result<CompactResponseReceipt> {
    let receipts = traces
        .iter()
        .map(compact_response_receipt_from_trace)
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
    if trace.output_payload.get("semantic_terminal").and_then(Value::as_str)
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
