use crate::ports::{RuntimeEventDurability, RuntimeEventPayload, RuntimeEventSource};
use serde_json::{json, Map, Value};
use uuid::Uuid;

pub const ANSWER_PRESENTATION_KIND: &str = "answer";

pub fn flow_accepted(run_id: Uuid) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "flow_accepted".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: false,
        trace_visible: false,
        payload: json!({
            "type": "flow_accepted",
            "run_id": run_id,
            "status": "queued"
        }),
    }
}

pub fn flow_started(run_id: Uuid) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "flow_started".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "flow_started",
            "run_id": run_id,
            "status": "running"
        }),
    }
}

pub fn flow_finished(run_id: Uuid, output: serde_json::Value) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "flow_finished".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "flow_finished",
            "run_id": run_id,
            "status": "succeeded",
            "output": output,
        }),
    }
}

pub fn flow_incomplete(run_id: Uuid, output: serde_json::Value) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "flow_incomplete".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "flow_incomplete",
            "run_id": run_id,
            "status": "incomplete",
            "reason": "output_limit",
            "output": output,
        }),
    }
}

pub fn flow_failed(run_id: Uuid, error_payload: serde_json::Value) -> RuntimeEventPayload {
    let error = error_payload
        .get("message")
        .and_then(|message| message.as_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            if error_payload.is_null() {
                "flow debug run failed".to_string()
            } else {
                error_payload.to_string()
            }
        });

    RuntimeEventPayload {
        event_type: "flow_failed".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "flow_failed",
            "run_id": run_id,
            "error": error,
            "error_payload": error_payload,
        }),
    }
}

pub fn flow_cancelled(run_id: Uuid) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "flow_cancelled".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "flow_cancelled",
            "run_id": run_id,
            "status": "cancelled",
            "reason": "manual_stop",
            "manual_stop": true,
        }),
    }
}

pub fn waiting_human(run_id: Uuid, node_run_id: Uuid, node_id: &str) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "waiting_human".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "waiting_human",
            "run_id": run_id,
            "node_run_id": node_run_id,
            "node_id": node_id,
            "status": "waiting_human",
        }),
    }
}

pub fn waiting_callback(run_id: Uuid, node_run_id: Uuid, node_id: &str) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "waiting_callback".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "waiting_callback",
            "run_id": run_id,
            "node_run_id": node_run_id,
            "node_id": node_id,
            "status": "waiting_callback",
        }),
    }
}

pub fn waiting_callback_with_task(
    run_id: Uuid,
    node_run_id: Uuid,
    node_id: &str,
    task: &domain::CallbackTaskRecord,
) -> RuntimeEventPayload {
    let action_type = if task.callback_kind == "llm_tool_calls" {
        "submit_tool_outputs"
    } else {
        "callback"
    };
    let tool_calls = task
        .request_payload
        .get("tool_calls")
        .cloned()
        .unwrap_or(Value::Null);
    let mut action_payload = json!({
        "callback_task_id": task.id,
        "callback_kind": task.callback_kind,
        "flow_run_id": task.flow_run_id,
        "node_run_id": task.node_run_id,
    });
    if task.callback_kind == "llm_tool_calls" {
        action_payload["tool_calls"] = tool_calls;
    } else {
        action_payload["request_payload"] = task.request_payload.clone();
    }

    RuntimeEventPayload {
        event_type: "waiting_callback".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "waiting_callback",
            "run_id": run_id,
            "node_run_id": node_run_id,
            "node_id": node_id,
            "status": "waiting_callback",
            "callback_task_id": task.id,
            "callback_kind": task.callback_kind,
            "required_action": {
                "action_type": action_type,
                "payload": action_payload,
            }
        }),
    }
}

pub fn assistant_tool_call_started(
    run_id: Uuid,
    node_run_id: Uuid,
    node_id: &str,
    tool_call: Value,
) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "assistant_tool_call_started".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "assistant_tool_call_started",
            "run_id": run_id,
            "node_run_id": node_run_id,
            "node_id": node_id,
            "tool_call": tool_call,
        }),
    }
}

pub fn assistant_tool_call_finished(
    run_id: Uuid,
    node_run_id: Uuid,
    node_id: &str,
    tool_call: Value,
    tool_result: Value,
    duration_ms: u64,
) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "assistant_tool_call_finished".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "assistant_tool_call_finished",
            "run_id": run_id,
            "node_run_id": node_run_id,
            "node_id": node_id,
            "tool_call": tool_call,
            "tool_result": tool_result,
            "duration_ms": duration_ms,
        }),
    }
}

pub fn visible_internal_llm_tool_route(
    run_id: Uuid,
    node_run_id: Uuid,
    node_id: &str,
    route_event: &Value,
) -> RuntimeEventPayload {
    let event_type = route_event
        .get("event_type")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("visible_internal_llm_tool_event");
    let mut payload = route_event.as_object().cloned().unwrap_or_else(Map::new);
    payload.insert("type".to_string(), Value::String(event_type.to_string()));
    payload.insert("run_id".to_string(), json!(run_id));
    payload.insert("node_run_id".to_string(), json!(node_run_id));
    payload.insert("node_id".to_string(), Value::String(node_id.to_string()));

    RuntimeEventPayload {
        event_type: event_type.to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: Value::Object(payload),
    }
}

pub fn node_started(node_run: &domain::NodeRunRecord) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "node_started".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "node_started",
            "node_run_id": node_run.id,
            "node_id": node_run.node_id,
            "node_type": node_run.node_type,
            "title": node_run.node_alias,
            "input_payload": node_run.input_payload,
            "started_at": node_run.started_at,
        }),
    }
}

pub fn node_finished(node_run: &domain::NodeRunRecord) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "node_finished".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({
            "type": "node_finished",
            "node_run_id": node_run.id,
            "node_id": node_run.node_id,
            "node_type": node_run.node_type,
            "status": node_run.status.as_str(),
            "output_payload": node_run.output_payload,
            "error_payload": node_run.error_payload,
            "metrics_payload": node_run.metrics_payload,
            "started_at": node_run.started_at,
            "finished_at": node_run.finished_at,
        }),
    }
}

pub fn text_delta(node_id: &str, node_run_id: Uuid, text: String) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "text_delta".to_string(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: true,
        trace_visible: false,
        payload: json!({
            "type": "text_delta",
            "node_run_id": node_run_id,
            "node_id": node_id,
            "text": text,
        }),
    }
}

pub fn usage_snapshot(
    node_id: &str,
    node_run_id: Uuid,
    usage: &plugin_framework::provider_contract::ProviderUsage,
) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "usage_snapshot".to_string(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: true,
        trace_visible: false,
        payload: json!({
            "type": "usage_snapshot",
            "node_run_id": node_run_id,
            "node_id": node_id,
            "usage": usage,
        }),
    }
}

pub fn context_snapshot(
    node_id: &str,
    node_run_id: Uuid,
    estimate: &plugin_framework::provider_contract::ProviderCountTokensResult,
    effective_context_window: Option<u64>,
) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "context_snapshot".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: false,
        payload: json!({
            "type": "context_snapshot",
            "node_run_id": node_run_id,
            "node_id": node_id,
            "input_tokens": estimate.input_tokens,
            "effective_context_window": effective_context_window,
            "remaining_tokens": effective_context_window
                .map(|window| window.saturating_sub(estimate.input_tokens)),
            "measurement": {
                "method": estimate.method,
                "accuracy": "estimated",
                "coverage": estimate.coverage,
                "unknown_block_count": estimate.unknown_block_count,
            },
        }),
    }
}

pub fn provider_native_event(
    node_id: &str,
    node_run_id: Uuid,
    protocol: String,
    event: Value,
) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "provider_native_event".to_string(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: false,
        trace_visible: false,
        payload: json!({
            "type": "provider_native_event",
            "node_run_id": node_run_id,
            "node_id": node_id,
            "protocol": protocol,
            "event": event,
        }),
    }
}

pub fn provider_output_item_added(
    node_id: &str,
    node_run_id: Uuid,
    output_index: usize,
    item: Value,
) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "provider_output_item_added".to_string(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: false,
        trace_visible: true,
        payload: json!({
            "type": "provider_output_item_added",
            "node_id": node_id,
            "node_run_id": node_run_id,
            "output_index": output_index,
            "item": item,
        }),
    }
}

pub fn provider_output_item_done(
    node_id: &str,
    node_run_id: Uuid,
    output_index: usize,
    item: Value,
) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "provider_output_item_done".to_string(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: false,
        trace_visible: true,
        payload: json!({
            "type": "provider_output_item_done",
            "node_id": node_id,
            "node_run_id": node_run_id,
            "output_index": output_index,
            "item": item,
        }),
    }
}

pub fn answer_text_delta(
    answer_node_id: &str,
    text: String,
    segment_index: usize,
    source_node_id: Option<&str>,
    source_node_run_id: Option<Uuid>,
    source_output_key: Option<&str>,
) -> RuntimeEventPayload {
    answer_delta(
        "text_delta",
        answer_node_id,
        text,
        segment_index,
        source_node_id,
        source_node_run_id,
        source_output_key,
    )
}

pub fn reasoning_delta(node_id: &str, node_run_id: Uuid, text: String) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "reasoning_delta".to_string(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: true,
        trace_visible: false,
        payload: json!({
            "type": "reasoning_delta",
            "node_run_id": node_run_id,
            "node_id": node_id,
            "text": text,
        }),
    }
}

pub fn reasoning_signature_delta(
    node_id: &str,
    node_run_id: Uuid,
    signature: String,
) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "reasoning_signature_delta".to_string(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: false,
        trace_visible: false,
        payload: json!({
            "type": "reasoning_signature_delta",
            "node_run_id": node_run_id,
            "node_id": node_id,
            "signature": signature,
        }),
    }
}

pub fn answer_reasoning_delta(
    answer_node_id: &str,
    text: String,
    segment_index: usize,
    source_node_id: Option<&str>,
    source_node_run_id: Option<Uuid>,
    source_output_key: Option<&str>,
) -> RuntimeEventPayload {
    answer_delta(
        "reasoning_delta",
        answer_node_id,
        text,
        segment_index,
        source_node_id,
        source_node_run_id,
        source_output_key,
    )
}

pub fn is_answer_presentation_delta_payload(payload: &Value) -> bool {
    payload
        .get("presentation")
        .and_then(Value::as_object)
        .and_then(|presentation| presentation.get("kind"))
        .and_then(Value::as_str)
        == Some(ANSWER_PRESENTATION_KIND)
}

fn answer_delta(
    event_type: &str,
    answer_node_id: &str,
    text: String,
    segment_index: usize,
    source_node_id: Option<&str>,
    source_node_run_id: Option<Uuid>,
    source_output_key: Option<&str>,
) -> RuntimeEventPayload {
    let mut presentation = Map::new();
    presentation.insert(
        "kind".to_string(),
        Value::String(ANSWER_PRESENTATION_KIND.to_string()),
    );
    presentation.insert(
        "answer_node_id".to_string(),
        Value::String(answer_node_id.to_string()),
    );
    presentation.insert("segment_index".to_string(), json!(segment_index));
    if let Some(source_node_id) = source_node_id {
        presentation.insert(
            "source_node_id".to_string(),
            Value::String(source_node_id.to_string()),
        );
    }
    if let Some(source_node_run_id) = source_node_run_id {
        presentation.insert(
            "source_node_run_id".to_string(),
            Value::String(source_node_run_id.to_string()),
        );
    }
    if let Some(source_output_key) = source_output_key {
        presentation.insert(
            "source_output_key".to_string(),
            Value::String(source_output_key.to_string()),
        );
    }

    RuntimeEventPayload {
        event_type: event_type.to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: true,
        trace_visible: false,
        payload: json!({
            "type": event_type,
            "node_id": answer_node_id,
            "text": text,
            "presentation": Value::Object(presentation),
        }),
    }
}

pub fn heartbeat() -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "heartbeat".to_string(),
        source: RuntimeEventSource::System,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: false,
        trace_visible: false,
        payload: json!({ "type": "heartbeat" }),
    }
}
