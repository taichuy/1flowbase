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
#[cfg(test)]
use control_plane::application_public_api::{
    callback_resume::ResumePublishedCallbackCommand, native::NativeRunStatus,
};
use control_plane::{
    application_public_api::{
        compat::openai::response_id_from_run_id,
        native::{NativeRunResult, NativeUsage},
    },
    orchestration_runtime::{
        debug_stream_events, OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{OrchestrationRuntimeRepository, RuntimeEventEnvelope},
};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

#[cfg(test)]
use crate::routes::application_public_api::tool_callback_ids::{
    encode_anthropic_callback_tool_use_id, encode_openai_callback_tool_call_id,
};
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

use event_forwarding::{is_answer_presentation_delta, send_compatible_runtime_event_stream};
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

#[cfg(test)]
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

#[cfg(test)]
fn completed_compatible_stream(events: Vec<Result<Event, Infallible>>) -> Response {
    Sse::new(tokio_stream::iter(events))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(10))
                .text("heartbeat"),
        )
        .into_response()
}

#[cfg(test)]
fn unsupported_compatible_resume_stream(sse_projection: &str) -> Response {
    const MESSAGE: &str = "compatible streaming callback resume is not supported";
    let payload = if sse_projection == ANTHROPIC_SSE_PROJECTION {
        json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": MESSAGE,
            }
        })
    } else {
        json!({
            "error": {
                "message": MESSAGE,
                "type": "invalid_request_error",
                "code": "unsupported_feature",
            }
        })
    };
    let event = Event::default()
        .event("error")
        .json_data(payload)
        .expect("compatible unsupported-resume error should serialize");
    completed_compatible_stream(vec![Ok(event)])
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
    let background_run = run.clone();
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
                application_id: background_run.application_id,
                flow_run_id: background_run.id,
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
                recover_missing_stream_terminal_winner(&background_state, &background_run).await
            {
                warn!(
                    flow_run_id = %background_run.id,
                    application_id = %background_run.application_id,
                    error = %recovery_error,
                    "failed to recover the durable winner after compatible streaming execution ended"
                );
            }
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

#[cfg(test)]
async fn start_compatible_resume_stream<F>(
    _state: Arc<ApiState>,
    _run: NativeRunResult,
    _command: ResumePublishedCallbackCommand,
    sse_projection: &'static str,
    _mapper: F,
) -> Result<Response, NativeApiError> {
    // Retain the protocol entry point, but callback continuation is explicitly unsupported for
    // compatible streams. It must not rewrite a durable WaitingCallback winner as failure.
    Ok(unsupported_compatible_resume_stream(sse_projection))
}

pub(crate) fn openai_chat_completion_id_from_run_id(run_id: uuid::Uuid) -> String {
    format!("chatcmpl-{run_id}")
}

#[cfg(test)]
pub(crate) fn openai_chat_completion_id_from_callback_task(
    run_id: uuid::Uuid,
    callback_task_id: uuid::Uuid,
) -> String {
    format!("chatcmpl-{run_id}-{callback_task_id}")
}
