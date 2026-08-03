use super::*;

pub(super) fn anthropic_history_entries(
    role: &str,
    content: &Value,
) -> Result<Vec<Value>, AnthropicCompatError> {
    let text = anthropic_history_text_content(content)?;
    if role == "assistant" {
        let mut message = json!({ "role": "assistant", "content": text });
        let reasoning_blocks = anthropic_history_reasoning_blocks(content);
        if !reasoning_blocks.is_empty() {
            message["content_blocks"] = Value::Array(reasoning_blocks);
        }
        let tool_calls = anthropic_history_tool_calls(content);
        if !tool_calls.is_empty() {
            message["tool_calls"] = Value::Array(tool_calls);
        }
        return Ok(vec![message]);
    }

    let mut messages = anthropic_history_tool_results(content);
    let media = query_media_content_blocks(content);
    if !text.trim().is_empty() || media.is_some() || messages.is_empty() {
        let mut message = json!({ "role": role, "content": text });
        if let Some(media) = media {
            message["content_blocks"] = media;
        }
        messages.push(message);
    }
    Ok(messages)
}

pub(super) fn anthropic_history_reasoning_blocks(content: &Value) -> Vec<Value> {
    let Some(blocks) = content.as_array() else {
        return Vec::new();
    };
    if !blocks.iter().any(|block| {
        matches!(
            block.get("type").and_then(Value::as_str),
            Some("thinking" | "redacted_thinking")
        )
    }) {
        return Vec::new();
    }
    blocks
        .iter()
        .filter_map(|block| match block.get("type").and_then(Value::as_str) {
            Some("text") => canonical_anthropic_text_block(block),
            Some("thinking") => {
                let mut reasoning = json!({
                    "type": "reasoning",
                    "text": block.get("thinking")?.as_str()?,
                });
                if let Some(signature) = block.get("signature").and_then(Value::as_str) {
                    reasoning["signature"] = Value::String(signature.to_string());
                }
                Some(reasoning)
            }
            Some("redacted_thinking") => Some(json!({
                "type": "reasoning_redacted",
                "data": block.get("data")?.as_str()?,
            })),
            _ => None,
        })
        .collect()
}

pub(super) fn anthropic_history_text_content(
    content: &Value,
) -> Result<String, AnthropicCompatError> {
    if let Some(text) = content.as_str() {
        return Ok(text.to_string());
    }
    let blocks = content
        .as_array()
        .ok_or_else(|| AnthropicCompatError::invalid("content must be text"))?;
    Ok(blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n"))
}

pub(super) fn anthropic_history_tool_calls(content: &Value) -> Vec<Value> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|block| {
            let id = native_anthropic_tool_call_id(block.get("id")?.as_str()?);
            Some(json!({
                "id": id,
                "name": block.get("name")?.as_str()?,
                "arguments": block.get("input").cloned().unwrap_or_else(|| json!({})),
            }))
        })
        .collect()
}

pub(super) fn anthropic_history_tool_results(content: &Value) -> Vec<Value> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
        .filter_map(|block| {
            let tool_call_id = native_anthropic_tool_call_id(block.get("tool_use_id")?.as_str()?);
            let mut message = json!({
                "role": "tool",
                "content": anthropic_tool_result_text(block),
                "tool_call_id": tool_call_id,
            });
            if let Some(is_error) = block.get("is_error").and_then(Value::as_bool) {
                message["is_error"] = Value::Bool(is_error);
            }
            let content_blocks = anthropic_tool_result_content_blocks("tool", block);
            if !content_blocks.is_empty() {
                message["content_blocks"] = Value::Array(content_blocks);
            }
            Some(message)
        })
        .collect()
}

pub(super) fn native_anthropic_tool_call_id(external_id: &str) -> String {
    decode_anthropic_callback_tool_use_id(external_id)
        .map(|(_, original_id)| original_id)
        .unwrap_or_else(|| external_id.to_string())
}

pub(super) fn anthropic_text_content(content: &Value) -> Result<String, AnthropicCompatError> {
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

pub(super) fn anthropic_current_user_text_content(
    content: &Value,
) -> Result<String, AnthropicCompatError> {
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

pub(super) fn anthropic_blocks_have_visible_user_text(blocks: &[Value]) -> bool {
    blocks.iter().any(|block| {
        block
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|block_type| block_type == "text")
            && block
                .get("text")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.trim().is_empty())
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
                    .is_some_and(|text| !text.trim().is_empty())
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    has_tool_result
}

pub(super) fn query_media_content_blocks(content: &Value) -> Option<Value> {
    let blocks = content.as_array()?;
    let media_blocks = blocks
        .iter()
        .filter(|block| {
            matches!(
                block.get("type").and_then(Value::as_str),
                Some("image" | "document")
            )
        })
        .filter_map(canonical_anthropic_media_block)
        .collect::<Vec<_>>();
    (!media_blocks.is_empty()).then_some(Value::Array(media_blocks))
}

pub(super) fn anthropic_tool_result_text(block: &Value) -> String {
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

pub(super) fn anthropic_content_block_is_media(block: &Value) -> bool {
    matches!(
        block.get("type").and_then(Value::as_str),
        Some("image" | "document")
    )
}

pub(super) fn canonical_anthropic_text_block(block: &Value) -> Option<Value> {
    let text = block.get("text")?.as_str()?.trim();
    (!text.is_empty()).then(|| json!({ "type": "text", "text": text }))
}

pub(super) fn anthropic_tool_result_content_blocks(_role: &str, block: &Value) -> Vec<Value> {
    block
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|entry| match entry.get("type").and_then(Value::as_str) {
                    Some("text") => canonical_anthropic_text_block(entry),
                    Some("image" | "document") => canonical_anthropic_media_block(entry),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn canonical_anthropic_media_block(block: &Value) -> Option<Value> {
    let object = block.as_object()?;
    let block_type = object.get("type")?.as_str()?;
    if !matches!(block_type, "image" | "document") {
        return None;
    }
    let source = object.get("source")?.as_object()?;
    let source_type = source.get("type")?.as_str()?;
    let mut canonical_source = Map::new();
    canonical_source.insert("type".to_string(), Value::String(source_type.to_string()));
    match source_type {
        "base64" => {
            canonical_source.insert(
                "media_type".to_string(),
                Value::String(source.get("media_type")?.as_str()?.to_string()),
            );
            canonical_source.insert(
                "data".to_string(),
                Value::String(source.get("data")?.as_str()?.to_string()),
            );
        }
        "url" => {
            canonical_source.insert(
                "url".to_string(),
                Value::String(source.get("url")?.as_str()?.to_string()),
            );
        }
        "text" if block_type == "document" => {
            if let Some(media_type) = source.get("media_type").and_then(Value::as_str) {
                canonical_source.insert(
                    "media_type".to_string(),
                    Value::String(media_type.to_string()),
                );
            }
            canonical_source.insert(
                "data".to_string(),
                Value::String(source.get("data")?.as_str()?.to_string()),
            );
        }
        _ => return None,
    }
    Some(json!({
        "type": block_type,
        "source": Value::Object(canonical_source),
    }))
}
