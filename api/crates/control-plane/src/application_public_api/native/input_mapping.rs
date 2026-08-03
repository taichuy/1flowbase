use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeInputMappingError {
    SelectorCollision { selector: String },
    InvalidSelector { selector: String },
    InvalidPromptContext,
    InvalidSystemPrompt,
    InvalidRequestContext,
    InvalidAttachments,
}

pub struct NativeInputMapper;

impl NativeInputMapper {
    pub fn map(
        request: &NativeRunRequest,
        mapping: &ApplicationApiMappingConfig,
    ) -> std::result::Result<NativeMappedInput, NativeInputMappingError> {
        let mut node_input_payload = Value::Object(Map::new());
        let input = &mapping.input;

        write_selector(
            &mut node_input_payload,
            &input.query_target,
            Value::String(request.query.clone()),
        )?;
        if let (Some(model), Some(model_target)) = (&request.model, &input.model_target) {
            write_selector(
                &mut node_input_payload,
                model_target,
                Value::String(model.clone()),
            )?;
        }
        write_optional_selector(
            &mut node_input_payload,
            input.inputs_target.as_deref(),
            request.inputs.as_value(),
        )?;
        write_selector(
            &mut node_input_payload,
            &operation_target(input)?,
            serde_json::to_value(request.execution.execution_operation())
                .expect("canonical AI Native operation must serialize"),
        )?;
        if input.inputs_target.is_none() {
            let (start_selector, _) = input.query_target.rsplit_once('.').ok_or_else(|| {
                NativeInputMappingError::InvalidSelector {
                    selector: input.query_target.clone(),
                }
            })?;
            for field in ["tools", "tool_choice"] {
                if let Some(value) = request.inputs.get(field) {
                    write_selector(
                        &mut node_input_payload,
                        &format!("{start_selector}.{field}"),
                        value.clone(),
                    )?;
                }
            }
        }
        let (system, history) = split_system_context_from_history(request)?;
        let native_model_prompt_context = NativeModelPromptContext {
            system: system.clone(),
            messages: history.clone(),
        };
        if !native_model_prompt_context.is_empty() {
            write_selector(
                &mut node_input_payload,
                NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY,
                serde_json::to_value(native_model_prompt_context)
                    .map_err(|_| NativeInputMappingError::InvalidPromptContext)?,
            )?;
        }
        write_optional_selector(
            &mut node_input_payload,
            input.history_target.as_deref(),
            Value::Array(history),
        )?;
        write_optional_selector(
            &mut node_input_payload,
            system_target(input).as_deref(),
            serde_json::to_value(system)
                .map_err(|_| NativeInputMappingError::InvalidSystemPrompt)?,
        )?;
        write_optional_selector(
            &mut node_input_payload,
            input.attachments_target.as_deref(),
            serde_json::to_value(&request.attachments)
                .map_err(|_| NativeInputMappingError::InvalidAttachments)?,
        )?;
        if let Some(envelope) = &request.client_protocol_envelope {
            write_selector(
                &mut node_input_payload,
                CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY,
                client_protocol_envelope_payload(envelope),
            )?;
        }
        if !request.request_context.is_empty() {
            write_selector(
                &mut node_input_payload,
                NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY,
                serde_json::to_value(&request.request_context)
                    .map_err(|_| NativeInputMappingError::InvalidRequestContext)?,
            )?;
        }

        Ok(NativeMappedInput {
            node_input_payload,
            metadata: build_run_metadata(request),
        })
    }
}

pub(super) fn operation_target(
    input: &super::mapping::ApplicationApiMappingInput,
) -> std::result::Result<String, NativeInputMappingError> {
    if let Some(inputs_target) = &input.inputs_target {
        return Ok(format!("{inputs_target}.operation"));
    }
    let (start_selector, _) = input.query_target.rsplit_once('.').ok_or_else(|| {
        NativeInputMappingError::InvalidSelector {
            selector: input.query_target.clone(),
        }
    })?;
    Ok(format!("{start_selector}.operation"))
}

pub(super) fn client_protocol_envelope_payload(envelope: &ProtocolContextEnvelope) -> Value {
    ProviderProtocolContextValue::from_envelope(envelope.clone())
        .expect("the typed protocol context must serialize")
        .original_locator()
        .as_value()
}

pub(super) fn split_system_context_from_history(
    request: &NativeRunRequest,
) -> std::result::Result<(Vec<NativePromptBlock>, Vec<Value>), NativeInputMappingError> {
    let mut system_blocks = request.system.clone();
    let mut history = Vec::new();

    for message in &request.history {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or(NativeInputMappingError::InvalidPromptContext)?;
        let content_value = message
            .get("content")
            .ok_or(NativeInputMappingError::InvalidPromptContext)?;
        if role == "system" {
            system_blocks.extend(native_system_content_blocks(content_value)?);
            continue;
        }
        let content = content_value
            .as_str()
            .ok_or(NativeInputMappingError::InvalidPromptContext)?;
        if !matches!(role, "user" | "assistant" | "tool") {
            return Err(NativeInputMappingError::InvalidPromptContext);
        }
        let mut normalized = Map::new();
        normalized.insert("role".to_string(), Value::String(role.to_owned()));
        normalized.insert("content".to_string(), Value::String(content.to_owned()));
        for field in [
            "name",
            "tool_call_id",
            "is_error",
            "tool_calls",
            "content_blocks",
        ] {
            if let Some(value) = message.get(field) {
                normalized.insert(field.to_string(), value.clone());
            }
        }
        history.push(Value::Object(normalized));
    }

    Ok((system_blocks, history))
}

pub(super) fn native_system_content_blocks(
    value: &Value,
) -> std::result::Result<Vec<NativePromptBlock>, NativeInputMappingError> {
    parse_native_prompt_blocks(value).map_err(|_| NativeInputMappingError::InvalidSystemPrompt)
}

pub(super) fn system_target(input: &super::mapping::ApplicationApiMappingInput) -> Option<String> {
    if let Some(history_target) = input.history_target.as_deref() {
        if let Some(prefix) = history_target.strip_suffix(".history") {
            return Some(format!("{prefix}.system"));
        }
    }

    input
        .inputs_target
        .as_deref()
        .map(|target| format!("{target}.system"))
}
