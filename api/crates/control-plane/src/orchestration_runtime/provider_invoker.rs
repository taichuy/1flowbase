use std::collections::BTreeSet;

use super::*;
use plugin_framework::provider_contract::{ProviderMessageRole, ProviderOutputItemPhase};

use super::canonical_stream::{
    CanonicalBlockId, CanonicalCallId, CanonicalContentKind, CanonicalItemId, CanonicalStreamEvent,
    CanonicalStreamState, CanonicalStreamTransitionError,
};

mod protocol_context;

const PROVIDER_LIVE_EVENT_LANE_CAPACITY: usize = 32;

const VISIBLE_INTERNAL_LLM_MEDIA_TOOLS_CONTEXT_KEY: &str = "visible_internal_llm_media_tools";

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::ProviderInvoker for RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository
        + OrchestrationRuntimeRepository
        + PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    async fn compact(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        mut input: ProviderInvocationInput,
    ) -> Result<plugin_framework::provider_contract::ProviderCompactResult> {
        self.apply_provider_transport(runtime, &mut input)?;
        let instance = self.resolve_llm_instance(runtime).await?;
        let installation = self.ready_installation(instance.installation_id).await?;
        let package = load_provider_package(&installation.installed_path)?;
        input.provider_config = build_provider_runtime_config(
            &self.repository,
            &self.provider_secret_master_key,
            &package,
            &instance,
        )
        .await?;

        self.runtime.compact(&installation, input).await
    }

    async fn count_tokens(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        mut input: plugin_framework::provider_contract::ProviderCountTokensInput,
    ) -> Result<plugin_framework::provider_contract::ProviderCountTokensResult> {
        let instance = self.resolve_llm_instance(runtime).await?;
        let installation = self.ready_installation(instance.installation_id).await?;
        let package = load_provider_package(&installation.installed_path)?;
        input.provider_config = build_provider_runtime_config(
            &self.repository,
            &self.provider_secret_master_key,
            &package,
            &instance,
        )
        .await?;

        self.runtime.count_tokens(&installation, input).await
    }

    async fn resolve_protocol_context_locator(&self, locator: &Value) -> Result<Option<Value>> {
        self.open_protocol_context_locator_value(locator).await
    }

    async fn invoke_llm(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        mut input: ProviderInvocationInput,
    ) -> Result<orchestration_runtime::execution_engine::ProviderInvocationOutput> {
        self.apply_provider_transport(runtime, &mut input)?;
        let provider_resolve_started = std::time::Instant::now();
        let instance = self.resolve_llm_instance(runtime).await?;
        tracing::debug!(
            provider_resolve_ms = provider_resolve_started.elapsed().as_millis() as u64,
            "provider resolve finished"
        );

        let installation_reconcile_started = std::time::Instant::now();
        let installation = self.ready_installation(instance.installation_id).await?;
        tracing::debug!(
            installation_reconcile_ms = installation_reconcile_started.elapsed().as_millis() as u64,
            "installation reconcile finished"
        );
        let assigned = self
            .repository
            .list_assignments(self.workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned
            || matches!(
                installation.desired_state,
                domain::PluginDesiredState::Disabled
            )
        {
            return Err(ControlPlaneError::InvalidInput("provider_code").into());
        }
        if installation.availability_status != domain::PluginAvailabilityStatus::Available {
            return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
        }

        let package_load_started = std::time::Instant::now();
        let package = load_provider_package(&installation.installed_path)?;
        tracing::debug!(
            package_load_ms = package_load_started.elapsed().as_millis() as u64,
            "package load finished"
        );
        adapt_or_ensure_model_supports_content_blocks(
            &self.repository,
            &instance,
            &package,
            &runtime.model,
            &mut input,
        )
        .await?;

        let runtime_config_started = std::time::Instant::now();
        input.provider_config = build_provider_runtime_config(
            &self.repository,
            &self.provider_secret_master_key,
            &package,
            &instance,
        )
        .await?;
        tracing::debug!(
            runtime_config_ms = runtime_config_started.elapsed().as_millis() as u64,
            "runtime config finished"
        );

        let canonical_tool_registry = input.tools.clone();
        let provider_invoke_started_at = OffsetDateTime::now_utc();
        let provider_invoke_started = std::time::Instant::now();
        let first_token_timing = Arc::new(Mutex::new(None::<FirstTokenTiming>));
        let mut required_forward_handle = None;
        let mut diagnostic_forward_handle = None;
        let active_node = self
            .flow_execution_context
            .as_ref()
            .and_then(|context| context.active_node.lock().ok()?.clone());
        let active_node = active_node.or_else(|| {
            Some(RuntimeActiveNode {
                node_id: self.active_node_id.clone()?,
                node_run_id: self.active_node_run_id?,
            })
        });
        let presentation_source_node_id = input.trace_context.get("node_id").cloned();
        let live_provider_events = if let Some(RuntimeActiveNode {
            node_id,
            node_run_id,
        }) = active_node
        {
            let live_sender = self.live_provider_events.clone();
            let runtime_event_stream = self.runtime_event_stream.clone();
            let flow_run_id = self.flow_run_id;
            let first_token_timing_for_task = first_token_timing.clone();
            let canonical_tool_registry_for_task = canonical_tool_registry.clone();
            let answer_presentation = answer_presentation_source_is_active(
                presentation_source_node_id.as_deref(),
                &node_id,
            )
            .then(|| self.answer_presentation.clone())
            .flatten();
            let (required_sender, mut required_receiver) =
                mpsc::channel::<ProviderStreamEvent>(PROVIDER_LIVE_EVENT_LANE_CAPACITY);
            let (diagnostic_sender, mut diagnostic_receiver) =
                mpsc::channel::<ProviderStreamEvent>(PROVIDER_LIVE_EVENT_LANE_CAPACITY);
            let diagnostic_node_id = node_id.clone();
            required_forward_handle = Some(tokio::spawn(async move {
                let mut canonical_writer = RuntimeCanonicalStreamWriter::new(node_id.clone());
                while let Some(mut event) = required_receiver.recv().await {
                    orchestration_runtime::execution_engine::canonicalize_provider_stream_event_tool_call_name(
                            &mut event,
                            &canonical_tool_registry_for_task,
                        );
                    record_first_token_timing(
                        &first_token_timing_for_task,
                        &event,
                        provider_invoke_started_at,
                        provider_invoke_started,
                    );
                    let canonical_deltas = canonical_writer.write(&event)?;
                    project_canonical_provider_deltas(
                        runtime_event_stream.as_ref(),
                        flow_run_id,
                        answer_presentation.as_ref(),
                        &node_id,
                        node_run_id,
                        &canonical_deltas,
                    )
                    .await;
                    if let Some(sender) = &live_sender {
                        sender
                            .send(LiveProviderStreamEvent {
                                node_id: node_id.clone(),
                                node_run_id,
                                event: event.clone(),
                            })
                            .await
                            .map_err(|_| anyhow!("required live provider event writer closed"))?;
                    }
                    if let (Some(stream), Some(flow_run_id)) = (&runtime_event_stream, flow_run_id)
                    {
                        let runtime_events = match &event {
                            ProviderStreamEvent::TextDelta { .. }
                            | ProviderStreamEvent::ReasoningDelta { .. }
                            | ProviderStreamEvent::Finish { .. }
                            | ProviderStreamEvent::Error { .. } => Vec::new(),
                            ProviderStreamEvent::ReasoningSignatureDelta { signature } => {
                                vec![debug_stream_events::reasoning_signature_delta(
                                    &node_id,
                                    node_run_id,
                                    signature.clone(),
                                )]
                            }
                            ProviderStreamEvent::OutputItem {
                                phase,
                                output_index,
                                item,
                            } => vec![match phase {
                                ProviderOutputItemPhase::Added => {
                                    debug_stream_events::provider_output_item_added(
                                        &node_id,
                                        node_run_id,
                                        *output_index,
                                        item.clone(),
                                    )
                                }
                                ProviderOutputItemPhase::Done => {
                                    debug_stream_events::provider_output_item_done(
                                        &node_id,
                                        node_run_id,
                                        *output_index,
                                        item.clone(),
                                    )
                                }
                            }],
                            _ => Vec::new(),
                        };
                        if runtime_events.is_empty() {
                            continue;
                        };
                        for runtime_event in runtime_events {
                            let event_type = runtime_event.event_type.clone();
                            let source = runtime_event.source;
                            let mut stream_event = runtime_event;
                            if debug_stream_events::is_answer_presentation_delta_payload(
                                &stream_event.payload,
                            ) {
                                stream_event.persist_required = false;
                            }
                            match stream.append(flow_run_id, stream_event).await {
                                Ok(_) => {}
                                Err(error) => {
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
                        }
                    }
                }
                let completion_deltas = canonical_writer.complete()?;
                project_canonical_provider_deltas(
                    runtime_event_stream.as_ref(),
                    flow_run_id,
                    answer_presentation.as_ref(),
                    &node_id,
                    node_run_id,
                    &completion_deltas,
                )
                .await;
                Ok::<_, anyhow::Error>(canonical_writer.into_state())
            }));
            let diagnostic_stream = self.runtime_event_stream.clone();
            let diagnostic_flow_run_id = self.flow_run_id;
            diagnostic_forward_handle = Some(tokio::spawn(async move {
                while let Some(event) = diagnostic_receiver.recv().await {
                    let ProviderStreamEvent::NativeEvent { protocol, event } = event else {
                        continue;
                    };
                    if let (Some(stream), Some(flow_run_id)) =
                        (&diagnostic_stream, diagnostic_flow_run_id)
                    {
                        let _ = stream
                            .append(
                                flow_run_id,
                                debug_stream_events::provider_native_event(
                                    &diagnostic_node_id,
                                    node_run_id,
                                    protocol,
                                    event,
                                ),
                            )
                            .await;
                    }
                }
            }));
            Some(crate::ports::ProviderLiveEventSenders {
                required: required_sender,
                diagnostic: diagnostic_sender,
            })
        } else {
            None
        };

        let invocation_result = self
            .runtime
            .invoke_stream_with_live_events(&installation, input, live_provider_events)
            .await;
        tracing::debug!(
            provider_invoke_ms = provider_invoke_started.elapsed().as_millis() as u64,
            "provider invoke finished"
        );
        if let Some(handle) = required_forward_handle {
            handle.await.map_err(|error| {
                anyhow!("provider live event forwarding task panicked: {error}")
            })??;
        }
        if let Some(handle) = diagnostic_forward_handle {
            handle.await.map_err(|error| {
                anyhow!("provider diagnostic event forwarding task panicked: {error}")
            })?;
        }
        let invocation_output = invocation_result?;
        self.stage_provider_continuation(runtime, invocation_output.result.response_id.as_deref())
            .await?;
        let captured_first_token_timing = first_token_timing.lock().ok().and_then(|timing| *timing);
        let mut output = orchestration_runtime::execution_engine::ProviderInvocationOutput {
            events: invocation_output.events,
            result: invocation_output.result,
            first_token_at: captured_first_token_timing.map(|timing| timing.first_token_at),
            time_to_first_token_ms: captured_first_token_timing
                .map(|timing| timing.time_to_first_token_ms),
        };
        orchestration_runtime::execution_engine::canonicalize_provider_output_tool_call_names(
            &mut output,
            &canonical_tool_registry,
        );
        Ok(output)
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
            ProviderStreamEvent::NativeEvent { .. }
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
                for mut presentation_event in presentation_events {
                    presentation_event.persist_required = false;
                    append_provider_runtime_event(stream, flow_run_id, presentation_event).await;
                }
            }
        }

        let (Some(stream), Some(flow_run_id)) = (stream, flow_run_id) else {
            continue;
        };
        let mut debug_event = match delta.kind {
            CanonicalContentKind::Text => {
                debug_stream_events::text_delta(node_id, node_run_id, delta.text.clone())
            }
            CanonicalContentKind::Reasoning => {
                debug_stream_events::reasoning_delta(node_id, node_run_id, delta.text.clone())
            }
        };
        debug_event.persist_required = false;
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
    fn apply_provider_transport(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        input: &mut ProviderInvocationInput,
    ) -> Result<()> {
        if let Some(continuation) = self.provider_continuation.as_ref() {
            self.ensure_provider_affinity(runtime, continuation.affinity())?;
            input.previous_response_id = Some(continuation.response_id().to_string());
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
        }

        input
            .required_capabilities
            .insert(native_transport_capability);
        let protocol = match payload.protocol() {
            crate::ports::ProviderTransportProtocol::OpenAiResponses => "openai_responses",
        };
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
    ) -> Result<domain::PluginInstallationRecord> {
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
            _ => reconcile_installation_snapshot(&self.repository, installation_id).await,
        }
    }
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
            flow_execution_context: self.flow_execution_context.clone(),
            answer_presentation: self.answer_presentation.clone(),
            provider_transport_payload: self.provider_transport_payload.clone(),
            provider_transport_store: self.provider_transport_store.clone(),
            provider_continuation: self.provider_continuation.clone(),
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

    pub(super) fn with_provider_transport_payload(
        &self,
        provider_transport_payload: Option<crate::ports::ProviderTransportPayload>,
    ) -> Self {
        let mut invoker = self.clone();
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

    async fn stage_provider_continuation(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        response_id: Option<&str>,
    ) -> Result<()> {
        let Some(response_id) = response_id.filter(|value| !value.trim().is_empty()) else {
            return Ok(());
        };
        let (Some(store), Some(flow_run_id)) =
            (self.provider_transport_store.as_ref(), self.flow_run_id)
        else {
            return Ok(());
        };
        if self.provider_transport_payload.is_none() && self.provider_continuation.is_none() {
            return Ok(());
        }
        let continuation = crate::ports::ProviderContinuation::new(
            response_id,
            crate::ports::ProviderTransportAffinity::new(
                &runtime.provider_instance_id,
                &runtime.provider_code,
                &runtime.protocol,
                &runtime.model,
            ),
        )?;
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
        ) || installation.availability_status != domain::PluginAvailabilityStatus::Available
        {
            return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
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

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::CapabilityInvoker
    for RuntimeProviderInvoker<R, H>
where
    R: ModelDefinitionRepository
        + OrchestrationRuntimeRepository
        + PluginRepository
        + Clone
        + Send
        + Sync,
    H: ProviderRuntimePort + CapabilityPluginRuntimePort + Clone + Send + Sync,
{
    async fn invoke_capability_node(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledPluginRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<orchestration_runtime::execution_engine::CapabilityInvocationOutput> {
        let installation = self.ready_installation(runtime.installation_id).await?;
        let assigned = self
            .repository
            .list_assignments(self.workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned
            || matches!(
                installation.desired_state,
                domain::PluginDesiredState::Disabled
            )
        {
            return Err(ControlPlaneError::InvalidInput("installation_id").into());
        }
        if installation.availability_status != domain::PluginAvailabilityStatus::Available {
            return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
        }

        let output = self
            .runtime
            .execute_node(ExecuteCapabilityNodeInput {
                installation,
                contribution_code: runtime.contribution_code.clone(),
                config_payload,
                input_payload,
            })
            .await?;

        Ok(
            orchestration_runtime::execution_engine::CapabilityInvocationOutput {
                output_payload: output.output_payload,
            },
        )
    }

    async fn invoke_data_model_node(
        &self,
        node: &orchestration_runtime::compiled_plan::CompiledNode,
        resolved_inputs: &serde_json::Map<String, Value>,
    ) -> Result<orchestration_runtime::execution_engine::DataModelInvocationOutput> {
        let context = self
            .flow_execution_context
            .as_ref()
            .ok_or_else(|| anyhow!("data model flow execution context is not configured"))?;
        let active_node = context
            .active_node
            .lock()
            .map_err(|_| anyhow!("active runtime node lock is poisoned"))?
            .clone()
            .ok_or_else(|| anyhow!("data model active runtime node is not set"))?;
        if active_node.node_id != node.node_id {
            return Err(anyhow!(
                "data model active runtime node mismatch: expected {}, got {}",
                node.node_id,
                active_node.node_id
            ));
        }

        let execution = data_model_runtime::execute_data_model_node(
            self.repository.clone(),
            context.data_model.runtime_engine.clone(),
            &context.data_model.actor,
            node,
            resolved_inputs,
            &data_model_runtime::DataModelRunContext {
                workspace_id: self.workspace_id,
                application_id: context.data_model.application_id,
                draft_id: context.data_model.draft_id,
                flow_run_id: context.data_model.flow_run_id,
                node_run_id: active_node.node_run_id,
            },
        )
        .await;

        let (pending_callback, debug_payload) = match execution.waiting_confirmation {
            Some(confirmation) => {
                let request_payload = json!({
                    "kind": "data_model_side_effect_confirmation",
                    "actor_user_id": context.data_model.actor.user_id,
                    "node_id": node.node_id,
                    "run_id": context.data_model.flow_run_id,
                    "payload_hash": confirmation.payload_hash,
                    "idempotency_key": confirmation.idempotency_key,
                    "expires_at": confirmation.expires_at,
                    "request_payload": confirmation.request_payload,
                });
                (
                    Some(orchestration_runtime::execution_engine::DataModelCallback {
                        callback_kind: "data_model_side_effect_confirmation".to_string(),
                        request_payload,
                    }),
                    json!({
                        "side_effect_policy": "confirm_each_run",
                        "idempotency_key": confirmation.idempotency_key,
                        "payload_hash": confirmation.payload_hash,
                        "expires_at": confirmation.expires_at,
                    }),
                )
            }
            None => (None, json!({})),
        };

        Ok(
            orchestration_runtime::execution_engine::DataModelInvocationOutput {
                output_payload: execution.output_payload,
                error_payload: execution.error_payload,
                metrics_payload: execution.metrics_payload,
                debug_payload,
                pending_callback,
            },
        )
    }

    async fn invoke_native_sql_node(
        &self,
        node: &orchestration_runtime::compiled_plan::CompiledNode,
        sql: &str,
    ) -> Result<orchestration_runtime::execution_engine::NativeSqlInvocationOutput> {
        let context = self
            .flow_execution_context
            .as_ref()
            .ok_or_else(|| anyhow!("native SQL flow execution context is not configured"))?;
        let data_source_instance_id = node
            .config
            .get("data_source_instance_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("SQL node is missing data_source_instance_id"))?;
        match context
            .data_model
            .runtime_engine
            .execute_native_sql(self.workspace_id, data_source_instance_id, sql)
            .await
        {
            Ok(output) => Ok(
                orchestration_runtime::execution_engine::NativeSqlInvocationOutput {
                    output_payload: serde_json::to_value(output)?,
                    error_payload: None,
                    metrics_payload: json!({}),
                    debug_payload: json!({
                        "data_source_instance_id": data_source_instance_id,
                        "sql": sql,
                    }),
                },
            ),
            Err(error) => Ok(
                orchestration_runtime::execution_engine::NativeSqlInvocationOutput {
                    output_payload: json!({ "results": [] }),
                    error_payload: Some(native_sql_error_payload(&error)),
                    metrics_payload: json!({}),
                    debug_payload: json!({
                        "data_source_instance_id": data_source_instance_id,
                        "sql": sql,
                    }),
                },
            ),
        }
    }
}

fn native_sql_error_payload(error: &anyhow::Error) -> Value {
    if let Some(runtime_error) = error.downcast_ref::<plugin_framework::ProviderRuntimeError>() {
        return native_sql_provider_error_payload(runtime_error);
    }
    if let Some(framework_error) = error.downcast_ref::<plugin_framework::PluginFrameworkError>() {
        return match framework_error {
            plugin_framework::PluginFrameworkError::RuntimeContract { error } => {
                native_sql_provider_error_payload(error)
            }
            plugin_framework::PluginFrameworkError::InvalidProviderContract { message } => {
                let code = if message.starts_with("data_source_capability_not_supported") {
                    "data_source_capability_not_supported"
                } else {
                    "invalid_native_sql_result_contract"
                };
                json!({
                    "kind": "contract_error",
                    "code": code,
                    "message": message,
                })
            }
            _ => json!({
                "kind": "transport_error",
                "code": "data_source_transport_error",
                "message": framework_error.to_string(),
            }),
        };
    }
    json!({
        "kind": "transport_error",
        "code": "data_source_transport_error",
        "message": error.to_string(),
    })
}

fn native_sql_provider_error_payload(error: &plugin_framework::ProviderRuntimeError) -> Value {
    let code = error
        .provider_details
        .as_ref()
        .and_then(|details| details.get("code"))
        .and_then(Value::as_str)
        .or(error.provider_summary.as_deref())
        .unwrap_or("data_source_error");
    let kind = if code == "outcome_unknown" {
        "outcome_unknown"
    } else if error.kind == plugin_framework::ProviderRuntimeErrorKind::ProviderUpstreamError {
        "source_error"
    } else {
        "transport_error"
    };
    let detail = error
        .provider_details
        .as_ref()
        .and_then(|details| details.get("detail"))
        .cloned()
        .or_else(|| error.provider_details.clone());
    json!({
        "kind": kind,
        "code": code,
        "message": error.message,
        "detail": detail,
    })
}

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::CodeInvoker for RuntimeProviderInvoker<R, H>
where
    R: Clone + Send + Sync,
    H: Clone + Send + Sync,
{
    async fn invoke_code_node(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledCodeRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<orchestration_runtime::execution_engine::CodeInvocationOutput> {
        let input_payload = self.expose_protocol_contexts_to_code(input_payload).await?;
        orchestration_runtime::execution_engine::CodeInvoker::invoke_code_node(
            &orchestration_runtime::execution_engine::QuickJsCodeInvoker::default(),
            runtime,
            config_payload,
            input_payload,
        )
        .await
    }

    async fn protect_protocol_context_output(
        &self,
        output: &mut orchestration_runtime::execution_engine::CodeInvocationOutput,
        selected_output_paths: &[Vec<String>],
    ) -> Result<()> {
        self.protect_code_protocol_context_output(output, selected_output_paths)
            .await
    }

    async fn protect_protocol_context_logs(
        &self,
        console_logs: &mut Vec<orchestration_runtime::execution_engine::ConsoleLogEntry>,
    ) -> Result<()> {
        self.protect_code_console_logs(console_logs).await
    }
}

async fn build_provider_runtime_config<R>(
    repository: &R,
    master_key: &str,
    package: &ProviderPackage,
    instance: &domain::ModelProviderInstanceRecord,
) -> Result<Value>
where
    R: ModelProviderRepository + PluginRepository,
{
    let secret_json = repository
        .get_secret_json(instance.id, master_key)
        .await?
        .unwrap_or_else(empty_object);
    validate_required_fields(
        &package.provider.form_schema,
        &instance.config_json,
        &secret_json,
    )?;
    merge_json_object(&instance.config_json, &secret_json)
}

fn validate_required_fields(
    form_schema: &[ProviderConfigField],
    public_config: &Value,
    secret_config: &Value,
) -> Result<()> {
    let public_object = public_config
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    let secret_object = secret_config
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    for field in form_schema {
        if !field.required {
            continue;
        }
        let value = if is_secret_field(&field.field_type) {
            secret_object.get(&field.key)
        } else {
            public_object.get(&field.key)
        };
        if value.is_none()
            || value == Some(&Value::Null)
            || value == Some(&Value::String(String::new()))
        {
            return Err(ControlPlaneError::InvalidInput("config_json").into());
        }
    }
    Ok(())
}

fn merge_json_object(base: &Value, patch: &Value) -> Result<Value> {
    let mut merged = base
        .as_object()
        .cloned()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    let patch_object = patch
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    for (key, value) in patch_object {
        merged.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(merged))
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

fn is_secret_field(field_type: &str) -> bool {
    field_type.trim().eq_ignore_ascii_case("secret")
}

fn load_provider_package(path: &str) -> Result<ProviderPackage> {
    ProviderPackage::load_from_dir(path)
        .map_err(|_| ControlPlaneError::InvalidInput("provider_package").into())
}

async fn adapt_or_ensure_model_supports_content_blocks<R>(
    repository: &R,
    instance: &domain::ModelProviderInstanceRecord,
    package: &ProviderPackage,
    model_id: &str,
    input: &mut ProviderInvocationInput,
) -> Result<()>
where
    R: ModelProviderRepository,
{
    if !provider_input_has_media_content_blocks(input) {
        return Ok(());
    }

    if selected_model_supports_multimodal(repository, instance, package, model_id).await? {
        return Ok(());
    }

    textualize_media_content_blocks_for_text_model(input);
    Ok(())
}

fn provider_input_has_media_content_blocks(input: &ProviderInvocationInput) -> bool {
    input.messages.iter().any(|message| {
        message
            .content_blocks
            .as_ref()
            .is_some_and(content_blocks_have_media)
    })
}

fn content_blocks_have_media(content_blocks: &Value) -> bool {
    content_blocks.as_array().is_some_and(|blocks| {
        blocks.iter().any(|block| {
            block
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(is_media_block_type)
        })
    })
}

pub(super) fn textualize_media_content_blocks_for_text_model(input: &mut ProviderInvocationInput) {
    let routed_media_tools = routed_media_tool_context(input);
    for message in &mut input.messages {
        let Some(content_blocks) = message.content_blocks.take() else {
            continue;
        };
        let media_blocks = summarize_media_blocks(&content_blocks);
        if media_blocks.as_array().is_none_or(Vec::is_empty) {
            message.content_blocks = Some(content_blocks);
            continue;
        }
        let fallback = if let Some(routed_media_tools) = &routed_media_tools {
            routed_media_guidance_content(routed_media_tools, &media_blocks)
        } else {
            unsupported_media_content(&message.role, &media_blocks)
        };
        if message.content.trim().is_empty() {
            message.content = fallback;
        } else {
            message.content = format!("{}\n{}", message.content.trim_end(), fallback);
        }
        if let Some(remaining_content_blocks) =
            retain_non_text_media_content_blocks(&content_blocks)
        {
            message.content_blocks = Some(remaining_content_blocks);
        }
    }
}

fn routed_media_tool_context(input: &ProviderInvocationInput) -> Option<Value> {
    let tools = input
        .run_context
        .get(VISIBLE_INTERNAL_LLM_MEDIA_TOOLS_CONTEXT_KEY)?
        .as_array()?
        .iter()
        .filter_map(|tool| {
            let object = tool.as_object()?;
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())?;
            let media_kind = object
                .get("media_kind")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|kind| !kind.is_empty())
                .unwrap_or("image");
            Some(json!({
                "name": name,
                "media_kind": media_kind,
            }))
        })
        .collect::<Vec<_>>();

    (!tools.is_empty()).then(|| Value::Array(tools))
}

fn routed_media_guidance_content(routed_media_tools: &Value, media_blocks: &Value) -> String {
    json!({
        "event": "routed_media_content_available",
        "message": "Media content is available in conversation history for routed media tools. Call the routed media tool again with the same media path and task; do not ask the user to re-upload the image.",
        "media_tools": routed_media_tools,
        "media_blocks": media_blocks,
    })
    .to_string()
}

fn unsupported_media_content(role: &ProviderMessageRole, media_blocks: &Value) -> String {
    let (error_code, message_text) = if matches!(role, ProviderMessageRole::Tool) {
        (
            "tool_result_media_unsupported",
            "Tool result contained media blocks that were not injected into the selected text model context.",
        )
    } else {
        (
            "message_media_unsupported",
            "Message contained media blocks that were not injected into the selected text model context.",
        )
    };
    json!({
        "error_code": error_code,
        "message": message_text,
        "recoverable": true,
        "media_blocks": media_blocks,
    })
    .to_string()
}

fn summarize_media_blocks(content_blocks: &Value) -> Value {
    let Some(blocks) = content_blocks.as_array() else {
        return Value::Array(Vec::new());
    };
    Value::Array(blocks.iter().filter_map(summarize_media_block).collect())
}

fn summarize_media_block(block: &Value) -> Option<Value> {
    let block_type = block.get("type").and_then(Value::as_str)?;
    if !is_media_block_type(block_type) {
        return None;
    }
    let mut summary = serde_json::Map::new();
    summary.insert("type".to_string(), Value::String(block_type.to_string()));
    if let Some(source_type) = block
        .get("source")
        .and_then(|source| source.get("type"))
        .and_then(Value::as_str)
    {
        summary.insert(
            "source_type".to_string(),
            Value::String(source_type.to_string()),
        );
    }
    if let Some(media_type) = block
        .get("source")
        .and_then(|source| source.get("media_type"))
        .or_else(|| block.get("media_type"))
        .and_then(Value::as_str)
    {
        summary.insert(
            "media_type".to_string(),
            Value::String(media_type.to_string()),
        );
    }
    if let Some(url) = block
        .get("image_url")
        .and_then(|image_url| image_url.get("url"))
        .or_else(|| block.get("source").and_then(|source| source.get("url")))
        .and_then(Value::as_str)
    {
        summary.insert("url".to_string(), Value::String(summarized_media_url(url)));
    }
    Some(Value::Object(summary))
}

fn summarized_media_url(url: &str) -> String {
    let trimmed = url.trim();
    if !trimmed.starts_with("data:") {
        return trimmed.to_string();
    }
    let prefix = trimmed
        .split_once(',')
        .map(|(prefix, _)| prefix)
        .unwrap_or("data:[redacted]");
    format!("{prefix},[redacted]")
}

fn retain_non_text_media_content_blocks(content_blocks: &Value) -> Option<Value> {
    let blocks = content_blocks.as_array()?;
    let retained = blocks
        .iter()
        .filter(|block| {
            !block
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|block_type| block_type == "text" || is_media_block_type(block_type))
        })
        .cloned()
        .collect::<Vec<_>>();
    (!retained.is_empty()).then_some(Value::Array(retained))
}

fn is_media_block_type(block_type: &str) -> bool {
    matches!(
        block_type,
        "image" | "document" | "image_url" | "input_image"
    )
}

async fn selected_model_supports_multimodal<R>(
    repository: &R,
    instance: &domain::ModelProviderInstanceRecord,
    package: &ProviderPackage,
    model_id: &str,
) -> Result<bool>
where
    R: ModelProviderRepository,
{
    if let Some(supports_multimodal) = instance
        .configured_models
        .iter()
        .find(|model| model.enabled && model.model_id == model_id)
        .and_then(|model| model.supports_multimodal)
    {
        return Ok(supports_multimodal);
    }

    if let Some(cache) = repository.get_catalog_cache(instance.id).await? {
        let models: Vec<ProviderModelDescriptor> = serde_json::from_value(cache.models_json)?;
        if let Some(model) = models.iter().find(|model| model.model_id == model_id) {
            return Ok(model.supports_multimodal);
        }
    }

    if let Some(model) = package
        .predefined_models
        .iter()
        .find(|model| model.model_id == model_id)
    {
        return Ok(model.supports_multimodal);
    }

    Ok(false)
}

pub(super) async fn freeze_failover_queue_routes<R>(
    repository: &R,
    workspace_id: Uuid,
    compiled_plan: &mut orchestration_runtime::compiled_plan::CompiledPlan,
) -> Result<()>
where
    R: ModelProviderRepository + PluginRepository,
{
    for node in compiled_plan.nodes.values_mut() {
        let Some(runtime) = node.llm_runtime.as_mut() else {
            continue;
        };
        let Some(routing) = runtime.routing.as_mut() else {
            continue;
        };
        if routing.routing_mode
            != orchestration_runtime::compiled_plan::LlmRoutingMode::FailoverQueue
            || !routing.queue_targets.is_empty()
        {
            continue;
        }

        let queue_template_id = routing
            .queue_template_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or(ControlPlaneError::InvalidInput("queue_template_id"))?;
        let queue = repository
            .get_failover_queue_template(queue_template_id)
            .await?
            .ok_or(ControlPlaneError::InvalidInput("queue_template_id"))?;
        if queue.status != "active" {
            return Err(ControlPlaneError::InvalidInput("queue_template_id").into());
        }
        let items = repository
            .list_failover_queue_items(queue_template_id)
            .await?;
        let snapshot_items = items
            .iter()
            .cloned()
            .map(FailoverQueueSnapshotItem::from)
            .collect::<Vec<_>>();
        let snapshot = repository
            .create_failover_queue_snapshot(&crate::ports::CreateModelFailoverQueueSnapshotInput {
                snapshot_id: Uuid::now_v7(),
                queue_template_id,
                version: queue.version,
                items: freeze_queue_items(&snapshot_items),
            })
            .await?;
        routing.queue_snapshot_id = Some(snapshot.id.to_string());
        let provider_display_names = routing
            .queue_targets
            .iter()
            .map(|target| {
                (
                    target.provider_instance_id.clone(),
                    target.provider_instance_display_name.clone(),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let mut provider_runtime_capabilities = std::collections::HashMap::new();
        for item in snapshot_items.iter().filter(|item| item.enabled) {
            let capabilities = match repository
                .get_instance(workspace_id, item.provider_instance_id)
                .await?
            {
                Some(instance) => repository
                    .get_installation(instance.installation_id)
                    .await?
                    .and_then(|installation| {
                        installation
                            .metadata_json
                            .get("runtime_capabilities")
                            .and_then(Value::as_array)
                            .map(|values| {
                                values
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_string)
                                    .collect::<BTreeSet<_>>()
                            })
                    })
                    .unwrap_or_default(),
                None => BTreeSet::new(),
            };
            provider_runtime_capabilities
                .insert(item.provider_instance_id.to_string(), capabilities);
        }
        routing.queue_targets = snapshot_items
            .into_iter()
            .filter(|item| item.enabled)
            .map(
                |item| orchestration_runtime::compiled_plan::CompiledLlmRouteTarget {
                    provider_instance_id: item.provider_instance_id.to_string(),
                    provider_instance_display_name: provider_display_names
                        .get(&item.provider_instance_id.to_string())
                        .cloned()
                        .unwrap_or_default(),
                    provider_code: item.provider_code,
                    protocol: item.protocol,
                    upstream_model_id: item.upstream_model_id,
                    runtime_capabilities: provider_runtime_capabilities
                        .get(&item.provider_instance_id.to_string())
                        .cloned()
                        .unwrap_or_default(),
                },
            )
            .collect();
        let Some(first_target) = routing.queue_targets.first() else {
            return Err(ControlPlaneError::InvalidInput("queue_template_id").into());
        };
        runtime.provider_instance_id = first_target.provider_instance_id.clone();
        runtime.provider_code = first_target.provider_code.clone();
        runtime.protocol = first_target.protocol.clone();
        runtime.model = first_target.upstream_model_id.clone();
    }

    Ok(())
}

#[cfg(test)]
mod canonical_writer_tests {
    use super::*;
    use plugin_framework::provider_contract::{ProviderFinishReason, ProviderUsage};

    #[test]
    fn answer_presentation_requires_the_provider_trace_to_match_the_active_node() {
        assert!(answer_presentation_source_is_active(
            Some("node-main"),
            "node-main"
        ));
        assert!(!answer_presentation_source_is_active(
            Some("node-fusion-panel"),
            "node-main"
        ));
        assert!(!answer_presentation_source_is_active(None, "node-main"));
    }

    #[test]
    fn ac_001_runtime_writer_preserves_partitioned_text_and_reasoning() {
        let mut writer = RuntimeCanonicalStreamWriter::new("item-1");

        for event in [
            ProviderStreamEvent::TextDelta {
                delta: "<think>same ".to_string(),
            },
            ProviderStreamEvent::TextDelta {
                delta: "same</think>answer  ".to_string(),
            },
            ProviderStreamEvent::TextDelta {
                delta: "\n`code`answer  ".to_string(),
            },
            ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Stop,
            },
        ] {
            writer.write(&event).unwrap();
        }

        assert_eq!(
            writer.state().accumulated().reasoning().as_str(),
            "same same"
        );
        assert_eq!(
            writer.state().accumulated().text().as_str(),
            "answer  \n`code`answer  "
        );
    }

    #[test]
    fn ac_003_runtime_writer_accumulates_tool_arguments_and_usage() {
        let mut writer = RuntimeCanonicalStreamWriter::new("item-1");
        for delta in ["{\"query\":\"", "same", "same", "\"}"] {
            writer
                .write(&ProviderStreamEvent::ToolCallDelta {
                    call_id: " call-1 ".to_string(),
                    delta: json!({ "function": { "arguments": delta } }),
                })
                .unwrap();
        }
        writer
            .write(&ProviderStreamEvent::UsageDelta {
                usage: ProviderUsage {
                    input_tokens: Some(2),
                    total_tokens: Some(2),
                    ..ProviderUsage::default()
                },
            })
            .unwrap();
        writer
            .write(&ProviderStreamEvent::UsageSnapshot {
                usage: ProviderUsage {
                    input_tokens: Some(5),
                    output_tokens: Some(3),
                    total_tokens: Some(8),
                    ..ProviderUsage::default()
                },
            })
            .unwrap();

        let call_id = CanonicalCallId::new(CanonicalItemId::new("item-1"), " call-1 ");
        assert_eq!(
            writer
                .state()
                .accumulated()
                .tool_call(&call_id)
                .unwrap()
                .arguments()
                .as_str(),
            "{\"query\":\"samesame\"}"
        );
        assert_eq!(
            writer.state().accumulated().usage().value(),
            &ProviderUsage {
                input_tokens: Some(5),
                output_tokens: Some(3),
                total_tokens: Some(8),
                ..ProviderUsage::default()
            }
        );
    }

    #[test]
    fn ac_006_runtime_writer_rejects_every_post_terminal_provider_event() {
        let mut writer = RuntimeCanonicalStreamWriter::new("item-1");
        writer
            .write(&ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Stop,
            })
            .unwrap();

        for event in [
            ProviderStreamEvent::TextDelta {
                delta: "late".to_string(),
            },
            ProviderStreamEvent::NativeEvent {
                protocol: "fixture".to_string(),
                event: json!({}),
            },
            ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Length,
            },
        ] {
            assert!(writer
                .write(&event)
                .unwrap_err()
                .to_string()
                .contains("already terminal"));
        }
        assert!(writer.state().accumulated().text().is_empty());
    }

    #[test]
    fn runtime_canonical_writer_applies_verified_provider_output_item_phases() {
        let mut writer = RuntimeCanonicalStreamWriter::new("item-1");
        let item = json!({
            "id": "approval_1",
            "type": "mcp_approval_request",
            "name": "delete_record"
        });
        writer
            .write(&ProviderStreamEvent::OutputItem {
                phase: ProviderOutputItemPhase::Added,
                output_index: 1,
                item: item.clone(),
            })
            .unwrap();
        writer
            .write(&ProviderStreamEvent::OutputItem {
                phase: ProviderOutputItemPhase::Done,
                output_index: 1,
                item: item.clone(),
            })
            .unwrap();

        let phases = writer.state().accumulated().output_items();
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].phase(), ProviderOutputItemPhase::Added);
        assert_eq!(phases[1].phase(), ProviderOutputItemPhase::Done);
        assert_eq!(phases[1].item(), &item);
    }
}

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/support.rs"]
pub(crate) mod test_support;
