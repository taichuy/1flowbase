use super::openai::native_response_id;
use std::{convert::Infallible, sync::Arc, time::Duration};

use control_plane::ports::OrchestrationRuntimeRepository;
#[cfg(test)]
use std::collections::HashSet;

use axum::response::{
    sse::{Event, KeepAlive, Sse},
    IntoResponse, Response,
};
use control_plane::application_public_api::{
    callback_resume::{
        ApplicationPublishedCallbackAttemptRepository, ApplicationPublishedCallbackResumeService,
        PreparedPublishedCallbackResume, PublishedCallbackResumeSource,
        PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
    },
    native::NativeRunStatus,
};
use control_plane::{
    application_public_api::native::{NativeRunResult, NativeUsage},
    orchestration_runtime::{
        debug_stream_events, OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{RuntimeEventDeliveryClaim, RuntimeEventEnvelope, RuntimeEventPayload},
};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::routes::application_public_api::delivery_receipt::RuntimeEventDeliveryReceipt;
use tracing::warn;
#[cfg(test)]
use tracing::{debug, info};

use crate::routes::application_public_api::tool_callback_ids::{
    encode_anthropic_callback_tool_use_id, encode_openai_callback_tool_call_id,
};
use crate::{
    app_state::ApiState,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResponsesProjectionMode {
    TransparentProviderResponses,
    SemanticNativeResponses,
}

impl ResponsesProjectionMode {
    pub(crate) fn from_native_transport(preserved: bool) -> Self {
        if preserved {
            Self::TransparentProviderResponses
        } else {
            Self::SemanticNativeResponses
        }
    }
}

pub(crate) struct CompatibleResumePlan {
    pub(crate) initial_run: NativeRunResult,
    pub(crate) command: ResumePublishedCallbackCommand,
}

pub(crate) enum CompatibleResumeAdmission {
    Resume(Box<CompatibleResumePlan>),
    StartNewTurnFromHistory {
        recovery: Option<
            Box<control_plane::application_public_api::callback_resume::NativeInferenceRecoveryGrant>,
        >,
    },
}

#[expect(
    clippy::large_enum_variant,
    reason = "the compatibility turn command is consumed immediately and preserves typed actor ownership"
)]
enum CompatibleTurnAction {
    Start {
        transport_connection_scope: Option<String>,
        observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    },
    ResumeForActor {
        command: ResumePublishedCallbackCommand,
        actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    },
}

/// One cursor-ordered runtime fact. Payload identity never participates in
/// admission; a terminal fact carries the latest durable run snapshot.
#[derive(Debug)]
pub(crate) struct CompatibleProjectionInput {
    run_snapshot: Arc<NativeRunResult>,
    envelope: RuntimeEventEnvelope,
    /// Present only for a committed tool delivery; the protocol writer settles it.
    delivery: Option<RuntimeEventDeliveryReceipt>,
}

pub(crate) struct CompatibleTypedTurnStream {
    initial_run: NativeRunResult,
    events: mpsc::Receiver<CompatibleProjectionInput>,
}

impl CompatibleProjectionInput {
    pub(crate) fn into_parts(
        self,
    ) -> (
        Arc<NativeRunResult>,
        RuntimeEventEnvelope,
        Option<RuntimeEventDeliveryReceipt>,
    ) {
        (self.run_snapshot, self.envelope, self.delivery)
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
    durable_round_replay: Vec<RuntimeEventEnvelope>,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    subscription: control_plane::ports::RuntimeEventSubscription,
    execution: tokio::task::JoinHandle<()>,
}

impl CompatibleTurnAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Start { .. } => "start",
            Self::ResumeForActor { .. } => "resume",
        }
    }

    fn resumed_callback_task_id(&self) -> Option<uuid::Uuid> {
        match self {
            Self::Start { .. } => None,
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

#[cfg(test)]
pub(crate) fn openai_responses_interface_projection(
    model: String,
    previous_response_id: Option<String>,
) -> CompatibleProtocolProjection {
    CompatibleProtocolProjection::OpenAiResponses(OpenAiResponseStreamMapper::new(
        model,
        previous_response_id,
    ))
}

pub(crate) fn openai_responses_interface_projection_with_mode(
    model: String,
    previous_response_id: Option<String>,
    mode: ResponsesProjectionMode,
) -> CompatibleProtocolProjection {
    CompatibleProtocolProjection::OpenAiResponses(OpenAiResponseStreamMapper::with_mode(
        model,
        previous_response_id,
        mode,
    ))
}

pub(crate) fn anthropic_interface_projection(model: String) -> CompatibleProtocolProjection {
    CompatibleProtocolProjection::AnthropicMessages(AnthropicStreamMapper::new(model))
}

pub(crate) async fn prepare_compatible_resume_for_actor(
    state: Arc<ApiState>,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    mut command: ResumePublishedCallbackCommand,
) -> Result<CompatibleResumeAdmission, NativeApiError> {
    let mcp_runtime_invoker = native::public_mcp_runtime_invoker_for_actor(&state, &actor).await?;
    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        native::api_provider_runtime(&state),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
        state.infrastructure.provider_transport_store(),
        state.model_billing_require_provider_usage,
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
    let service =
        ApplicationPublishedCallbackResumeService::new(state.store.clone(), runtime_service)
            .with_last_used_cache(state.infrastructure.cache_store());
    let mut retried_reservation_race = false;
    loop {
        let prepared = service
            .prepare_callback_resume_for_actor(actor.clone(), &command)
            .await
            .map_err(service_error)?;
        return Ok(match prepared {
            PreparedPublishedCallbackResume::Resume { initial_run } => {
                if command.native_transport.is_some() || command.responses_continuation.is_some() {
                    match service
                        .reserve_native_callback_for_actor(actor.clone(), &command)
                        .await
                    {
                        Ok(attempt_id) => command.reserved_attempt_id = Some(attempt_id),
                        Err(error)
                            if !retried_reservation_race
                                && command.source
                                    == PublishedCallbackResumeSource::OpenAiResponses
                                && command.native_transport.is_some()
                                && error
                                    .downcast_ref::<control_plane::errors::ControlPlaneError>()
                                    .is_some_and(|error| {
                                        matches!(
                                            error,
                                            control_plane::errors::ControlPlaneError::Conflict(
                                                "callback_resume_already_admitted"
                                            )
                                        )
                                    }) =>
                        {
                            // Another delivery won the durable reservation after our read.
                            // Re-read its terminal before deciding replay versus a new turn.
                            retried_reservation_race = true;
                            continue;
                        }
                        Err(error) => return Err(service_error(error)),
                    }
                }
                compatible_resume_plan(initial_run, command, false)
            }
            PreparedPublishedCallbackResume::Attach { initial_run } => {
                compatible_resume_plan(initial_run, command, true)
            }
            PreparedPublishedCallbackResume::StartNewTurnFromHistory => {
                CompatibleResumeAdmission::StartNewTurnFromHistory { recovery: None }
            }
            PreparedPublishedCallbackResume::RecoverInference { grant } => {
                CompatibleResumeAdmission::StartNewTurnFromHistory {
                    recovery: Some(grant),
                }
            }
        });
    }
}

fn compatible_resume_plan(
    mut initial_run: Box<NativeRunResult>,
    command: ResumePublishedCallbackCommand,
    attach_active: bool,
) -> CompatibleResumeAdmission {
    if let Some(metadata) = initial_run.metadata.as_object_mut() {
        metadata.remove("responses_round");
    }
    let round_id = callback_task_id_from_resume_command(&command);
    if initial_run.metadata["native_inference_recovery_replay"] != true {
        initial_run.metadata["response_round_id"] = json!(round_id);
    }
    if attach_active {
        initial_run.metadata["active_callback_attach"] = json!(true);
    }
    CompatibleResumeAdmission::Resume(Box::new(CompatibleResumePlan {
        initial_run: *initial_run,
        command,
    }))
}

pub(crate) async fn execute_compatible_resume_for_actor(
    dependencies: CompatibilityExecutionDependencies,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    command: ResumePublishedCallbackCommand,
) -> Result<NativeRunResult, NativeApiError> {
    let round_id = Some(callback_task_id_from_resume_command(&command));
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
    .map(|mut result| {
        if let Some(round_id) = round_id {
            if result.run.metadata["native_inference_recovery_replay"] != true {
                result.run.metadata["response_round_id"] = json!(round_id);
            }
        }
        result.run
    })
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
    transport_connection_scope: Option<String>,
    observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let mcp_runtime_invoker =
        native::runtime_internal_tool_invoker_for_run(&dependencies.native, &actor, &initial_run)
            .await?;
    let opened = open_compatible_turn_with_invoker(
        dependencies.clone(),
        initial_run,
        CompatibleTurnAction::Start {
            transport_connection_scope,
            observation_context,
        },
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
    if initial_run.metadata["active_callback_attach"] == true {
        let round_id = callback_task_id_from_resume_command(&command);
        if let Some(start) =
            live_compatible_round_start(&dependencies, &initial_run, round_id).await
        {
            return attach_compatible_typed_stream_with_replay(
                native::native_run_terminal_dependencies(&dependencies.native),
                dependencies.native.runtime_event_stream.clone(),
                initial_run,
                Some(start),
                Vec::new(),
            )
            .await;
        }
        return follow_missing_compatible_round(dependencies, initial_run, command, actor).await;
    }
    if initial_run.metadata["native_inference_recovery_replay"] == true {
        return attach_compatible_typed_stream(
            native::native_run_terminal_dependencies(&dependencies.native),
            dependencies.native.runtime_event_stream.clone(),
            initial_run,
            None,
        )
        .await;
    }
    let mcp_runtime_invoker =
        native::runtime_internal_tool_invoker_for_run(&dependencies.native, &actor, &initial_run)
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

async fn attach_terminal_compatible_round(
    dependencies: &CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    durable_round_replay: Vec<RuntimeEventEnvelope>,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let terminal = durable_round_replay
        .last()
        .expect("terminal replay is nonempty");
    let close_reason = control_plane::ports::RuntimeEventCloseReason::from_terminal_event_type(
        &terminal.event_type,
    )
    .expect("terminal replay ends in a public terminal");
    let (live_sender, live_events) = mpsc::unbounded_channel();
    drop(live_sender);
    let (_closure_sender, closure) =
        tokio::sync::watch::channel(Some(control_plane::ports::RuntimeEventClosure {
            reason: close_reason,
            final_sequence: i64::MAX,
        }));
    let subscription = control_plane::ports::RuntimeEventSubscription {
        terminal_writer: Arc::new(ReplayOnlyTerminalWriter),
        replay: Vec::new(),
        live_events: control_plane::ports::RuntimeEventReceiver::from_unbounded(live_events),
        closure,
    };
    Ok(attach_compatible_typed_stream_from_subscription(
        native::native_run_terminal_dependencies(&dependencies.native),
        initial_run,
        Some(i64::MAX),
        durable_round_replay,
        subscription,
    ))
}

struct ReplayOnlyTerminalWriter;

#[async_trait::async_trait]
impl control_plane::ports::RuntimeEventTerminalWriter for ReplayOnlyTerminalWriter {
    async fn append_terminal_if_missing_and_close(
        &self,
        _event: RuntimeEventPayload,
    ) -> anyhow::Result<control_plane::ports::AppendTerminalIfMissingAndCloseOutcome> {
        anyhow::bail!("durable terminal replay is read-only")
    }
}

async fn follow_missing_compatible_round(
    dependencies: CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    command: ResumePublishedCallbackCommand,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let (sender, events) = mpsc::channel(32);
    let result = CompatibleTypedTurnStream {
        initial_run: initial_run.clone(),
        events,
    };
    tokio::spawn(async move {
        if let Err(error) = follow_missing_compatible_round_events(
            dependencies,
            initial_run,
            command,
            actor,
            sender,
        )
        .await
        {
            warn!(error = ?error, "durable callback round follower failed");
        }
    });
    Ok(result)
}

async fn follow_missing_compatible_round_events(
    dependencies: CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    mut command: ResumePublishedCallbackCommand,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    sender: mpsc::Sender<CompatibleProjectionInput>,
) -> Result<(), NativeApiError> {
    let round_id = callback_task_id_from_resume_command(&command);
    let mut cursor = dependencies
        .native
        .store
        .get_runtime_event_sequence_for_callback_task(initial_run.id, round_id)
        .await
        .map_err(service_error)?
        .ok_or_else(|| service_error(anyhow::anyhow!("callback round boundary missing")))?;
    loop {
        if sender.is_closed() {
            return Ok(());
        }
        let attached = if let Some(start) =
            live_compatible_round_start(&dependencies, &initial_run, round_id).await
        {
            Some(
                attach_compatible_typed_stream_with_replay(
                    native::native_run_terminal_dependencies(&dependencies.native),
                    dependencies.native.runtime_event_stream.clone(),
                    initial_run.clone(),
                    Some(start),
                    Vec::new(),
                )
                .await?,
            )
        } else {
            None
        };
        if let Some(mut attached) = attached {
            while let Some(event) = attached.events.recv().await {
                if sender.send(event).await.is_err() {
                    return Ok(());
                }
            }
            return Ok(());
        }
        // The cursor detects durable completion without retaining the round's
        // history in memory. Replay is loaded once only when the round settles.
        let records = dependencies
            .native
            .store
            .list_runtime_events(initial_run.id, cursor)
            .await
            .map_err(service_error)?;
        if let Some(last) = records.last() {
            cursor = last.sequence;
        }
        if records
            .iter()
            .any(|record| event_forwarding::is_public_terminal_runtime_event(&record.event_type))
        {
            let durable_round_replay =
                durable_compatible_round_replay(&dependencies, &initial_run, round_id).await?;
            let mut attached = attach_terminal_compatible_round(
                &dependencies,
                initial_run.clone(),
                durable_round_replay,
            )
            .await?;
            while let Some(event) = attached.events.recv().await {
                if sender.send(event).await.is_err() {
                    return Ok(());
                }
            }
            return Ok(());
        }
        let attempt = dependencies
            .native
            .store
            .get_published_callback_resume_attempt(round_id)
            .await
            .map_err(service_error)?;
        if let Some(attempt) = &attempt {
            if attempt.response_payload != command.response_payload
                || attempt.source != command.source.as_str()
            {
                return Err(service_error(anyhow::anyhow!(
                    "callback resume attempt changed while following"
                )));
            }
        }
        if let Some(attempt) = attempt.filter(|attempt| {
            attempt.status == domain::FlowRunCallbackResumeAttemptStatus::Processing
                && attempt.updated_at + time::Duration::minutes(5)
                    <= time::OffsetDateTime::now_utc()
        }) {
            let invoker = native::runtime_internal_tool_invoker_for_run(
                &dependencies.native,
                &actor,
                &initial_run,
            )
            .await?;
            let runtime_service =
                native::native_runtime_service(&dependencies.native, invoker.clone())
                    .with_runtime_event_stream(dependencies.native.runtime_event_stream.clone());
            let service = ApplicationPublishedCallbackResumeService::new(
                dependencies.native.store.clone(),
                runtime_service,
            )
            .with_last_used_cache(dependencies.native.cache_store.clone());
            match service
                .reserve_native_callback_for_actor(actor.clone(), &command)
                .await
            {
                Ok(reclaimed_id) if reclaimed_id == attempt.id => {
                    let current = dependencies
                        .native
                        .store
                        .get_published_callback_resume_attempt(round_id)
                        .await
                        .map_err(service_error)?;
                    if !current.is_some_and(|current| {
                        current.id == reclaimed_id
                            && current.status
                                == domain::FlowRunCallbackResumeAttemptStatus::Processing
                    }) {
                        continue;
                    }
                    command.reserved_attempt_id = Some(reclaimed_id);
                    let mut recovered_run = initial_run.clone();
                    recovered_run.metadata["active_callback_attach"] = json!(false);
                    let opened = open_compatible_turn_with_invoker(
                        dependencies.clone(),
                        recovered_run,
                        CompatibleTurnAction::ResumeForActor { command, actor },
                        None,
                        invoker,
                    )
                    .await?;
                    let mut recovered = opened_compatible_typed_stream(dependencies, opened)?;
                    while let Some(event) = recovered.events.recv().await {
                        if sender.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                    return Ok(());
                }
                Ok(_) => {
                    return Err(service_error(anyhow::anyhow!(
                        "callback resume attempt changed while reclaiming"
                    )));
                }
                Err(error)
                    if error
                        .downcast_ref::<control_plane::errors::ControlPlaneError>()
                        .is_some_and(|error| {
                            matches!(
                                error,
                                control_plane::errors::ControlPlaneError::Conflict(
                                    "callback_resume_already_admitted"
                                )
                            )
                        }) => {}
                Err(error) => return Err(service_error(error)),
            }
        }
        tokio::select! {
            _ = sender.closed() => return Ok(()),
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
    }
}

fn opened_compatible_typed_stream(
    dependencies: CompatibilityExecutionDependencies,
    opened: OpenedCompatibleTurn,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let OpenedCompatibleTurn {
        initial_run,
        from_sequence,
        durable_round_replay,
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
            durable_round_replay,
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
    attach_compatible_typed_stream(
        NativeRunTerminalDependencies::new(
            state.store.clone(),
            state.runtime_engine.clone(),
            state.provider_runtime.clone(),
            state.provider_secret_master_key.clone(),
            state.model_billing_require_provider_usage,
            state.infrastructure.provider_transport_store(),
            state.runtime_event_stream.clone(),
        ),
        state.runtime_event_stream.clone(),
        initial_run,
        from_sequence,
    )
    .await
}

async fn attach_compatible_typed_stream(
    terminal_dependencies: NativeRunTerminalDependencies,
    runtime_event_stream: Arc<dyn control_plane::ports::RuntimeEventStream>,
    initial_run: NativeRunResult,
    from_sequence: Option<i64>,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    attach_compatible_typed_stream_with_replay(
        terminal_dependencies,
        runtime_event_stream,
        initial_run,
        from_sequence,
        Vec::new(),
    )
    .await
}

async fn attach_compatible_typed_stream_with_replay(
    terminal_dependencies: NativeRunTerminalDependencies,
    runtime_event_stream: Arc<dyn control_plane::ports::RuntimeEventStream>,
    initial_run: NativeRunResult,
    from_sequence: Option<i64>,
    durable_round_replay: Vec<RuntimeEventEnvelope>,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let subscription = runtime_event_stream
        .subscribe(initial_run.id, from_sequence)
        .await
        .map_err(service_error)?;
    Ok(attach_compatible_typed_stream_from_subscription(
        terminal_dependencies,
        initial_run,
        from_sequence,
        durable_round_replay,
        subscription,
    ))
}

fn attach_compatible_typed_stream_from_subscription(
    terminal_dependencies: NativeRunTerminalDependencies,
    initial_run: NativeRunResult,
    from_sequence: Option<i64>,
    durable_round_replay: Vec<RuntimeEventEnvelope>,
    subscription: control_plane::ports::RuntimeEventSubscription,
) -> CompatibleTypedTurnStream {
    let (sender, events) = mpsc::channel(32);
    tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            terminal_dependencies,
            initial_run: initial_run.clone(),
            from_sequence,
            durable_round_replay,
            ignored_waiting_callback_task_id: None,
            subscription,
            sender,
        },
    ));
    CompatibleTypedTurnStream {
        initial_run,
        events,
    }
}

async fn compatible_round_replay(
    dependencies: &CompatibilityExecutionDependencies,
    initial_run: &NativeRunResult,
    round_id: uuid::Uuid,
) -> Result<(Option<i64>, Vec<RuntimeEventEnvelope>), NativeApiError> {
    let live_round_start = live_compatible_round_start(dependencies, initial_run, round_id).await;
    if live_round_start.is_some() {
        return Ok((live_round_start, Vec::new()));
    }
    Ok((
        None,
        durable_compatible_round_replay(dependencies, initial_run, round_id).await?,
    ))
}

async fn durable_compatible_round_replay(
    dependencies: &CompatibilityExecutionDependencies,
    initial_run: &NativeRunResult,
    round_id: uuid::Uuid,
) -> Result<Vec<RuntimeEventEnvelope>, NativeApiError> {
    let boundary = dependencies
        .native
        .store
        .get_runtime_event_sequence_for_callback_task(initial_run.id, round_id)
        .await
        .map_err(service_error)?
        .ok_or_else(|| service_error(anyhow::anyhow!("callback round boundary missing")))?;
    let records = dependencies
        .native
        .store
        .list_runtime_events(initial_run.id, boundary)
        .await
        .map_err(service_error)?;
    let mut replay = event_forwarding::durable_round_prefix(records);
    if !replay.is_empty() && replay[0].event_type != "flow_started" {
        replay.insert(
            0,
            RuntimeEventEnvelope::new(
                initial_run.id,
                0,
                debug_stream_events::flow_started(initial_run.id),
            ),
        );
    }
    Ok(replay)
}

async fn live_compatible_round_start(
    dependencies: &CompatibilityExecutionDependencies,
    initial_run: &NativeRunResult,
    round_id: uuid::Uuid,
) -> Option<i64> {
    dependencies
        .native
        .runtime_event_stream
        .replay(initial_run.id, None, usize::MAX)
        .await
        .ok()
        .and_then(|events| {
            events
                .iter()
                .rposition(|event| {
                    event.event_type == "flow_started"
                        && event.payload["response_round_id"] == json!(round_id)
                })
                .filter(|index| {
                    !events[*index + 1..].iter().any(|event| {
                        event_forwarding::is_public_terminal_runtime_event(&event.event_type)
                    })
                })
                .map(|index| events[index].sequence.saturating_sub(1))
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
    let ignored_waiting_callback_task_id = action.resumed_callback_task_id();
    let (live_round_start, durable_round_replay) =
        if let Some(round_id) = ignored_waiting_callback_task_id {
            compatible_round_replay(&dependencies, &initial_run, round_id).await?
        } else {
            (None, Vec::new())
        };
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

    let from_sequence = if let Some(sequence) = live_round_start {
        Some(sequence)
    } else if let Some(round_id) = ignored_waiting_callback_task_id {
        if durable_round_replay.is_empty() {
            let mut started = debug_stream_events::flow_started(initial_run.id);
            started.payload["response_round_id"] = json!(round_id);
            let started = dependencies
                .native
                .runtime_event_stream
                .append(initial_run.id, started)
                .await
                .map_err(service_error)?;
            Some(started.sequence.saturating_sub(1))
        } else {
            None
        }
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

    let terminal_writer = subscription.terminal_writer.clone();
    let background_dependencies = dependencies.clone();
    let background_run = initial_run.clone();
    let execution = tokio::spawn(async move {
        let runtime_service =
            native::native_runtime_service(&background_dependencies.native, mcp_runtime_invoker)
                .with_runtime_event_stream(
                    background_dependencies.native.runtime_event_stream.clone(),
                );
        match action {
            CompatibleTurnAction::Start {
                transport_connection_scope,
                observation_context,
            } => {
                if let Err(runtime_error) = runtime_service
                    .start_published_flow_run(StartPublishedFlowRunCommand {
                        application_id: background_run.application_id,
                        flow_run_id: background_run.id,
                        provider_transport_slot,
                        transport_connection_scope,
                        observation_context,
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
                            terminal_writer.as_ref(),
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
                        let _ = terminal_writer
                            .append_terminal_if_missing_and_close(debug_stream_events::flow_failed(
                                background_run.id,
                                json!({ "message": error.to_string() }),
                            ))
                            .await;
                    }
                }
            }
        }
    });

    Ok(OpenedCompatibleTurn {
        initial_run,
        from_sequence,
        durable_round_replay,
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
