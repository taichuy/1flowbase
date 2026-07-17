use serde_json::{json, Value};

use super::{
    chat_max_output_tokens, openai_message_content, reject_unknown_chat_fields,
    reject_unknown_response_fields, response_max_output_tokens, response_metadata,
    response_stream_mode, responses_conversation, responses_input_to_query_and_history,
    system_from_parts, validate_chat_message_fields, validate_responses_input, OpenAiCompatError,
};
use crate::application_public_api::native::NativeRunRequest;
use crate::application_public_api::protocol_translation::{
    TranslatedNativeRunRequest, TranslationDecisionKind, TranslationProtocol, TranslationReport,
    TranslationSafeRepresentation,
};

pub fn translate_chat_completion_request(
    request: Value,
) -> Result<TranslatedNativeRunRequest, OpenAiCompatError> {
    let mut report = TranslationReport::new(TranslationProtocol::OpenAiChat);
    let object = request
        .as_object()
        .ok_or_else(|| OpenAiCompatError::invalid("body", "request body must be an object"))?;
    reject_unknown_chat_fields(object, &mut report)?;
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| OpenAiCompatError::invalid("model", "model is required"))?;
    report.record(
        "$.model",
        Some("$.model"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| OpenAiCompatError::invalid("messages", "messages is required"))?;
    report.record(
        "$.messages",
        Some("$.query,$.history,$.system"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );

    let last_user_index = messages
        .iter()
        .rposition(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .ok_or_else(|| OpenAiCompatError::invalid("messages", "user message is required"))?;
    let mut system_parts = Vec::new();
    let mut history = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        validate_chat_message_fields(message, index, &mut report)?;
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| OpenAiCompatError::invalid("messages", "message role is required"))?;
        let role_path = format!("$.messages[{index}].role");
        report.record(
            &role_path,
            Some("$.query,$.history,$.system"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        let content =
            openai_message_content(message).map_err(|error| error.with_report(report.clone()))?;
        let content_path = format!("$.messages[{index}].content");
        report.record(
            &content_path,
            Some("$.query,$.history,$.system"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        if index == last_user_index {
            continue;
        }
        if matches!(role, "system" | "developer") {
            if content.content_blocks.is_some() {
                return Err(OpenAiCompatError::unsupported("messages").with_report(report));
            }
            if !content.trim().is_empty() {
                system_parts.push(content.text);
            }
            continue;
        }
        let mut history_entry = serde_json::json!({ "role": role, "content": content.text });
        if let Some(content_blocks) = content.content_blocks {
            history_entry["content_blocks"] = content_blocks;
        }
        history.push(history_entry);
    }
    let latest_user_content = openai_message_content(&messages[last_user_index])
        .map_err(|error| error.with_report(report.clone()))?;
    let query = latest_user_content.text;
    if let Some(content_blocks) = latest_user_content.content_blocks {
        history.push(serde_json::json!({
            "role": "user",
            "content": query.clone(),
            "content_blocks": content_blocks,
        }));
    }

    let response_mode = match object.get("stream") {
        Some(Value::Bool(true)) => {
            report.record(
                "$.stream",
                Some("$.response_mode"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
            Some("streaming".to_string())
        }
        Some(Value::Bool(false)) => {
            report.record(
                "$.stream",
                Some("$.response_mode"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
            None
        }
        Some(_) => {
            return Err(
                OpenAiCompatError::invalid("stream", "stream must be a boolean")
                    .with_report(report),
            );
        }
        None => {
            report.record(
                "$.stream",
                Some("$.response_mode"),
                TranslationDecisionKind::Defaulted,
                Some("blocking is the default response mode"),
                TranslationSafeRepresentation::Defaulted,
            );
            None
        }
    };
    let conversation = object
        .get("user")
        .and_then(Value::as_str)
        .map(|user| serde_json::json!({ "user": user }))
        .unwrap_or_else(|| serde_json::json!({}));
    if object.contains_key("user") {
        report.record(
            "$.user",
            Some("$.conversation.user"),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Redacted,
        );
    }
    let metadata = match object.get("metadata") {
        Some(Value::Object(_)) => {
            report.record(
                "$.metadata",
                Some("$.metadata"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            object.get("metadata").cloned().unwrap_or_else(|| json!({}))
        }
        Some(_) => {
            return Err(
                OpenAiCompatError::invalid("metadata", "metadata must be an object")
                    .with_report(report),
            );
        }
        None => {
            report.record(
                "$.metadata",
                Some("$.metadata"),
                TranslationDecisionKind::Defaulted,
                Some("empty metadata"),
                TranslationSafeRepresentation::Defaulted,
            );
            json!({})
        }
    };
    let execution = match chat_max_output_tokens(object, &mut report)? {
        Some(max_output_tokens) => json!({
            "model_parameters": { "max_output_tokens": max_output_tokens }
        }),
        None => json!({}),
    };

    let mut native = serde_json::json!({
        "query": query,
        "model": model,
        "inputs": {},
        "history": history,
        "conversation": conversation,
        "response_mode": response_mode,
        "metadata": metadata,
        "execution": execution,
    }
    );
    if let Some(system) = system_from_parts(system_parts) {
        native["system"] = Value::String(system);
    }
    if response_mode.is_none() {
        native
            .as_object_mut()
            .expect("native request object")
            .remove("response_mode");
    }

    let request: NativeRunRequest = serde_json::from_value(native).map_err(|_| {
        OpenAiCompatError::invalid("body", "failed to build Native request")
            .with_report(report.clone())
    })?;
    Ok(TranslatedNativeRunRequest { request, report })
}

pub fn translate_response_request(
    request: Value,
) -> Result<TranslatedNativeRunRequest, OpenAiCompatError> {
    let mut report = TranslationReport::new(TranslationProtocol::OpenAiResponses);
    let object = request
        .as_object()
        .ok_or_else(|| OpenAiCompatError::invalid("body", "request body must be an object"))?;
    reject_unknown_response_fields(object, &mut report)?;
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| OpenAiCompatError::invalid("model", "model is required"))?;
    report.record(
        "$.model",
        Some("$.model"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    let input = object
        .get("input")
        .ok_or_else(|| OpenAiCompatError::invalid("input", "input is required"))?;
    validate_responses_input(input, &mut report)?;
    let (query, history) = responses_input_to_query_and_history(input)
        .map_err(|error| error.with_report(report.clone()))?;
    let system = match object.get("instructions") {
        Some(Value::String(value)) if !value.trim().is_empty() => {
            report.record(
                "$.instructions",
                Some("$.system"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            Some(value.trim().to_string())
        }
        Some(Value::String(_)) | None => {
            report.record(
                "$.instructions",
                Some("$.system"),
                TranslationDecisionKind::Defaulted,
                Some("no system instructions"),
                TranslationSafeRepresentation::Defaulted,
            );
            None
        }
        Some(_) => {
            return Err(
                OpenAiCompatError::invalid("instructions", "instructions must be text")
                    .with_report(report),
            );
        }
    };

    let response_mode = response_stream_mode(object, &mut report)?;
    let conversation = responses_conversation(object);
    if object.contains_key("user") {
        report.record(
            "$.user",
            Some("$.conversation.user"),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Redacted,
        );
    }
    let metadata = response_metadata(object, &mut report)?;
    let execution = match response_max_output_tokens(object, &mut report)? {
        Some(max_output_tokens) => json!({
            "model_parameters": { "max_output_tokens": max_output_tokens }
        }),
        None => json!({}),
    };

    let mut native = serde_json::json!({
        "query": query,
        "model": model,
        "inputs": {},
        "history": history,
        "conversation": conversation,
        "response_mode": response_mode,
        "metadata": metadata,
        "execution": execution,
    });
    if let Some(system) = system {
        native["system"] = Value::String(system);
    }
    if response_mode.is_none() {
        native
            .as_object_mut()
            .expect("native request object")
            .remove("response_mode");
    }

    let request: NativeRunRequest = serde_json::from_value(native).map_err(|_| {
        OpenAiCompatError::invalid("body", "failed to build Native request")
            .with_report(report.clone())
    })?;
    Ok(TranslatedNativeRunRequest { request, report })
}
