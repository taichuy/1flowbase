use serde_json::{json, Value};

use super::super::conversations::ApplicationPublicConversationMessageRecord;

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
