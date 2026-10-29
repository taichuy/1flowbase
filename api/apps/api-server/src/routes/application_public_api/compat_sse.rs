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
        ApplicationPublishedCallbackResumeService, PreparedPublishedCallbackResume,
        PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    },
    native::NativeRunStatus,
};
use control_plane::{
    application_public_api::native::{NativeRunResult, NativeUsage},
    orchestration_runtime::{
        debug_stream_events, start_runtime_debug_event_persister,
        wait_for_runtime_debug_event_persister, OrchestrationRuntimeService,
        StartPublishedFlowRunCommand,
    },
    ports::{
        RuntimeEventDeliveryClaim, RuntimeEventDeliveryScope, RuntimeEventEnvelope,
        RuntimeEventPayload,
    },
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
            cold_runtime_event_subscription,
            durable_canonical_partial_runtime_events_from_native_run,
            durable_native_run_matches_terminal,
            load_durable_native_run_for_terminal_projection_with_dependencies,
            recover_missing_stream_terminal_winner_with_dependencies,
            terminal_runtime_event_from_native_run, NativeRunTerminalDependencies,
        },
    },
};

mod callback_attachment;
mod callback_execution;
mod callback_reservation;
use callback_attachment::{
    attach_live_compatible_callback_round, callback_round_replay_established,
    follow_missing_compatible_round_events,
};
pub(crate) use callback_execution::execute_compatible_resume_for_actor;
use callback_execution::open_compatible_turn_with_invoker;
pub(crate) use callback_reservation::CompatibleCallbackReservation;
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
    AwaitingHostSelection,
}

impl ResponsesProjectionMode {
    pub(crate) fn from_transport(
        payload: Option<&control_plane::ports::ProviderTransportPayload>,
    ) -> Self {
        match payload {
            None => Self::SemanticNativeResponses,
            Some(payload) if payload.is_preferred() => Self::AwaitingHostSelection,
            Some(_) => Self::TransparentProviderResponses,
        }
    }
}

pub(crate) struct CompatibleResumePlan {
    pub(crate) initial_run: NativeRunResult,
    pub(crate) command: ResumePublishedCallbackCommand,
    pub(crate) reservation: Option<CompatibleCallbackReservation>,
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
        reservation: Option<CompatibleCallbackReservation>,
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

/// Host-selected consumer capability, never inferred from request JSON or scope absence.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompatibleDeliveryPolicy {
    ScopedResponses,
    NativeRunAttachment,
}

/// A Native-only consumer cannot be passed to a Responses stream projector.
/// Its sole production constructor is the authenticated Native WS attach path.
pub(crate) struct NativeRunAttachmentStream {
    events: mpsc::Receiver<CompatibleProjectionInput>,
}

impl NativeRunAttachmentStream {
    pub(crate) async fn next_native(
        &mut self,
    ) -> Option<(Arc<NativeRunResult>, RuntimeEventEnvelope)> {
        self.events
            .recv()
            .await
            .map(|input| (input.run_snapshot, input.envelope))
    }
}

pub(crate) struct CompatibleTypedTurnStream {
    initial_run: NativeRunResult,
    events: mpsc::Receiver<CompatibleProjectionInput>,
}

impl CompatibleProjectionInput {
    /// Ephemeral protocol failure only: never persist this synthetic snapshot or
    /// confuse it with the durable business outcome or a delivery ACK.
    fn protocol_failure(run: &NativeRunResult, code: &'static str, sequence: i64) -> Self {
        let mut snapshot = run.clone();
        snapshot.status = NativeRunStatus::Failed;
        snapshot.error = Some(control_plane::application_public_api::native::NativeError {
            code: code.into(),
            message: "Responses tool delivery ownership could not be established".into(),
            details: json!({}),
        });
        let envelope = RuntimeEventEnvelope::new(
            snapshot.id,
            sequence,
            debug_stream_events::flow_failed(snapshot.id, json!({"code":code})),
        );
        Self {
            run_snapshot: Arc::new(snapshot),
            envelope,
            delivery: None,
        }
    }

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
    delivery_scope: RuntimeEventDeliveryScope,
    from_sequence: Option<i64>,
    durable_round_replay: Vec<RuntimeEventEnvelope>,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    subscription: control_plane::ports::RuntimeEventSubscription,
    execution: tokio::task::JoinHandle<()>,
}

fn selected_delivery_scope(run: &NativeRunResult) -> RuntimeEventDeliveryScope {
    match run.metadata["response_round_id"]
        .as_str()
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
    {
        Some(id) if id == run.id => RuntimeEventDeliveryScope::Start,
        Some(callback_task_id) => RuntimeEventDeliveryScope::Resume { callback_task_id },
        None => RuntimeEventDeliveryScope::Unresolved,
    }
}

fn freeze_delivery_scope(run: &mut NativeRunResult, scope: RuntimeEventDeliveryScope) {
    let id = match scope {
        RuntimeEventDeliveryScope::Start => run.id,
        RuntimeEventDeliveryScope::Resume { callback_task_id } => callback_task_id,
        RuntimeEventDeliveryScope::Unresolved => return,
    };
    run.metadata["response_round_id"] = json!(id);
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
    usage_baseline: super::openai::chat_usage::ChatUsageBaseline,
) -> CompatibleProtocolProjection {
    CompatibleProtocolProjection::OpenAiChat(
        OpenAiChatStreamMapper::new(model, completion_id).with_usage_baseline(usage_baseline),
    )
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
    .with_published_plan_cache(state.infrastructure.published_plan_cache())
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
    .with_file_storage_registry(state.file_storage_registry.clone())
    .with_notification_sender(state.notification_service.clone())
    .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue())
    .with_runtime_event_stream(state.runtime_event_stream.clone());
    let service = Arc::new(
        ApplicationPublishedCallbackResumeService::new(state.store.clone(), runtime_service)
            .with_last_used_cache(state.infrastructure.cache_store()),
    );
    let mut retried_reservation_race = false;
    loop {
        let prepared = service
            .prepare_callback_resume_for_actor(actor.clone(), &command)
            .await
            .map_err(service_error)?;
        return Ok(match prepared {
            PreparedPublishedCallbackResume::Resume { initial_run } => {
                let mut reservation = None;
                if command.native_transport.is_some() || command.responses_continuation.is_some() {
                    match service
                        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
                        .await
                    {
                        Ok(acquired) => {
                            command.reserved_attempt_id = Some(acquired.attempt_id());
                            let mut owned =
                                CompatibleCallbackReservation::new(acquired, Arc::clone(&service));
                            owned
                                .capture_previous_generation(
                                    state.runtime_event_stream.as_ref(),
                                    initial_run.id,
                                )
                                .await;
                            reservation = Some(owned);
                        }
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
                compatible_resume_plan(initial_run, command, false, reservation)
            }
            PreparedPublishedCallbackResume::Attach { initial_run } => {
                compatible_resume_plan(initial_run, command, true, None)
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
    reservation: Option<CompatibleCallbackReservation>,
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
        reservation,
    }))
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
    mut reservation: Option<CompatibleCallbackReservation>,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    if reservation.is_none() && initial_run.metadata["active_callback_attach"] == true {
        let round_id = callback_task_id_from_resume_command(&command);
        if let Some(start) =
            live_compatible_round_start(&dependencies, &initial_run, round_id).await
        {
            return attach_live_compatible_callback_round(
                &dependencies,
                initial_run,
                round_id,
                start,
            )
            .await;
        }
        return follow_missing_compatible_round(dependencies, initial_run, command, actor);
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
    if reservation
        .as_ref()
        .is_some_and(|reservation| !reservation.is_acquired())
    {
        return match durable_compatible_round_replay(
            &dependencies,
            &initial_run,
            callback_task_id_from_resume_command(&command),
        )
        .await?
        {
            DurableCompatibleRoundReplay::Ready(replay)
                if replay.last().is_some_and(|event| {
                    event_forwarding::is_public_terminal_runtime_event(&event.event_type)
                }) =>
            {
                attach_terminal_compatible_round(
                    &dependencies,
                    initial_run,
                    callback_task_id_from_resume_command(&command),
                    replay,
                )
                .await
            }
            _ => Err(service_error(anyhow::anyhow!(
                "settled callback has no attributable durable round terminal"
            ))),
        };
    }
    let mcp_runtime_invoker = match native::runtime_internal_tool_invoker_for_run(
        &dependencies.native,
        &actor,
        &initial_run,
    )
    .await
    {
        Ok(invoker) => invoker,
        Err(error) => {
            if let Some(reservation) = reservation.as_mut() {
                reservation.release().await;
            }
            return Err(error);
        }
    };
    let opened = open_compatible_turn_with_invoker(
        dependencies.clone(),
        initial_run,
        CompatibleTurnAction::ResumeForActor {
            command,
            actor,
            reservation,
        },
        None,
        mcp_runtime_invoker,
    )
    .await?;
    opened_compatible_typed_stream(dependencies, opened)
}

async fn attach_terminal_compatible_round(
    dependencies: &CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    round_id: uuid::Uuid,
    durable_round_replay: Vec<RuntimeEventEnvelope>,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let subscription = terminal_compatible_round_subscription(&durable_round_replay);
    Ok(attach_compatible_typed_stream_from_subscription(
        native::native_run_terminal_dependencies(&dependencies.native),
        initial_run,
        Some(i64::MAX),
        durable_round_replay,
        RuntimeEventDeliveryScope::Resume {
            callback_task_id: round_id,
        },
        subscription,
    ))
}

fn terminal_compatible_round_subscription(
    durable_round_replay: &[RuntimeEventEnvelope],
) -> control_plane::ports::RuntimeEventSubscription {
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
    control_plane::ports::RuntimeEventSubscription {
        terminal_writer: Arc::new(ReplayOnlyTerminalWriter),
        replay: Vec::new(),
        live_events: control_plane::ports::RuntimeEventReceiver::from_unbounded(live_events),
        closure,
    }
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

// Subscription creation is synchronous; the spawned worker owns all awaits.
// Keeping this boundary concrete breaks the resume/follower opaque-future cycle.
fn follow_missing_compatible_round(
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
            initial_run.clone(),
            command,
            actor,
            sender.clone(),
        )
        .await
        {
            warn!(error = ?error, "durable callback round follower failed");
            let code = if error.code == "runtime_event_delivery_scope_unresolved" {
                error.code
            } else {
                "runtime_event_delivery_proof_unavailable"
            };
            let _ = sender
                .send(CompatibleProjectionInput::protocol_failure(
                    &initial_run,
                    code,
                    0,
                ))
                .await;
        }
    });
    Ok(result)
}

fn opened_compatible_typed_stream(
    dependencies: CompatibilityExecutionDependencies,
    opened: OpenedCompatibleTurn,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let OpenedCompatibleTurn {
        mut initial_run,
        delivery_scope,
        from_sequence,
        durable_round_replay,
        ignored_waiting_callback_task_id,
        subscription,
        execution,
    } = opened;
    freeze_delivery_scope(&mut initial_run, delivery_scope);
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
            delivery_policy: CompatibleDeliveryPolicy::ScopedResponses,
            delivery_scope,
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

/// Called only after the Native WebSocket owner authenticated and read this run.
/// Native cold recovery deliberately does not select a Responses callback round.
pub(crate) async fn start_native_run_attachment_stream(
    terminal_dependencies: NativeRunTerminalDependencies,
    runtime_event_stream: Arc<dyn control_plane::ports::RuntimeEventStream>,
    initial_run: NativeRunResult,
    from_sequence: Option<i64>,
) -> Result<NativeRunAttachmentStream, NativeApiError> {
    let subscription = match runtime_event_stream
        .subscribe(initial_run.id, from_sequence)
        .await
    {
        Ok(subscription) => subscription,
        Err(_) => {
            cold_runtime_event_subscription(&terminal_dependencies, &initial_run, from_sequence)
                .await
                .map_err(service_error)?
        }
    };
    let (sender, events) = mpsc::channel(32);
    tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            terminal_dependencies,
            initial_run,
            delivery_policy: CompatibleDeliveryPolicy::NativeRunAttachment,
            // Explicitly unavailable Responses identity: this capability never reads it.
            delivery_scope: RuntimeEventDeliveryScope::Unresolved,
            from_sequence,
            durable_round_replay: Vec::new(),
            ignored_waiting_callback_task_id: None,
            subscription,
            sender,
        },
    ));
    Ok(NativeRunAttachmentStream { events })
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
    mut durable_round_replay: Vec<RuntimeEventEnvelope>,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let delivery_scope = selected_delivery_scope(&initial_run);
    let subscription = match (
        runtime_event_stream
            .subscribe(initial_run.id, from_sequence)
            .await,
        delivery_scope,
    ) {
        (Ok(subscription), _) => subscription,
        (Err(_), RuntimeEventDeliveryScope::Resume { callback_task_id }) => {
            if durable_round_replay.is_empty() {
                durable_round_replay = match durable_replay::durable_callback_replay(
                    &terminal_dependencies,
                    &initial_run,
                    callback_task_id,
                )
                .await?
                {
                    DurableCompatibleRoundReplay::Ready(replay)
                        if replay.last().is_some_and(|event| {
                            event_forwarding::is_public_terminal_runtime_event(&event.event_type)
                        }) =>
                    {
                        replay
                    }
                    _ => {
                        return Err(NativeApiError::new(
                            axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            "runtime_event_delivery_proof_unavailable",
                            "selected callback round is not durably recoverable",
                        ))
                    }
                };
            }
            if !durable_round_replay.last().is_some_and(|event| {
                event_forwarding::is_public_terminal_runtime_event(&event.event_type)
            }) {
                return Err(NativeApiError::new(
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    "runtime_event_delivery_proof_unavailable",
                    "selected callback round has no recoverable terminal",
                ));
            }
            terminal_compatible_round_subscription(&durable_round_replay)
        }
        (Err(_), _) => {
            cold_runtime_event_subscription(&terminal_dependencies, &initial_run, from_sequence)
                .await
                .map_err(service_error)?
        }
    };
    Ok(attach_compatible_typed_stream_from_subscription(
        terminal_dependencies,
        initial_run,
        from_sequence,
        durable_round_replay,
        delivery_scope,
        subscription,
    ))
}

fn attach_compatible_typed_stream_from_subscription(
    terminal_dependencies: NativeRunTerminalDependencies,
    mut initial_run: NativeRunResult,
    from_sequence: Option<i64>,
    durable_round_replay: Vec<RuntimeEventEnvelope>,
    delivery_scope: RuntimeEventDeliveryScope,
    subscription: control_plane::ports::RuntimeEventSubscription,
) -> CompatibleTypedTurnStream {
    freeze_delivery_scope(&mut initial_run, delivery_scope);
    let (sender, events) = mpsc::channel(32);
    tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            terminal_dependencies,
            initial_run: initial_run.clone(),
            delivery_policy: CompatibleDeliveryPolicy::ScopedResponses,
            delivery_scope,
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

// A newly committed anchor can precede the persister's flow_started batch.
// Followers wait for that identity; cold recovery cannot treat it as replayable.

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

mod responses_projection;
pub(crate) use responses_projection::ResponsesProjectionSelector;

#[path = "compat_sse/durable_replay.rs"]
mod durable_replay;

use durable_replay::*;
