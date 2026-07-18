use anyhow::{anyhow, bail, Result};
use plugin_framework::provider_contract::{
    ProviderCompactProfile, ProviderCompactResult, ProviderFinishReason, ProviderInvocationResult,
    ProviderStreamEvent,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

const COMPACT_RESPONSE_TERMINAL_KIND: &str = "compact_response";

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
    if trace.node_type != COMPACT_RESPONSE_TERMINAL_KIND {
        return Ok(None);
    }

    CompactResponseReceipt::from_payload(&trace.output_payload).map(Some)
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
