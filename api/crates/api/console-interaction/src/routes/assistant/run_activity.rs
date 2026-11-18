use serde::Serialize;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

use crate::routes::debug_run_stream::RuntimeEventStreamEnvelopeResponse;

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantRunActivityPageResponse {
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub items: Vec<AssistantRunActivityItem>,
    pub trace_events: Vec<RuntimeEventStreamEnvelopeResponse>,
    pub has_more: bool,
    pub next_sequence: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssistantRunActivityItem {
    Reasoning {
        event_id: String,
        sequence_start: i64,
        sequence_end: i64,
        created_at: String,
        text: String,
    },
    Output {
        event_id: String,
        sequence_start: i64,
        sequence_end: i64,
        created_at: String,
        text: String,
        segment_index: Option<i64>,
    },
    Tool {
        event_id: String,
        sequence_start: i64,
        sequence_end: i64,
        created_at: String,
        tool_call_id: String,
        tool_name: String,
        input: Value,
        output: Option<Value>,
        duration_ms: Option<u64>,
        is_error: bool,
        status: AssistantRunToolStatus,
    },
    Error {
        event_id: String,
        sequence_start: i64,
        sequence_end: i64,
        created_at: String,
        error: String,
    },
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssistantRunToolStatus {
    Running,
    Succeeded,
    Failed,
}

pub(super) fn format_assistant_activity_time(value: time::OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap_or_else(|_| value.to_string())
}

pub(super) fn project_assistant_run_activity(
    event: &RuntimeEventStreamEnvelopeResponse,
) -> Option<AssistantRunActivityItem> {
    let common = || {
        (
            event.event_id.clone(),
            event
                .payload
                .get("sequence_start")
                .and_then(Value::as_i64)
                .unwrap_or(event.sequence),
            event
                .payload
                .get("sequence_end")
                .and_then(Value::as_i64)
                .unwrap_or(event.sequence),
            event.created_at.clone(),
        )
    };
    match event.event_type.as_str() {
        "reasoning_delta"
            if event
                .payload
                .pointer("/presentation/kind")
                .and_then(Value::as_str)
                == Some("answer") =>
        {
            let (event_id, sequence_start, sequence_end, created_at) = common();
            Some(AssistantRunActivityItem::Reasoning {
                event_id,
                sequence_start,
                sequence_end,
                created_at,
                text: event.text.clone()?,
            })
        }
        "text_delta"
            if event
                .payload
                .pointer("/presentation/kind")
                .and_then(Value::as_str)
                == Some("answer") =>
        {
            let (event_id, sequence_start, sequence_end, created_at) = common();
            Some(AssistantRunActivityItem::Output {
                event_id,
                sequence_start,
                sequence_end,
                created_at,
                text: event.text.clone()?,
                segment_index: event
                    .payload
                    .pointer("/presentation/segment_index")
                    .and_then(Value::as_i64),
            })
        }
        "assistant_tool_call_started" | "assistant_tool_call_finished" => {
            let tool_call = serde_json::from_value::<
                plugin_framework::provider_contract::ProviderToolCall,
            >(event.payload.get("tool_call")?.clone())
            .ok()?;
            let finished = event.event_type == "assistant_tool_call_finished";
            let output = finished
                .then(|| event.payload.get("tool_result").cloned())
                .flatten();
            let is_error = output
                .as_ref()
                .and_then(|value| value.get("is_error"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let (event_id, sequence_start, sequence_end, created_at) = common();
            Some(AssistantRunActivityItem::Tool {
                event_id,
                sequence_start,
                sequence_end,
                created_at,
                tool_call_id: tool_call.id,
                tool_name: tool_call.name,
                input: tool_call.arguments,
                output,
                duration_ms: finished
                    .then(|| event.payload.get("duration_ms").and_then(Value::as_u64))
                    .flatten(),
                is_error,
                status: if !finished {
                    AssistantRunToolStatus::Running
                } else if is_error {
                    AssistantRunToolStatus::Failed
                } else {
                    AssistantRunToolStatus::Succeeded
                },
            })
        }
        "flow_failed" => {
            let (event_id, sequence_start, sequence_end, created_at) = common();
            Some(AssistantRunActivityItem::Error {
                event_id,
                sequence_start,
                sequence_end,
                created_at,
                error: event
                    .payload
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("flow debug run failed")
                    .to_string(),
            })
        }
        _ => None,
    }
}
