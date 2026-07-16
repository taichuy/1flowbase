use plugin_framework::provider_contract::{NativeModelRequestContext, NativePromptBlock};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::application_public_api::client_protocol_envelope::{
    anthropic_messages_envelope_with_beta, ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE,
};
use crate::application_public_api::native::{NativeProtocolRequestKind, NativeRunRequest};

const CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of the conversation so far";
const CLAUDE_CODE_PARTIAL_COMPACT_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of the RECENT portion of the conversation";
const CLAUDE_CODE_CONTEXT_CONTINUATION_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of this conversation. This summary will be placed at the start of a continuing session";
const CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX: &str =
    "The user stepped away and is coming back. Write exactly 1-3 short sentences.";
const CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER: &str = "Next: the concrete next step.";
const CLAUDE_CODE_COMPACT_RESUME_MARKER: &str =
    "This session is being continued from a previous conversation that ran out of context.";
const CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER: &str =
    "The summary below covers the earlier portion of the conversation.";
const CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER: &str =
    "If you need specific details from before compaction";
const CLAUDE_CODE_SESSION_TITLE_SYSTEM_MARKER: &str = "Generate a concise, sentence-case title";
const CLAUDE_CODE_SESSION_TITLE_JSON_MARKER: &str = "Return JSON with a single \"title\" field";
const ANTHROPIC_MESSAGES_COMPATIBILITY_MODE: &str = "anthropic-messages-v1";
const ANTHROPIC_CONTEXT_1M_MODEL_SUFFIX: &str = "[1m]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicCompatError {
    pub message: String,
    pub error_type: String,
}

impl AnthropicCompatError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: "invalid_request".to_string(),
        }
    }

    fn unsupported(param: &'static str) -> Self {
        Self {
            message: format!("{param} is not supported by this endpoint"),
            error_type: "unsupported_feature".to_string(),
        }
    }
}

pub fn map_messages_request(request: Value) -> Result<NativeRunRequest, AnthropicCompatError> {
    let object = request
        .as_object()
        .ok_or_else(|| AnthropicCompatError::invalid("request body must be an object"))?;
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| AnthropicCompatError::invalid("model is required"))?;
    let (model, context_beta) = normalize_anthropic_model_for_native(model);
    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| AnthropicCompatError::invalid("messages is required"))?;

    let mut system_parts = anthropic_system_content_parts(object.get("system"))?;
    let last_user_index = messages
        .iter()
        .rposition(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .ok_or_else(|| AnthropicCompatError::invalid("user message is required"))?;
    let latest_user_content = messages[last_user_index]
        .get("content")
        .ok_or_else(|| AnthropicCompatError::invalid("message content is required"))?;
    let latest_user_text = anthropic_current_user_text_content(latest_user_content)?;
    collect_system_reminder_parts(&mut system_parts, &latest_user_text);
    let query = sanitize_anthropic_compat_text("user", &latest_user_text).unwrap_or_default();
    let latest_user_media_blocks = query_media_content_blocks(latest_user_content);
    let latest_user_is_tool_result_only =
        anthropic_content_is_tool_result_only(latest_user_content);

    let mut history = Vec::new();
    let mut hidden_control_kind = None;
    for (index, message) in messages.iter().enumerate() {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| AnthropicCompatError::invalid("message role is required"))?;
        let content_value = message
            .get("content")
            .ok_or_else(|| AnthropicCompatError::invalid("message content is required"))?;
        if index == last_user_index {
            continue;
        }
        let raw_content = anthropic_text_content(content_value)?;
        if role == "user" {
            collect_system_reminder_parts(&mut system_parts, &raw_content);
        }
        let content = sanitize_anthropic_compat_text(role, &raw_content).unwrap_or_default();
        let keep_tool_use_blocks =
            latest_user_is_tool_result_only && role == "assistant" && index + 1 == last_user_index;
        let content_blocks = history_content_blocks(role, content_value, keep_tool_use_blocks);
        if content.trim().is_empty() && content_blocks.is_none() {
            continue;
        }
        let message_control_kind = (role == "user")
            .then(|| claude_code_control_kind(&raw_content))
            .flatten();
        if role == "user" {
            hidden_control_kind = message_control_kind;
        }
        let mut history_entry = serde_json::json!({ "role": role, "content": content });
        if let Some(content_blocks) = content_blocks {
            history_entry["content_blocks"] = content_blocks;
        }
        if let Some(control_kind) = message_control_kind.or_else(|| {
            (role == "assistant")
                .then_some(hidden_control_kind)
                .flatten()
        }) {
            history_entry["metadata"] = serde_json::json!({
                "hidden_from_conversation": true,
                "claude_code_control": control_kind,
            });
        }
        history.push(history_entry);
    }
    history = dedupe_anthropic_compat_history(history);
    history = drop_replayed_current_user_turn(history, &query);
    if let Some(content_blocks) = latest_user_media_blocks {
        history.push(serde_json::json!({
            "role": "user",
            "content": "",
            "content_blocks": content_blocks
        }));
    }

    let response_mode = object
        .get("stream")
        .and_then(Value::as_bool)
        .filter(|stream| *stream)
        .map(|_| "streaming".to_string());
    let conversation = metadata_conversation(object.get("metadata"));
    let current_control_kind = claude_code_control_kind(&latest_user_text)
        .or_else(|| claude_code_system_control_kind(&system_parts));
    let metadata = object
        .get("metadata")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let compatibility = compatibility_payload(
        object,
        current_control_kind,
        latest_user_is_tool_result_only,
    );
    let end_user_reference = metadata
        .get("user_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let mut metadata = metadata;
    if let Some(metadata) = metadata.as_object_mut() {
        metadata.remove("user_id");
    }
    if !compatibility.is_null() {
        metadata["compatibility"] = compatibility.clone();
    }

    let mut native = serde_json::json!({
        "query": query,
        "model": model,
        "inputs": compatibility_inputs(compatibility),
        "history": history,
        "conversation": conversation,
        "response_mode": response_mode,
        "metadata": metadata,
        "request_context": NativeModelRequestContext { end_user_reference },
        "compatibility_mode": ANTHROPIC_MESSAGES_COMPATIBILITY_MODE
    });
    if !system_parts.is_empty() {
        native["system"] = serde_json::to_value(system_parts)
            .map_err(|_| AnthropicCompatError::invalid("failed to build Native system prompt"))?;
    }
    if response_mode.is_none() {
        native
            .as_object_mut()
            .expect("native request object")
            .remove("response_mode");
    }

    let mut request: NativeRunRequest = serde_json::from_value(native)
        .map_err(|_| AnthropicCompatError::invalid("failed to build Native request"))?;
    request.protocol_compatibility_mode = Some(ANTHROPIC_MESSAGES_COMPATIBILITY_MODE.to_string());
    if let Some(beta) = context_beta {
        request.client_protocol_envelope = Some(anthropic_messages_envelope_with_beta(beta));
    }
    if latest_user_is_tool_result_only {
        request.protocol_request_kind =
            Some(NativeProtocolRequestKind::AnthropicToolResultContinuation);
    }
    Ok(request)
}

fn normalize_anthropic_model_for_native(model: &str) -> (String, Option<&'static str>) {
    let trimmed_end = model.trim_end();
    let suffix_start = trimmed_end
        .len()
        .saturating_sub(ANTHROPIC_CONTEXT_1M_MODEL_SUFFIX.len());
    let has_one_m_suffix = trimmed_end
        .get(suffix_start..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(ANTHROPIC_CONTEXT_1M_MODEL_SUFFIX));

    if has_one_m_suffix {
        let native_model = trimmed_end
            .get(..suffix_start)
            .unwrap_or(trimmed_end)
            .to_string();
        return (native_model, Some(ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE));
    }

    (model.to_string(), None)
}

fn anthropic_system_content_parts(
    value: Option<&Value>,
) -> Result<Vec<NativePromptBlock>, AnthropicCompatError> {
    let mut parts = Vec::new();
    match value {
        Some(Value::String(text)) => push_system_part(&mut parts, text),
        Some(Value::Array(blocks)) => {
            for block in blocks {
                match block {
                    Value::String(text) => push_system_part(&mut parts, text),
                    Value::Object(_) => {
                        let block: NativePromptBlock = serde_json::from_value(block.clone())
                            .map_err(|_| AnthropicCompatError::unsupported("system"))?;
                        if !block.text_content().trim().is_empty() {
                            parts.push(block);
                        }
                    }
                    _ => return Err(AnthropicCompatError::unsupported("system")),
                }
            }
        }
        None => {}
        _ => {
            return Err(AnthropicCompatError::invalid(
                "system must be text or text blocks",
            ))
        }
    }
    Ok(parts)
}

fn collect_system_reminder_parts(parts: &mut Vec<NativePromptBlock>, content: &str) {
    for reminder in xml_tag_block_contents(content, "system-reminder") {
        push_unique_system_part(parts, &reminder);
    }
}

fn xml_tag_block_contents(content: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut output = Vec::new();
    let mut offset = 0;

    while let Some(start) = content[offset..].find(&open) {
        let content_start = offset + start + open.len();
        let Some(end) = content[content_start..].find(&close) else {
            break;
        };
        let content_end = content_start + end;
        output.push(content[content_start..content_end].to_string());
        offset = content_end + close.len();
    }

    output
}

fn push_system_part(parts: &mut Vec<NativePromptBlock>, content: &str) {
    let content = content.trim();
    if content.is_empty() {
        return;
    }
    parts.push(NativePromptBlock::text(content));
}

fn push_unique_system_part(parts: &mut Vec<NativePromptBlock>, content: &str) {
    let content = content.trim();
    if content.is_empty() || parts.iter().any(|part| part.text_content() == content) {
        return;
    }
    parts.push(NativePromptBlock::text(content));
}

fn compatibility_payload(
    object: &serde_json::Map<String, Value>,
    claude_code_control: Option<&'static str>,
    latest_user_is_tool_result_only: bool,
) -> Value {
    let mut compatibility = serde_json::Map::new();
    for key in ["tools", "tool_choice"] {
        if let Some(value) = object.get(key) {
            compatibility.insert(key.to_string(), value.clone());
        }
    }
    if let Some(claude_code_control) = claude_code_control {
        compatibility.insert(
            "claude_code_control".to_string(),
            Value::String(claude_code_control.to_string()),
        );
    }
    if latest_user_is_tool_result_only {
        compatibility.insert(
            "anthropic_message_kind".to_string(),
            Value::String("tool_result_only".to_string()),
        );
    }
    if compatibility.is_empty() {
        Value::Null
    } else {
        Value::Object(compatibility)
    }
}

fn compatibility_inputs(compatibility: Value) -> Value {
    let Some(object) = compatibility.as_object() else {
        return serde_json::json!({});
    };
    let mut inputs = serde_json::Map::new();
    if let Some(tools) = object
        .get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(normalize_anthropic_tool)
                .collect::<Vec<_>>()
        })
        .filter(|tools| !tools.is_empty())
    {
        inputs.insert("tools".to_string(), Value::Array(tools));
    }
    if let Some(tool_choice) = object.get("tool_choice") {
        inputs.insert("tool_choice".to_string(), tool_choice.clone());
    }
    if let Some(claude_code_control) = object.get("claude_code_control") {
        inputs.insert(
            "compatibility".to_string(),
            serde_json::json!({
                "claude_code_control": claude_code_control,
            }),
        );
    }
    Value::Object(inputs)
}

pub fn claude_code_control_kind(content: &str) -> Option<&'static str> {
    if content.contains(CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX)
        || content.contains(CLAUDE_CODE_PARTIAL_COMPACT_SUMMARY_PROMPT_PREFIX)
        || content.contains(CLAUDE_CODE_CONTEXT_CONTINUATION_SUMMARY_PROMPT_PREFIX)
    {
        return Some("compact_summary");
    }
    if content.contains(CLAUDE_CODE_COMPACT_RESUME_MARKER)
        && (content.contains(CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER)
            || content.contains(CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER))
    {
        return Some("compact_resume");
    }
    if content.contains(CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX)
        && content.contains(CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER)
    {
        return Some("away_summary");
    }
    None
}

fn claude_code_system_control_kind(system_parts: &[NativePromptBlock]) -> Option<&'static str> {
    system_parts
        .iter()
        .any(|part| {
            part.text_content()
                .contains(CLAUDE_CODE_SESSION_TITLE_SYSTEM_MARKER)
                && part
                    .text_content()
                    .contains(CLAUDE_CODE_SESSION_TITLE_JSON_MARKER)
        })
        .then_some("session_title")
}

fn metadata_conversation(metadata: Option<&Value>) -> Value {
    let mut conversation = Map::new();
    let Some(metadata) = metadata.and_then(Value::as_object) else {
        return Value::Object(conversation);
    };
    let user_id = metadata
        .get("user_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let user_id_payload = user_id.and_then(|value| serde_json::from_str::<Value>(value).ok());
    if let Some(user) = metadata
        .get("expand_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| metadata_user_from_user_id(user_id, user_id_payload.as_ref()))
    {
        conversation.insert("user".to_string(), Value::String(user));
    }
    if let Some(session_id) = user_id_payload
        .as_ref()
        .and_then(|payload| payload.get("session_id"))
        .and_then(Value::as_str)
        .or_else(|| metadata.get("session_id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| user_id.and_then(claude_code_session_id_from_identity))
    {
        conversation.insert("id".to_string(), Value::String(session_id));
    }
    Value::Object(conversation)
}

fn metadata_user_from_user_id(user_id: Option<&str>, payload: Option<&Value>) -> Option<String> {
    payload
        .and_then(|payload| {
            payload
                .get("account_uuid")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    payload
                        .get("device_id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                })
        })
        .or(user_id)
        .map(ToOwned::to_owned)
}

fn claude_code_session_id_from_identity(identity: &str) -> Option<String> {
    let marker = "_session_";
    let start = identity.rfind(marker)? + marker.len();
    let candidate = identity[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
        .collect::<String>();
    Uuid::parse_str(&candidate)
        .ok()
        .map(|session_id| session_id.to_string())
}

fn normalize_anthropic_tool(tool: &Value) -> Option<Value> {
    let object = tool.as_object()?;
    let name = object.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    let mut normalized = serde_json::Map::new();
    normalized.insert("name".to_string(), Value::String(name.to_string()));
    normalized.insert(
        "source".to_string(),
        Value::String("anthropic_compatible".to_string()),
    );
    if let Some(description) = object
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        normalized.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
    if let Some(input_schema) = object.get("input_schema") {
        normalized.insert("input_schema".to_string(), input_schema.clone());
    }
    Some(Value::Object(normalized))
}

fn anthropic_text_content(content: &Value) -> Result<String, AnthropicCompatError> {
    if let Some(text) = content.as_str() {
        return Ok(text.to_string());
    }
    let blocks = content
        .as_array()
        .ok_or_else(|| AnthropicCompatError::invalid("content must be text"))?;
    let mut text = String::new();
    for block in blocks {
        let block_type = block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match block_type {
            "text" => {
                if let Some(value) = block.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(value);
                }
            }
            "tool_result" => {
                let value = anthropic_tool_result_text(block);
                if !value.is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&value);
                }
            }
            "tool_use" | "server_tool_use" => {
                if block
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == "computer")
                {
                    return Err(AnthropicCompatError::unsupported("computer_use"));
                }
            }
            "computer_use" => {
                return Err(AnthropicCompatError::unsupported("computer_use"));
            }
            "thinking" | "redacted_thinking" => {}
            "image" | "document" => {}
            _ => return Err(AnthropicCompatError::unsupported("messages")),
        }
    }
    Ok(text)
}

fn anthropic_current_user_text_content(content: &Value) -> Result<String, AnthropicCompatError> {
    if let Some(text) = content.as_str() {
        return Ok(text.to_string());
    }
    let blocks = content
        .as_array()
        .ok_or_else(|| AnthropicCompatError::invalid("content must be text"))?;
    if !anthropic_blocks_have_visible_user_text(blocks) {
        return anthropic_text_content(content);
    }

    let mut text = String::new();
    for block in blocks {
        let block_type = block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match block_type {
            "text" => {
                if let Some(value) = block.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(value);
                }
            }
            "tool_result" => {}
            "tool_use" | "server_tool_use" => {
                if block
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == "computer")
                {
                    return Err(AnthropicCompatError::unsupported("computer_use"));
                }
            }
            "computer_use" => {
                return Err(AnthropicCompatError::unsupported("computer_use"));
            }
            "thinking" | "redacted_thinking" => {}
            "image" | "document" => {}
            _ => return Err(AnthropicCompatError::unsupported("messages")),
        }
    }
    Ok(text)
}

fn anthropic_blocks_have_visible_user_text(blocks: &[Value]) -> bool {
    blocks.iter().any(|block| {
        block
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|block_type| block_type == "text")
            && block
                .get("text")
                .and_then(Value::as_str)
                .and_then(|text| sanitize_anthropic_compat_text("user", text))
                .is_some()
    })
}

pub fn anthropic_content_is_tool_result_only(content: &Value) -> bool {
    let Some(blocks) = content.as_array() else {
        return false;
    };
    let mut has_tool_result = false;
    for block in blocks {
        match block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "tool_result" => has_tool_result = true,
            "thinking" | "redacted_thinking" => {}
            "text" => {
                if block
                    .get("text")
                    .and_then(Value::as_str)
                    .and_then(|text| sanitize_anthropic_compat_text("user", text))
                    .is_some()
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    has_tool_result
}

fn history_content_blocks(
    role: &str,
    content: &Value,
    keep_tool_use_blocks: bool,
) -> Option<Value> {
    let blocks = content.as_array()?;
    let has_media_blocks = blocks.iter().any(anthropic_history_block_has_media);
    let mut mapped_blocks = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("thinking" | "redacted_thinking") => {}
            Some("text") if has_media_blocks => {
                if let Some(text_block) = sanitized_anthropic_text_block(role, block) {
                    mapped_blocks.push(text_block);
                }
            }
            Some("text") => {}
            Some("image" | "document") => mapped_blocks.push(block.clone()),
            Some("tool_result") if has_media_blocks => {
                mapped_blocks.extend(anthropic_tool_result_content_blocks(role, block));
            }
            Some("tool_use" | "server_tool_use") if keep_tool_use_blocks => {
                mapped_blocks.push(block.clone());
            }
            _ => {}
        }
    }
    (!mapped_blocks.is_empty()).then_some(Value::Array(mapped_blocks))
}

fn query_media_content_blocks(content: &Value) -> Option<Value> {
    let blocks = content.as_array()?;
    let media_blocks = blocks
        .iter()
        .filter(|block| {
            matches!(
                block.get("type").and_then(Value::as_str),
                Some("image" | "document")
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    (!media_blocks.is_empty()).then_some(Value::Array(media_blocks))
}

fn anthropic_tool_result_text(block: &Value) -> String {
    let Some(content) = block.get("content") else {
        return String::new();
    };
    if let Some(text) = content.as_str() {
        return text.to_string();
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
            || !text.trim().is_empty()
        {
            return text;
        }
        if blocks.iter().any(anthropic_content_block_is_media) {
            return String::new();
        }
        return content.to_string();
    }
    content.to_string()
}

fn anthropic_history_block_has_media(block: &Value) -> bool {
    match block.get("type").and_then(Value::as_str) {
        Some("image" | "document") => true,
        Some("tool_result") => block
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|blocks| blocks.iter().any(anthropic_content_block_is_media)),
        _ => false,
    }
}

fn anthropic_content_block_is_media(block: &Value) -> bool {
    matches!(
        block.get("type").and_then(Value::as_str),
        Some("image" | "document")
    )
}

fn sanitized_anthropic_text_block(role: &str, block: &Value) -> Option<Value> {
    let sanitized = sanitize_anthropic_compat_text(
        role,
        block
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )?;
    let mut block = block.as_object()?.clone();
    block.insert("text".to_string(), Value::String(sanitized));
    Some(Value::Object(block))
}

fn anthropic_tool_result_content_blocks(role: &str, block: &Value) -> Vec<Value> {
    block
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|entry| match entry.get("type").and_then(Value::as_str) {
                    Some("text") => sanitized_anthropic_text_block(role, entry),
                    Some("image" | "document") => Some(entry.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn sanitize_anthropic_compat_assistant_text(content: &str) -> Option<String> {
    sanitize_anthropic_compat_text("assistant", content)
}

fn sanitize_anthropic_compat_text(role: &str, content: &str) -> Option<String> {
    let visible_content = match role {
        "assistant" => {
            let without_thinking = strip_xml_tag_blocks(content, "think");
            let without_tool_calls = strip_xml_tag_blocks(&without_thinking, "tool_call");
            content_after_beautified_marker(&without_tool_calls)
                .unwrap_or(without_tool_calls.as_str())
                .to_string()
        }
        "user" => strip_xml_tag_blocks(content, "system-reminder"),
        _ => content.to_string(),
    };
    trimmed_compat_text(&visible_content)
}

fn strip_xml_tag_blocks(content: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut output = content.to_string();

    while let Some(start) = output.find(&open) {
        let search_start = start + open.len();
        let Some(end) = output[search_start..].find(&close) else {
            break;
        };
        let end = search_start + end + close.len();
        output.replace_range(start..end, "");
    }

    output
}

fn content_after_beautified_marker(content: &str) -> Option<&str> {
    let marker = "下面是美化后内容";
    let marker_start = content.find(marker)?;
    Some(
        content[marker_start + marker.len()..].trim_start_matches(|value: char| {
            value.is_whitespace() || value == '-' || value == '—'
        }),
    )
}

fn trimmed_compat_text(content: &str) -> Option<String> {
    let trimmed = content.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn dedupe_anthropic_compat_history(history: Vec<Value>) -> Vec<Value> {
    let mut deduped = Vec::new();
    let mut previous_user_turn: Option<(usize, String)> = None;

    for message in history {
        if message.get("role").and_then(Value::as_str) == Some("user") {
            if let Some(key) = dedupe_user_turn_key(&message) {
                if previous_user_turn
                    .as_ref()
                    .is_some_and(|(_, previous_key)| previous_key == &key)
                {
                    if let Some((start, _)) = previous_user_turn.take() {
                        deduped.truncate(start);
                    }
                }
                previous_user_turn = Some((deduped.len(), key));
            } else {
                previous_user_turn = None;
            }
        }
        deduped.push(message);
    }

    deduped
}

fn drop_replayed_current_user_turn(mut history: Vec<Value>, current_query: &str) -> Vec<Value> {
    let current_query = current_query.trim();
    if current_query.is_empty() {
        return history;
    }

    for (index, message) in history.iter().enumerate().rev() {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        if dedupe_user_turn_key(message).is_some_and(|key| key == current_query) {
            history.truncate(index);
        }
        break;
    }

    history
}

fn dedupe_user_turn_key(message: &Value) -> Option<String> {
    if message
        .get("content_blocks")
        .and_then(Value::as_array)
        .is_some_and(|blocks| {
            blocks.iter().any(|block| {
                matches!(
                    block.get("type").and_then(Value::as_str),
                    Some("tool_result" | "tool_use" | "server_tool_use")
                )
            })
        })
    {
        return None;
    }

    message
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            message
                .get("content_blocks")
                .filter(|content_blocks| !content_blocks.is_null())
                .map(Value::to_string)
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn maps_tools_into_start_tool_registry_variables() {
        let request = map_messages_request(json!({
            "model": "claude-compatible",
            "messages": [
                { "role": "user", "content": "say hello" }
            ],
            "tools": [
                {
                    "name": "read_file",
                    "description": "Read a file",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string" }
                        }
                    }
                }
            ],
            "tool_choice": { "type": "auto" }
        }))
        .unwrap();

        let inputs = request.inputs.as_value();
        assert_eq!(inputs["tools"][0]["name"], json!("read_file"));
        assert_eq!(inputs["tools"][0]["source"], json!("anthropic_compatible"));
        assert_eq!(
            inputs["tools"][0]["input_schema"]["properties"]["file_path"]["type"],
            json!("string")
        );
        assert_eq!(inputs["tool_choice"], json!({ "type": "auto" }));
        assert!(inputs.get("compatibility").is_none());
    }

    #[test]
    fn maps_latest_claude_code_query_without_system_reminder() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {
                    "role": "user",
                    "content": "<system-reminder>internal tools</system-reminder>\n\nhi？"
                }
            ]
        }))
        .unwrap();

        assert_eq!(request.query, "hi？");
        assert_eq!(request.system_text().as_deref(), Some("internal tools"));
        assert!(request.history.is_empty());
    }

    #[test]
    fn maps_top_level_system_content_blocks_into_native_system() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "system": [
                {
                    "type": "text",
                    "text": "Use Claude Code project instructions.",
                    "cache_control": { "type": "ephemeral" }
                },
                {
                    "type": "text",
                    "text": "Preserve repository safety rules."
                }
            ],
            "messages": [
                { "role": "user", "content": "hi？" }
            ]
        }))
        .unwrap();

        assert_eq!(
            request.system_text().as_deref(),
            Some("Use Claude Code project instructions.\n\nPreserve repository safety rules.")
        );
        assert_eq!(request.query, "hi？");
    }

    #[test]
    fn maps_claude_code_history_without_internal_transcript_payloads() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {
                    "role": "user",
                    "content": "<system-reminder>available tools</system-reminder>\n\nhi？"
                },
                {
                    "role": "assistant",
                    "content": "<think>private reasoning</think>嗨，有什么需要我帮忙的？"
                },
                {
                    "role": "user",
                    "content": "uploads/agent-flow-preview-debug.png 描述一下这幅图说什么？"
                }
            ],
            "tools": [
                {
                    "name": "Read",
                    "description": "Read a file",
                    "input_schema": { "type": "object" }
                }
            ]
        }))
        .unwrap();

        assert_eq!(
            request.query,
            "uploads/agent-flow-preview-debug.png 描述一下这幅图说什么？"
        );
        assert_eq!(
            request.history,
            vec![
                json!({"role": "user", "content": "hi？"}),
                json!({"role": "assistant", "content": "嗨，有什么需要我帮忙的？"}),
            ]
        );
        assert_eq!(request.inputs.as_value()["tools"][0]["name"], json!("Read"));
        assert_eq!(request.system_text().as_deref(), Some("available tools"));
    }

    #[test]
    fn maps_claude_code_history_keeps_last_duplicate_user_turn() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "Describe image"},
                {"role": "assistant", "content": "old draft"},
                {"role": "user", "content": "Describe image"},
                {"role": "assistant", "content": "<think>retry</think>final draft"},
                {"role": "user", "content": "Continue"}
            ]
        }))
        .unwrap();

        assert_eq!(request.query, "Continue");
        assert_eq!(
            request.history,
            vec![
                json!({"role": "user", "content": "Describe image"}),
                json!({"role": "assistant", "content": "final draft"}),
            ]
        );
    }

    #[test]
    fn maps_claude_code_history_drops_replayed_current_user_turn() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "Describe image"},
                {"role": "assistant", "content": "old image answer"},
                {"role": "user", "content": "Describe image"}
            ]
        }))
        .unwrap();

        assert_eq!(request.query, "Describe image");
        assert!(request.history.is_empty());
    }

    #[test]
    fn maps_claude_code_history_preserves_latest_media_after_dropping_replay() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "Describe image"},
                {"role": "assistant", "content": "old image answer"},
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "Describe image"},
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "aW1hZ2U="
                            }
                        }
                    ]
                }
            ]
        }))
        .unwrap();

        assert_eq!(request.query, "Describe image");
        assert_eq!(request.history.len(), 1);
        assert_eq!(request.history[0]["role"], json!("user"));
        assert_eq!(request.history[0]["content"], json!(""));
        assert_eq!(
            request.history[0]["content_blocks"][0]["type"],
            json!("image")
        );
    }

    #[test]
    fn maps_claude_code_history_keeps_visible_text_after_beautified_marker() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "hi？"},
                {
                    "role": "assistant",
                    "content": "<think>draft</think>嗨！\n\n---\n\n下面是美化后内容\n\n你好，有需要我随时帮你。"
                },
                {"role": "user", "content": "继续"}
            ]
        }))
        .unwrap();

        assert_eq!(
            request.history,
            vec![
                json!({"role": "user", "content": "hi？"}),
                json!({"role": "assistant", "content": "你好，有需要我随时帮你。"}),
            ]
        );
    }

    #[test]
    fn maps_claude_code_history_does_not_keep_raw_internal_content_blocks() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "<system-reminder>private Claude Code context</system-reminder>\n\nhi？"
                        }
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        {"type": "thinking", "thinking": "private reasoning"},
                        {"type": "text", "text": "<think>draft</think>你好"}
                    ]
                },
                {"role": "user", "content": "继续"}
            ]
        }))
        .unwrap();

        assert_eq!(
            request.history,
            vec![
                json!({"role": "user", "content": "hi？"}),
                json!({"role": "assistant", "content": "你好"}),
            ]
        );
    }

    #[test]
    fn maps_claude_code_tool_result_history_preserves_image_blocks() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "describe image"},
                {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "toolu_read",
                        "name": "Read",
                        "input": {"file_path": "uploads/agent-flow-preview-debug.png"}
                    }]
                },
                {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "toolu_read",
                        "content": [{
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "aW1hZ2U="
                            }
                        }]
                    }]
                },
                {"role": "user", "content": "next question"}
            ]
        }))
        .unwrap();

        assert_eq!(request.query, "next question");
        assert_eq!(request.history[1]["role"], json!("user"));
        assert_eq!(request.history[1]["content"], json!(""));
        assert_eq!(
            request.history[1]["content_blocks"][0]["type"],
            json!("image")
        );
        assert_eq!(
            request.history[1]["content_blocks"][0]["source"]["media_type"],
            json!("image/png")
        );
    }

    #[test]
    fn maps_latest_tool_result_only_as_protocol_continuation() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "toolu_read",
                        "name": "Read",
                        "input": {"file_path": "uploads/test-01.png"}
                    }]
                },
                {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "toolu_read",
                        "content": "-rw-r--r-- 1 Lw 197121 17907 Jun 12 15:25 uploads/test-01.png"
                    }]
                }
            ]
        }))
        .unwrap();

        assert_eq!(
            request.protocol_request_kind,
            Some(NativeProtocolRequestKind::AnthropicToolResultContinuation)
        );
        assert_eq!(
            request.metadata.as_value()["compatibility"]["anthropic_message_kind"],
            json!("tool_result_only")
        );
    }
}
