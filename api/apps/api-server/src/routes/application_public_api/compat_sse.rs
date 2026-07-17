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
use control_plane::{
    application_public_api::{
        callback_resume::{
            ApplicationPublishedCallbackResumeService, PublishedCallbackResumeTarget,
            ResumePublishedCallbackCommand,
        },
        compat::openai::response_id_from_run_id,
        native::{NativeRunResult, NativeRunStatus, NativeUsage},
    },
    orchestration_runtime::{
        debug_stream_events, OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{OrchestrationRuntimeRepository, RuntimeEventEnvelope, RuntimeEventPayload},
};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    routes::application_public_api::{
        native::{service_error, NativeApiError},
        stream_terminal_fallback::{
            enrich_terminal_runtime_event_with_durable_answer,
            load_latest_native_run_for_terminal_fallback, terminal_answer_deltas_from_payload,
            terminal_answer_text_from_payload, terminal_runtime_event_from_native_run,
            TerminalAnswerDelta, TerminalAnswerDeltaKind,
        },
        tool_callback_ids::{
            encode_anthropic_callback_tool_use_id, encode_openai_callback_tool_call_id,
        },
    },
};

mod event_forwarding;
mod protocol_mappers;
#[cfg(test)]
mod tests;

use event_forwarding::{
    append_compatible_resume_terminal_event, is_answer_presentation_delta,
    send_compatible_runtime_event_stream,
};
use protocol_mappers::{
    anthropic_completed_run_to_sse, terminal_answer_deltas_from_run_or_payload,
    AnthropicStreamMapper, OpenAiChatStreamMapper, OpenAiResponseStreamMapper,
};

type CompatRunSseStream = tokio_stream::wrappers::ReceiverStream<Result<Event, Infallible>>;

const OPENAI_CHAT_SSE_PROJECTION: &str = "openai_chat";
const OPENAI_RESPONSES_SSE_PROJECTION: &str = "openai_responses";
const ANTHROPIC_SSE_PROJECTION: &str = "anthropic";

#[derive(Debug, Default)]
struct CompatibleStreamStats {
    emitted_public_event: bool,
    emitted_content_bytes: usize,
    emitted_text_content: bool,
    emitted_reasoning_content: bool,
    forwarded_event_identities: HashSet<CompatiblePublicEventIdentity>,
    forwarded_answer_chunks: HashSet<CompatiblePublicEventChunkIdentity>,
    forwarded_answer_text: HashMap<CompatiblePublicEventIdentity, String>,
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
        if is_batched_durable_delta && !forwarded.is_empty() {
            if text.starts_with(forwarded.as_str()) {
                let suffix = text[forwarded.len()..].to_string();
                if suffix.is_empty() {
                    return false;
                }
                *forwarded = text.to_string();
                event.text = Some(suffix.clone());
                if let Some(payload) = event.payload.as_object_mut() {
                    payload.insert("text".to_string(), Value::String(suffix));
                }
                return true;
            }
            if forwarded.starts_with(text) {
                return false;
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

        if !matches!(event.event_type.as_str(), "flow_finished" | "flow_failed") {
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
    let mut mapper =
        OpenAiChatStreamMapper::new(model, openai_chat_completion_id_from_run_id(run.id), true);
    start_compatible_run_stream(
        state,
        run,
        OPENAI_CHAT_SSE_PROJECTION,
        move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
    )
    .await
}

pub(crate) async fn start_openai_response_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
    previous_response_id: Option<String>,
) -> Result<Response, NativeApiError> {
    let mut mapper = OpenAiResponseStreamMapper::new(model, previous_response_id, true);
    start_compatible_run_stream(
        state,
        run,
        OPENAI_RESPONSES_SSE_PROJECTION,
        move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
    )
    .await
}

pub(crate) async fn start_openai_chat_resume_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
    chat_completion_id: String,
    command: ResumePublishedCallbackCommand,
) -> Result<Response, NativeApiError> {
    let mut mapper = OpenAiChatStreamMapper::new(model, chat_completion_id, true);
    start_compatible_resume_stream(
        state,
        run,
        command,
        OPENAI_CHAT_SSE_PROJECTION,
        move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
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
    let mut mapper = OpenAiResponseStreamMapper::new(model, previous_response_id, true);
    start_compatible_resume_stream(
        state,
        run,
        command,
        OPENAI_RESPONSES_SSE_PROJECTION,
        move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
    )
    .await
}

pub(crate) async fn start_anthropic_run_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
) -> Result<Response, NativeApiError> {
    let mut stateful_mapper = AnthropicStreamMapper::new(model);
    start_compatible_run_stream(
        state,
        run,
        ANTHROPIC_SSE_PROJECTION,
        move |run, envelope| stateful_mapper.runtime_event_to_sse(run, envelope),
    )
    .await
}

pub(crate) async fn start_anthropic_resume_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
    command: ResumePublishedCallbackCommand,
) -> Result<Response, NativeApiError> {
    let mut mapper = AnthropicStreamMapper::new(model);
    start_compatible_resume_stream(
        state,
        run,
        command,
        ANTHROPIC_SSE_PROJECTION,
        move |run, envelope| mapper.runtime_event_to_sse(run, envelope),
    )
    .await
}

pub(crate) fn completed_anthropic_stream(run: NativeRunResult, model: String) -> Response {
    completed_compatible_stream(anthropic_completed_run_to_sse(&run, &model))
}

fn completed_compatible_stream(events: Vec<Result<Event, Infallible>>) -> Response {
    Sse::new(tokio_stream::iter(events))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(10))
                .text("heartbeat"),
        )
        .into_response()
}

async fn start_compatible_run_stream<F>(
    state: Arc<ApiState>,
    run: NativeRunResult,
    sse_projection: &'static str,
    mut mapper: F,
) -> Result<Response, NativeApiError>
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>
        + Send
        + 'static,
{
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

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(send_compatible_runtime_event_stream(
        state.clone(),
        run.clone(),
        sse_projection,
        None,
        None,
        sender,
        move |run, envelope| mapper(run, envelope),
    ));

    let background_state = state.clone();
    tokio::spawn(async move {
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
        .with_runtime_event_stream(background_state.runtime_event_stream.clone());
        if let Err(runtime_error) = runtime_service
            .start_published_flow_run(StartPublishedFlowRunCommand {
                application_id: run.application_id,
                flow_run_id: run.id,
            })
            .await
        {
            warn!(
                flow_run_id = %run.id,
                application_id = %run.application_id,
                error = %runtime_error,
                "compatible public API streamed run failed"
            );
            let _ = background_state
                .runtime_event_stream
                .append(
                    run.id,
                    debug_stream_events::flow_failed(
                        run.id,
                        json!({ "message": runtime_error.to_string() }),
                    ),
                )
                .await;
            let _ = background_state
                .runtime_event_stream
                .close_run(
                    run.id,
                    control_plane::ports::RuntimeEventCloseReason::Failed,
                )
                .await;
        }
    });

    info!(
        flow_run_id = %run.id,
        application_id = %run.application_id,
        sse_projection = %sse_projection,
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

async fn start_compatible_resume_stream<F>(
    state: Arc<ApiState>,
    run: NativeRunResult,
    command: ResumePublishedCallbackCommand,
    sse_projection: &'static str,
    mut mapper: F,
) -> Result<Response, NativeApiError>
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>
        + Send
        + 'static,
{
    state
        .runtime_event_stream
        .open_run(
            run.id,
            control_plane::ports::RuntimeEventStreamPolicy::debug_default(),
        )
        .await
        .map_err(service_error)?;
    let resume_started = state
        .runtime_event_stream
        .append(run.id, debug_stream_events::flow_started(run.id))
        .await
        .map_err(service_error)?;

    let (sender, receiver) = mpsc::channel(32);
    let (resume_done_sender, resume_done_receiver) = oneshot::channel::<()>();
    let resume_done_guard_sender = sender.clone();
    tokio::spawn(async move {
        let _ = resume_done_receiver.await;
        drop(resume_done_guard_sender);
    });
    tokio::spawn(send_compatible_runtime_event_stream(
        state.clone(),
        run.clone(),
        sse_projection,
        Some(resume_started.sequence.saturating_sub(1)),
        Some(callback_task_id_from_resume_command(&command)),
        sender,
        move |run, envelope| mapper(run, envelope),
    ));

    let background_state = state.clone();
    tokio::spawn(async move {
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
        .with_runtime_event_stream(background_state.runtime_event_stream.clone());
        match ApplicationPublishedCallbackResumeService::new(
            background_state.store.clone(),
            runtime_service,
        )
        .with_last_used_cache(background_state.infrastructure.cache_store())
        .resume_callback(command)
        .await
        {
            Ok(result) => {
                append_compatible_resume_terminal_event(&background_state, &result.run).await;
            }
            Err(error) => {
                let _ = background_state
                    .runtime_event_stream
                    .append(
                        run.id,
                        debug_stream_events::flow_failed(
                            run.id,
                            json!({ "message": error.to_string() }),
                        ),
                    )
                    .await;
                let _ = background_state
                    .runtime_event_stream
                    .close_run(
                        run.id,
                        control_plane::ports::RuntimeEventCloseReason::Failed,
                    )
                    .await;
            }
        }
        let _ = resume_done_sender.send(());
    });

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
