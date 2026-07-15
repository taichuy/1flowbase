use super::event_forwarding::is_answer_presentation_delta;
use super::*;
use crate::routes::application_public_api::llm_tool_visibility::{
    external_llm_tool_call_values, payload_has_only_internal_llm_tool_calls,
};

pub(super) struct OpenAiChatStreamMapper {
    model: String,
    chat_completion_id: String,
    terminal_answer_fallback: bool,
    emitted_reasoning_delta: bool,
    emitted_text_delta: bool,
}

impl OpenAiChatStreamMapper {
    pub(super) fn new(
        model: String,
        chat_completion_id: String,
        terminal_answer_fallback: bool,
    ) -> Self {
        Self {
            model,
            chat_completion_id,
            terminal_answer_fallback,
            emitted_reasoning_delta: false,
            emitted_text_delta: false,
        }
    }

    pub(super) fn runtime_event_to_sse(
        &mut self,
        initial_run: &NativeRunResult,
        envelope: RuntimeEventEnvelope,
    ) -> Vec<Result<Event, Infallible>> {
        let is_answer_presentation_delta = is_answer_presentation_delta(&envelope);
        match envelope.event_type.as_str() {
            "reasoning_delta"
                if is_answer_presentation_delta
                    && envelope
                        .text
                        .as_deref()
                        .is_some_and(|text| !text.is_empty()) =>
            {
                self.emitted_reasoning_delta = true;
            }
            "text_delta"
                if is_answer_presentation_delta
                    && envelope
                        .text
                        .as_deref()
                        .is_some_and(|text| !text.is_empty()) =>
            {
                self.emitted_text_delta = true;
            }
            _ => {}
        }

        let terminal_deltas = if self.terminal_answer_fallback
            && matches!(
                envelope.event_type.as_str(),
                "flow_finished" | "flow_failed"
            ) {
            terminal_answer_deltas_from_run_or_payload(initial_run, &envelope.payload)
        } else {
            Vec::new()
        };

        let mut events = Vec::new();
        let had_reasoning_delta = self.emitted_reasoning_delta;
        let had_text_delta = self.emitted_text_delta;
        for delta in terminal_deltas {
            match delta.kind {
                TerminalAnswerDeltaKind::Reasoning if !had_reasoning_delta => {
                    if let Some(payload) = openai_delta_chunk_payload(
                        initial_run,
                        &self.model,
                        &self.chat_completion_id,
                        "reasoning_delta",
                        delta.text,
                    ) {
                        events.push(json_sse(payload));
                        self.emitted_reasoning_delta = true;
                    }
                }
                TerminalAnswerDeltaKind::Text if !had_text_delta => {
                    if let Some(payload) = openai_delta_chunk_payload(
                        initial_run,
                        &self.model,
                        &self.chat_completion_id,
                        "text_delta",
                        delta.text,
                    ) {
                        events.push(json_sse(payload));
                        self.emitted_text_delta = true;
                    }
                }
                _ => {}
            }
        }
        events.extend(openai_runtime_event_to_sse(
            initial_run,
            &self.model,
            &self.chat_completion_id,
            envelope,
        ));
        events
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenAiResponseOutputItemKind {
    Reasoning,
    Message,
}

pub(super) struct OpenAiResponseStreamMapper {
    model: String,
    previous_response_id: Option<String>,
    terminal_answer_fallback: bool,
    emitted_reasoning_delta: bool,
    emitted_text_delta: bool,
    active_output_item: Option<OpenAiResponseOutputItemKind>,
    active_output_item_text: String,
    output_item_index: usize,
}

impl OpenAiResponseStreamMapper {
    pub(super) fn new(
        model: String,
        previous_response_id: Option<String>,
        terminal_answer_fallback: bool,
    ) -> Self {
        Self {
            model,
            previous_response_id,
            terminal_answer_fallback,
            emitted_reasoning_delta: false,
            emitted_text_delta: false,
            active_output_item: None,
            active_output_item_text: String::new(),
            output_item_index: 0,
        }
    }

    fn open_output_item(
        &mut self,
        initial_run: &NativeRunResult,
        kind: OpenAiResponseOutputItemKind,
        events: &mut Vec<Result<Event, Infallible>>,
    ) {
        if self.active_output_item == Some(kind) {
            return;
        }
        self.close_output_item(initial_run, events);
        events.push(event_json_sse(
            "response.output_item.added",
            json!({
                "type": "response.output_item.added",
                "response_id": response_id_from_run_id(initial_run.id),
                "output_index": self.output_item_index,
                "item": openai_response_output_item_payload(initial_run, kind, None)
            }),
        ));
        self.active_output_item = Some(kind);
        self.active_output_item_text = String::new();
    }

    fn close_output_item(
        &mut self,
        initial_run: &NativeRunResult,
        events: &mut Vec<Result<Event, Infallible>>,
    ) {
        let Some(kind) = self.active_output_item.take() else {
            return;
        };
        let text = std::mem::take(&mut self.active_output_item_text);
        events.push(event_json_sse(
            "response.output_item.done",
            json!({
                "type": "response.output_item.done",
                "response_id": response_id_from_run_id(initial_run.id),
                "output_index": self.output_item_index,
                "item": openai_response_output_item_payload(initial_run, kind, Some(text))
            }),
        ));
        self.output_item_index += 1;
    }

    pub(super) fn runtime_event_to_sse(
        &mut self,
        initial_run: &NativeRunResult,
        envelope: RuntimeEventEnvelope,
    ) -> Vec<Result<Event, Infallible>> {
        let is_answer_presentation_delta = is_answer_presentation_delta(&envelope);
        let mut events = Vec::new();
        match envelope.event_type.as_str() {
            "reasoning_delta" if is_answer_presentation_delta => {
                self.open_output_item(
                    initial_run,
                    OpenAiResponseOutputItemKind::Reasoning,
                    &mut events,
                );
                if let Some(text) = envelope.text.as_deref().filter(|text| !text.is_empty()) {
                    self.active_output_item_text.push_str(text);
                    self.emitted_reasoning_delta = true;
                }
            }
            "text_delta" if is_answer_presentation_delta => {
                self.open_output_item(
                    initial_run,
                    OpenAiResponseOutputItemKind::Message,
                    &mut events,
                );
                if let Some(text) = envelope.text.as_deref().filter(|text| !text.is_empty()) {
                    self.active_output_item_text.push_str(text);
                    self.emitted_text_delta = true;
                }
            }
            _ => {}
        }

        let terminal_deltas = if self.terminal_answer_fallback
            && matches!(
                envelope.event_type.as_str(),
                "flow_finished" | "flow_failed"
            ) {
            terminal_answer_deltas_from_run_or_payload(initial_run, &envelope.payload)
        } else {
            Vec::new()
        };

        let had_reasoning_delta = self.emitted_reasoning_delta;
        let had_text_delta = self.emitted_text_delta;
        for delta in terminal_deltas {
            match delta.kind {
                TerminalAnswerDeltaKind::Reasoning if !had_reasoning_delta => {
                    self.open_output_item(
                        initial_run,
                        OpenAiResponseOutputItemKind::Reasoning,
                        &mut events,
                    );
                    self.active_output_item_text.push_str(&delta.text);
                    events.push(event_json_sse(
                        "response.reasoning_text.delta",
                        openai_response_reasoning_text_delta_payload(initial_run, delta.text),
                    ));
                    self.emitted_reasoning_delta = true;
                }
                TerminalAnswerDeltaKind::Text if !had_text_delta => {
                    self.open_output_item(
                        initial_run,
                        OpenAiResponseOutputItemKind::Message,
                        &mut events,
                    );
                    self.active_output_item_text.push_str(&delta.text);
                    events.push(event_json_sse(
                        "response.output_text.delta",
                        openai_response_output_text_delta_payload(initial_run, delta.text),
                    ));
                    self.emitted_text_delta = true;
                }
                _ => {}
            }
        }
        if matches!(
            envelope.event_type.as_str(),
            "flow_finished" | "flow_failed" | "flow_cancelled" | "waiting_callback"
        ) {
            self.close_output_item(initial_run, &mut events);
        }
        events.extend(openai_response_runtime_event_to_sse(
            initial_run,
            &self.model,
            self.previous_response_id.as_deref(),
            envelope,
        ));
        events
    }
}

fn openai_response_output_item_payload(
    initial_run: &NativeRunResult,
    kind: OpenAiResponseOutputItemKind,
    text: Option<String>,
) -> Value {
    match kind {
        OpenAiResponseOutputItemKind::Reasoning => json!({
            "type": "reasoning",
            "id": format!("rs_{}", initial_run.id),
            "summary": [],
            "content": text
                .map(|text| json!([{ "type": "reasoning_text", "text": text }]))
                .unwrap_or_else(|| json!([])),
            "encrypted_content": null
        }),
        OpenAiResponseOutputItemKind::Message => json!({
            "type": "message",
            "id": format!("msg_{}", initial_run.id),
            "role": "assistant",
            "content": text
                .map(|text| json!([{ "type": "output_text", "text": text }]))
                .unwrap_or_else(|| json!([]))
        }),
    }
}

fn terminal_answer_text(run: &NativeRunResult, payload: &Value) -> Option<String> {
    terminal_answer_text_from_payload(payload).or_else(|| {
        run.answer
            .as_ref()
            .filter(|answer| !answer.is_empty())
            .cloned()
    })
}

pub(super) fn terminal_answer_deltas_from_run_or_payload(
    run: &NativeRunResult,
    payload: &Value,
) -> Vec<TerminalAnswerDelta> {
    let payload_deltas = terminal_answer_deltas_from_payload(payload);
    if !payload_deltas.is_empty() {
        return payload_deltas;
    }

    if let Some(answer_segments) = run
        .answer_segments
        .as_ref()
        .filter(|segments| !segments.is_empty())
    {
        return terminal_answer_deltas_from_payload(&json!({
            "answer_segments": answer_segments
        }));
    }

    terminal_answer_text(run, payload)
        .map(|answer| terminal_answer_deltas_from_payload(&json!({ "answer": answer })))
        .unwrap_or_default()
}

fn openai_runtime_event_to_sse(
    initial_run: &NativeRunResult,
    model: &str,
    chat_completion_id: &str,
    envelope: RuntimeEventEnvelope,
) -> Vec<Result<Event, Infallible>> {
    match envelope.event_type.as_str() {
        "flow_started" => vec![json_sse(json!({
            "id": chat_completion_id,
            "object": "chat.completion.chunk",
            "created": initial_run.created_at.unix_timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "delta": { "role": "assistant" },
                "finish_reason": null
            }]
        }))],
        "text_delta" | "reasoning_delta" if is_answer_presentation_delta(&envelope) => {
            openai_delta_chunk_payload(
                initial_run,
                model,
                chat_completion_id,
                envelope.event_type.as_str(),
                envelope.text.unwrap_or_default(),
            )
            .map(json_sse)
            .into_iter()
            .collect()
        }
        "text_delta" | "reasoning_delta" => Vec::new(),
        "flow_finished" => vec![
            json_sse(openai_finish_chunk_payload(
                initial_run,
                model,
                chat_completion_id,
                "stop",
            )),
            done_sse(),
        ],
        "flow_failed" => vec![
            if terminal_answer_text(initial_run, &envelope.payload).is_some() {
                json_sse(openai_finish_chunk_payload(
                    initial_run,
                    model,
                    chat_completion_id,
                    "stop",
                ))
            } else {
                json_sse(json!({
                    "error": {
                        "message": runtime_error_message(&envelope.payload),
                        "type": "server_error",
                        "param": null,
                        "code": "runtime_error"
                    }
                }))
            },
            done_sse(),
        ],
        "flow_cancelled" => vec![done_sse()],
        "waiting_callback" => {
            if let Some(payload) = openai_tool_call_chunk_payload(
                initial_run,
                model,
                chat_completion_id,
                &envelope.payload,
            ) {
                vec![
                    json_sse(payload),
                    json_sse(openai_finish_chunk_payload(
                        initial_run,
                        model,
                        chat_completion_id,
                        "tool_calls",
                    )),
                    done_sse(),
                ]
            } else if payload_has_only_internal_llm_tool_calls(&envelope.payload) {
                vec![
                    json_sse(openai_finish_chunk_payload(
                        initial_run,
                        model,
                        chat_completion_id,
                        "stop",
                    )),
                    done_sse(),
                ]
            } else {
                required_action_not_supported_openai_sse()
            }
        }
        "waiting_human" => required_action_not_supported_openai_sse(),
        _ => Vec::new(),
    }
}

fn openai_response_runtime_event_to_sse(
    initial_run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
    envelope: RuntimeEventEnvelope,
) -> Vec<Result<Event, Infallible>> {
    match envelope.event_type.as_str() {
        "flow_started" => vec![event_json_sse(
            "response.created",
            json!({
                "type": "response.created",
                "response": openai_response_stream_snapshot(
                    initial_run,
                    model,
                    previous_response_id,
                    "in_progress"
                )
            }),
        )],
        "text_delta" if is_answer_presentation_delta(&envelope) => vec![event_json_sse(
            "response.output_text.delta",
            openai_response_output_text_delta_payload(
                initial_run,
                envelope.text.unwrap_or_default(),
            ),
        )],
        "reasoning_delta" if is_answer_presentation_delta(&envelope) => vec![event_json_sse(
            "response.reasoning_text.delta",
            json!({
                "type": "response.reasoning_text.delta",
                "response_id": response_id_from_run_id(initial_run.id),
                "item_id": format!("msg_{}", initial_run.id),
                "output_index": 0,
                "content_index": 0,
                "delta": envelope.text.unwrap_or_default()
            }),
        )],
        "text_delta" | "reasoning_delta" => Vec::new(),
        "flow_finished" => vec![event_json_sse(
            "response.completed",
            json!({
                "type": "response.completed",
                "response": openai_response_completed_snapshot(
                    initial_run,
                    model,
                    previous_response_id
                )
            }),
        )],
        "flow_failed" => {
            if terminal_answer_text(initial_run, &envelope.payload).is_some() {
                vec![event_json_sse(
                    "response.completed",
                    json!({
                        "type": "response.completed",
                        "response": openai_response_completed_snapshot(
                            initial_run,
                            model,
                            previous_response_id
                        )
                    }),
                )]
            } else {
                vec![event_json_sse(
                    "response.failed",
                    json!({
                        "type": "response.failed",
                        "response": openai_response_stream_snapshot(
                            initial_run,
                            model,
                            previous_response_id,
                            "failed"
                        ),
                        "error": {
                            "message": runtime_error_message(&envelope.payload),
                            "type": "server_error",
                            "param": null,
                            "code": "runtime_error"
                        }
                    }),
                )]
            }
        }
        "flow_cancelled" => vec![event_json_sse(
            "response.failed",
            json!({
                "type": "response.failed",
                "response": openai_response_stream_snapshot(
                    initial_run,
                    model,
                    previous_response_id,
                    "failed"
                ),
                "error": {
                    "message": "published run cancelled",
                    "type": "invalid_request_error",
                    "param": null,
                    "code": "run_cancelled"
                }
            }),
        )],
        "waiting_callback" => {
            if let Some(items) = openai_response_function_call_output_items(&envelope.payload) {
                openai_response_function_call_sse(initial_run, model, previous_response_id, items)
            } else if payload_has_only_internal_llm_tool_calls(&envelope.payload) {
                vec![event_json_sse(
                    "response.completed",
                    json!({
                        "type": "response.completed",
                        "response": openai_response_completed_snapshot(
                            initial_run,
                            model,
                            previous_response_id
                        )
                    }),
                )]
            } else {
                required_action_not_supported_openai_response_sse(
                    initial_run,
                    model,
                    previous_response_id,
                )
            }
        }
        "waiting_human" => required_action_not_supported_openai_response_sse(
            initial_run,
            model,
            previous_response_id,
        ),
        _ => Vec::new(),
    }
}

fn openai_response_stream_snapshot(
    initial_run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
    status: &'static str,
) -> Value {
    json!({
        "id": response_id_from_run_id(initial_run.id),
        "object": "response",
        "created_at": initial_run.created_at.unix_timestamp(),
        "status": status,
        "model": model,
        "output": [],
        "output_text": "",
        "previous_response_id": previous_response_id
    })
}

fn openai_response_completed_snapshot(
    initial_run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
) -> Value {
    let mut response =
        openai_response_stream_snapshot(initial_run, model, previous_response_id, "completed");
    response["usage"] = openai_responses_usage_payload(initial_run.usage.as_ref());
    response
}

fn openai_responses_usage_payload(usage: Option<&NativeUsage>) -> Value {
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

fn openai_response_output_text_delta_payload(initial_run: &NativeRunResult, text: String) -> Value {
    json!({
        "type": "response.output_text.delta",
        "response_id": response_id_from_run_id(initial_run.id),
        "item_id": format!("msg_{}", initial_run.id),
        "output_index": 0,
        "content_index": 0,
        "delta": text
    })
}

fn openai_response_reasoning_text_delta_payload(
    initial_run: &NativeRunResult,
    text: String,
) -> Value {
    json!({
        "type": "response.reasoning_text.delta",
        "response_id": response_id_from_run_id(initial_run.id),
        "item_id": format!("msg_{}", initial_run.id),
        "output_index": 0,
        "content_index": 0,
        "delta": text
    })
}

pub(super) fn openai_delta_chunk_payload(
    initial_run: &NativeRunResult,
    model: &str,
    chat_completion_id: &str,
    event_type: &str,
    text: String,
) -> Option<Value> {
    let delta = match event_type {
        "text_delta" => json!({ "content": text }),
        "reasoning_delta" => json!({ "reasoning_content": text }),
        _ => return None,
    };

    Some(json!({
        "id": chat_completion_id,
        "object": "chat.completion.chunk",
        "created": initial_run.created_at.unix_timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": delta,
            "finish_reason": null
        }]
    }))
}

pub(super) fn openai_tool_call_chunk_payload(
    initial_run: &NativeRunResult,
    model: &str,
    chat_completion_id: &str,
    payload: &Value,
) -> Option<Value> {
    let callback_task_id = llm_tool_callback_task_id(payload)?;
    let calls = llm_tool_calls(payload)?;
    let tool_calls = calls
        .iter()
        .enumerate()
        .filter_map(|(index, call)| {
            let name = call.get("name").and_then(Value::as_str)?;
            let original_id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("tool_call")
                .to_string();
            let arguments = call.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Some(json!({
                "index": index,
                "id": encode_openai_callback_tool_call_id(callback_task_id, &original_id),
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": tool_call_arguments_string(arguments)
                }
            }))
        })
        .collect::<Vec<_>>();
    if tool_calls.is_empty() {
        return None;
    }

    Some(json!({
        "id": chat_completion_id,
        "object": "chat.completion.chunk",
        "created": initial_run.created_at.unix_timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": { "tool_calls": tool_calls },
            "finish_reason": null
        }]
    }))
}

pub(super) fn openai_finish_chunk_payload(
    initial_run: &NativeRunResult,
    model: &str,
    chat_completion_id: &str,
    finish_reason: &'static str,
) -> Value {
    json!({
        "id": chat_completion_id,
        "object": "chat.completion.chunk",
        "created": initial_run.created_at.unix_timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "content": "",
                "role": null
            },
            "finish_reason": finish_reason
        }],
        "usage": openai_chat_usage_payload(initial_run.usage.as_ref())
    })
}

fn openai_chat_usage_payload(usage: Option<&NativeUsage>) -> Value {
    let Some(usage) = usage else {
        return json!({
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0
        });
    };

    json!({
        "prompt_tokens": usage.prompt_tokens.unwrap_or_default(),
        "completion_tokens": usage.completion_tokens.unwrap_or_default(),
        "total_tokens": usage.total_tokens.unwrap_or_default()
    })
}

fn anthropic_message_start_usage_payload(usage: Option<&NativeUsage>) -> Value {
    let Some(usage) = usage else {
        return json!({
            "input_tokens": 0,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0,
            "output_tokens": 0
        });
    };

    json!({
        "input_tokens": usage.prompt_tokens.unwrap_or_default(),
        "cache_creation_input_tokens": usage.cache_write_tokens.unwrap_or_default(),
        "cache_read_input_tokens": anthropic_cache_read_input_tokens(usage),
        "output_tokens": 0
    })
}

fn anthropic_message_delta_usage_payload(usage: Option<&NativeUsage>) -> Value {
    let Some(usage) = usage else {
        return json!({
            "input_tokens": 0,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0,
            "output_tokens": 0
        });
    };

    json!({
        "input_tokens": usage.prompt_tokens.unwrap_or_default(),
        "cache_creation_input_tokens": usage.cache_write_tokens.unwrap_or_default(),
        "cache_read_input_tokens": anthropic_cache_read_input_tokens(usage),
        "output_tokens": usage.completion_tokens.unwrap_or_default()
    })
}

fn anthropic_cache_read_input_tokens(usage: &NativeUsage) -> u64 {
    usage
        .cache_read_tokens
        .or(usage.input_cache_hit_tokens)
        .unwrap_or_default()
}

pub(super) fn openai_response_function_call_output_items(payload: &Value) -> Option<Vec<Value>> {
    let callback_task_id = llm_tool_callback_task_id(payload)?;
    let calls = llm_tool_calls(payload)?;
    let output = calls
        .iter()
        .filter_map(|call| {
            let name = call.get("name").and_then(Value::as_str)?;
            let original_id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("tool_call")
                .to_string();
            let arguments = call.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Some(json!({
                "id": format!("fc_{}", original_id),
                "type": "function_call",
                "call_id": encode_openai_callback_tool_call_id(callback_task_id, &original_id),
                "name": name,
                "arguments": tool_call_arguments_string(arguments),
                "status": "completed"
            }))
        })
        .collect::<Vec<_>>();
    (!output.is_empty()).then_some(output)
}

fn openai_response_function_call_sse(
    initial_run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
    output: Vec<Value>,
) -> Vec<Result<Event, Infallible>> {
    let mut events = Vec::with_capacity(output.len() * 2 + 1);
    for (index, item) in output.iter().enumerate() {
        events.push(event_json_sse(
            "response.output_item.added",
            json!({
                "type": "response.output_item.added",
                "response_id": response_id_from_run_id(initial_run.id),
                "output_index": index,
                "item": item
            }),
        ));
        events.push(event_json_sse(
            "response.output_item.done",
            json!({
                "type": "response.output_item.done",
                "response_id": response_id_from_run_id(initial_run.id),
                "output_index": index,
                "item": item
            }),
        ));
    }
    events.push(event_json_sse(
        "response.completed",
        json!({
            "type": "response.completed",
            "response": openai_response_stream_snapshot_with_output(
                initial_run,
                model,
                previous_response_id,
                "completed",
                output
            )
        }),
    ));
    events
}

fn openai_response_stream_snapshot_with_output(
    initial_run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
    status: &'static str,
    output: Vec<Value>,
) -> Value {
    let mut response = json!({
        "id": response_id_from_run_id(initial_run.id),
        "object": "response",
        "created_at": initial_run.created_at.unix_timestamp(),
        "status": status,
        "model": model,
        "output": output,
        "output_text": "",
        "previous_response_id": previous_response_id
    });
    response["usage"] = openai_responses_usage_payload(initial_run.usage.as_ref());
    response
}

pub(super) fn anthropic_tool_use_blocks_from_waiting_payload(
    payload: &Value,
) -> Option<Vec<Value>> {
    let callback_task_id = llm_tool_callback_task_id(payload)?;
    let calls = llm_tool_calls(payload)?;
    let blocks = calls
        .iter()
        .filter_map(|call| {
            let name = call.get("name").and_then(Value::as_str)?;
            let original_id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("toolu_call")
                .to_string();
            let input = call.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Some(json!({
                "type": "tool_use",
                "id": encode_anthropic_callback_tool_use_id(callback_task_id, &original_id),
                "name": name,
                "input": input
            }))
        })
        .collect::<Vec<_>>();
    (!blocks.is_empty()).then_some(blocks)
}

pub(super) fn anthropic_completed_run_to_sse(
    run: &NativeRunResult,
    model: &str,
) -> Vec<Result<Event, Infallible>> {
    let mut mapper = AnthropicStreamMapper::new(model.to_string());
    let mut events = mapper.runtime_event_to_sse(
        run,
        RuntimeEventEnvelope::new(run.id, 0, debug_stream_events::flow_started(run.id)),
    );
    if let Some(payload) = waiting_payload_from_run(run) {
        if let Some(tool_events) = mapper.anthropic_tool_use_events(&payload, run.usage.as_ref()) {
            events.extend(tool_events);
            return events;
        }
    }
    for (index, delta) in terminal_answer_deltas_from_run_or_payload(run, &json!({}))
        .into_iter()
        .enumerate()
    {
        let event = terminal_answer_delta_to_runtime_event(run, index as i64 + 1, delta);
        events.extend(mapper.runtime_event_to_sse(run, event));
    }
    events.extend(mapper.anthropic_stop_events(run.usage.as_ref()));
    events
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

fn waiting_payload_from_run(run: &NativeRunResult) -> Option<Value> {
    let action = run.required_action.as_ref()?;
    Some(json!({
        "callback_kind": action.payload.get("callback_kind").cloned().unwrap_or(Value::Null),
        "callback_task_id": action.payload.get("callback_task_id").cloned().unwrap_or(Value::Null),
        "tool_calls": run.tool_calls.clone().unwrap_or(Value::Null),
    }))
}

fn llm_tool_callback_task_id(payload: &Value) -> Option<uuid::Uuid> {
    if payload.get("callback_kind").and_then(Value::as_str) != Some("llm_tool_calls") {
        return None;
    }
    payload
        .get("callback_task_id")
        .and_then(Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
}

fn llm_tool_calls(payload: &Value) -> Option<Vec<&Value>> {
    let calls = payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .or_else(|| {
            payload
                .get("request_payload")
                .and_then(|request| request.get("tool_calls"))
                .and_then(Value::as_array)
        })?;

    external_llm_tool_call_values(calls)
}

fn tool_call_arguments_string(arguments: Value) -> String {
    match arguments {
        Value::String(value) => value,
        value => value.to_string(),
    }
}

fn required_action_not_supported_openai_sse() -> Vec<Result<Event, Infallible>> {
    vec![
        json_sse(json!({
            "error": {
                "message": "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs",
                "type": "invalid_request_error",
                "param": null,
                "code": "required_action_not_supported"
            }
        })),
        done_sse(),
    ]
}

fn required_action_not_supported_openai_response_sse(
    initial_run: &NativeRunResult,
    model: &str,
    previous_response_id: Option<&str>,
) -> Vec<Result<Event, Infallible>> {
    vec![event_json_sse(
        "response.failed",
        json!({
            "type": "response.failed",
            "response": openai_response_stream_snapshot(
                initial_run,
                model,
                previous_response_id,
                "failed"
            ),
            "error": {
                "message": "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs",
                "type": "invalid_request_error",
                "param": null,
                "code": "required_action_not_supported"
            }
        }),
    )]
}

fn required_action_not_supported_anthropic_sse() -> Vec<Result<Event, Infallible>> {
    vec![event_json_sse(
        "error",
        json!({
            "type": "error",
            "error": {
                "type": "required_action_not_supported",
                "message": "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs"
            }
        }),
    )]
}

mod anthropic_stream;

#[cfg(test)]
pub(super) use anthropic_stream::anthropic_delta_payload;
pub(super) use anthropic_stream::AnthropicStreamMapper;

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
