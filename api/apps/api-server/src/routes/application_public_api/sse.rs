use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::response::sse::Event;
use control_plane::{
    application_public_api::native::NativeRunResult, orchestration_runtime::debug_stream_events,
    ports::RuntimeEventEnvelope,
};
use serde::Serialize;
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use tokio::sync::mpsc;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    routes::application_public_api::stream_terminal_fallback::{
        load_latest_native_run_for_terminal_fallback, terminal_answer_deltas_from_payload,
        terminal_runtime_event_from_native_run, TerminalAnswerDelta, TerminalAnswerDeltaKind,
    },
};

pub type NativeRunSseStream = tokio_stream::wrappers::ReceiverStream<Result<Event, Infallible>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeWorkflowEvents {
    None,
    Public,
}

#[derive(Debug, Serialize)]
struct NativeSsePayload {
    run_id: Uuid,
    status: &'static str,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required_action: Option<Value>,
}

fn event_created_at(envelope: &RuntimeEventEnvelope) -> String {
    envelope
        .occurred_at
        .format(&Rfc3339)
        .unwrap_or_else(|_| envelope.occurred_at.to_string())
}

fn native_terminal_payload(
    initial_run: &NativeRunResult,
    envelope: &RuntimeEventEnvelope,
    status: &'static str,
) -> NativeSsePayload {
    let output = envelope
        .payload
        .get("output")
        .cloned()
        .unwrap_or(Value::Null);
    NativeSsePayload {
        run_id: initial_run.id,
        status,
        created_at: event_created_at(envelope),
        delta: None,
        answer: output
            .get("answer")
            .or_else(|| output.get("text"))
            .or_else(|| output.get("output"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        conversation: initial_run.metadata.get("request").and_then(|request| {
            request
                .get("conversation")
                .cloned()
                .filter(|value| !value.is_null())
        }),
        usage: output.get("usage").cloned(),
        attachments: output.get("attachments").cloned(),
        metadata: Some(initial_run.metadata.clone()),
        error: None,
        workflow: None,
        required_action: None,
    }
}

fn workflow_payload(envelope: &RuntimeEventEnvelope) -> Value {
    json!({
        "type": envelope.event_type,
        "run_id": envelope.run_id,
        "node": {
            "id": envelope.payload.get("node_id").cloned().unwrap_or(Value::Null),
            "type": envelope.payload.get("node_type").cloned().unwrap_or(Value::Null),
            "title": envelope.payload.get("title").cloned().unwrap_or(Value::Null),
        },
        "status": envelope.payload.get("status").cloned().unwrap_or(Value::Null),
    })
}

fn runtime_event_to_native_sse(
    initial_run: &NativeRunResult,
    include_workflow_events: IncludeWorkflowEvents,
    envelope: RuntimeEventEnvelope,
) -> Option<Result<Event, Infallible>> {
    let event_id = envelope.event_id.clone();
    let (event_name, payload) =
        native_sse_payload_for_runtime_event(initial_run, include_workflow_events, envelope)?;

    Some(Ok(Event::default()
        .id(event_id)
        .event(event_name)
        .json_data(payload)
        .expect("native SSE payload should serialize")))
}

fn native_sse_payload_for_runtime_event(
    initial_run: &NativeRunResult,
    include_workflow_events: IncludeWorkflowEvents,
    envelope: RuntimeEventEnvelope,
) -> Option<(&'static str, NativeSsePayload)> {
    let created_at = event_created_at(&envelope);
    Some(match envelope.event_type.as_str() {
        "flow_started" => (
            "run.started",
            NativeSsePayload {
                run_id: initial_run.id,
                status: "running",
                created_at,
                delta: None,
                answer: None,
                conversation: initial_run.metadata.get("request").and_then(|request| {
                    request
                        .get("conversation")
                        .cloned()
                        .filter(|value| !value.is_null())
                }),
                usage: None,
                attachments: None,
                metadata: Some(initial_run.metadata.clone()),
                error: None,
                workflow: None,
                required_action: None,
            },
        ),
        "reasoning_delta" if is_answer_presentation_delta(&envelope) => (
            "reasoning.delta",
            NativeSsePayload {
                run_id: initial_run.id,
                status: "running",
                created_at,
                delta: envelope.text.clone(),
                answer: None,
                conversation: None,
                usage: None,
                attachments: None,
                metadata: None,
                error: None,
                workflow: None,
                required_action: None,
            },
        ),
        "text_delta" if is_answer_presentation_delta(&envelope) => (
            "message.delta",
            NativeSsePayload {
                run_id: initial_run.id,
                status: "running",
                created_at,
                delta: envelope.text.clone(),
                answer: None,
                conversation: None,
                usage: None,
                attachments: None,
                metadata: None,
                error: None,
                workflow: None,
                required_action: None,
            },
        ),
        "reasoning_delta" | "text_delta" => return None,
        "node_started" | "node_finished"
            if include_workflow_events == IncludeWorkflowEvents::Public =>
        {
            (
                "workflow.event",
                NativeSsePayload {
                    run_id: initial_run.id,
                    status: "running",
                    created_at,
                    delta: None,
                    answer: None,
                    conversation: None,
                    usage: None,
                    attachments: None,
                    metadata: None,
                    error: None,
                    workflow: Some(workflow_payload(&envelope)),
                    required_action: None,
                },
            )
        }
        "waiting_human" | "waiting_callback" => (
            "required_action",
            NativeSsePayload {
                run_id: initial_run.id,
                status: "waiting",
                created_at,
                delta: None,
                answer: None,
                conversation: initial_run.metadata.get("request").and_then(|request| {
                    request
                        .get("conversation")
                        .cloned()
                        .filter(|value| !value.is_null())
                }),
                usage: None,
                attachments: None,
                metadata: Some(initial_run.metadata.clone()),
                error: None,
                workflow: None,
                required_action: Some(native_required_action_payload(initial_run, &envelope)),
            },
        ),
        "flow_finished" => (
            "run.completed",
            native_terminal_payload(initial_run, &envelope, "succeeded"),
        ),
        "flow_failed" => (
            "run.failed",
            NativeSsePayload {
                run_id: initial_run.id,
                status: "failed",
                created_at,
                delta: None,
                answer: None,
                conversation: initial_run.metadata.get("request").and_then(|request| {
                    request
                        .get("conversation")
                        .cloned()
                        .filter(|value| !value.is_null())
                }),
                usage: None,
                attachments: None,
                metadata: Some(initial_run.metadata.clone()),
                error: Some(json!({
                    "code": "runtime_error",
                    "message": envelope
                        .payload
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("published run failed"),
                })),
                workflow: None,
                required_action: None,
            },
        ),
        "flow_cancelled" => (
            "run.cancelled",
            NativeSsePayload {
                run_id: initial_run.id,
                status: "cancelled",
                created_at,
                delta: None,
                answer: None,
                conversation: initial_run.metadata.get("request").and_then(|request| {
                    request
                        .get("conversation")
                        .cloned()
                        .filter(|value| !value.is_null())
                }),
                usage: None,
                attachments: None,
                metadata: Some(initial_run.metadata.clone()),
                error: None,
                workflow: None,
                required_action: None,
            },
        ),
        "usage_delta" => (
            "usage.delta",
            NativeSsePayload {
                run_id: initial_run.id,
                status: "running",
                created_at,
                delta: None,
                answer: None,
                conversation: None,
                usage: Some(envelope.payload.clone()),
                attachments: None,
                metadata: None,
                error: None,
                workflow: None,
                required_action: None,
            },
        ),
        _ => return None,
    })
}

fn native_required_action_payload(
    initial_run: &NativeRunResult,
    envelope: &RuntimeEventEnvelope,
) -> Value {
    envelope
        .payload
        .get("required_action")
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "type": envelope.event_type,
                "run_id": initial_run.id,
            })
        })
}

fn is_public_terminal_runtime_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "flow_finished" | "flow_failed" | "flow_cancelled" | "waiting_human" | "waiting_callback"
    )
}

fn is_answer_presentation_delta(envelope: &RuntimeEventEnvelope) -> bool {
    matches!(
        envelope.event_type.as_str(),
        "reasoning_delta" | "text_delta"
    ) && debug_stream_events::is_answer_presentation_delta_payload(&envelope.payload)
}

pub async fn send_native_runtime_event_stream(
    state: Arc<ApiState>,
    initial_run: NativeRunResult,
    include_workflow_events: IncludeWorkflowEvents,
    from_sequence: Option<i64>,
    ignored_waiting_callback_task_id: Option<Uuid>,
    sender: mpsc::Sender<Result<Event, Infallible>>,
) {
    let stream = state.runtime_event_stream.clone();
    let Ok(mut subscription) = stream.subscribe(initial_run.id, from_sequence).await else {
        return;
    };

    let mut emitted_public_event = false;
    let mut emitted_answer_delta = false;
    for event in subscription.replay {
        if is_ignored_waiting_callback(&event, ignored_waiting_callback_task_id) {
            continue;
        }
        let event_type = event.event_type.clone();
        let is_terminal = is_public_terminal_runtime_event(&event_type);
        let is_answer_delta = is_answer_presentation_delta(&event);
        if is_terminal && !emitted_answer_delta {
            let answer_events =
                terminal_answer_delta_sse_events(&initial_run, include_workflow_events, &event);
            emitted_answer_delta |= !answer_events.is_empty();
            if !send_native_sse_events(&sender, answer_events).await {
                return;
            }
        }
        let sse = runtime_event_to_native_sse(&initial_run, include_workflow_events, event);
        emitted_public_event |= sse.is_some();
        emitted_answer_delta |= is_answer_delta && sse.is_some();
        if !send_native_sse_event(&sender, sse).await {
            return;
        }
        if is_terminal {
            return;
        }
    }

    let mut durable_terminal_check = tokio::time::interval(Duration::from_millis(500));
    durable_terminal_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            maybe_event = subscription.live_events.recv() => {
                let Some(event) = maybe_event else {
                    break;
                };
                if is_ignored_waiting_callback(&event, ignored_waiting_callback_task_id) {
                    continue;
                }
                let event_type = event.event_type.clone();
                let is_terminal = is_public_terminal_runtime_event(&event_type);
                let is_answer_delta = is_answer_presentation_delta(&event);
                if is_terminal && !emitted_answer_delta {
                    let answer_events =
                        terminal_answer_delta_sse_events(&initial_run, include_workflow_events, &event);
                    emitted_answer_delta |= !answer_events.is_empty();
                    if !send_native_sse_events(&sender, answer_events).await {
                        return;
                    }
                }
                let sse = runtime_event_to_native_sse(&initial_run, include_workflow_events, event);
                emitted_public_event |= sse.is_some();
                emitted_answer_delta |= is_answer_delta && sse.is_some();
                if !send_native_sse_event(&sender, sse).await {
                    return;
                }
                if is_terminal {
                    return;
                }
            }
            _ = durable_terminal_check.tick() => {
                if emit_native_terminal_fallback(NativeTerminalFallback {
                    state: &state,
                    initial_run: &initial_run,
                    include_workflow_events,
                    sender: &sender,
                    emitted_public_event,
                    emitted_answer_delta,
                    trigger: "durable_poll",
                    warn_if_not_terminal: false,
                    ignored_waiting_callback_task_id,
                })
                .await
                {
                    return;
                }
            }
        }
    }

    emit_native_terminal_fallback(NativeTerminalFallback {
        state: &state,
        initial_run: &initial_run,
        include_workflow_events,
        sender: &sender,
        emitted_public_event,
        emitted_answer_delta,
        trigger: "stream_closed",
        warn_if_not_terminal: true,
        ignored_waiting_callback_task_id,
    })
    .await;
}

async fn send_native_sse_event(
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    event: Option<Result<Event, Infallible>>,
) -> bool {
    let Some(event) = event else {
        return true;
    };
    sender.send(event).await.is_ok()
}

async fn send_native_sse_events(
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    events: Vec<Result<Event, Infallible>>,
) -> bool {
    for event in events {
        if sender.send(event).await.is_err() {
            return false;
        }
    }
    true
}

struct NativeTerminalFallback<'a> {
    state: &'a ApiState,
    initial_run: &'a NativeRunResult,
    include_workflow_events: IncludeWorkflowEvents,
    sender: &'a mpsc::Sender<Result<Event, Infallible>>,
    emitted_public_event: bool,
    emitted_answer_delta: bool,
    trigger: &'static str,
    warn_if_not_terminal: bool,
    ignored_waiting_callback_task_id: Option<Uuid>,
}

async fn emit_native_terminal_fallback(fallback: NativeTerminalFallback<'_>) -> bool {
    let NativeTerminalFallback {
        state,
        initial_run,
        include_workflow_events,
        sender,
        emitted_public_event,
        emitted_answer_delta,
        trigger,
        warn_if_not_terminal,
        ignored_waiting_callback_task_id,
    } = fallback;

    let latest_run = load_latest_native_run_for_terminal_fallback(state, initial_run).await;
    let Some(terminal_event) = terminal_runtime_event_from_native_run(&latest_run) else {
        if warn_if_not_terminal {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                status = ?latest_run.status,
                trigger = %trigger,
                "native stream ended before durable run reached a terminal state"
            );
        }
        return false;
    };

    warn!(
        flow_run_id = %initial_run.id,
        application_id = %initial_run.application_id,
        status = ?latest_run.status,
        trigger = %trigger,
        "native stream missing runtime terminal event; emitting durable terminal fallback"
    );

    if is_ignored_waiting_callback(&terminal_event, ignored_waiting_callback_task_id) {
        debug!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            trigger = %trigger,
            "native resume stream ignored stale waiting callback terminal fallback"
        );
        return false;
    }

    if !emitted_public_event {
        let started_event = RuntimeEventEnvelope::new(
            latest_run.id,
            0,
            debug_stream_events::flow_started(latest_run.id),
        );
        if !send_native_sse_event(
            sender,
            runtime_event_to_native_sse(&latest_run, include_workflow_events, started_event),
        )
        .await
        {
            debug!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                "native stream client disconnected before terminal fallback"
            );
            return true;
        }
    }
    if !emitted_answer_delta {
        let answer_events =
            terminal_answer_delta_sse_events(&latest_run, include_workflow_events, &terminal_event);
        if !send_native_sse_events(sender, answer_events).await {
            return true;
        }
    }
    let _ = send_native_sse_event(
        sender,
        runtime_event_to_native_sse(&latest_run, include_workflow_events, terminal_event),
    )
    .await;
    true
}

fn is_ignored_waiting_callback(
    event: &RuntimeEventEnvelope,
    ignored_waiting_callback_task_id: Option<Uuid>,
) -> bool {
    if event.event_type != "waiting_callback" {
        return false;
    }
    let Some(ignored_task_id) = ignored_waiting_callback_task_id else {
        return false;
    };
    event
        .payload
        .get("callback_task_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        == Some(ignored_task_id)
}

fn terminal_answer_delta_to_runtime_event(
    run: &NativeRunResult,
    sequence: i64,
    delta: TerminalAnswerDelta,
) -> RuntimeEventEnvelope {
    let payload = match delta.kind {
        TerminalAnswerDeltaKind::Reasoning => debug_stream_events::answer_reasoning_delta(
            "assistant",
            delta.text,
            sequence as usize,
            None,
            None,
            None,
        ),
        TerminalAnswerDeltaKind::Text => debug_stream_events::answer_text_delta(
            "assistant",
            delta.text,
            sequence as usize,
            None,
            None,
            None,
        ),
    };
    RuntimeEventEnvelope::new(run.id, sequence, payload)
}

fn terminal_answer_delta_sse_events(
    run: &NativeRunResult,
    include_workflow_events: IncludeWorkflowEvents,
    terminal_event: &RuntimeEventEnvelope,
) -> Vec<Result<Event, Infallible>> {
    terminal_answer_deltas_from_payload(&terminal_event.payload)
        .into_iter()
        .enumerate()
        .filter_map(|(index, delta)| {
            runtime_event_to_native_sse(
                run,
                include_workflow_events,
                terminal_answer_delta_to_runtime_event(run, index as i64 + 1, delta),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane::orchestration_runtime::debug_stream_events;
    use control_plane::ports::{
        RuntimeEventDurability, RuntimeEventEnvelope, RuntimeEventPayload, RuntimeEventSource,
    };
    use serde_json::json;
    use time::OffsetDateTime;

    fn native_run() -> NativeRunResult {
        NativeRunResult {
            id: Uuid::from_u128(0x11111111111111111111111111111111),
            application_id: Uuid::from_u128(0x22222222222222222222222222222222),
            api_key_id: Uuid::from_u128(0x33333333333333333333333333333333),
            publication_version_id: Uuid::from_u128(0x44444444444444444444444444444444),
            status: control_plane::application_public_api::native::NativeRunStatus::Running,
            node_input_payload: json!({}),
            metadata: json!({}),
            answer: None,
            answer_segments: None,
            required_action: None,
            tool_calls: None,
            usage: None,
            error: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn native_sse_maps_reasoning_delta_to_public_reasoning_event() {
        let run = native_run();
        let event = RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::answer_reasoning_delta(
                "node-answer",
                "先分析用户问题".to_string(),
                0,
                Some("node-llm"),
                Some(Uuid::from_u128(0x55555555555555555555555555555555)),
                Some("text"),
            ),
        );

        let (event_name, payload) =
            native_sse_payload_for_runtime_event(&run, IncludeWorkflowEvents::None, event)
                .expect("reasoning delta should be public native SSE");
        let payload = serde_json::to_value(payload).expect("payload serializes");

        assert_eq!(event_name, "reasoning.delta");
        assert_eq!(payload["delta"], json!("先分析用户问题"));
        assert_eq!(payload.get("workflow"), None);
    }

    #[test]
    fn native_sse_projects_answer_presentation_delta_not_provider_raw_delta() {
        let run = native_run();
        let provider_event = RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::text_delta(
                "node-llm",
                Uuid::from_u128(0x55555555555555555555555555555555),
                "provider raw".to_string(),
            ),
        );
        let presentation_event = RuntimeEventEnvelope::new(
            run.id,
            2,
            RuntimeEventPayload {
                event_type: "text_delta".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: false,
                payload: json!({
                    "type": "text_delta",
                    "node_run_id": Uuid::from_u128(0x66666666666666666666666666666666),
                    "node_id": "node-answer",
                    "text": "answer presentation",
                    "presentation": { "kind": "answer" }
                }),
            },
        );

        assert!(native_sse_payload_for_runtime_event(
            &run,
            IncludeWorkflowEvents::None,
            provider_event
        )
        .is_none());
        let (event_name, payload) = native_sse_payload_for_runtime_event(
            &run,
            IncludeWorkflowEvents::None,
            presentation_event,
        )
        .expect("answer presentation delta should be public native SSE");
        let payload = serde_json::to_value(payload).expect("payload serializes");

        assert_eq!(event_name, "message.delta");
        assert_eq!(payload["delta"], json!("answer presentation"));
    }

    #[tokio::test]
    async fn native_terminal_answer_delta_sse_events_project_thinking_before_completed() {
        use axum::response::{sse::Sse, IntoResponse};

        let run = native_run();
        let terminal_event = RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({ "answer": "<think>先分析</think>\n最终回答" }),
            ),
        );
        let events =
            terminal_answer_delta_sse_events(&run, IncludeWorkflowEvents::None, &terminal_event);
        let response = Sse::new(tokio_stream::iter(events)).into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.contains("event: reasoning.delta"), "{body}");
        assert!(body.contains("\"delta\":\"先分析\""), "{body}");
        assert!(body.contains("event: message.delta"), "{body}");
        assert!(body.contains("\"delta\":\"\\n最终回答\""), "{body}");
        assert!(!body.contains("<think>"), "{body}");
    }

    #[test]
    fn native_sse_includes_waiting_callback_required_action_payload() {
        let run = native_run();
        let callback_task = domain::CallbackTaskRecord {
            id: Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa),
            flow_run_id: run.id,
            node_run_id: Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb),
            callback_kind: "llm_tool_calls".to_string(),
            status: domain::CallbackTaskStatus::Pending,
            request_payload: json!({
                "tool_calls": [
                    {
                        "id": "call_weather",
                        "name": "lookup_weather",
                        "arguments": {"city": "Hangzhou"}
                    }
                ]
            }),
            response_payload: None,
            external_ref_payload: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            completed_at: None,
        };
        let event = RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::waiting_callback_with_task(
                run.id,
                callback_task.node_run_id,
                "node-llm",
                &callback_task,
            ),
        );

        let (event_name, payload) =
            native_sse_payload_for_runtime_event(&run, IncludeWorkflowEvents::None, event)
                .expect("waiting callback should be public native SSE");
        let payload = serde_json::to_value(payload).expect("payload serializes");

        assert_eq!(event_name, "required_action");
        assert_eq!(payload["status"], json!("waiting"));
        assert_eq!(
            payload["required_action"]["action_type"],
            json!("submit_tool_outputs")
        );
        assert_eq!(
            payload["required_action"]["payload"]["callback_task_id"],
            json!(callback_task.id)
        );
        assert_eq!(
            payload["required_action"]["payload"]["tool_calls"][0]["name"],
            json!("lookup_weather")
        );
    }
}
