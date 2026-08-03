use super::*;

pub(super) fn deserialize_native_object<'de, D>(
    deserializer: D,
) -> std::result::Result<NativeObject, D::Error>
where
    D: Deserializer<'de>,
{
    NativeObject::deserialize(deserializer)
}

pub(super) fn deserialize_optional_string_reject_null<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(value) => Ok(Some(value)),
        Value::Null => Err(de::Error::custom("expected string, found null")),
        _ => Err(de::Error::custom("expected string")),
    }
}

pub(super) fn deserialize_native_prompt_blocks<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<NativePromptBlock>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    parse_native_prompt_blocks(&value).map_err(de::Error::custom)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeHistoryEntry {
    role: String,
    content: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tool_call_id: Option<String>,
    #[serde(default)]
    is_error: Option<bool>,
    #[serde(default)]
    tool_calls: Option<Vec<Value>>,
    #[serde(default)]
    content_blocks: Option<Vec<Value>>,
}

pub(super) fn deserialize_native_history<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    let entries = Vec::<NativeHistoryEntry>::deserialize(deserializer)?;
    entries
        .into_iter()
        .map(|entry| {
            if !matches!(
                entry.role.as_str(),
                "system" | "user" | "assistant" | "tool"
            ) {
                return Err(de::Error::custom("unsupported Native history role"));
            }
            if entry.role == "tool" && entry.tool_call_id.is_none() {
                return Err(de::Error::custom(
                    "Native tool history requires tool_call_id",
                ));
            }
            let mut message = Map::new();
            message.insert("role".to_string(), Value::String(entry.role));
            message.insert("content".to_string(), Value::String(entry.content));
            if let Some(name) = entry.name {
                message.insert("name".to_string(), Value::String(name));
            }
            if let Some(tool_call_id) = entry.tool_call_id {
                message.insert("tool_call_id".to_string(), Value::String(tool_call_id));
            }
            if let Some(is_error) = entry.is_error {
                message.insert("is_error".to_string(), Value::Bool(is_error));
            }
            if let Some(tool_calls) = entry.tool_calls {
                message.insert("tool_calls".to_string(), Value::Array(tool_calls));
            }
            if let Some(content_blocks) = entry.content_blocks {
                message.insert("content_blocks".to_string(), Value::Array(content_blocks));
            }
            Ok(Value::Object(message))
        })
        .collect()
}

pub(super) fn build_run_metadata(request: &NativeRunRequest) -> Value {
    let idempotency_key = request.execution.idempotency_key().map(ToOwned::to_owned);
    let external_user = request
        .expand_id
        .clone()
        .or_else(|| string_field(&request.conversation, "user"));
    let external_conversation_id = string_field(&request.conversation, "id");
    let external_trace_id = request.metadata.trace_id().map(ToOwned::to_owned);
    let title = build_flow_run_title(request.title.as_deref(), &request.query);

    json!({
        "model": request.model,
        "execution": request.execution.as_value(),
        "metadata": request.metadata.as_value(),
        "title": title,
        "expand_id": external_user,
        "idempotency_key": idempotency_key,
        "external_user": external_user,
        "external_conversation_id": external_conversation_id,
        "external_trace_id": external_trace_id,
        "request": {
            "conversation": request.conversation.as_value(),
            "response_mode": request.response_mode,
            "stream_options": request.stream_options.as_value()
        }
    })
}

pub(in super::super) fn durable_metadata_from_flow_run(flow_run: &domain::FlowRunRecord) -> Value {
    json!({
        "title": flow_run.title,
        "expand_id": flow_run.external_user,
        "external_user": flow_run.external_user,
        "external_conversation_id": flow_run.external_conversation_id,
        "external_trace_id": flow_run.external_trace_id,
        "idempotency_key": flow_run.idempotency_key,
        "request": {
            "conversation": {
                "id": flow_run.external_conversation_id,
                "user": flow_run.external_user,
            }
        }
    })
}

pub(super) fn published_run_belongs_to_actor(
    flow_run: &domain::FlowRunRecord,
    application_id: Uuid,
    api_key_id: Uuid,
) -> bool {
    flow_run.run_mode == domain::FlowRunMode::PublishedApiRun
        && flow_run.application_id == application_id
        && flow_run.api_key_id == Some(api_key_id)
}

pub(super) fn string_field(object: &NativeObject, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

pub(super) fn write_optional_selector(
    root: &mut Value,
    selector: Option<&str>,
    value: Value,
) -> std::result::Result<(), NativeInputMappingError> {
    let Some(selector) = selector else {
        return Ok(());
    };
    write_selector(root, selector, value)
}

pub(crate) fn write_selector(
    root: &mut Value,
    selector: &str,
    value: Value,
) -> std::result::Result<(), NativeInputMappingError> {
    let parts = selector.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(NativeInputMappingError::InvalidSelector {
            selector: selector.to_string(),
        });
    }

    let mut cursor = root;
    for part in parts.iter().take(parts.len() - 1) {
        let object =
            cursor
                .as_object_mut()
                .ok_or_else(|| NativeInputMappingError::SelectorCollision {
                    selector: selector.to_string(),
                })?;
        cursor = object
            .entry((*part).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }

    let leaf = parts[parts.len() - 1];
    let object =
        cursor
            .as_object_mut()
            .ok_or_else(|| NativeInputMappingError::SelectorCollision {
                selector: selector.to_string(),
            })?;
    if let Some(existing) = object.get_mut(leaf) {
        if let (Some(existing), Value::Object(next)) = (existing.as_object_mut(), value) {
            for (key, value) in next {
                if existing.contains_key(&key) {
                    return Err(NativeInputMappingError::SelectorCollision {
                        selector: format!("{selector}.{key}"),
                    });
                }
                existing.insert(key, value);
            }
            return Ok(());
        }

        return Err(NativeInputMappingError::SelectorCollision {
            selector: selector.to_string(),
        });
    }
    object.insert(leaf.to_string(), value);
    Ok(())
}
