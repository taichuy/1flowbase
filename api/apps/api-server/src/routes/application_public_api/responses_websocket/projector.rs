use control_plane::{
    application_public_api::{
        compat::openai::response_id_from_run_id,
        native::{NativeRunResult, NativeUsage},
    },
    orchestration_runtime::debug_stream_events,
    ports::RuntimeEventEnvelope,
};
use serde_json::{json, Value};
use thiserror::Error;

use crate::routes::application_public_api::{
    llm_tool_visibility::external_llm_tool_call_values,
    tool_callback_ids::encode_openai_callback_tool_call_id,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ResponsesWebSocketProjectionError {
    #[error("Responses WebSocket sequence number overflowed")]
    SequenceOverflow,
    #[error("Responses WebSocket event could not be serialized")]
    SerializationFailed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputItemKind {
    Reasoning,
    Message,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProjectionState {
    Initial,
    Streaming,
    Terminal,
}

/// Projects ordered typed runtime facts onto one Responses WebSocket turn.
pub(crate) struct ResponsesWebSocketProjector {
    model: String,
    previous_response_id: Option<String>,
    active_output_item: Option<OutputItemKind>,
    active_output_item_text: String,
    completed_output_items: Vec<Value>,
    output_item_index: usize,
    next_sequence_number: u64,
    state: ProjectionState,
}

impl ResponsesWebSocketProjector {
    pub(crate) fn new(model: String, previous_response_id: Option<String>) -> Self {
        Self {
            model,
            previous_response_id,
            active_output_item: None,
            active_output_item_text: String::new(),
            completed_output_items: Vec::new(),
            output_item_index: 0,
            next_sequence_number: 0,
            state: ProjectionState::Initial,
        }
    }

    pub(crate) fn project(
        &mut self,
        run: &NativeRunResult,
        envelope: RuntimeEventEnvelope,
    ) -> Result<Vec<String>, ResponsesWebSocketProjectionError> {
        if self.state == ProjectionState::Terminal || envelope.event_type == "provider_native_event"
        {
            return Ok(Vec::new());
        }

        let terminal = is_terminal_event(&envelope.event_type);
        let mut events = Vec::new();
        match envelope.event_type.as_str() {
            "flow_started" if self.state == ProjectionState::Initial => {
                self.state = ProjectionState::Streaming;
                events.push(json!({
                    "type": "response.created",
                    "response": response_snapshot(
                        run,
                        &self.model,
                        self.previous_response_id.as_deref(),
                        "in_progress"
                    )
                }));
            }
            "flow_started" => {}
            "reasoning_delta" if is_answer_presentation_delta(&envelope) => {
                self.begin_streaming();
                self.open_output_item(run, OutputItemKind::Reasoning, &mut events);
                let delta = envelope.text.unwrap_or_default();
                self.active_output_item_text.push_str(&delta);
                events.push(json!({
                    "type": "response.reasoning_text.delta",
                    "response_id": response_id_from_run_id(run.id),
                    "item_id": format!("rs_{}", run.id),
                    "output_index": self.output_item_index,
                    "content_index": 0,
                    "delta": delta
                }));
            }
            "text_delta" if is_answer_presentation_delta(&envelope) => {
                self.begin_streaming();
                self.open_output_item(run, OutputItemKind::Message, &mut events);
                let delta = envelope.text.unwrap_or_default();
                self.active_output_item_text.push_str(&delta);
                events.push(json!({
                    "type": "response.output_text.delta",
                    "response_id": response_id_from_run_id(run.id),
                    "item_id": format!("msg_{}", run.id),
                    "output_index": self.output_item_index,
                    "content_index": 0,
                    "delta": delta
                }));
            }
            "text_delta" | "reasoning_delta" => {}
            _ if terminal => {
                self.begin_streaming();
                self.close_output_item(run, &mut events);
                events.extend(self.terminal_events(run, envelope));
            }
            _ => self.begin_streaming(),
        }

        let frames = self.serialize(events)?;
        if terminal {
            self.state = ProjectionState::Terminal;
        }
        Ok(frames)
    }

    pub(crate) fn has_terminal(&self) -> bool {
        self.state == ProjectionState::Terminal
    }

    fn begin_streaming(&mut self) {
        if self.state == ProjectionState::Initial {
            self.state = ProjectionState::Streaming;
        }
    }

    fn open_output_item(
        &mut self,
        run: &NativeRunResult,
        kind: OutputItemKind,
        events: &mut Vec<Value>,
    ) {
        if self.active_output_item == Some(kind) {
            return;
        }
        self.close_output_item(run, events);
        events.push(json!({
            "type": "response.output_item.added",
            "response_id": response_id_from_run_id(run.id),
            "output_index": self.output_item_index,
            "item": output_item_payload(run, kind, None)
        }));
        self.active_output_item = Some(kind);
        self.active_output_item_text.clear();
    }

    fn close_output_item(&mut self, run: &NativeRunResult, events: &mut Vec<Value>) {
        let Some(kind) = self.active_output_item.take() else {
            return;
        };
        let text = std::mem::take(&mut self.active_output_item_text);
        let item = output_item_payload(run, kind, Some(text));
        events.push(json!({
            "type": "response.output_item.done",
            "response_id": response_id_from_run_id(run.id),
            "output_index": self.output_item_index,
            "item": item.clone()
        }));
        self.completed_output_items.push(item);
        self.output_item_index += 1;
    }

    fn terminal_events(
        &mut self,
        run: &NativeRunResult,
        envelope: RuntimeEventEnvelope,
    ) -> Vec<Value> {
        match envelope.event_type.as_str() {
            "flow_finished" => vec![json!({
                "type": "response.completed",
                "response": completed_response_snapshot_with_output(
                    run,
                    &self.model,
                    self.previous_response_id.as_deref(),
                    self.completed_output_items.clone()
                )
            })],
            "flow_incomplete" => vec![json!({
                "type": "response.incomplete",
                "response": incomplete_response_snapshot_with_output(
                    run,
                    &self.model,
                    self.previous_response_id.as_deref(),
                    self.completed_output_items.clone()
                )
            })],
            "flow_failed" => vec![failed_response_event(
                run,
                &self.model,
                self.previous_response_id.as_deref(),
                run.error
                    .as_ref()
                    .map(|error| error.message.as_str())
                    .unwrap_or("published run failed"),
                "server_error",
                run.error
                    .as_ref()
                    .map(|error| error.code.as_str())
                    .unwrap_or("runtime_error"),
            )],
            "flow_cancelled" => vec![json!({
                "type": "response.cancelled",
                "response": response_snapshot(
                    run,
                    &self.model,
                    self.previous_response_id.as_deref(),
                    "cancelled"
                )
            })],
            "waiting_callback" => self.waiting_callback_events(run, &envelope.payload),
            "waiting_human" => vec![failed_response_event(
                run,
                &self.model,
                self.previous_response_id.as_deref(),
                "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs",
                "invalid_request_error",
                "required_action_not_supported",
            )],
            _ => Vec::new(),
        }
    }

    fn waiting_callback_events(&mut self, run: &NativeRunResult, payload: &Value) -> Vec<Value> {
        let Some(output) = function_call_output_items(payload) else {
            return vec![failed_response_event(
                run,
                &self.model,
                self.previous_response_id.as_deref(),
                "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs",
                "invalid_request_error",
                "required_action_not_supported",
            )];
        };
        let mut events = Vec::with_capacity(output.len() * 2 + 1);
        for item in &output {
            events.push(json!({
                "type": "response.output_item.added",
                "response_id": response_id_from_run_id(run.id),
                "output_index": self.output_item_index,
                "item": item
            }));
            events.push(json!({
                "type": "response.output_item.done",
                "response_id": response_id_from_run_id(run.id),
                "output_index": self.output_item_index,
                "item": item
            }));
            self.completed_output_items.push(item.clone());
            self.output_item_index += 1;
        }
        events.push(json!({
            "type": "response.completed",
            "response": completed_response_snapshot_with_output(
                run,
                &self.model,
                self.previous_response_id.as_deref(),
                self.completed_output_items.clone()
            )
        }));
        events
    }

    fn serialize(
        &mut self,
        events: Vec<Value>,
    ) -> Result<Vec<String>, ResponsesWebSocketProjectionError> {
        let mut frames = Vec::with_capacity(events.len());
        for mut event in events {
            let sequence_number = self.next_sequence_number;
            self.next_sequence_number = sequence_number
                .checked_add(1)
                .ok_or(ResponsesWebSocketProjectionError::SequenceOverflow)?;
            if let Some(object) = event.as_object_mut() {
                object.insert("sequence_number".to_string(), Value::from(sequence_number));
            }
            frames.push(
                serde_json::to_string(&event)
                    .map_err(|_| ResponsesWebSocketProjectionError::SerializationFailed)?,
            );
        }
        Ok(frames)
    }
}

fn is_terminal_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "flow_finished"
            | "flow_incomplete"
            | "flow_failed"
            | "flow_cancelled"
            | "waiting_human"
            | "waiting_callback"
    )
}

fn is_answer_presentation_delta(envelope: &RuntimeEventEnvelope) -> bool {
    debug_stream_events::is_answer_presentation_delta_payload(&envelope.payload)
}

fn output_item_payload(run: &NativeRunResult, kind: OutputItemKind, text: Option<String>) -> Value {
    match kind {
        OutputItemKind::Reasoning => json!({
            "type": "reasoning",
            "id": format!("rs_{}", run.id),
            "summary": [],
            "content": text
                .map(|text| json!([{ "type": "reasoning_text", "text": text }]))
                .unwrap_or_else(|| json!([])),
            "encrypted_content": null
        }),
        OutputItemKind::Message => json!({
            "type": "message",
            "id": format!("msg_{}", run.id),
            "role": "assistant",
            "content": text
                .map(|text| json!([{ "type": "output_text", "text": text }]))
                .unwrap_or_else(|| json!([]))
        }),
    }
}

fn response_snapshot(
    run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
    status: &'static str,
) -> Value {
    json!({
        "id": response_id_from_run_id(run.id),
        "object": "response",
        "created_at": run.created_at.unix_timestamp(),
        "status": status,
        "model": model,
        "output": [],
        "output_text": "",
        "previous_response_id": previous_response_id
    })
}

fn completed_response_snapshot(
    run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
) -> Value {
    let mut response = response_snapshot(run, model, previous_response_id, "completed");
    response["usage"] = usage_payload(run.usage.as_ref());
    response
}

fn incomplete_response_snapshot_with_output(
    run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
    output: Vec<Value>,
) -> Value {
    let mut response = response_snapshot(run, model, previous_response_id, "incomplete");
    response["incomplete_details"] = json!({ "reason": "max_output_tokens" });
    response["usage"] = usage_payload(run.usage.as_ref());
    response["output"] = Value::Array(output);
    response
}

fn completed_response_snapshot_with_output(
    run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
    output: Vec<Value>,
) -> Value {
    let mut response = completed_response_snapshot(run, model, previous_response_id);
    response["output"] = Value::Array(output);
    response
}

fn usage_payload(usage: Option<&NativeUsage>) -> Value {
    let Some(usage) = usage else {
        return json!({
            "input_tokens": 0,
            "output_tokens": 0,
            "total_tokens": 0
        });
    };
    json!({
        "input_tokens": usage.prompt_tokens.unwrap_or_default(),
        "output_tokens": usage.completion_tokens.unwrap_or_default(),
        "total_tokens": usage.total_tokens.unwrap_or_default()
    })
}

fn failed_response_event(
    run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
    message: &str,
    error_type: &str,
    code: &str,
) -> Value {
    json!({
        "type": "response.failed",
        "response": response_snapshot(run, model, previous_response_id, "failed"),
        "error": {
            "message": message,
            "type": error_type,
            "param": null,
            "code": code
        }
    })
}

fn function_call_output_items(payload: &Value) -> Option<Vec<Value>> {
    let callback_task_id = callback_task_id(payload)?;
    let calls = tool_calls(payload)?;
    let output = calls
        .iter()
        .filter_map(|call| {
            let name = call.get("name").and_then(Value::as_str)?;
            let original_id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("tool_call");
            let arguments = call.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Some(json!({
                "id": format!("fc_{original_id}"),
                "type": "function_call",
                "call_id": encode_openai_callback_tool_call_id(callback_task_id, original_id),
                "name": name,
                "arguments": match arguments {
                    Value::String(value) => value,
                    value => value.to_string(),
                },
                "status": "completed"
            }))
        })
        .collect::<Vec<_>>();
    (!output.is_empty()).then_some(output)
}

fn callback_task_id(payload: &Value) -> Option<uuid::Uuid> {
    if payload.get("callback_kind").and_then(Value::as_str) != Some("llm_tool_calls") {
        return None;
    }
    payload
        .get("callback_task_id")
        .and_then(Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
}

fn tool_calls(payload: &Value) -> Option<Vec<&Value>> {
    let calls = payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .or_else(|| {
            payload
                .get("request_payload")
                .and_then(|request| request.get("tool_calls"))
                .and_then(Value::as_array)
        })
        .or_else(|| {
            payload
                .get("required_action")
                .and_then(|action| action.get("payload"))
                .and_then(|action_payload| action_payload.get("tool_calls"))
                .and_then(Value::as_array)
        })?;
    external_llm_tool_call_values(calls)
}
