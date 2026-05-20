use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::response::{
    sse::{Event, KeepAlive, Sse},
    IntoResponse, Response,
};
use control_plane::{
    application_public_api::native::NativeRunResult,
    orchestration_runtime::{
        debug_stream_events, OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{RuntimeEventEnvelope, RuntimeEventStream},
};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    routes::application_public_api::native::{service_error, NativeApiError},
};

type CompatRunSseStream = tokio_stream::wrappers::ReceiverStream<Result<Event, Infallible>>;

pub(crate) async fn start_openai_run_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
) -> Result<Response, NativeApiError> {
    start_compatible_run_stream(state, run, move |run, envelope| {
        openai_runtime_event_to_sse(run, &model, envelope)
    })
    .await
}

pub(crate) async fn start_anthropic_run_stream(
    state: Arc<ApiState>,
    run: NativeRunResult,
    model: String,
) -> Result<Response, NativeApiError> {
    let mut stateful_mapper = AnthropicStreamMapper::new(model);
    start_compatible_run_stream(state, run, move |run, envelope| {
        stateful_mapper.runtime_event_to_sse(run, envelope)
    })
    .await
}

async fn start_compatible_run_stream<F>(
    state: Arc<ApiState>,
    run: NativeRunResult,
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
        state.runtime_event_stream.clone(),
        run.clone(),
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

async fn send_compatible_runtime_event_stream<F>(
    stream: Arc<dyn RuntimeEventStream>,
    initial_run: NativeRunResult,
    sender: mpsc::Sender<Result<Event, Infallible>>,
    mut mapper: F,
) where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let Ok(mut subscription) = stream.subscribe(initial_run.id, None).await else {
        warn!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            "failed to subscribe compatible public API runtime event stream"
        );
        return;
    };

    for event in subscription.replay {
        let is_terminal = is_public_terminal_runtime_event(&event.event_type);
        for sse in mapper(&initial_run, event) {
            if sender.send(sse).await.is_err() {
                debug!(
                    flow_run_id = %initial_run.id,
                    application_id = %initial_run.application_id,
                    "compatible public API stream client disconnected during replay"
                );
                return;
            }
        }
        if is_terminal {
            debug!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                "compatible public API stream replay reached terminal event"
            );
            return;
        }
    }

    while let Some(event) = subscription.live_events.recv().await {
        let event_type = event.event_type.clone();
        let is_terminal = is_public_terminal_runtime_event(&event_type);
        for sse in mapper(&initial_run, event) {
            if sender.send(sse).await.is_err() {
                debug!(
                    flow_run_id = %initial_run.id,
                    application_id = %initial_run.application_id,
                    "compatible public API stream client disconnected"
                );
                return;
            }
        }
        if is_terminal {
            debug!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                event_type = %event_type,
                "compatible public API stream reached terminal event"
            );
            return;
        }
    }
}

fn is_public_terminal_runtime_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "flow_finished" | "flow_failed" | "flow_cancelled" | "waiting_human" | "waiting_callback"
    )
}

fn openai_runtime_event_to_sse(
    initial_run: &NativeRunResult,
    model: &str,
    envelope: RuntimeEventEnvelope,
) -> Vec<Result<Event, Infallible>> {
    match envelope.event_type.as_str() {
        "flow_started" => vec![json_sse(json!({
            "id": format!("chatcmpl-{}", initial_run.id),
            "object": "chat.completion.chunk",
            "created": initial_run.created_at.unix_timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "delta": { "role": "assistant" },
                "finish_reason": null
            }]
        }))],
        "text_delta" => vec![json_sse(json!({
            "id": format!("chatcmpl-{}", initial_run.id),
            "object": "chat.completion.chunk",
            "created": initial_run.created_at.unix_timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "delta": { "content": envelope.text.unwrap_or_default() },
                "finish_reason": null
            }]
        }))],
        "reasoning_delta" => vec![json_sse(json!({
            "id": format!("chatcmpl-{}", initial_run.id),
            "object": "chat.completion.chunk",
            "created": initial_run.created_at.unix_timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "delta": { "reasoning_content": envelope.text.unwrap_or_default() },
                "finish_reason": null
            }]
        }))],
        "flow_finished" => vec![
            json_sse(json!({
                "id": format!("chatcmpl-{}", initial_run.id),
                "object": "chat.completion.chunk",
                "created": initial_run.created_at.unix_timestamp(),
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": "stop"
                }]
            })),
            done_sse(),
        ],
        "flow_failed" => vec![
            json_sse(json!({
                "error": {
                    "message": runtime_error_message(&envelope.payload),
                    "type": "server_error",
                    "param": null,
                    "code": "runtime_error"
                }
            })),
            done_sse(),
        ],
        "flow_cancelled" => vec![done_sse()],
        "waiting_human" | "waiting_callback" => vec![
            json_sse(json!({
                "error": {
                    "message": "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs",
                    "type": "invalid_request_error",
                    "param": null,
                    "code": "required_action_not_supported"
                }
            })),
            done_sse(),
        ],
        _ => Vec::new(),
    }
}

struct AnthropicStreamMapper {
    model: String,
    content_started: bool,
}

impl AnthropicStreamMapper {
    fn new(model: String) -> Self {
        Self {
            model,
            content_started: false,
        }
    }

    fn runtime_event_to_sse(
        &mut self,
        initial_run: &NativeRunResult,
        envelope: RuntimeEventEnvelope,
    ) -> Vec<Result<Event, Infallible>> {
        match envelope.event_type.as_str() {
            "flow_started" => vec![event_json_sse(
                "message_start",
                json!({
                    "type": "message_start",
                    "message": {
                        "id": format!("msg_{}", initial_run.id),
                        "type": "message",
                        "role": "assistant",
                        "model": self.model,
                        "content": [],
                        "stop_reason": null,
                        "usage": {"input_tokens": 0, "output_tokens": 0}
                    }
                }),
            )],
            "text_delta" => {
                let mut events = Vec::new();
                if !self.content_started {
                    self.content_started = true;
                    events.push(event_json_sse(
                        "content_block_start",
                        json!({
                            "type": "content_block_start",
                            "index": 0,
                            "content_block": {"type": "text", "text": ""}
                        }),
                    ));
                }
                events.push(event_json_sse(
                    "content_block_delta",
                    json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": {
                            "type": "text_delta",
                            "text": envelope.text.unwrap_or_default()
                        }
                    }),
                ));
                events
            }
            "flow_finished" => self.anthropic_stop_events(),
            "waiting_human" | "waiting_callback" => vec![event_json_sse(
                "error",
                json!({
                    "type": "error",
                    "error": {
                        "type": "required_action_not_supported",
                        "message": "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs"
                    }
                }),
            )],
            "flow_failed" => vec![event_json_sse(
                "error",
                json!({
                    "type": "error",
                    "error": {
                        "type": "api_error",
                        "message": runtime_error_message(&envelope.payload)
                    }
                }),
            )],
            "flow_cancelled" => self.anthropic_stop_events(),
            _ => Vec::new(),
        }
    }

    fn anthropic_stop_events(&mut self) -> Vec<Result<Event, Infallible>> {
        let mut events = Vec::new();
        if !self.content_started {
            self.content_started = true;
            events.push(event_json_sse(
                "content_block_start",
                json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": {"type": "text", "text": ""}
                }),
            ));
        }
        events.push(event_json_sse(
            "content_block_stop",
            json!({"type": "content_block_stop", "index": 0}),
        ));
        events.push(event_json_sse(
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": {"stop_reason": "end_turn"},
                "usage": {"output_tokens": 0}
            }),
        ));
        events.push(event_json_sse(
            "message_stop",
            json!({"type": "message_stop"}),
        ));
        events
    }
}

fn runtime_error_message(payload: &Value) -> String {
    payload
        .get("error")
        .or_else(|| payload.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("published run failed")
        .to_string()
}

fn json_sse(payload: Value) -> Result<Event, Infallible> {
    Ok(Event::default()
        .json_data(payload)
        .expect("compatible SSE payload should serialize"))
}

fn event_json_sse(event_name: &'static str, payload: Value) -> Result<Event, Infallible> {
    Ok(Event::default()
        .event(event_name)
        .json_data(payload)
        .expect("compatible SSE payload should serialize"))
}

fn done_sse() -> Result<Event, Infallible> {
    Ok(Event::default().data("[DONE]"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, response::IntoResponse};
    use control_plane::{
        application_public_api::native::NativeRunStatus,
        ports::{RuntimeEventDurability, RuntimeEventPayload, RuntimeEventSource},
    };
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn native_run(run_id: Uuid) -> NativeRunResult {
        NativeRunResult {
            id: run_id,
            application_id: Uuid::nil(),
            api_key_id: Uuid::nil(),
            publication_version_id: Uuid::nil(),
            status: NativeRunStatus::Running,
            node_input_payload: json!({}),
            metadata: json!({}),
            answer: None,
            reasoning: None,
            required_action: None,
            tool_calls: None,
            usage: None,
            error: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn reasoning_envelope(run_id: Uuid) -> RuntimeEventEnvelope {
        RuntimeEventEnvelope::new(
            run_id,
            1,
            RuntimeEventPayload {
                event_type: "reasoning_delta".to_string(),
                source: RuntimeEventSource::Provider,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: false,
                payload: json!({ "type": "reasoning_delta", "text": "先分析用户意图" }),
            },
        )
    }

    #[tokio::test]
    async fn openai_stream_projects_reasoning_delta_as_reasoning_content() {
        let run_id = Uuid::now_v7();
        let events = openai_runtime_event_to_sse(
            &native_run(run_id),
            "provider/model",
            reasoning_envelope(run_id),
        );

        let response = Sse::new(tokio_stream::iter(events)).into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(
            body.contains("\"reasoning_content\":\"先分析用户意图\""),
            "{body}"
        );
        assert!(!body.contains("\"content\":\"先分析用户意图\""), "{body}");
    }
}
