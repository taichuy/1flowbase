//! A consumed tool receipt and a later user turn have separate identities.
//! Admission already verified the actor, key, callback and complete source body.
use super::*;

pub(super) fn is_new_user_turn(
    context: &PublishedCallbackResumeContext,
    existing: &domain::FlowRunCallbackResumeAttemptRecord,
    command: &ResumePublishedCallbackCommand,
) -> Result<bool> {
    let Some(grant) = command.anthropic_continuation.as_ref() else {
        return Ok(false);
    };
    if command.source != PublishedCallbackResumeSource::AnthropicMessages
        || context.callback_task.callback_kind != "llm_tool_calls"
        || context.callback_task.status != domain::CallbackTaskStatus::Completed
        || !context.flow_run.status.is_terminal()
        || !matches!(
            existing.status,
            domain::FlowRunCallbackResumeAttemptStatus::Succeeded
                | domain::FlowRunCallbackResumeAttemptStatus::Failed
                | domain::FlowRunCallbackResumeAttemptStatus::Cancelled
        )
    {
        return Ok(false);
    }
    if existing.source != command.source.as_str() {
        return Err(ControlPlaneError::Conflict("callback_resume_source_conflict").into());
    }
    // The callback's accepted results remain immutable even when later user
    // context starts a fresh inference. Never reopen or overwrite this receipt.
    if existing.response_payload.get("tool_results") != command.response_payload.get("tool_results")
    {
        return Err(ControlPlaneError::Conflict("callback_resume_payload_conflict").into());
    }
    let Some(admitted) = source_history(
        &existing.response_payload["anthropic_continuation"]["client_protocol_envelope"],
    )?
    else {
        return Ok(false);
    };
    let Some(incoming) = source_history(&grant.continuation().client_protocol_envelope)? else {
        return Ok(false);
    };
    Ok(extends_with_user_input(&admitted, &incoming))
}

fn source_history(envelope: &Value) -> Result<Option<Vec<Value>>> {
    let Some(messages) = envelope
        .pointer("/source_request/body/messages")
        .and_then(Value::as_array)
        .filter(|messages| !messages.is_empty())
    else {
        return Ok(None);
    };
    // Reuse ingress's semantic projection, which restores protocol tool IDs,
    // separates tool results from visible user input and ignores cache hints.
    // Do not compare the workflow's synthesized query fallback: a tool-only
    // request may fill that slot from tool output, without any human message.
    let mut history = Vec::new();
    for message in messages {
        let role = message["role"].as_str().ok_or(ControlPlaneError::Conflict(
            "callback_resume_payload_conflict",
        ))?;
        if role != "system" {
            history.extend(
                crate::application_public_api::compat::anthropic::anthropic_history_entries(
                    role,
                    &message["content"],
                )
                .map_err(|_| ControlPlaneError::Conflict("callback_resume_payload_conflict"))?,
            );
        }
    }
    Ok(Some(history))
}

fn user_input(message: &Value) -> bool {
    message["role"] == "user"
        && ["content", "content_blocks"]
            .iter()
            .any(|field| message.get(field).is_some_and(nonempty_content))
}

fn nonempty_content(content: &Value) -> bool {
    match content {
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(parts) => !parts.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
        _ => false,
    }
}

fn extends_with_user_input(admitted: &[Value], incoming: &[Value]) -> bool {
    if incoming.starts_with(admitted) {
        return incoming[admitted.len()..].iter().any(user_input);
    }
    // Adjacent user input may extend the last user message instead of adding
    // another message. Keep the preceding context and accepted text/parts as
    // exact prefixes; replacing an earlier instruction is still a conflict.
    let Some((last, prefix)) = admitted.split_last() else {
        return false;
    };
    if !incoming.starts_with(prefix) || last["role"] != "user" {
        return false;
    }
    let Some(next) = incoming
        .get(prefix.len())
        .filter(|next| next["role"] == "user")
    else {
        return false;
    };
    // Only visible text/media may grow. Other fields remain bound to the
    // admitted value. Media can be present even when the text is empty.
    let (Some(old), Some(new)) = (last.as_object(), next.as_object()) else {
        return false;
    };
    if new
        .keys()
        .any(|key| !matches!(key.as_str(), "content" | "content_blocks") && !old.contains_key(key))
        || old.iter().any(|(key, value)| {
            !matches!(key.as_str(), "content" | "content_blocks") && new.get(key) != Some(value)
        })
    {
        return false;
    }
    let Some(text_added) = added_content(&last["content"], &next["content"]) else {
        return false;
    };
    let empty = Value::Array(Vec::new());
    let Some(media_added) = added_content(
        old.get("content_blocks").unwrap_or(&empty),
        new.get("content_blocks").unwrap_or(&empty),
    ) else {
        return false;
    };
    text_added || media_added
}

fn added_content(old: &Value, new: &Value) -> Option<bool> {
    match (old, new) {
        (Value::String(old), Value::String(new)) => new
            .strip_prefix(old.as_str())
            .map(|added| !added.trim().is_empty()),
        (Value::Array(old), Value::Array(new)) if new.starts_with(old) => {
            Some(new[old.len()..].iter().any(nonempty_content))
        }
        _ => None,
    }
}
