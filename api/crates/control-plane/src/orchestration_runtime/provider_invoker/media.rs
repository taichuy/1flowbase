use super::*;

pub(super) async fn adapt_or_ensure_model_supports_content_blocks<R>(
    repository: &R,
    instance: &domain::ModelProviderInstanceRecord,
    package: &ProviderPackage,
    model_id: &str,
    input: &mut ProviderInvocationInput,
) -> Result<()>
where
    R: ModelProviderRepository,
{
    if !provider_input_has_media_content_blocks(input) {
        return Ok(());
    }

    if selected_model_supports_multimodal(repository, instance, package, model_id).await? {
        return Ok(());
    }

    textualize_media_content_blocks_for_text_model(input);
    Ok(())
}

fn provider_input_has_media_content_blocks(input: &ProviderInvocationInput) -> bool {
    input.messages.iter().any(|message| {
        message
            .content_blocks
            .as_ref()
            .is_some_and(content_blocks_have_media)
    })
}

fn content_blocks_have_media(content_blocks: &Value) -> bool {
    content_blocks.as_array().is_some_and(|blocks| {
        blocks.iter().any(|block| {
            block
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(is_media_block_type)
        })
    })
}

pub(super) fn textualize_media_content_blocks_for_text_model(input: &mut ProviderInvocationInput) {
    let routed_media_tools = routed_media_tool_context(input);
    for message in &mut input.messages {
        let Some(content_blocks) = message.content_blocks.take() else {
            continue;
        };
        let media_blocks = summarize_media_blocks(&content_blocks);
        if media_blocks.as_array().is_none_or(Vec::is_empty) {
            message.content_blocks = Some(content_blocks);
            continue;
        }
        let fallback = if let Some(routed_media_tools) = &routed_media_tools {
            routed_media_guidance_content(routed_media_tools, &media_blocks)
        } else {
            unsupported_media_content(&message.role, &media_blocks)
        };
        if message.content.trim().is_empty() {
            message.content = fallback;
        } else {
            message.content = format!("{}\n{}", message.content.trim_end(), fallback);
        }
        if let Some(remaining_content_blocks) =
            retain_non_text_media_content_blocks(&content_blocks)
        {
            message.content_blocks = Some(remaining_content_blocks);
        }
    }
}

fn routed_media_tool_context(input: &ProviderInvocationInput) -> Option<Value> {
    let tools = input
        .run_context
        .get(VISIBLE_INTERNAL_LLM_MEDIA_TOOLS_CONTEXT_KEY)?
        .as_array()?
        .iter()
        .filter_map(|tool| {
            let object = tool.as_object()?;
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())?;
            let media_kind = object
                .get("media_kind")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|kind| !kind.is_empty())
                .unwrap_or("image");
            Some(json!({
                "name": name,
                "media_kind": media_kind,
            }))
        })
        .collect::<Vec<_>>();

    (!tools.is_empty()).then(|| Value::Array(tools))
}

fn routed_media_guidance_content(routed_media_tools: &Value, media_blocks: &Value) -> String {
    json!({
        "event": "routed_media_content_available",
        "message": "Media content is available in conversation history for routed media tools. Call the routed media tool again with the same media path and task; do not ask the user to re-upload the image.",
        "media_tools": routed_media_tools,
        "media_blocks": media_blocks,
    })
    .to_string()
}

fn unsupported_media_content(role: &ProviderMessageRole, media_blocks: &Value) -> String {
    let (error_code, message_text) = if matches!(role, ProviderMessageRole::Tool) {
        (
            "tool_result_media_unsupported",
            "Tool result contained media blocks that were not injected into the selected text model context.",
        )
    } else {
        (
            "message_media_unsupported",
            "Message contained media blocks that were not injected into the selected text model context.",
        )
    };
    json!({
        "error_code": error_code,
        "message": message_text,
        "recoverable": true,
        "media_blocks": media_blocks,
    })
    .to_string()
}

fn summarize_media_blocks(content_blocks: &Value) -> Value {
    let Some(blocks) = content_blocks.as_array() else {
        return Value::Array(Vec::new());
    };
    Value::Array(blocks.iter().filter_map(summarize_media_block).collect())
}

fn summarize_media_block(block: &Value) -> Option<Value> {
    let block_type = block.get("type").and_then(Value::as_str)?;
    if !is_media_block_type(block_type) {
        return None;
    }
    let mut summary = serde_json::Map::new();
    summary.insert("type".to_string(), Value::String(block_type.to_string()));
    if let Some(source_type) = block
        .get("source")
        .and_then(|source| source.get("type"))
        .and_then(Value::as_str)
    {
        summary.insert(
            "source_type".to_string(),
            Value::String(source_type.to_string()),
        );
    }
    if let Some(media_type) = block
        .get("source")
        .and_then(|source| source.get("media_type"))
        .or_else(|| block.get("media_type"))
        .and_then(Value::as_str)
    {
        summary.insert(
            "media_type".to_string(),
            Value::String(media_type.to_string()),
        );
    }
    if let Some(url) = block
        .get("image_url")
        .and_then(|image_url| image_url.get("url"))
        .or_else(|| block.get("source").and_then(|source| source.get("url")))
        .and_then(Value::as_str)
    {
        summary.insert("url".to_string(), Value::String(summarized_media_url(url)));
    }
    Some(Value::Object(summary))
}

fn summarized_media_url(url: &str) -> String {
    let trimmed = url.trim();
    if !trimmed.starts_with("data:") {
        return trimmed.to_string();
    }
    let prefix = trimmed
        .split_once(',')
        .map(|(prefix, _)| prefix)
        .unwrap_or("data:[redacted]");
    format!("{prefix},[redacted]")
}

fn retain_non_text_media_content_blocks(content_blocks: &Value) -> Option<Value> {
    let blocks = content_blocks.as_array()?;
    let retained = blocks
        .iter()
        .filter(|block| {
            !block
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|block_type| block_type == "text" || is_media_block_type(block_type))
        })
        .cloned()
        .collect::<Vec<_>>();
    (!retained.is_empty()).then_some(Value::Array(retained))
}

fn is_media_block_type(block_type: &str) -> bool {
    matches!(
        block_type,
        "image" | "document" | "image_url" | "input_image"
    )
}

async fn selected_model_supports_multimodal<R>(
    repository: &R,
    instance: &domain::ModelProviderInstanceRecord,
    package: &ProviderPackage,
    model_id: &str,
) -> Result<bool>
where
    R: ModelProviderRepository,
{
    if let Some(supports_multimodal) = instance
        .configured_models
        .iter()
        .find(|model| model.enabled && model.model_id == model_id)
        .and_then(|model| model.supports_multimodal)
    {
        return Ok(supports_multimodal);
    }

    if let Some(cache) = repository.get_catalog_cache(instance.id).await? {
        let models: Vec<ProviderModelDescriptor> = serde_json::from_value(cache.models_json)?;
        if let Some(model) = models.iter().find(|model| model.model_id == model_id) {
            return Ok(model.supports_multimodal);
        }
    }

    if let Some(model) = package
        .predefined_models
        .iter()
        .find(|model| model.model_id == model_id)
    {
        return Ok(model.supports_multimodal);
    }

    Ok(false)
}
