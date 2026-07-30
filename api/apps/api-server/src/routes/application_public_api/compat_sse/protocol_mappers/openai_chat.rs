use super::*;
use crate::routes::application_public_api::compat_sse::event_forwarding::is_answer_presentation_delta;

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenAiChatStreamState {
    Streaming,
    Terminal,
}

pub(in crate::routes::application_public_api::compat_sse) struct OpenAiChatStreamMapper {
    model: String,
    chat_completion_id: String,
    state: OpenAiChatStreamState,
}

impl OpenAiChatStreamMapper {
    pub(in crate::routes::application_public_api::compat_sse) fn new(
        model: String,
        chat_completion_id: String,
    ) -> Self {
        Self {
            model,
            chat_completion_id,
            state: OpenAiChatStreamState::Streaming,
        }
    }

    pub(in crate::routes::application_public_api::compat_sse) fn runtime_event_to_sse(
        &mut self,
        initial_run: &NativeRunResult,
        event: impl Into<CompatibleRuntimeEventView>,
    ) -> Vec<Result<Event, Infallible>> {
        if self.state == OpenAiChatStreamState::Terminal {
            return Vec::new();
        }

        let envelope = event.into().into_envelope();
        match envelope.event_type.as_str() {
            "flow_started" => vec![json_sse(json!({
                "id": self.chat_completion_id,
                "object": "chat.completion.chunk",
                "created": initial_run.created_at.unix_timestamp(),
                "model": self.model,
                "choices": [{
                    "index": 0,
                    "delta": { "role": "assistant" },
                    "finish_reason": null
                }]
            }))],
            "text_delta" | "reasoning_delta" if is_answer_presentation_delta(&envelope) => {
                openai_delta_chunk_payload(
                    initial_run,
                    &self.model,
                    &self.chat_completion_id,
                    envelope.event_type.as_str(),
                    envelope.text.unwrap_or_default(),
                )
                .map(json_sse)
                .into_iter()
                .collect()
            }
            "text_delta" | "reasoning_delta" => Vec::new(),
            "flow_finished" => self.finish(initial_run, "stop"),
            "flow_incomplete" => self.finish(initial_run, "length"),
            "flow_failed" => {
                self.state = OpenAiChatStreamState::Terminal;
                vec![json_sse(json!({
                    "error": {
                        "message": canonical_runtime_error_message(initial_run),
                        "type": "server_error",
                        "param": null,
                        "code": canonical_runtime_error_code(initial_run)
                    }
                }))]
            }
            "flow_cancelled" => {
                self.state = OpenAiChatStreamState::Terminal;
                vec![json_sse(json!({
                    "error": {
                        "message": "published run cancelled",
                        "type": "invalid_request_error",
                        "param": null,
                        "code": "run_cancelled"
                    }
                }))]
            }
            "waiting_callback" => {
                self.state = OpenAiChatStreamState::Terminal;
                if let Some(payload) = openai_tool_call_chunk_payload(
                    initial_run,
                    &self.model,
                    &self.chat_completion_id,
                    &envelope.payload,
                ) {
                    vec![
                        json_sse(payload),
                        json_sse(openai_finish_chunk_payload(
                            initial_run,
                            &self.model,
                            &self.chat_completion_id,
                            "tool_calls",
                        )),
                        done_sse(),
                    ]
                } else {
                    required_action_not_supported_sse()
                }
            }
            "waiting_human" => {
                self.state = OpenAiChatStreamState::Terminal;
                required_action_not_supported_sse()
            }
            _ => Vec::new(),
        }
    }

    fn finish(
        &mut self,
        initial_run: &NativeRunResult,
        finish_reason: &'static str,
    ) -> Vec<Result<Event, Infallible>> {
        self.state = OpenAiChatStreamState::Terminal;
        vec![
            json_sse(openai_finish_chunk_payload(
                initial_run,
                &self.model,
                &self.chat_completion_id,
                finish_reason,
            )),
            done_sse(),
        ]
    }
}

pub(in crate::routes::application_public_api::compat_sse) fn openai_delta_chunk_payload(
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

pub(in crate::routes::application_public_api::compat_sse) fn openai_tool_call_chunk_payload(
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

pub(in crate::routes::application_public_api::compat_sse) fn openai_finish_chunk_payload(
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

fn required_action_not_supported_sse() -> Vec<Result<Event, Infallible>> {
    vec![json_sse(json!({
        "error": {
            "message": "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs",
            "type": "invalid_request_error",
            "param": null,
            "code": "required_action_not_supported"
        }
    }))]
}
