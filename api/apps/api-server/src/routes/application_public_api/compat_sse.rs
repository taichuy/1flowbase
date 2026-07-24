use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    sync::Arc,
    time::Duration,
};

use axum::response::{
    sse::{Event, KeepAlive, Sse},
    IntoResponse, Response,
};
use control_plane::application_public_api::{
    callback_resume::{
        ApplicationPublishedCallbackResumeService, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    },
    native::NativeRunStatus,
};
use control_plane::{
    application_public_api::{
        compat::openai::response_id_from_run_id,
        native::{NativeRunResult, NativeUsage},
    },
    orchestration_runtime::{
        debug_stream_events, OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{OrchestrationRuntimeRepository, RuntimeEventEnvelope, RuntimeEventPayload},
};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::routes::application_public_api::tool_callback_ids::encode_anthropic_callback_tool_use_id;
#[cfg(test)]
use crate::routes::application_public_api::tool_callback_ids::encode_openai_callback_tool_call_id;
use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    routes::application_public_api::{
        native::{service_error, NativeApiError},
        stream_terminal_fallback::{
            enrich_terminal_runtime_event_with_durable_answer,
            load_latest_native_run_for_terminal_fallback, recover_missing_stream_terminal_winner,
            terminal_answer_deltas_from_payload, terminal_answer_text_from_payload,
            terminal_runtime_event_from_native_run, TerminalAnswerDelta, TerminalAnswerDeltaKind,
        },
    },
};

mod event_forwarding;
mod protocol_mappers;
#[cfg(test)]
mod tests;

#[cfg(test)]
use event_forwarding::send_compatible_runtime_event_stream;
use event_forwarding::{
    append_compatible_resume_terminal_event, is_answer_presentation_delta,
    send_subscribed_compatible_runtime_event_stream, SubscribedCompatibleRuntimeEventStream,
};
#[cfg(test)]
use protocol_mappers::anthropic_completed_run_to_sse;
use protocol_mappers::{
    terminal_answer_deltas_from_run_or_payload, AnthropicStreamMapper, OpenAiChatStreamMapper,
    OpenAiResponseStreamMapper,
};

type CompatRunSseStream = tokio_stream::wrappers::ReceiverStream<Result<Event, Infallible>>;

const OPENAI_CHAT_SSE_PROJECTION: &str = "openai_chat";
const OPENAI_RESPONSES_SSE_PROJECTION: &str = "openai_responses";
const ANTHROPIC_SSE_PROJECTION: &str = "anthropic";

pub(crate) struct CompatibleResumePlan {
    pub(crate) initial_run: NativeRunResult,
    pub(crate) command: ResumePublishedCallbackCommand,
}

enum CompatibleTurnAction {
    Start,
    Resume(ResumePublishedCallbackCommand),
}

impl CompatibleTurnAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Resume(_) => "resume",
        }
    }

    fn resumed_callback_task_id(&self) -> Option<uuid::Uuid> {
        match self {
            Self::Start => None,
            Self::Resume(command) => Some(callback_task_id_from_resume_command(command)),
        }
    }
}

enum CompatibleProtocolProjection {
    OpenAiChat(OpenAiChatStreamMapper),
    OpenAiResponses(OpenAiResponseStreamMapper),
    AnthropicMessages(AnthropicStreamMapper),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompatibleAnswerDeltaKind {
    Reasoning,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompatibleTerminalKind {
    Finished,
    Incomplete,
    Failed,
    Cancelled,
    WaitingHuman,
    WaitingCallback,
}

struct CompatibleRuntimeEventView {
    envelope: RuntimeEventEnvelope,
    answer_delta: Option<CompatibleAnswerDeltaKind>,
    terminal: Option<CompatibleTerminalKind>,
}

impl CompatibleRuntimeEventView {
    fn answer_delta(&self) -> Option<CompatibleAnswerDeltaKind> {
        self.answer_delta
    }

    fn terminal(&self) -> Option<CompatibleTerminalKind> {
        self.terminal
    }

    fn envelope(&self) -> &RuntimeEventEnvelope {
        &self.envelope
    }

    fn into_envelope(self) -> RuntimeEventEnvelope {
        self.envelope
    }
}

impl From<RuntimeEventEnvelope> for CompatibleRuntimeEventView {
    fn from(envelope: RuntimeEventEnvelope) -> Self {
        let answer_delta = is_answer_presentation_delta(&envelope)
            .then_some(match envelope.event_type.as_str() {
                "reasoning_delta" => Some(CompatibleAnswerDeltaKind::Reasoning),
                "text_delta" => Some(CompatibleAnswerDeltaKind::Text),
                _ => None,
            })
            .flatten();
        let terminal = match envelope.event_type.as_str() {
            "flow_finished" => Some(CompatibleTerminalKind::Finished),
            "flow_incomplete" => Some(CompatibleTerminalKind::Incomplete),
            "flow_failed" => Some(CompatibleTerminalKind::Failed),
            "flow_cancelled" => Some(CompatibleTerminalKind::Cancelled),
            "waiting_human" => Some(CompatibleTerminalKind::WaitingHuman),
            "waiting_callback" => Some(CompatibleTerminalKind::WaitingCallback),
            _ => None,
        };
        Self {
            envelope,
            answer_delta,
            terminal,
        }
    }
}

impl CompatibleProtocolProjection {
    fn name(&self) -> &'static str {
        match self {
            Self::OpenAiChat(_) => OPENAI_CHAT_SSE_PROJECTION,
            Self::OpenAiResponses(_) => OPENAI_RESPONSES_SSE_PROJECTION,
            Self::AnthropicMessages(_) => ANTHROPIC_SSE_PROJECTION,
        }
    }

    fn runtime_event_to_sse(
        &mut self,
        run: &NativeRunResult,
        envelope: RuntimeEventEnvelope,
    ) -> Vec<Result<Event, Infallible>> {
        let event = CompatibleRuntimeEventView::from(envelope);
        match self {
            Self::OpenAiChat(mapper) => mapper.runtime_event_to_sse(run, event),
            Self::OpenAiResponses(mapper) => mapper.runtime_event_to_sse(run, event),
            Self::AnthropicMessages(mapper) => mapper.runtime_event_to_sse(run, event),
        }
    }
}

pub(crate) async fn prepare_compatible_resume(
    state: Arc<ApiState>,
    command: ResumePublishedCallbackCommand,
) -> Result<CompatibleResumePlan, NativeApiError> {
    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
    .with_file_storage_registry(state.file_storage_registry.clone())
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue())
    .with_runtime_event_stream(state.runtime_event_stream.clone());
    let prepared =
        ApplicationPublishedCallbackResumeService::new(state.store.clone(), runtime_service)
            .with_last_used_cache(state.infrastructure.cache_store())
            .prepare_callback_resume(&command)
            .await
            .map_err(service_error)?;
    Ok(CompatibleResumePlan {
        initial_run: prepared.initial_run,
        command,
    })
}

#[derive(Debug, Default)]
struct CompatibleStreamStats {
    emitted_public_event: bool,
    emitted_content_bytes: usize,
    emitted_text_content: bool,
    emitted_reasoning_content: bool,
    forwarded_event_identities: HashSet<CompatiblePublicEventIdentity>,
    forwarded_answer_chunks: HashSet<CompatiblePublicEventChunkIdentity>,
    forwarded_answer_text: HashMap<CompatiblePublicEventIdentity, String>,
    durable_answer_text: HashMap<CompatiblePublicEventIdentity, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompatiblePublicEventIdentity {
    event_type: String,
    node_id: Option<String>,
    answer_node_id: Option<String>,
    segment_index: Option<String>,
    source_node_id: Option<String>,
    source_node_run_id: Option<String>,
    source_output_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompatiblePublicEventChunkIdentity {
    projection: CompatiblePublicEventIdentity,
    text: String,
}

impl CompatibleStreamStats {
    fn emitted_content(&self) -> bool {
        self.emitted_content_bytes > 0
    }

    fn claim_runtime_event(&mut self, event: &mut RuntimeEventEnvelope) -> bool {
        let Some(identity) = compatible_public_event_identity(event) else {
            return true;
        };
        if !is_answer_presentation_delta(event) {
            return self.forwarded_event_identities.insert(identity);
        }
        let Some(text) = event.text.as_deref().filter(|text| !text.is_empty()) else {
            return true;
        };
        let is_batched_durable_delta = event
            .payload
            .get("event_ids")
            .and_then(Value::as_array)
            .is_some();
        let forwarded = self
            .forwarded_answer_text
            .entry(identity.clone())
            .or_default();
        if is_batched_durable_delta {
            // Durable batches can split the live stream at different boundaries. Reconcile the
            // cumulative durable prefix instead of comparing each batch with the full live text.
            let durable = self
                .durable_answer_text
                .entry(identity.clone())
                .or_default();
            durable.push_str(text);
            if forwarded.starts_with(durable.as_str()) {
                return false;
            }
            if durable.starts_with(forwarded.as_str()) {
                let suffix = durable[forwarded.len()..].to_string();
                if suffix.is_empty() {
                    return false;
                }
                forwarded.push_str(&suffix);
                event.text = Some(suffix.clone());
                if let Some(payload) = event.payload.as_object_mut() {
                    payload.insert("text".to_string(), Value::String(suffix));
                }
                return true;
            }
        }
        let chunk = CompatiblePublicEventChunkIdentity {
            projection: identity,
            text: text.to_string(),
        };
        if !self.forwarded_answer_chunks.insert(chunk) {
            return false;
        }
        forwarded.push_str(text);
        true
    }

    fn record_sent_runtime_event(
        &mut self,
        run: &NativeRunResult,
        event: &RuntimeEventEnvelope,
        emitted_public_event: bool,
    ) {
        self.emitted_public_event |= emitted_public_event;
        if is_answer_presentation_delta(event) {
            if !emitted_public_event {
                return;
            }
            let Some(text) = event.text.as_deref().filter(|text| !text.is_empty()) else {
                return;
            };
            match event.event_type.as_str() {
                "reasoning_delta" => self.record_reasoning_content(text),
                "text_delta" => self.record_text_content(text),
                _ => {}
            }
            return;
        }

        if !matches!(
            event.event_type.as_str(),
            "flow_finished" | "flow_incomplete"
        ) {
            return;
        }
        for delta in terminal_answer_deltas_from_run_or_payload(run, &event.payload) {
            match delta.kind {
                TerminalAnswerDeltaKind::Reasoning if !self.emitted_reasoning_content => {
                    self.record_reasoning_content(&delta.text);
                }
                TerminalAnswerDeltaKind::Text if !self.emitted_text_content => {
                    self.record_text_content(&delta.text);
                }
                _ => {}
            }
        }
    }

    fn record_text_content(&mut self, text: &str) {
        self.emitted_text_content = true;
        self.emitted_content_bytes += text.len();
    }

    fn record_reasoning_content(&mut self, text: &str) {
        self.emitted_reasoning_content = true;
        self.emitted_content_bytes += text.len();
    }
}

fn compatible_public_event_identity(
    event: &RuntimeEventEnvelope,
) -> Option<CompatiblePublicEventIdentity> {
    if event.event_type == "flow_started" {
        return Some(CompatiblePublicEventIdentity {
            event_type: event.event_type.clone(),
            node_id: None,
            answer_node_id: None,
            segment_index: None,
            source_node_id: None,
            source_node_run_id: None,
            source_output_key: None,
        });
    }
    if !is_answer_presentation_delta(event) {
        return None;
    }

    let presentation = event.payload.get("presentation");
    Some(CompatiblePublicEventIdentity {
        event_type: event.event_type.clone(),
        node_id: event
            .payload
            .get("node_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        answer_node_id: presentation_value_identity(presentation, "answer_node_id"),
        segment_index: presentation_value_identity(presentation, "segment_index"),
        source_node_id: presentation_value_identity(presentation, "source_node_id"),
        source_node_run_id: presentation_value_identity(presentation, "source_node_run_id"),
        source_output_key: presentation_value_identity(presentation, "source_output_key"),
    })
}

fn presentation_value_identity(presentation: Option<&Value>, key: &str) -> Option<String> {
    presentation
        .and_then(|value| value.get(key))
        .map(|value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
}

pub(crate) async fn start_openai_run_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
) -> Result<Response, NativeApiError> {
    let mapper =
        OpenAiChatStreamMapper::new(model, openai_chat_completion_id_from_run_id(run.id), true);
    start_compatible_turn_stream(
        state,
        run,
        CompatibleTurnAction::Start,
        None,
        CompatibleProtocolProjection::OpenAiChat(mapper),
    )
    .await
}

pub(crate) async fn start_openai_response_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
    previous_response_id: Option<String>,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
) -> Result<Response, NativeApiError> {
    let mapper = OpenAiResponseStreamMapper::new(model, previous_response_id, true);
    start_compatible_turn_stream(
        state,
        run,
        CompatibleTurnAction::Start,
        provider_transport_slot,
        CompatibleProtocolProjection::OpenAiResponses(mapper),
    )
    .await
}

/// Compact is a unary provider operation, so its completed Responses event is
/// projected directly instead of opening a runtime event stream or creating a
/// flow run.
pub(crate) fn openai_compact_sse_response(response: Value) -> Result<Response, NativeApiError> {
    let event = Event::default()
        .event("response.completed")
        .json_data(json!({
            "type": "response.completed",
            "response": response,
        }))
        .map_err(|_| {
            NativeApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "openai_compact_response_serialization_failed",
                "could not serialize OpenAI Compact response",
            )
        })?;

    Ok(
        Sse::new(tokio_stream::iter([Ok::<Event, Infallible>(event)]))
            .keep_alive(
                KeepAlive::new()
                    .interval(Duration::from_secs(10))
                    .text("heartbeat"),
            )
            .into_response(),
    )
}

pub(crate) async fn start_openai_chat_resume_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
    chat_completion_id: String,
    command: ResumePublishedCallbackCommand,
) -> Result<Response, NativeApiError> {
    let mapper = OpenAiChatStreamMapper::new(model, chat_completion_id, true);
    start_compatible_turn_stream(
        state,
        run,
        CompatibleTurnAction::Resume(command),
        None,
        CompatibleProtocolProjection::OpenAiChat(mapper),
    )
    .await
}

pub(crate) async fn start_openai_response_resume_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
    previous_response_id: Option<String>,
    command: ResumePublishedCallbackCommand,
) -> Result<Response, NativeApiError> {
    let mapper = OpenAiResponseStreamMapper::new(model, previous_response_id, true);
    start_compatible_turn_stream(
        state,
        run,
        CompatibleTurnAction::Resume(command),
        None,
        CompatibleProtocolProjection::OpenAiResponses(mapper),
    )
    .await
}

pub(crate) async fn start_anthropic_run_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
) -> Result<Response, NativeApiError> {
    let mapper = AnthropicStreamMapper::new(model);
    start_compatible_turn_stream(
        state,
        run,
        CompatibleTurnAction::Start,
        None,
        CompatibleProtocolProjection::AnthropicMessages(mapper),
    )
    .await
}

pub(crate) async fn start_anthropic_resume_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
    command: ResumePublishedCallbackCommand,
) -> Result<Response, NativeApiError> {
    start_compatible_turn_stream(
        state,
        run,
        CompatibleTurnAction::Resume(command),
        None,
        CompatibleProtocolProjection::AnthropicMessages(AnthropicStreamMapper::new(model)),
    )
    .await
}

#[cfg(test)]
fn test_projected_events_response(events: Vec<Result<Event, Infallible>>) -> Response {
    Sse::new(tokio_stream::iter(events))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(10))
                .text("heartbeat"),
        )
        .into_response()
}

async fn start_compatible_turn_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    action: CompatibleTurnAction,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
    mut projection: CompatibleProtocolProjection,
) -> Result<Response, NativeApiError> {
    let sse_projection = projection.name();
    let turn_action = action.name();
    if let Err(error) = state
        .runtime_event_stream
        .open_run(
            run.id,
            control_plane::ports::RuntimeEventStreamPolicy::debug_default(),
        )
        .await
    {
        warn!(
            flow_run_id = %run.id,
            application_id = %run.application_id,
            error = %error,
            "failed to open compatible public API runtime event stream"
        );
        return Err(service_error(error));
    }

    let ignored_waiting_callback_task_id = action.resumed_callback_task_id();
    let from_sequence = if ignored_waiting_callback_task_id.is_some() {
        let resume_started = state
            .runtime_event_stream
            .append(run.id, debug_stream_events::flow_started(run.id))
            .await
            .map_err(service_error)?;
        Some(resume_started.sequence.saturating_sub(1))
    } else {
        None
    };
    let subscription = state
        .runtime_event_stream
        .subscribe(run.id, from_sequence)
        .await
        .map_err(|error| {
            warn!(
                flow_run_id = %run.id,
                application_id = %run.application_id,
                turn_action,
                error = %error,
                "failed to subscribe compatible public API runtime event stream"
            );
            service_error(error)
        })?;

    let (sender, receiver) = mpsc::channel(32);
    let execution_sender_guard = sender.clone();
    tokio::spawn(send_subscribed_compatible_runtime_event_stream(
        SubscribedCompatibleRuntimeEventStream {
            state: state.clone(),
            initial_run: run.clone(),
            sse_projection,
            from_sequence,
            ignored_waiting_callback_task_id,
            subscription,
            sender,
            mapper: move |run: &NativeRunResult, envelope: RuntimeEventEnvelope| {
                projection.runtime_event_to_sse(run, envelope)
            },
        },
    ));

    let background_state = state.clone();
    let background_run = run.clone();
    tokio::spawn(async move {
        let _execution_sender_guard = execution_sender_guard;
        let runtime_service = OrchestrationRuntimeService::new(
            background_state.store.clone(),
            ApiProviderRuntime::new(background_state.provider_runtime.clone()),
            background_state.runtime_engine.clone(),
            background_state.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            background_state.api_node_id.clone(),
            background_state.provider_install_root.clone(),
        )
        .with_file_storage_registry(background_state.file_storage_registry.clone())
        .with_llm_routing_counter_store(background_state.infrastructure.cache_store())
        .with_provider_request_log_queue(background_state.infrastructure.task_queue())
        .with_provider_transport_store(background_state.infrastructure.provider_transport_store())
        .with_runtime_event_stream(background_state.runtime_event_stream.clone());
        match action {
            CompatibleTurnAction::Start => {
                if let Err(runtime_error) = runtime_service
                    .start_published_flow_run(StartPublishedFlowRunCommand {
                        application_id: background_run.application_id,
                        flow_run_id: background_run.id,
                        provider_transport_slot,
                    })
                    .await
                {
                    warn!(
                        flow_run_id = %background_run.id,
                        application_id = %background_run.application_id,
                        error = %runtime_error,
                        "compatible public API streamed run failed"
                    );
                    if let Err(recovery_error) =
                        recover_missing_stream_terminal_winner(&background_state, &background_run)
                            .await
                    {
                        warn!(
                            flow_run_id = %background_run.id,
                            application_id = %background_run.application_id,
                            error = %recovery_error,
                            "failed to recover the durable winner after compatible streaming execution ended"
                        );
                    }
                }
            }
            CompatibleTurnAction::Resume(command) => {
                match ApplicationPublishedCallbackResumeService::new(
                    background_state.store.clone(),
                    runtime_service,
                )
                .with_last_used_cache(background_state.infrastructure.cache_store())
                .resume_callback(command)
                .await
                {
                    Ok(result) => {
                        append_compatible_resume_terminal_event(&background_state, &result.run)
                            .await
                    }
                    Err(error) => {
                        warn!(
                            flow_run_id = %background_run.id,
                            error = %error,
                            "compatible callback resume failed"
                        );
                        let _ = background_state
                            .runtime_event_stream
                            .append_terminal_if_missing_and_close(
                                background_run.id,
                                debug_stream_events::flow_failed(
                                    background_run.id,
                                    json!({ "message": error.to_string() }),
                                ),
                            )
                            .await;
                    }
                }
            }
        }
    });

    info!(
        flow_run_id = %run.id,
        application_id = %run.application_id,
        sse_projection = %sse_projection,
        turn_action,
        heartbeat_interval_secs = 10_u64,
        heartbeat_text = "heartbeat",
        "compatible public API stream opened"
    );

    Ok(Sse::new(CompatRunSseStream::new(receiver))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(10))
                .text("heartbeat"),
        )
        .into_response())
}

fn callback_task_id_from_resume_command(command: &ResumePublishedCallbackCommand) -> uuid::Uuid {
    match &command.target {
        PublishedCallbackResumeTarget::FlowRun {
            callback_task_id, ..
        }
        | PublishedCallbackResumeTarget::CallbackTask { callback_task_id } => *callback_task_id,
    }
}

pub(crate) fn openai_chat_completion_id_from_run_id(run_id: uuid::Uuid) -> String {
    format!("chatcmpl-{run_id}")
}

pub(crate) fn openai_chat_completion_id_from_callback_task(
    run_id: uuid::Uuid,
    callback_task_id: uuid::Uuid,
) -> String {
    format!("chatcmpl-{run_id}-{callback_task_id}")
}
