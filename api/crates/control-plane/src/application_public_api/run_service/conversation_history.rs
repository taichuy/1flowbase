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

    for message in messages {
        if message.role != "tool" {
            append_interrupted_tool_outputs(&mut history, &mut pending_tool_calls);
        }

        if message.role == "assistant" {
            let mut value = message.into_value();
            let complete_tool_calls = value
                .get("tool_calls")
                .and_then(Value::as_array)
                .map(|tool_calls| {
                    tool_calls
                        .iter()
                        .filter_map(|tool_call| {
                            complete_tool_call(tool_call).and_then(|(id, name)| {
                                if pending_tool_calls
                                    .iter()
                                    .any(|(pending_id, _)| pending_id == &id)
                                {
                                    None
                                } else {
                                    pending_tool_calls.push((id, name));
                                    Some(tool_call.clone())
                                }
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if complete_tool_calls.is_empty() {
                value
                    .as_object_mut()
                    .expect("Native history is an object")
                    .remove("tool_calls");
            } else {
                value["tool_calls"] = Value::Array(complete_tool_calls);
            }
            history.push(value);
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
mod tests {
    use super::*;

    fn native_message(
        role: &str,
        content: &str,
        tool_call_id: Option<&str>,
        tool_calls: Option<Vec<Value>>,
    ) -> AssistantConversationNativeMessage {
        AssistantConversationNativeMessage {
            role: role.to_string(),
            content: content.to_string(),
            name: None,
            tool_call_id: tool_call_id.map(ToOwned::to_owned),
            is_error: None,
            content_blocks: None,
            tool_calls,
        }
    }

    #[test]
    fn assistant_history_closes_complete_interrupted_tool_calls_and_drops_orphan_outputs() {
        let history = assistant_conversation_native_history_to_values(vec![
            native_message("user", "Find an order", None, None),
            native_message(
                "assistant",
                "",
                None,
                Some(vec![
                    json!({ "id": "call_order", "name": "find_order", "arguments": { "id": "42" } }),
                    json!({ "id": "call_order", "name": "find_order", "arguments": { "id": "42" } }),
                    json!({ "id": "", "name": "incomplete", "arguments": {} }),
                ]),
            ),
            native_message("tool", "stale", Some("unknown_call"), None),
            native_message("user", "Try again", None, None),
        ]);

        assert_eq!(history.len(), 4);
        assert_eq!(
            history[0],
            json!({ "role": "user", "content": "Find an order" })
        );
        assert_eq!(
            history[1]["tool_calls"],
            json!([
                { "id": "call_order", "name": "find_order", "arguments": { "id": "42" } }
            ])
        );
        assert_eq!(history[2]["role"], "tool");
        assert_eq!(history[2]["tool_call_id"], "call_order");
        assert_eq!(history[2]["is_error"], true);
        assert_eq!(history[2]["content"], INTERRUPTED_TOOL_CALL_CONTENT);
        assert_eq!(
            history[3],
            json!({ "role": "user", "content": "Try again" })
        );
    }
}
