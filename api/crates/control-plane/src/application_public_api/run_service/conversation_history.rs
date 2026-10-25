use std::collections::{hash_map::Entry, HashMap};

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
    let mut pending_tool_calls = HashMap::new();

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
                    let order = pending_tool_calls.len();
                    match pending_tool_calls.entry(id) {
                        Entry::Vacant(entry) => {
                            entry.insert((order, name));
                            true
                        }
                        Entry::Occupied(_) => false,
                    }
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
            if pending_tool_calls.remove(tool_call_id).is_none() {
                continue;
            }
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
    pending_tool_calls: &mut HashMap<String, (usize, String)>,
) {
    let mut interrupted = pending_tool_calls.drain().collect::<Vec<_>>();
    interrupted.sort_unstable_by_key(|(_, (order, _))| *order);
    for (tool_call_id, (_, name)) in interrupted {
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
