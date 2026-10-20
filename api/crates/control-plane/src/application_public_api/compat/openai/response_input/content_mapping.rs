use super::*;

pub(in super::super) fn parse_openai_tool_arguments(arguments: &Value) -> Value {
    match arguments {
        Value::String(arguments) => {
            serde_json::from_str(arguments).unwrap_or_else(|_| Value::String(arguments.clone()))
        }
        arguments => arguments.clone(),
    }
}

pub(in super::super) fn responses_previous_history(
    previous: Option<&OpenAiPreviousResponseContext>,
) -> Vec<Value> {
    previous
        .and_then(|previous| previous.answer.as_ref())
        .map(|answer| vec![json!({ "role": "assistant", "content": answer })])
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq)]
pub(in super::super) struct ResponsesInputMapping {
    pub(in super::super) query: String,
    pub(in super::super) history: Vec<Value>,
    pub(in super::super) system_parts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(in super::super) struct ResponsesInputMessage {
    pub(super) role: String,
    pub(super) content: String,
    pub(super) content_blocks: Option<Value>,
}

pub(in super::super) fn responses_input_message(
    item: &Value,
) -> Result<ResponsesInputMessage, OpenAiCompatError> {
    let object = item
        .as_object()
        .ok_or_else(|| OpenAiCompatError::invalid("input", "input message must be an object"))?;
    let role = object
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("user")
        .to_string();
    let content = object
        .get("content")
        .ok_or_else(|| OpenAiCompatError::invalid("input", "input content is required"))
        .and_then(openai_content)?;
    Ok(ResponsesInputMessage {
        role,
        content: content.text,
        content_blocks: content.content_blocks,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub(in super::super) struct OpenAiMappedContent {
    pub(in super::super) text: String,
    pub(in super::super) content_blocks: Option<Value>,
}

impl OpenAiMappedContent {
    pub(in super::super) fn trim(&self) -> &str {
        self.text.trim()
    }
}

pub(in super::super) fn openai_message_content(
    message: &Value,
) -> Result<OpenAiMappedContent, OpenAiCompatError> {
    match message.get("content") {
        Some(Value::Null) | None if message.get("tool_calls").is_some() => {
            Ok(OpenAiMappedContent {
                text: String::new(),
                content_blocks: None,
            })
        }
        Some(content) => openai_content(content),
        None => Err(OpenAiCompatError::invalid(
            "messages",
            "message content is required",
        )),
    }
}

pub(in super::super) fn openai_content(
    content: &Value,
) -> Result<OpenAiMappedContent, OpenAiCompatError> {
    if let Some(text) = content.as_str() {
        return Ok(OpenAiMappedContent {
            text: text.to_owned(),
            content_blocks: None,
        });
    }
    let parts = content
        .as_array()
        .ok_or_else(|| OpenAiCompatError::invalid("messages", "content must be text"))?;
    let mut text = String::new();
    let mut blocks = Vec::new();
    let mut has_media_blocks = false;
    for part in parts {
        let part_type = part.get("type").and_then(Value::as_str).unwrap_or_default();
        match part_type {
            "text" | "input_text" | "output_text" => {
                if let Some(value) = part.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(value);
                    blocks.push(json!({ "type": "text", "text": value }));
                }
            }
            "image_url" | "input_image" => {
                let Some(block) = openai_image_content_block(part) else {
                    return Err(OpenAiCompatError::invalid(
                        "messages",
                        "image content is invalid",
                    ));
                };
                has_media_blocks = true;
                blocks.push(block);
            }
            "input_audio" | "file" | "input_file" => {
                return Err(OpenAiCompatError::unsupported("messages"));
            }
            _ => return Err(OpenAiCompatError::unsupported("messages")),
        }
    }
    Ok(OpenAiMappedContent {
        text,
        content_blocks: has_media_blocks.then_some(Value::Array(blocks)),
    })
}

pub(in super::super) fn openai_image_content_block(part: &Value) -> Option<Value> {
    let object = part.as_object()?;
    let image_url = object.get("image_url")?;
    let (url, nested_detail) = match image_url {
        Value::String(url) => (url.clone(), None),
        Value::Object(image) => (
            image.get("url")?.as_str()?.to_string(),
            image
                .get("detail")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        ),
        _ => return None,
    };
    let detail = object
        .get("detail")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or(nested_detail);
    let mut canonical_image_url = Map::new();
    canonical_image_url.insert("url".to_string(), Value::String(url));
    if let Some(detail) = detail {
        canonical_image_url.insert("detail".to_string(), Value::String(detail));
    }
    Some(json!({
        "type": "image_url",
        "image_url": Value::Object(canonical_image_url)
    }))
}

/// Translate a verified callback suffix without extracting a latest user message or
/// moving system/context entries away from their submitted position.
pub(crate) fn continuation_messages(items: &[Value]) -> Result<Vec<Value>, OpenAiCompatError> {
    let mut report = TranslationReport::new(TranslationProtocol::OpenAiResponses);
    validate_responses_items(&Value::Array(items.to_vec()), false, &mut report, true)?;
    items.iter().map(|item| match item.get("type").and_then(Value::as_str) {
        Some("function_call_output") if super::super::is_standalone_function_output(item) => external_context_message(item),
        Some("function_call_output") => Ok(json!({"role":"tool", "tool_call_id":item["call_id"],
            "content":match &item["output"] { Value::String(text) => text.clone(), other => other.to_string() }})),
        None | Some("message") => {
            let message = responses_input_message(item)?;
            if !matches!(message.role.as_str(), "user" | "assistant") {
                return Err(OpenAiCompatError::invalid("input", "this semantic recovery mapper cannot preserve this new context role"));
            }
            let mut value = json!({"role":message.role,"content":message.content});
            if let Some(blocks) = message.content_blocks { value["content_blocks"] = blocks; }
            Ok(value)
        }
        _ => Err(OpenAiCompatError::invalid("input", "unsupported semantic continuation item")),
    }).collect()
}

pub(super) fn external_context_message(item: &Value) -> Result<Value, OpenAiCompatError> {
    let context = extension_contracts::provider_contract::ProviderExternalContext::from_responses_notification(item)
        .map_err(|message| OpenAiCompatError::invalid("input", message))?;
    Ok(json!({"role":"user", "content":"", "external_context":context}))
}
