use std::{convert::Infallible, sync::Arc, time::Duration};

#[cfg(test)]
use control_plane::ports::OrchestrationRuntimeRepository;
#[cfg(test)]
use std::collections::HashSet;

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
    ports::{RuntimeEventEnvelope, RuntimeEventPayload},
};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tracing::warn;
#[cfg(test)]
use tracing::{debug, info};

use crate::routes::application_public_api::tool_callback_ids::{
    encode_anthropic_callback_tool_use_id, encode_openai_callback_tool_call_id,
};
use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    routes::application_public_api::{
        compatibility_interface::CompatibilityExecutionDependencies,
        native::{self, service_error, NativeApiError},
        stream_terminal_fallback::{
            durable_canonical_partial_runtime_events_from_native_run,
            durable_native_run_matches_terminal,
            load_durable_native_run_for_terminal_projection_with_dependencies,
            recover_missing_stream_terminal_winner_with_dependencies,
            terminal_runtime_event_from_native_run, NativeRunTerminalDependencies,
        },
    },
};

mod event_forwarding;
mod protocol_mappers;
#[cfg(test)]
mod tests;

use event_forwarding::{
    append_compatible_resume_terminal_event, is_answer_presentation_delta,
    send_subscribed_compatible_typed_event_stream, SubscribedCompatibleTypedEventStream,
};
#[cfg(test)]
use event_forwarding::{send_compatible_runtime_event_stream, take_ordered_compatible_event};
#[cfg(test)]
use protocol_mappers::anthropic_completed_run_to_sse;
use protocol_mappers::{AnthropicStreamMapper, OpenAiChatStreamMapper, OpenAiResponseStreamMapper};

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
    ResumeForActor {
        command: ResumePublishedCallbackCommand,
        actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    },
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
    execution: tokio::task::JoinHandle<()>,
}

impl CompatibleTurnAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::ResumeForActor { .. } => "resume",
        }
    }

    fn resumed_callback_task_id(&self) -> Option<uuid::Uuid> {
        match self {
            Self::Start => None,
            Self::ResumeForActor { command, .. } => {
                Some(callback_task_id_from_resume_command(command))
            }
        }
    }
}

pub(crate) enum CompatibleProtocolProjection {
    OpenAiChat(OpenAiChatStreamMapper),
    OpenAiChatDeferred {
        model: String,
        mapper: Option<OpenAiChatStreamMapper>,
    },
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
    pub(crate) fn runtime_event_to_sse(
        &mut self,
        run: &NativeRunResult,
        envelope: RuntimeEventEnvelope,
    ) -> Vec<Result<Event, Infallible>> {
        let event = CompatibleRuntimeEventView::from(envelope);
        match self {
            Self::OpenAiChat(mapper) => mapper.runtime_event_to_sse(run, event),
            Self::OpenAiChatDeferred { model, mapper } => mapper
                .get_or_insert_with(|| {
                    OpenAiChatStreamMapper::new(
                        model.clone(),
                        openai_chat_completion_id_from_run_id(run.id),
                    )
                })
                .runtime_event_to_sse(run, event),
            Self::OpenAiResponses(mapper) => mapper.runtime_event_to_sse(run, event),
            Self::AnthropicMessages(mapper) => mapper.runtime_event_to_sse(run, event),
        }
    }
}

pub(crate) fn openai_chat_interface_projection(model: String) -> CompatibleProtocolProjection {
    CompatibleProtocolProjection::OpenAiChatDeferred {
        model,
        mapper: None,
    }
}

pub(crate) fn openai_chat_resume_interface_projection(
    model: String,
    completion_id: String,
) -> CompatibleProtocolProjection {
    CompatibleProtocolProjection::OpenAiChat(OpenAiChatStreamMapper::new(model, completion_id))
}

pub(crate) fn openai_responses_interface_projection(
    model: String,
    previous_response_id: Option<String>,
) -> CompatibleProtocolProjection {
    CompatibleProtocolProjection::OpenAiResponses(OpenAiResponseStreamMapper::new(
        model,
        previous_response_id,
    ))
}

pub(crate) fn anthropic_interface_projection(model: String) -> CompatibleProtocolProjection {
    CompatibleProtocolProjection::AnthropicMessages(AnthropicStreamMapper::new(model))
}

pub(crate) async fn prepare_compatible_resume_for_actor(
    state: Arc<ApiState>,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    command: ResumePublishedCallbackCommand,
) -> Result<CompatibleResumeAdmission, NativeApiError> {
    let mcp_runtime_invoker = native::public_mcp_runtime_invoker_for_actor(&state, &actor).await?;
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
    .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue())
    .with_runtime_event_stream(state.runtime_event_stream.clone());
    let prepared =
        ApplicationPublishedCallbackResumeService::new(state.store.clone(), runtime_service)
            .with_last_used_cache(state.infrastructure.cache_store())
            .prepare_callback_resume_for_actor(actor, &command)
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

pub(crate) async fn execute_compatible_resume_for_actor(
    dependencies: CompatibilityExecutionDependencies,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    command: ResumePublishedCallbackCommand,
) -> Result<NativeRunResult, NativeApiError> {
    let runtime_internal_tool_invoker = dependencies
        .native
        .runtime_invoker_factory
        .for_actor(&actor)
        .await?;
    let runtime_service =
        native::native_runtime_service(&dependencies.native, runtime_internal_tool_invoker)
            .with_runtime_event_stream(dependencies.native.runtime_event_stream.clone());
    ApplicationPublishedCallbackResumeService::new(
        dependencies.native.store.clone(),
        runtime_service,
    )
    .with_last_used_cache(dependencies.native.cache_store.clone())
    .resume_callback_for_actor(actor, command)
    .await
    .map(|result| result.run)
    .map_err(service_error)
}

#[cfg(test)]
#[derive(Debug, Default)]
struct CompatibleStreamStats {
    emitted_public_event: bool,
    emitted_content_bytes: usize,
    forwarded_event_identities: HashSet<CompatiblePublicEventIdentity>,
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn presentation_value_identity(presentation: Option<&Value>, key: &str) -> Option<String> {
    presentation
        .and_then(|value| value.get(key))
        .map(|value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
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

pub(crate) async fn start_compatible_typed_start_stream_for_actor(
    dependencies: CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let mcp_runtime_invoker = dependencies
        .native
        .runtime_invoker_factory
        .for_actor(&actor)
        .await?;
    let opened = open_compatible_turn_with_invoker(
        dependencies.clone(),
        initial_run,
        CompatibleTurnAction::Start,
        provider_transport_slot,
        mcp_runtime_invoker,
    )
    .await?;
    opened_compatible_typed_stream(dependencies, opened)
}

pub(crate) async fn start_compatible_typed_resume_stream_for_actor(
    dependencies: CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    command: ResumePublishedCallbackCommand,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let mcp_runtime_invoker = dependencies
        .native
        .runtime_invoker_factory
        .for_actor(&actor)
        .await?;
    let opened = open_compatible_turn_with_invoker(
        dependencies.clone(),
        initial_run,
        CompatibleTurnAction::ResumeForActor { command, actor },
        None,
        mcp_runtime_invoker,
    )
    .await?;
    opened_compatible_typed_stream(dependencies, opened)
}

fn opened_compatible_typed_stream(
    dependencies: CompatibilityExecutionDependencies,
    opened: OpenedCompatibleTurn,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let OpenedCompatibleTurn {
        initial_run,
        from_sequence,
        ignored_waiting_callback_task_id,
        subscription,
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
            terminal_dependencies: native::native_run_terminal_dependencies(&dependencies.native),
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

pub(crate) async fn start_compatible_typed_attach_stream(
    state: Arc<ApiState>,
    initial_run: NativeRunResult,
    from_sequence: Option<i64>,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let subscription = state
        .runtime_event_stream
        .subscribe(initial_run.id, from_sequence)
        .await
        .map_err(service_error)?;
    let (sender, events) = mpsc::channel(32);
    tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            terminal_dependencies: NativeRunTerminalDependencies::new(
                state.store.clone(),
                state.runtime_engine.clone(),
                state.provider_runtime.clone(),
                state.provider_secret_master_key.clone(),
                state.runtime_event_stream.clone(),
            ),
            initial_run: initial_run.clone(),
            from_sequence,
            ignored_waiting_callback_task_id: None,
            subscription,
            sender,
        },
    ));
    Ok(CompatibleTypedTurnStream {
        initial_run,
        events,
    })
}

async fn open_compatible_turn_with_invoker(
    dependencies: CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    action: CompatibleTurnAction,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
    mcp_runtime_invoker: Arc<
        dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
    >,
) -> Result<OpenedCompatibleTurn, NativeApiError> {
    let turn_action = action.name();
    if let Err(error) = dependencies
        .native
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
        let resume_started = dependencies
            .native
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
    let subscription = dependencies
        .native
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

    let background_dependencies = dependencies.clone();
    let background_run = initial_run.clone();
    let execution = tokio::spawn(async move {
        let runtime_service =
            native::native_runtime_service(&background_dependencies.native, mcp_runtime_invoker)
                .with_runtime_event_stream(
                    background_dependencies.native.runtime_event_stream.clone(),
                );
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
                        recover_missing_stream_terminal_winner_with_dependencies(
                            &native::native_run_terminal_dependencies(
                                &background_dependencies.native,
                            ),
                            &background_run,
                        )
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
            CompatibleTurnAction::ResumeForActor { command, actor } => {
                match ApplicationPublishedCallbackResumeService::new(
                    background_dependencies.native.store.clone(),
                    runtime_service,
                )
                .with_last_used_cache(background_dependencies.native.cache_store.clone())
                .resume_callback_for_actor(actor, command)
                .await
                {
                    Ok(result) => {
                        append_compatible_resume_terminal_event(
                            &background_dependencies.native.runtime_event_stream,
                            &result.run,
                        )
                        .await
                    }
                    Err(error) => {
                        warn!(
                            flow_run_id = %background_run.id,
                            error = %error,
                            "compatible callback resume failed"
                        );
                        let _ = background_dependencies
                            .native
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
#[cfg(test)]
const OPENAI_CHAT_SSE_PROJECTION: &str = "openai_chat";
#[cfg(test)]
const ANTHROPIC_SSE_PROJECTION: &str = "anthropic";
