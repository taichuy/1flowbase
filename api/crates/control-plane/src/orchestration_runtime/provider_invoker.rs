use super::*;
use plugin_framework::provider_contract::{
    ProviderMessageRole, ProviderOutputItemPhase, ProviderRuntimeError, ProviderRuntimeErrorKind,
};
use plugin_framework::{
    provider_contract::ProviderCountTokensFallbackReason,
    provider_count_tokens_estimator::estimate_provider_count_tokens,
};
use sha2::{Digest, Sha256};

use super::canonical_stream::{
    CanonicalBlockId, CanonicalCallId, CanonicalContentKind, CanonicalItemId, CanonicalStreamEvent,
    CanonicalStreamState, CanonicalStreamTransitionError,
};
use crate::installed_provider_package::load_installed_provider_package;

mod failover_queue;
mod fee_lifecycle;
mod internal_compaction;
mod main_instance_routing;
mod native_trajectory;
mod protocol_context;
mod protocol_observation;
mod runtime_events;
mod session_binding;
mod stream_timing;
pub(super) use failover_queue::freeze_failover_queue_routes;
use stream_timing::{ProviderStreamTiming, serialized_frame_size};

const PROVIDER_LIVE_EVENT_LANE_CAPACITY: usize = 32;
const RESPONSES_WEBSOCKET_PREWARM_MAX_MS: i64 = 30_000;

const VISIBLE_INTERNAL_LLM_MEDIA_TOOLS_CONTEXT_KEY: &str = "visible_internal_llm_media_tools";

fn completed_native_output_items(
    items: &std::collections::BTreeMap<usize, Option<Value>>,
) -> Option<Vec<Value>> {
    // The provider's output_index orders items but can skip positions. The canonical
    // stream writer validates each Added/Done pair; every observed slot must close.
    items.values().cloned().collect()
}

pub(super) fn committed_tool_delivery_candidate(
    node_id: &str,
    node_run_id: Uuid,
    event: &ProviderStreamEvent,
) -> Result<Option<crate::ports::RuntimeEventPayload>> {
    let ProviderStreamEvent::OutputItem {
        phase: ProviderOutputItemPhase::Done,
        output_index,
        item,
    } = event
    else {
        return Ok(None);
    };
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
    if !item_type.ends_with("_call") {
        return Ok(None);
    }
    if item
        .get("call_id")
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .is_none()
    {
        return Err(anyhow!(
            "provider tool output item is missing a stable call id"
        ));
    }
    Ok(Some(debug_stream_events::provider_output_item_done(
        node_id,
        node_run_id,
        *output_index,
        item.clone(),
    )))
}

/// Add host evidence before billing can return the original transport error early.
fn seal_native_failure_binding(
    error: anyhow::Error,
    binding: &Value,
    configuration_digest: Option<&str>,
) -> anyhow::Error {
    if let Some(plugin_framework::PluginFrameworkError::RuntimeContract { error: original }) =
        error.downcast_ref::<plugin_framework::PluginFrameworkError>()
    {
        let mut original = original.clone();
        let mut details = match original.provider_details.take() {
            Some(Value::Object(details)) => details,
            Some(value) => {
                serde_json::Map::from_iter([("original_provider_details".into(), value)])
            }
            None => serde_json::Map::new(),
        };
        details.insert("native_inference_binding".into(), binding.clone());
        // This is the failed invocation's request, which may differ from the preceding
        // successful callback round after a legitimate full-context configuration change.
        // Never trust a provider-supplied value for host admission evidence.
        details.remove("native_inference_configuration_digest");
        if let Some(digest) = configuration_digest {
            details.insert(
                "native_inference_configuration_digest".into(),
                json!(digest),
            );
        }
        original.provider_details = Some(Value::Object(details));
        return plugin_framework::PluginFrameworkError::runtime(*original).into();
    }
    error
}

fn is_responses_prewarm(input: &ProviderInvocationInput) -> bool {
    input.operation == plugin_framework::provider_contract::ProviderWireOperation::Generate
        && input.native_transport.as_ref().is_some_and(|transport| {
            transport.protocol == "openai_responses"
                && transport.wire_body.get("generate").and_then(Value::as_bool) == Some(false)
        })
}

fn provider_execution_deadline_unix_ms(
    input: &ProviderInvocationInput,
    now: OffsetDateTime,
    transport_connection_scope: Option<&str>,
) -> i64 {
    let task_deadline = input
        .run_context
        .get("task_deadline_unix_ms")
        .and_then(Value::as_i64)
        .unwrap_or_else(|| {
            i64::try_from((now + time::Duration::minutes(30)).unix_timestamp_nanos() / 1_000_000)
                .unwrap_or(i64::MAX)
        });
    if transport_connection_scope.is_some() && is_responses_prewarm(input) {
        // The client can abandon startup prewarm while its original Provider
        // invocation is still running. Bound only this setup operation so a
        // successor cannot wait behind an orphan for the ordinary 30-minute budget.
        let now_ms = i64::try_from(now.unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX);
        task_deadline.min(now_ms.saturating_add(RESPONSES_WEBSOCKET_PREWARM_MAX_MS))
    } else {
        task_deadline
    }
}

#[cfg(test)]
#[path = "provider_invoker/_tests/deadline_tests.rs"]
mod deadline_tests;

#[derive(Clone)]
struct RuntimeProviderInvocationPin {
    instance: domain::ModelProviderInstanceRecord,
    installation: domain::LocalPluginInstallationRecord,
    package: ProviderPackage,
}

const GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY: &str = "1flowbase_gateway_provider_stages";

fn bounded_timing_millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis())
        .unwrap_or(u64::MAX)
        .min(24 * 60 * 60 * 1_000)
}

fn attach_gateway_stage_timing(
    metadata: &mut Value,
    flow_ms: u64,
    ingress_ms: Option<u64>,
    flush_ms: Option<u64>,
) -> Result<()> {
    let timing = json!({
        "schema_version": 1,
        "ingress_ms": ingress_ms,
        "flow_ms": flow_ms,
        "flush_ms": flush_ms,
    });
    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY.to_string(),
            timing,
        );
    } else {
        let upstream = std::mem::take(metadata);
        *metadata = json!({
            GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY: timing,
            "_1flowbase_upstream_provider_metadata": upstream,
        });
    }
    Ok(())
}

fn provider_stream_event_kind(event: &ProviderStreamEvent) -> &'static str {
    match event {
        ProviderStreamEvent::ProtocolObservation { .. } => "protocol_observation",
        ProviderStreamEvent::NativeEvent { .. } => "native_event",
        ProviderStreamEvent::TextDelta { .. } => "text_delta",
        ProviderStreamEvent::ReasoningDelta { .. } => "reasoning_delta",
        ProviderStreamEvent::ReasoningSignatureDelta { .. } => "reasoning_signature_delta",
        ProviderStreamEvent::ToolCallDelta { .. } => "tool_call_delta",
        ProviderStreamEvent::ToolCallCommit { .. } => "tool_call_commit",
        ProviderStreamEvent::McpCallDelta { .. } => "mcp_call_delta",
        ProviderStreamEvent::McpCallCommit { .. } => "mcp_call_commit",
        ProviderStreamEvent::ResponsesOutputDelta { .. } => "responses_output_delta",
        ProviderStreamEvent::OutputItem { .. } => "output_item",
        ProviderStreamEvent::UsageDelta { .. } => "usage_delta",
        ProviderStreamEvent::UsageSnapshot { .. } => "usage_snapshot",
        ProviderStreamEvent::Finish { .. } => "finish",
        ProviderStreamEvent::Error { .. } => "error",
        ProviderStreamEvent::OutputProtocolFailure { .. } => "output_protocol_failure",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CanonicalProviderDelta {
    pub(super) kind: CanonicalContentKind,
    pub(super) text: String,
}

pub(super) struct RuntimeCanonicalStreamWriter {
    item_id: CanonicalItemId,
    text_block_id: CanonicalBlockId,
    reasoning_block_id: CanonicalBlockId,
    state: CanonicalStreamState,
    think_tag_splitter: ThinkTagStreamSplitter,
}

impl RuntimeCanonicalStreamWriter {
    pub(super) fn new(item_id: impl Into<String>) -> Self {
        let item_id = CanonicalItemId::new(item_id);
        Self {
            text_block_id: CanonicalBlockId::new(item_id.clone(), "text"),
            reasoning_block_id: CanonicalBlockId::new(item_id.clone(), "reasoning"),
            item_id,
            state: CanonicalStreamState::default(),
            think_tag_splitter: ThinkTagStreamSplitter::default(),
        }
    }

    pub(super) fn write(
        &mut self,
        event: &ProviderStreamEvent,
    ) -> Result<Vec<CanonicalProviderDelta>> {
        if self.state.terminal().is_some() {
            return Err(CanonicalStreamTransitionError::StreamAlreadyTerminal.into());
        }

        match event {
            ProviderStreamEvent::TextDelta { delta } => {
                let parts = self.think_tag_splitter.split(delta);
                self.append_content_parts(parts)
            }
            ProviderStreamEvent::ReasoningDelta { delta } => {
                self.append_content_parts(vec![DebugDeltaPart {
                    kind: DebugDeltaKind::Reasoning,
                    text: delta.clone(),
                }])
            }
            ProviderStreamEvent::ReasoningSignatureDelta { .. } => Ok(Vec::new()),
            ProviderStreamEvent::ToolCallDelta { call_id, delta } => {
                if let Some(arguments) = tool_argument_delta(delta) {
                    self.state.apply(CanonicalStreamEvent::ToolArgumentsDelta {
                        call_id: CanonicalCallId::new(self.item_id.clone(), call_id.clone()),
                        delta: arguments.to_string(),
                    })?;
                }
                Ok(Vec::new())
            }
            ProviderStreamEvent::UsageDelta { usage } => {
                self.state.apply(CanonicalStreamEvent::UsageDelta {
                    usage: usage.clone(),
                })?;
                Ok(Vec::new())
            }
            ProviderStreamEvent::UsageSnapshot { usage } => {
                self.state.apply(CanonicalStreamEvent::UsageSnapshot {
                    usage: usage.clone(),
                })?;
                Ok(Vec::new())
            }
            ProviderStreamEvent::ResponsesOutputDelta { event } => {
                self.state
                    .apply(CanonicalStreamEvent::ResponsesOutputDelta {
                        event: event.clone(),
                    })?;
                Ok(Vec::new())
            }
            ProviderStreamEvent::OutputItem {
                phase,
                output_index,
                item,
            } => {
                self.state.apply(CanonicalStreamEvent::OutputItem {
                    phase: *phase,
                    output_index: *output_index,
                    item: item.clone(),
                })?;
                Ok(Vec::new())
            }
            ProviderStreamEvent::Finish { reason } => {
                let deltas = self.flush_pending_content()?;
                self.state.apply(CanonicalStreamEvent::Finish {
                    reason: reason.clone(),
                })?;
                Ok(deltas)
            }
            ProviderStreamEvent::Error { error } => {
                let deltas = self.flush_pending_content()?;
                self.state.apply(CanonicalStreamEvent::Fail {
                    error: error.clone(),
                })?;
                Ok(deltas)
            }
            ProviderStreamEvent::OutputProtocolFailure { failure } => {
                let deltas = self.flush_pending_content()?;
                self.state.apply(CanonicalStreamEvent::Fail {
                    error: ProviderRuntimeError::new(
                        ProviderRuntimeErrorKind::ProviderInvalidResponse,
                        failure.message.clone(),
                    )
                    .with_provider_details(failure.provider_details.clone()),
                })?;
                Ok(deltas)
            }
            ProviderStreamEvent::ProtocolObservation { .. }
            | ProviderStreamEvent::NativeEvent { .. }
            | ProviderStreamEvent::ToolCallCommit { .. }
            | ProviderStreamEvent::McpCallDelta { .. }
            | ProviderStreamEvent::McpCallCommit { .. } => Ok(Vec::new()),
        }
    }

    pub(super) fn complete(&mut self) -> Result<Vec<CanonicalProviderDelta>> {
        if self.state.terminal().is_some() {
            return Ok(Vec::new());
        }
        self.flush_pending_content()
    }

    pub(super) fn state(&self) -> &CanonicalStreamState {
        &self.state
    }

    pub(super) fn into_state(self) -> CanonicalStreamState {
        self.state
    }

    fn flush_pending_content(&mut self) -> Result<Vec<CanonicalProviderDelta>> {
        let parts = self.think_tag_splitter.finish();
        self.append_content_parts(parts)
    }

    fn append_content_parts(
        &mut self,
        parts: Vec<DebugDeltaPart>,
    ) -> Result<Vec<CanonicalProviderDelta>> {
        let mut deltas = Vec::with_capacity(parts.len());
        for part in parts {
            let (kind, event) = match part.kind {
                DebugDeltaKind::Text => (
                    CanonicalContentKind::Text,
                    CanonicalStreamEvent::TextDelta {
                        block_id: self.text_block_id.clone(),
                        delta: part.text.clone(),
                    },
                ),
                DebugDeltaKind::Reasoning => (
                    CanonicalContentKind::Reasoning,
                    CanonicalStreamEvent::ReasoningDelta {
                        block_id: self.reasoning_block_id.clone(),
                        delta: part.text.clone(),
                    },
                ),
            };
            self.state.apply(event)?;
            deltas.push(CanonicalProviderDelta {
                kind,
                text: part.text,
            });
        }
        Ok(deltas)
    }
}

fn tool_argument_delta(delta: &Value) -> Option<&str> {
    delta
        .pointer("/function/arguments")
        .or_else(|| delta.get("arguments"))
        .and_then(Value::as_str)
}

fn record_first_token_timing(
    first_token_timing: &Arc<Mutex<Option<FirstTokenTiming>>>,
    event: &ProviderStreamEvent,
    provider_invoke_started_at: OffsetDateTime,
    provider_invoke_started: std::time::Instant,
) {
    if !matches!(
        event,
        ProviderStreamEvent::TextDelta { .. } | ProviderStreamEvent::ReasoningDelta { .. }
    ) {
        return;
    }

    let Ok(mut timing) = first_token_timing.lock() else {
        return;
    };
    if timing.is_some() {
        return;
    }
    let elapsed = provider_invoke_started.elapsed();
    *timing = Some(FirstTokenTiming {
        first_token_at: provider_invoke_started_at + elapsed,
        time_to_first_token_ms: elapsed.as_millis() as u64,
    });
}

pub(super) fn is_expected_runtime_event_stream_closed_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("runtime event stream is closed")
        || message.contains("runtime event stream is not open")
}

fn answer_presentation_source_is_active(trace_node_id: Option<&str>, active_node_id: &str) -> bool {
    trace_node_id == Some(active_node_id)
}

async fn project_canonical_provider_deltas(
    stream: Option<&Arc<dyn RuntimeEventStream>>,
    flow_run_id: Option<Uuid>,
    answer_presentation: Option<
        &Arc<tokio::sync::Mutex<answer_presentation::AnswerPresentationCursor>>,
    >,
    node_id: &str,
    node_run_id: Uuid,
    deltas: &[CanonicalProviderDelta],
) {
    for delta in deltas {
        let canonical_event = match delta.kind {
            CanonicalContentKind::Text => ProviderStreamEvent::TextDelta {
                delta: delta.text.clone(),
            },
            CanonicalContentKind::Reasoning => ProviderStreamEvent::ReasoningDelta {
                delta: delta.text.clone(),
            },
        };
        if let Some(cursor) = answer_presentation {
            let presentation_events =
                cursor
                    .lock()
                    .await
                    .push_provider_event(node_id, node_run_id, &canonical_event);
            if let (Some(stream), Some(flow_run_id)) = (stream, flow_run_id) {
                for presentation_event in presentation_events {
                    append_provider_runtime_event(stream, flow_run_id, presentation_event).await;
                }
            }
        }

        let (Some(stream), Some(flow_run_id)) = (stream, flow_run_id) else {
            continue;
        };
        let debug_event = match delta.kind {
            CanonicalContentKind::Text => {
                debug_stream_events::text_delta(node_id, node_run_id, delta.text.clone())
            }
            CanonicalContentKind::Reasoning => {
                debug_stream_events::reasoning_delta(node_id, node_run_id, delta.text.clone())
            }
        };
        append_provider_runtime_event(stream, flow_run_id, debug_event).await;
    }
}

async fn append_provider_runtime_event(
    stream: &Arc<dyn RuntimeEventStream>,
    flow_run_id: Uuid,
    event: crate::ports::RuntimeEventPayload,
) {
    let event_type = event.event_type.clone();
    let source = event.source;
    if let Err(error) = stream.append(flow_run_id, event).await {
        if is_expected_runtime_event_stream_closed_error(&error) {
            tracing::debug!(
                flow_run_id = %flow_run_id,
                event_type = %event_type,
                source = ?source,
                error = %error,
                "provider runtime event append skipped because stream is already closed"
            );
        } else {
            tracing::warn!(
                flow_run_id = %flow_run_id,
                event_type = %event_type,
                source = ?source,
                error = %error,
                "failed to append provider runtime event"
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DebugDeltaKind {
    Text,
    Reasoning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DebugDeltaPart {
    pub(super) kind: DebugDeltaKind,
    pub(super) text: String,
}

#[derive(Debug, Default)]
pub(super) struct ThinkTagStreamSplitter {
    inside_think: bool,
    pending: String,
}

impl ThinkTagStreamSplitter {
    pub(super) fn split(&mut self, delta: &str) -> Vec<DebugDeltaPart> {
        self.pending.push_str(delta);
        let mut parts = Vec::new();

        loop {
            let tag = if self.inside_think {
                "</think>"
            } else {
                "<think>"
            };

            if let Some(tag_index) = self.pending.find(tag) {
                let text = self.pending[..tag_index].to_string();
                push_debug_delta_part(
                    &mut parts,
                    if self.inside_think {
                        DebugDeltaKind::Reasoning
                    } else {
                        DebugDeltaKind::Text
                    },
                    text,
                );
                self.pending.drain(..tag_index + tag.len());
                self.inside_think = !self.inside_think;
                continue;
            }

            let keep_len = partial_tag_prefix_len(&self.pending, tag);
            let emit_len = self.pending.len().saturating_sub(keep_len);
            if emit_len > 0 {
                let text = self.pending[..emit_len].to_string();
                self.pending.drain(..emit_len);
                push_debug_delta_part(
                    &mut parts,
                    if self.inside_think {
                        DebugDeltaKind::Reasoning
                    } else {
                        DebugDeltaKind::Text
                    },
                    text,
                );
            }
            break;
        }

        parts
    }

    pub(super) fn finish(&mut self) -> Vec<DebugDeltaPart> {
        let text = std::mem::take(&mut self.pending);
        let mut parts = Vec::new();
        push_debug_delta_part(
            &mut parts,
            if self.inside_think {
                DebugDeltaKind::Reasoning
            } else {
                DebugDeltaKind::Text
            },
            text,
        );
        parts
    }
}

fn push_debug_delta_part(parts: &mut Vec<DebugDeltaPart>, kind: DebugDeltaKind, text: String) {
    if text.is_empty() {
        return;
    }

    if let Some(previous) = parts.last_mut().filter(|part| part.kind == kind) {
        previous.text.push_str(&text);
        return;
    }

    parts.push(DebugDeltaPart { kind, text });
}

fn partial_tag_prefix_len(buffer: &str, tag: &str) -> usize {
    let max_len = buffer.len().min(tag.len().saturating_sub(1));
    (1..=max_len)
        .rev()
        .find(|length| {
            let start = buffer.len() - length;
            buffer.is_char_boundary(start) && tag.starts_with(&buffer[start..])
        })
        .unwrap_or(0)
}

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: PluginRepository + Clone + Send + Sync,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    fn continuation_affinity(&self) -> Option<&crate::ports::ProviderTransportAffinity> {
        self.provider_continuation
            .as_ref()
            .map(crate::ports::ProviderContinuation::affinity)
            .or_else(|| {
                self.provider_transport_payload
                    .as_ref()
                    .and_then(crate::ports::ProviderTransportPayload::affinity)
            })
    }

    fn ensure_continuation_route(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        runtime_capabilities: &std::collections::BTreeSet<String>,
    ) -> Result<()> {
        let Some(affinity) = self.continuation_affinity() else {
            return Ok(());
        };
        self.ensure_provider_affinity(runtime, affinity)?;
        let capability = plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported
            .manifest_capability_name();
        if runtime_capabilities.contains(capability) {
            Ok(())
        } else {
            Err(provider_transport_error(
                ProviderRuntimeErrorKind::SemanticCapabilityUnsupported,
                "selected LLM Provider does not support native continuation",
            ))
        }
    }

    pub(super) fn apply_selected_provider_transport(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        capabilities: &std::collections::BTreeSet<String>,
        input: &mut ProviderInvocationInput,
    ) -> Result<()> {
        use orchestration_runtime::execution_engine::internal_compaction::{
            POLICY_KEY, SUMMARY_KEY, policy_enabled,
        };
        if input.run_context.get(SUMMARY_KEY).and_then(Value::as_bool) == Some(true) {
            // A host summary is a new semantic Generate, never a replay of the
            // client's original native wire body or its continuation delta.
            return Ok(());
        }
        let auto_compact = policy_enabled(
            input
                .run_context
                .get(POLICY_KEY)
                .and_then(|v| v.get("enabled")),
        );
        if self
            .provider_transport_payload
            .as_ref()
            .is_some_and(|payload| payload.is_preferred())
            && input.native_transport.is_none()
            && !input.required_capabilities.contains(
                &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough)
            && !input.required_capabilities.contains(
                &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativeOutputV1)
            && self.continuation_affinity().is_none()
            && (auto_compact || !(capabilities.contains("responses.native_passthrough")
                && capabilities.contains("responses.native_output.v1")))
        {
            input.native_transport = None;
            input.required_capabilities.remove(
                &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough);
            input.required_capabilities.remove(
                &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativeOutputV1);
            return Ok(());
        }
        self.apply_provider_transport(runtime, input)?;
        if self
            .provider_transport_payload
            .as_ref()
            .is_some_and(|payload| payload.is_preferred())
        {
            input.required_capabilities.insert(
                plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativeOutputV1);
        }
        Ok(())
    }

    pub(super) fn apply_provider_transport(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        input: &mut ProviderInvocationInput,
    ) -> Result<()> {
        if let Some(continuation) = self.provider_continuation.as_ref() {
            self.ensure_provider_affinity(runtime, continuation.affinity())?;
            input.previous_response_id = Some(continuation.response_id().to_string());
            input.required_capabilities.insert(
                plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported,
            );
        }
        let native_transport_capability =
            plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough;
        let Some(payload) = self.provider_transport_payload.as_ref() else {
            if input
                .required_capabilities
                .contains(&native_transport_capability)
            {
                return Err(anyhow!("ephemeral_transport_missing"));
            }
            return Ok(());
        };

        if let Some(affinity) = payload.affinity() {
            self.ensure_provider_affinity(runtime, affinity)?;
            input.required_capabilities.insert(
                plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported,
            );
            // The sealed native request already owns the continuation delta. Canonical history is
            // retained for routing, then removed at the selected Provider boundary to prevent a
            // second full-history wire representation from accompanying the native transport.
            input.messages.clear();
            input.system.clear();
            input.tools.clear();
            input.mcp_bindings.clear();
            input.required_capabilities.remove(
                &plugin_framework::provider_contract::ProviderInvocationCapability::MessageBlocksReasoningHistoryV1,
            );
            input.required_capabilities.remove(
                &plugin_framework::provider_contract::ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1,
            );
        }

        input
            .required_capabilities
            .insert(native_transport_capability);
        let protocol = match payload.protocol() {
            crate::ports::ProviderTransportProtocol::OpenAiResponses => "openai_responses",
        };
        // This hint is host evidence from the owned predecessor, not the current
        // delivery carrier. Cursor-free full-context fallback must remain independent.
        if payload
            .wire_body()
            .get("previous_response_id")
            .is_some_and(|v| !v.is_null())
        {
            if let Some(receipt) = payload.predecessor_recovery_receipt() {
                input.client_transport = Some(match receipt.transport {
                    plugin_framework::provider_contract::RecoveryTransport::ProviderHttp => {
                        plugin_framework::provider_contract::ProviderClientTransport::Http
                    }
                    plugin_framework::provider_contract::RecoveryTransport::AiNativeWebSocket => {
                        plugin_framework::provider_contract::ProviderClientTransport::Websocket
                    }
                });
            }
        }
        input.native_transport = Some(
            plugin_framework::provider_contract::ProviderNativeTransport {
                protocol: protocol.to_string(),
                wire_body: payload.wire_body().clone(),
                digest: payload.digest().to_string(),
                size_bytes: payload.size_bytes() as u64,
            },
        );
        Ok(())
    }

    fn ensure_provider_affinity(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        affinity: &crate::ports::ProviderTransportAffinity,
    ) -> Result<()> {
        if affinity.matches(
            &runtime.provider_instance_id,
            &runtime.provider_code,
            &runtime.protocol,
            &runtime.model,
        ) {
            return Ok(());
        }
        Err(plugin_framework::PluginFrameworkError::runtime(
            plugin_framework::provider_contract::ProviderRuntimeError::new(
                plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderAffinityMismatch,
                "selected LLM Provider does not own the opaque continuation",
            ),
        )
        .into())
    }

    async fn ready_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<domain::LocalPluginInstallationRecord> {
        match (
            self.api_node_id.as_deref(),
            self.provider_install_root.as_deref(),
        ) {
            (Some(node_id), Some(install_root)) => {
                ready_current_node_plugin_installation(
                    &self.repository,
                    node_id,
                    install_root,
                    installation_id,
                )
                .await
            }
            _ => Err(ControlPlaneError::Conflict("plugin_node_context_required").into()),
        }
    }
}

fn provider_transport_error(
    kind: ProviderRuntimeErrorKind,
    message: &'static str,
) -> anyhow::Error {
    plugin_framework::PluginFrameworkError::runtime(ProviderRuntimeError::new(kind, message)).into()
}

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository + PluginRepository + Clone + Send + Sync,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    pub(super) fn for_flow_run(&self, flow_run_id: Uuid) -> Self {
        Self {
            repository: self.repository.clone(),
            runtime: self.runtime.clone(),
            workspace_id: self.workspace_id,
            provider_secret_master_key: self.provider_secret_master_key.clone(),
            live_provider_events: self.live_provider_events.clone(),
            runtime_event_stream: self.runtime_event_stream.clone(),
            flow_run_id: Some(flow_run_id),
            active_node_id: self.active_node_id.clone(),
            active_node_run_id: self.active_node_run_id,
            api_node_id: self.api_node_id.clone(),
            provider_install_root: self.provider_install_root.clone(),
            provider_session_scope: self.provider_session_scope.clone(),
            flow_execution_context: self.flow_execution_context.clone(),
            answer_presentation: self.answer_presentation.clone(),
            transport_connection_scope_override: self.transport_connection_scope_override.clone(),
            observation_context: self.observation_context.clone(),
            provider_transport_payload: self.provider_transport_payload.clone(),
            provider_transport_store: self.provider_transport_store.clone(),
            provider_continuation: self.provider_continuation.clone(),
            response_round_id: self.response_round_id,
            native_user_messages_digest: self.native_user_messages_digest.clone(),
            native_history: self.native_history.clone(),
            model_pricing_cache_store: self.model_pricing_cache_store.clone(),
        }
    }

    pub(super) fn with_flow_execution_context(
        &self,
        context: Arc<RuntimeFlowExecutionContext>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.flow_execution_context = Some(context);
        invoker
    }

    pub(super) fn with_answer_presentation(
        &self,
        answer_presentation: Arc<tokio::sync::Mutex<answer_presentation::AnswerPresentationCursor>>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.answer_presentation = Some(answer_presentation);
        invoker
    }

    pub(super) fn with_observation_context(
        &self,
        context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.observation_context = context;
        invoker
    }

    pub(super) fn with_transport_connection_scope_override(&self, scope: Option<String>) -> Self {
        let mut invoker = self.clone();
        invoker.transport_connection_scope_override = Some(scope);
        invoker
    }

    pub(super) fn with_provider_transport_payload(
        &self,
        provider_transport_payload: Option<crate::ports::ProviderTransportPayload>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.native_history = provider_transport_payload
            .as_ref()
            .and_then(|payload| payload.native_history().cloned());
        invoker.provider_transport_payload = provider_transport_payload;
        invoker
    }

    pub(super) fn with_provider_continuation(
        &self,
        provider_continuation: Option<crate::ports::ProviderContinuation>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.provider_continuation = provider_continuation;
        invoker
    }

    #[cfg(test)]
    async fn stage_provider_continuation(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        response_id: Option<&str>,
    ) -> Result<()> {
        self.stage_provider_continuation_with_history(runtime, response_id, None, None)
            .await
    }

    async fn stage_provider_continuation_with_history(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        response_id: Option<&str>,
        history: Option<Value>,
        session_identity: Option<&str>,
    ) -> Result<()> {
        let Some(response_id) = response_id.filter(|value| !value.trim().is_empty()) else {
            return Ok(());
        };
        let (Some(store), Some(flow_run_id)) =
            (self.provider_transport_store.as_ref(), self.flow_run_id)
        else {
            return Ok(());
        };
        // A first native Responses turn can expose only the Provider response id; the opaque
        // request payload and an already-claimed continuation are both absent on that path.
        // Keep the id available for the later MCP approval turn instead of treating those
        // absent inputs as proof that no continuation exists.
        let continuation = crate::ports::ProviderContinuation::new(
            response_id,
            crate::ports::ProviderTransportAffinity::new(
                &runtime.provider_instance_id,
                &runtime.provider_code,
                &runtime.protocol,
                &runtime.model,
            ),
        )?
        .with_native_history(history)
        .with_session_identity(session_identity)?;
        store
            .put_continuation(
                crate::ports::ProviderContinuationSlotId::for_response_round(
                    flow_run_id,
                    self.response_round_id.unwrap_or(flow_run_id),
                ),
                continuation.clone(),
            )
            .await?;
        store
            .put_continuation(
                crate::ports::ProviderContinuationSlotId::for_flow_run(flow_run_id),
                continuation,
            )
            .await
            .map_err(|_| {
                anyhow::Error::from(plugin_framework::PluginFrameworkError::runtime(
                    plugin_framework::provider_contract::ProviderRuntimeError::new(
                        plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderTransportUnavailable,
                        "opaque Provider continuation could not be staged",
                    ),
                ))
            })
    }

    pub(super) async fn resolve_llm_instance(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    ) -> Result<domain::ModelProviderInstanceRecord> {
        let provider_instance_id = Uuid::parse_str(&runtime.provider_instance_id)
            .map_err(|_| ControlPlaneError::InvalidInput("source_instance_id"))?;
        let instance = self
            .repository
            .get_instance(self.workspace_id, provider_instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        if instance.provider_code != runtime.provider_code {
            return Err(ControlPlaneError::InvalidInput("provider_code").into());
        }
        if instance.status != domain::ModelProviderInstanceStatus::Ready {
            return Err(ControlPlaneError::Conflict("provider_instance_not_ready").into());
        }
        if !instance.included_in_main {
            return Err(ControlPlaneError::Conflict("provider_instance_not_in_main").into());
        }
        let installation = self
            .repository
            .get_installation(instance.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        let assigned = self
            .repository
            .list_assignments(self.workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned {
            return Err(ControlPlaneError::Conflict("plugin_assignment_required").into());
        }
        if matches!(
            installation.desired_state,
            domain::PluginDesiredState::Disabled
        ) {
            return Err(ControlPlaneError::PluginUnavailable.into());
        }
        if !instance.enabled_model_ids.is_empty()
            && !instance
                .enabled_model_ids
                .iter()
                .any(|model_id| model_id == &runtime.model)
        {
            return Err(ControlPlaneError::InvalidInput("model").into());
        }

        Ok(instance)
    }
}

mod config;
use config::build_provider_runtime_config;

mod media;
use media::adapt_or_ensure_model_supports_content_blocks;
#[cfg(test)]
pub(super) use media::textualize_media_content_blocks_for_text_model;

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/provider_invoker/canonical_writer_tests.rs"]
mod canonical_writer_tests;

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/provider_invoker/continuation_claim_tests.rs"]
mod continuation_claim_tests;

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/provider_invoker/credit_command_tests.rs"]
mod credit_command_tests;

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/provider_invoker/timing_receipt_tests.rs"]
mod timing_receipt_tests;

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/support.rs"]
pub(crate) mod test_support;

/// The host consumes this private header before dispatching to the Provider.
/// Explicit None on an HTTP callback clears a previous WebSocket owner.
fn apply_transport_connection_scope_override(
    input: &mut ProviderInvocationInput,
    scope: Option<&str>,
) {
    if let Some(envelope) = input.client_protocol_envelope.as_mut() {
        envelope
            .headers
            .remove(crate::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER);
    }
    if let Some(scope) = scope {
        let envelope = input.client_protocol_envelope.get_or_insert_with(|| {
            plugin_framework::provider_contract::ProtocolContextEnvelope {
                source_protocol: input.protocol.clone(),
                ..Default::default()
            }
        });
        envelope.headers.insert(
            crate::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER.into(),
            vec![scope.into()],
        );
    }
}

mod node_execution;
mod provider_execution;

#[path = "provider_invoker/provider_usage.rs"]
mod provider_usage;

use provider_usage::*;

impl<R: OrchestrationRuntimeRepository, H> RuntimeProviderInvoker<R, H> {
    /// A scoped invocation reads its own durable receipt. A process-wide active
    /// node is only valid for the pre-existing, unscoped sequential executor.
    async fn active_runtime_node(&self) -> Result<Option<RuntimeActiveNode>> {
        if let Some(identity) =
            orchestration_runtime::execution_engine::container::scoped_execution_identity()
        {
            let flow_run_id = self
                .flow_run_id
                .ok_or_else(|| anyhow!("scoped invocation has no flow run"))?;
            let node_run_id = self
                .repository
                .get_container_node_run_id(flow_run_id, identity.execution_id)
                .await?
                .ok_or_else(|| anyhow!("scoped invocation has no committed node receipt"))?;
            return Ok(Some(RuntimeActiveNode {
                node_id: identity.node_id,
                node_run_id,
            }));
        }
        let active = self
            .flow_execution_context
            .as_ref()
            .map(|context| {
                context
                    .active_node
                    .lock()
                    .map(|node| node.clone())
                    .map_err(|_| anyhow!("active runtime node lock is poisoned"))
            })
            .transpose()?
            .flatten();
        Ok(active.or_else(|| {
            Some(RuntimeActiveNode {
                node_id: self.active_node_id.clone()?,
                node_run_id: self.active_node_run_id?,
            })
        }))
    }
}
