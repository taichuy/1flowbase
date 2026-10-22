use serde_json::{json, Value};

use super::{
    super::conversations::ApplicationPublicConversationMessageRecord,
    AssistantConversationNativeMessage,
};

const INTERRUPTED_TOOL_CALL_CONTENT: &str =
    "Tool call was interrupted and may have partially executed; it was not retried.";

/// Conversation storage is already canonical. Rehydration preserves its
/// chronological user/assistant transcript exactly; protocol marker parsing,
/// hidden-control rules, and duplicate-turn rewriting do not belong here.
pub(super) fn application_public_conversation_messages_to_native_history(
    messages: Vec<ApplicationPublicConversationMessageRecord>,
) -> Vec<Value> {
    messages
        .into_iter()
        .filter(|message| matches!(message.role.as_str(), "user" | "assistant"))
        .map(|message| {
            json!({
                "role": message.role,
                "content": message.content,
            })
        })
        .collect()
}

/// Closes durable assistant history for a new provider request. A persisted tool call can outlive
/// its run, so only complete calls are replayed and every unmatched call receives one synthetic,
/// error-marked result instead of being re-executed.
pub fn assistant_conversation_native_history_to_values(
    messages: Vec<AssistantConversationNativeMessage>,
) -> Vec<Value> {
    let mut history = Vec::new();
    let mut pending_tool_calls = Vec::new();

    for mut message in messages {
        if message.role != "tool" {
            append_interrupted_tool_outputs(&mut history, &mut pending_tool_calls);
        }

        if message.role == "assistant" {
            if let Some(tool_calls) = message.tool_calls.as_mut() {
                tool_calls.retain(|tool_call| {
                    let Some((id, name)) = complete_tool_call(tool_call) else {
                        return false;
                    };
                    if pending_tool_calls
                        .iter()
                        .any(|(pending_id, _)| pending_id == &id)
                    {
                        return false;
                    }
                    pending_tool_calls.push((id, name));
                    true
                });
                if tool_calls.is_empty() {
                    message.tool_calls = None;
                }
            }
            history.push(message.into_value());
            continue;
        }

        if message.role == "tool" {
            let Some(tool_call_id) = message.tool_call_id.as_deref() else {
                continue;
            };
            let Some(index) = pending_tool_calls
                .iter()
                .position(|(id, _)| id == tool_call_id)
            else {
                continue;
            };
            pending_tool_calls.remove(index);
        }
        history.push(message.into_value());
    }

    append_interrupted_tool_outputs(&mut history, &mut pending_tool_calls);
    history
}

fn complete_tool_call(tool_call: &Value) -> Option<(String, String)> {
    let object = tool_call.as_object()?;
    let id = object.get("id")?.as_str()?.trim();
    let name = object.get("name")?.as_str()?.trim();
    object.get("arguments")?;
    (!id.is_empty() && !name.is_empty()).then(|| (id.to_string(), name.to_string()))
}

fn append_interrupted_tool_outputs(
    history: &mut Vec<Value>,
    pending_tool_calls: &mut Vec<(String, String)>,
) {
    for (tool_call_id, name) in pending_tool_calls.drain(..) {
        history.push(json!({
            "role": "tool",
            "name": name,
            "tool_call_id": tool_call_id,
            "content": INTERRUPTED_TOOL_CALL_CONTENT,
            "is_error": true,
        }));
    }
}

#[cfg(test)]
#[path = "_tests/conversation_history.rs"]
mod tests;
