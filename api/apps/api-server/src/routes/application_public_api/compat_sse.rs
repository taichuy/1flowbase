use std::{collections::HashSet, convert::Infallible, sync::Arc, time::Duration};

use axum::response::{
    sse::{Event, KeepAlive, Sse},
    IntoResponse, Response,
};
use control_plane::application_public_api::{
    callback_resume::{
        ApplicationPublishedCallbackResumeService, PreparedPublishedCallbackResume,
        PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
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

use crate::routes::application_public_api::tool_callback_ids::{
    encode_anthropic_callback_tool_use_id, encode_openai_callback_tool_call_id,
};
use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    routes::application_public_api::{
        native::{service_error, NativeApiError},
        stream_terminal_fallback::{
            durable_canonical_partial_runtime_events_from_native_run,
            durable_native_run_matches_terminal, load_durable_native_run_for_terminal_projection,
            recover_missing_stream_terminal_winner, terminal_runtime_event_from_native_run,
        },
    },
};

mod event_forwarding;
mod protocol_mappers;
#[cfg(test)]
mod tests;

use event_forwarding::{
    append_compatible_resume_terminal_event, is_answer_presentation_delta,
    send_subscribed_compatible_runtime_event_stream, send_subscribed_compatible_typed_event_stream,
    SubscribedCompatibleRuntimeEventStream, SubscribedCompatibleTypedEventStream,
};
#[cfg(test)]
use event_forwarding::{send_compatible_runtime_event_stream, take_ordered_compatible_event};
#[cfg(test)]
use protocol_mappers::anthropic_completed_run_to_sse;
use protocol_mappers::{AnthropicStreamMapper, OpenAiChatStreamMapper, OpenAiResponseStreamMapper};

type CompatRunSseStream = tokio_stream::wrappers::ReceiverStream<Result<Event, Infallible>>;

const OPENAI_CHAT_SSE_PROJECTION: &str = "openai_chat";
const OPENAI_RESPONSES_SSE_PROJECTION: &str = "openai_responses";
const ANTHROPIC_SSE_PROJECTION: &str = "anthropic";

pub(crate) struct CompatibleResumePlan {
    pub(crate) initial_run: NativeRunResult,
    pub(crate) command: ResumePublishedCallbackCommand,
}

pub(crate) enum CompatibleResumeAdmission {
    Resume(Box<CompatibleResumePlan>),
    StartNewTurnFromHistory,
}

enum CompatibleTurnAction {
    Start,
    Resume(ResumePublishedCallbackCommand),
}

pub(crate) struct PreparedCompatibleTurn {
    initial_run: NativeRunResult,
    action: CompatibleTurnAction,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
}

impl PreparedCompatibleTurn {
    pub(crate) fn start(
        initial_run: NativeRunResult,
        provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
    ) -> Self {
        Self {
            initial_run,
            action: CompatibleTurnAction::Start,
            provider_transport_slot,
        }
    }

    pub(crate) fn resume(
        initial_run: NativeRunResult,
        command: ResumePublishedCallbackCommand,
    ) -> Self {
        Self {
            initial_run,
            action: CompatibleTurnAction::Resume(command),
            provider_transport_slot: None,
        }
    }
}

/// One cursor-ordered runtime fact. Payload identity never participates in
/// admission; a terminal fact carries the latest durable run snapshot.
#[derive(Debug)]
pub(crate) struct CompatibleProjectionInput {
    run_snapshot: NativeRunResult,
    envelope: RuntimeEventEnvelope,
}

pub(crate) struct CompatibleTypedTurnStream {
    initial_run: NativeRunResult,
    events: mpsc::Receiver<CompatibleProjectionInput>,
}

impl CompatibleProjectionInput {
    pub(crate) fn into_parts(self) -> (NativeRunResult, RuntimeEventEnvelope) {
        (self.run_snapshot, self.envelope)
    }
}

impl CompatibleTypedTurnStream {
    pub(crate) fn into_parts(self) -> (NativeRunResult, mpsc::Receiver<CompatibleProjectionInput>) {
        (self.initial_run, self.events)
    }
}

struct OpenedCompatibleTurn {
    initial_run: NativeRunResult,
    from_sequence: Option<i64>,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    subscription: control_plane::ports::RuntimeEventSubscription,
    turn_action: &'static str,
    execution: tokio::task::JoinHandle<()>,
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
) -> Result<CompatibleResumeAdmission, NativeApiError> {
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
    Ok(match prepared {
        PreparedPublishedCallbackResume::Resume { initial_run } => {
            CompatibleResumeAdmission::Resume(Box::new(CompatibleResumePlan {
                initial_run: *initial_run,
                command,
            }))
        }
        PreparedPublishedCallbackResume::StartNewTurnFromHistory => {
            CompatibleResumeAdmission::StartNewTurnFromHistory
        }
    })
}

#[derive(Debug, Default)]
struct CompatibleStreamStats {
    emitted_public_event: bool,
    emitted_content_bytes: usize,
    forwarded_event_identities: HashSet<CompatiblePublicEventIdentity>,
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

impl CompatibleStreamStats {
    fn emitted_content(&self) -> bool {
        self.emitted_content_bytes > 0
    }

    fn claim_runtime_event(&mut self, event: &RuntimeEventEnvelope) -> bool {
        let Some(identity) = compatible_public_event_identity(event) else {
            return true;
        };
        if is_answer_presentation_delta(event) {
            // Presentation deltas are ordered facts. Equal text is valid and must never
            // participate in identity, deduplication, or durable-prefix reconciliation.
            return true;
        }
        self.forwarded_event_identities.insert(identity)
    }

    fn record_sent_runtime_event(
        &mut self,
        _run: &NativeRunResult,
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
            self.emitted_content_bytes += text.len();
        }
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
    let mapper = OpenAiChatStreamMapper::new(model, openai_chat_completion_id_from_run_id(run.id));
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
    let mapper = OpenAiResponseStreamMapper::new(model, previous_response_id);
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
    let mapper = OpenAiChatStreamMapper::new(model, chat_completion_id);
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
    let mapper = OpenAiResponseStreamMapper::new(model, previous_response_id);
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
    let opened = open_compatible_turn(
        state.clone(),
        PreparedCompatibleTurn {
            initial_run: run,
            action,
            provider_transport_slot,
        },
    )
    .await?;
    let OpenedCompatibleTurn {
        initial_run,
        from_sequence,
        ignored_waiting_callback_task_id,
        subscription,
        turn_action,
        execution,
    } = opened;

    let (sender, receiver) = mpsc::channel(32);
    let execution_sender_guard = sender.clone();
    tokio::spawn(async move {
        let _execution_sender_guard = execution_sender_guard;
        if let Err(error) = execution.await {
            warn!(error = %error, "compatible public API turn task did not exit cleanly");
        }
    });
    tokio::spawn(send_subscribed_compatible_runtime_event_stream(
        SubscribedCompatibleRuntimeEventStream {
            state: state.clone(),
            initial_run: initial_run.clone(),
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

    info!(
        flow_run_id = %initial_run.id,
        application_id = %initial_run.application_id,
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

pub(crate) async fn start_compatible_typed_turn_stream(
    state: Arc<ApiState>,
    prepared: PreparedCompatibleTurn,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let opened = open_compatible_turn(state.clone(), prepared).await?;
    let OpenedCompatibleTurn {
        initial_run,
        from_sequence,
        ignored_waiting_callback_task_id,
        subscription,
        turn_action: _,
        execution,
    } = opened;
    let (sender, events) = mpsc::channel(32);
    let execution_sender_guard = sender.clone();
    tokio::spawn(async move {
        let _execution_sender_guard = execution_sender_guard;
        if let Err(error) = execution.await {
            warn!(error = %error, "compatible typed turn task did not exit cleanly");
        }
    });
    tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            state,
            initial_run: initial_run.clone(),
            from_sequence,
            ignored_waiting_callback_task_id,
            subscription,
            sender,
        },
    ));
    Ok(CompatibleTypedTurnStream {
        initial_run,
        events,
    })
}

async fn open_compatible_turn(
    state: Arc<ApiState>,
    prepared: PreparedCompatibleTurn,
) -> Result<OpenedCompatibleTurn, NativeApiError> {
    let PreparedCompatibleTurn {
        initial_run,
        action,
        provider_transport_slot,
    } = prepared;
    let turn_action = action.name();
    if let Err(error) = state
        .runtime_event_stream
        .open_run(
            initial_run.id,
            control_plane::ports::RuntimeEventStreamPolicy::debug_default(),
        )
        .await
    {
        warn!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            error = %error,
            "failed to open compatible public API runtime event stream"
        );
        return Err(service_error(error));
    }

    let ignored_waiting_callback_task_id = action.resumed_callback_task_id();
    let from_sequence = if ignored_waiting_callback_task_id.is_some() {
        let resume_started = state
            .runtime_event_stream
            .append(
                initial_run.id,
                debug_stream_events::flow_started(initial_run.id),
            )
            .await
            .map_err(service_error)?;
        Some(resume_started.sequence.saturating_sub(1))
    } else {
        None
    };
    let subscription = state
        .runtime_event_stream
        .subscribe(initial_run.id, from_sequence)
        .await
        .map_err(|error| {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                turn_action,
                error = %error,
                "failed to subscribe compatible public API runtime event stream"
            );
            service_error(error)
        })?;

    let background_state = state.clone();
    let background_run = initial_run.clone();
    let execution = tokio::spawn(async move {
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

    Ok(OpenedCompatibleTurn {
        initial_run,
        from_sequence,
        ignored_waiting_callback_task_id,
        subscription,
        turn_action,
        execution,
    })
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
