use std::collections::BTreeSet;

use control_plane::application_public_api::callback_tool_ids::{
    decode_anthropic_callback_tool_use_id, decode_openai_callback_tool_call_id,
};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CorrelatedToolCallback {
    pub(crate) callback_task_id: Uuid,
    pub(crate) tool_results: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallbackCorrelationError {
    pub(crate) param: &'static str,
    pub(crate) message: &'static str,
}

impl CallbackCorrelationError {
    fn new(param: &'static str, message: &'static str) -> Self {
        Self { param, message }
    }
}

/// Recognizes only a complete tool-result turn paired with the immediately
/// preceding assistant tool-call turn. Historical markers followed by new
/// user text deliberately do not participate in callback admission.
pub(crate) fn correlate_openai_chat_callback(
    request: &Value,
) -> Result<Option<CorrelatedToolCallback>, CallbackCorrelationError> {
    let Some(messages) = request.get("messages").and_then(Value::as_array) else {
        return Ok(None);
    };
    let trailing_start = messages
        .iter()
        .rposition(|message| message.get("role").and_then(Value::as_str) != Some("tool"))
        .map_or(0, |index| index + 1);
    let trailing = &messages[trailing_start..];
    if trailing.is_empty() || !contains_encoded_openai_tool_result(trailing) {
        return Ok(None);
    }

    let assistant_ids = messages
        .get(trailing_start.saturating_sub(1))
        .filter(|_| trailing_start > 0)
        .filter(|message| message.get("role").and_then(Value::as_str) == Some("assistant"))
        .and_then(|message| message.get("tool_calls"))
        .and_then(Value::as_array)
        .map(openai_chat_assistant_tool_ids)
        .filter(|ids| !ids.is_empty())
        .ok_or_else(|| {
            CallbackCorrelationError::new(
                "messages",
                "callback tool results require the immediately preceding assistant tool calls",
            )
        })?;

    let results = trailing.iter().map(|message| {
        let external_id = message
            .get("tool_call_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CallbackCorrelationError::new(
                    "messages",
                    "callback tool result tool_call_id is required",
                )
            })?;
        let content = openai_tool_message_content(message);
        Ok((external_id, content))
    });
    correlate_openai_results(results, "messages", Some(&assistant_ids))
}

/// Responses callbacks are correlated by all three public markers: the prior
/// response cursor, the callback task encoded in each call_id, and the
/// original provider call id encoded alongside it.
pub(crate) fn correlate_openai_responses_callback(
    request: &Value,
    previous_response_id: Option<&str>,
) -> Result<Option<CorrelatedToolCallback>, CallbackCorrelationError> {
    let Some(items) = request.get("input").and_then(Value::as_array) else {
        return Ok(None);
    };
    let trailing_start = items
        .iter()
        .rposition(|item| item.get("type").and_then(Value::as_str) != Some("function_call_output"))
        .map_or(0, |index| index + 1);
    let trailing = &items[trailing_start..];
    if trailing.is_empty() || !contains_encoded_responses_tool_result(trailing) {
        return Ok(None);
    }
    previous_response_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            CallbackCorrelationError::new(
                "previous_response_id",
                "callback function_call_output requires previous_response_id",
            )
        })?;
    let results = trailing.iter().map(|item| {
        let external_id = item.get("call_id").and_then(Value::as_str).ok_or_else(|| {
            CallbackCorrelationError::new(
                "input",
                "callback function_call_output call_id is required",
            )
        })?;
        let output = match item.get("output") {
            Some(Value::String(output)) => output.clone(),
            Some(output) => output.to_string(),
            None => String::new(),
        };
        Ok((external_id, output))
    });
    correlate_openai_results(results, "input", None)
}

pub(crate) fn correlate_anthropic_callback(
    request: &Value,
) -> Result<Option<CorrelatedToolCallback>, CallbackCorrelationError> {
    let Some(messages) = request.get("messages").and_then(Value::as_array) else {
        return Ok(None);
    };
    let trailing_start = messages
        .iter()
        .rposition(|message| !anthropic_message_has_only_tool_results(message))
        .map_or(0, |index| index + 1);
    let trailing = &messages[trailing_start..];
    if trailing.is_empty() || !contains_encoded_anthropic_tool_result(trailing) {
        return Ok(None);
    }

    let assistant_ids = messages[..trailing_start]
        .iter()
        .rev()
        .take_while(|message| message.get("role").and_then(Value::as_str) == Some("assistant"))
        .flat_map(anthropic_assistant_tool_ids)
        .collect::<BTreeSet<_>>();
    if assistant_ids.is_empty() {
        return Err(CallbackCorrelationError::new(
            "messages",
            "callback tool results require the immediately preceding assistant tool calls",
        ));
    }

    let mut decoded = Vec::new();
    for message in trailing {
        let blocks = message
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                CallbackCorrelationError::new("messages", "tool_result content must be an array")
            })?;
        for block in blocks {
            let external_id = block
                .get("tool_use_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    CallbackCorrelationError::new(
                        "messages",
                        "callback tool_result tool_use_id is required",
                    )
                })?;
            let (task_id, original_id) = decode_anthropic_callback_tool_use_id(external_id)
                .ok_or_else(|| {
                    CallbackCorrelationError::new(
                        "messages",
                        "callback tool_result tool_use_id is invalid",
                    )
                })?;
            if !assistant_ids.contains(external_id) {
                return Err(CallbackCorrelationError::new(
                    "messages",
                    "callback tool_result does not match the preceding assistant tool calls",
                ));
            }
            let mut result = json!({
                "tool_call_id": original_id,
                "content": anthropic_tool_result_content(block),
            });
            if let Some(is_error) = block.get("is_error").and_then(Value::as_bool) {
                result["is_error"] = Value::Bool(is_error);
            }
            decoded.push((task_id, external_id.to_string(), result));
        }
    }
    let result_ids = decoded
        .iter()
        .map(|(_, external_id, _)| external_id.clone())
        .collect::<BTreeSet<_>>();
    if result_ids != assistant_ids {
        return Err(CallbackCorrelationError::new(
            "messages",
            "callback tool results must cover the preceding assistant tool calls exactly",
        ));
    }
    correlated_results(decoded, "messages")
}

fn correlate_openai_results<'a>(
    results: impl Iterator<Item = Result<(&'a str, String), CallbackCorrelationError>>,
    param: &'static str,
    paired_ids: Option<&BTreeSet<String>>,
) -> Result<Option<CorrelatedToolCallback>, CallbackCorrelationError> {
    let mut decoded = Vec::new();
    for result in results {
        let (external_id, content) = result?;
        let (task_id, original_id) =
            decode_openai_callback_tool_call_id(external_id).ok_or_else(|| {
                CallbackCorrelationError::new(param, "callback tool result id is invalid")
            })?;
        if paired_ids.is_some_and(|ids| !ids.contains(external_id)) {
            return Err(CallbackCorrelationError::new(
                param,
                "callback tool result does not match the preceding assistant tool calls",
            ));
        }
        decoded.push((
            task_id,
            external_id.to_string(),
            json!({ "tool_call_id": original_id, "content": content }),
        ));
    }
    if let Some(paired_ids) = paired_ids {
        let result_ids = decoded
            .iter()
            .map(|(_, external_id, _)| external_id.clone())
            .collect::<BTreeSet<_>>();
        if result_ids != *paired_ids {
            return Err(CallbackCorrelationError::new(
                param,
                "callback tool results must cover the preceding assistant tool calls exactly",
            ));
        }
    }
    correlated_results(decoded, param)
}

fn correlated_results(
    decoded: Vec<(Uuid, String, Value)>,
    param: &'static str,
) -> Result<Option<CorrelatedToolCallback>, CallbackCorrelationError> {
    let Some(callback_task_id) = decoded.first().map(|(task_id, _, _)| *task_id) else {
        return Ok(None);
    };
    let mut seen_external_ids = BTreeSet::new();
    let mut seen_original_ids = BTreeSet::new();
    let mut tool_results = Vec::with_capacity(decoded.len());
    for (task_id, external_id, result) in decoded {
        if task_id != callback_task_id {
            return Err(CallbackCorrelationError::new(
                param,
                "tool results must belong to one callback task",
            ));
        }
        if !seen_external_ids.insert(external_id) {
            return Err(CallbackCorrelationError::new(
                param,
                "callback tool result ids must be unique",
            ));
        }
        let original_id = result
            .get("tool_call_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if !seen_original_ids.insert(original_id) {
            return Err(CallbackCorrelationError::new(
                param,
                "callback call ids must be unique",
            ));
        }
        tool_results.push(result);
    }
    Ok(Some(CorrelatedToolCallback {
        callback_task_id,
        tool_results: Value::Array(tool_results),
    }))
}

fn contains_encoded_openai_tool_result(messages: &[Value]) -> bool {
    messages.iter().any(|message| {
        message
            .get("tool_call_id")
            .and_then(Value::as_str)
            .and_then(decode_openai_callback_tool_call_id)
            .is_some()
    })
}

fn contains_encoded_responses_tool_result(items: &[Value]) -> bool {
    items.iter().any(|item| {
        item.get("call_id")
            .and_then(Value::as_str)
            .and_then(decode_openai_callback_tool_call_id)
            .is_some()
    })
}

fn contains_encoded_anthropic_tool_result(messages: &[Value]) -> bool {
    messages.iter().any(|message| {
        message
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|blocks| {
                blocks.iter().any(|block| {
                    block
                        .get("tool_use_id")
                        .and_then(Value::as_str)
                        .and_then(decode_anthropic_callback_tool_use_id)
                        .is_some()
                })
            })
    })
}

fn openai_chat_assistant_tool_ids(tool_calls: &[Value]) -> BTreeSet<String> {
    tool_calls
        .iter()
        .filter_map(|call| call.get("id").and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn anthropic_assistant_tool_ids(message: &Value) -> Vec<String> {
    message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|block| block.get("id").and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn openai_tool_message_content(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(content)) => content.clone(),
        Some(Value::Null) | None => String::new(),
        Some(content) => content.to_string(),
    }
}

fn anthropic_message_has_only_tool_results(message: &Value) -> bool {
    message.get("role").and_then(Value::as_str) == Some("user")
        && message
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|blocks| {
                !blocks.is_empty()
                    && blocks.iter().all(|block| {
                        block.get("type").and_then(Value::as_str) == Some("tool_result")
                    })
            })
}

fn anthropic_tool_result_content(block: &Value) -> Value {
    let Some(content) = block.get("content") else {
        return Value::String(String::new());
    };
    if let Some(text) = content.as_str() {
        return Value::String(text.to_string());
    }
    if let Some(blocks) = content.as_array() {
        let text = blocks
            .iter()
            .filter_map(|entry| entry.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if blocks
            .iter()
            .all(|entry| entry.get("type").and_then(Value::as_str) == Some("text"))
        {
            return Value::String(text);
        }
        return Value::Array(blocks.clone());
    }
    content.clone()
}

#[cfg(test)]
mod tests;
