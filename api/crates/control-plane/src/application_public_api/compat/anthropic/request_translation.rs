use plugin_framework::provider_contract::NativeModelRequestContext;
use serde_json::Value;

use super::{
    anthropic_current_user_text_content, anthropic_max_output_tokens, anthropic_metadata,
    anthropic_response_mode, anthropic_system_content_parts, anthropic_text_content,
    history_content_blocks, metadata_conversation, normalize_anthropic_model_for_native,
    query_media_content_blocks, record_anthropic_system_decision, reject_legacy_anthropic_control,
    reject_unknown_anthropic_fields, validate_anthropic_message, AnthropicCompatError,
};
use crate::application_public_api::client_protocol_envelope::anthropic_messages_envelope_with_beta;
use crate::application_public_api::native::NativeRunRequest;
use crate::application_public_api::protocol_translation::{
    TranslatedNativeRunRequest, TranslationDecisionKind, TranslationProtocol, TranslationReport,
    TranslationSafeRepresentation,
};

pub fn translate_messages_request(
    request: Value,
) -> Result<TranslatedNativeRunRequest, AnthropicCompatError> {
    let mut report = TranslationReport::new(TranslationProtocol::AnthropicMessages);
    let object = request.as_object().ok_or_else(|| {
        AnthropicCompatError::invalid("request body must be an object").with_report(report.clone())
    })?;
    reject_unknown_anthropic_fields(object, &mut report)?;
    let model = object.get("model").and_then(Value::as_str).ok_or_else(|| {
        AnthropicCompatError::invalid("model is required").with_report(report.clone())
    })?;
    let (model, context_beta) = normalize_anthropic_model_for_native(model);
    report.record(
        "$.model",
        Some("$.model"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Present,
    );
    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AnthropicCompatError::invalid("messages is required").with_report(report.clone())
        })?;
    report.record(
        "$.messages",
        Some("$.query,$.history"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );

    let system_parts = anthropic_system_content_parts(object.get("system"), &mut report)
        .map_err(|error| error.with_report(report.clone()))?;
    reject_legacy_anthropic_control(&system_parts, "$.system", &mut report)?;
    record_anthropic_system_decision(object.get("system"), &mut report);
    let last_user_index = messages
        .iter()
        .rposition(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .ok_or_else(|| {
            AnthropicCompatError::invalid("user message is required").with_report(report.clone())
        })?;
    for (index, message) in messages.iter().enumerate() {
        validate_anthropic_message(message, index, &mut report)?;
    }
    let latest_user_content = messages[last_user_index].get("content").ok_or_else(|| {
        AnthropicCompatError::invalid("message content is required").with_report(report.clone())
    })?;
    let latest_user_text = anthropic_current_user_text_content(latest_user_content)
        .map_err(|error| error.with_report(report.clone()))?;
    let query = latest_user_text;
    let latest_user_media_blocks = query_media_content_blocks(latest_user_content);

    let mut history = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let role = message.get("role").and_then(Value::as_str).ok_or_else(|| {
            AnthropicCompatError::invalid("message role is required").with_report(report.clone())
        })?;
        let content_value = message.get("content").ok_or_else(|| {
            AnthropicCompatError::invalid("message content is required").with_report(report.clone())
        })?;
        if index == last_user_index {
            continue;
        }
        let content = anthropic_text_content(content_value)
            .map_err(|error| error.with_report(report.clone()))?;
        let content_blocks = history_content_blocks(role, content_value, false);
        if content.trim().is_empty() && content_blocks.is_none() {
            continue;
        }
        let mut history_entry = serde_json::json!({ "role": role, "content": content });
        if let Some(content_blocks) = content_blocks {
            history_entry["content_blocks"] = content_blocks;
        }
        history.push(history_entry);
    }
    if let Some(content_blocks) = latest_user_media_blocks {
        history.push(serde_json::json!({
            "role": "user",
            "content": "",
            "content_blocks": content_blocks
        }));
    }

    let metadata = anthropic_metadata(object, &mut report)?;
    let conversation = metadata_conversation(Some(&metadata));
    let response_mode = anthropic_response_mode(object, &mut report)?;
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

    let mut native = serde_json::json!({
        "query": query,
        "model": model,
        "inputs": {},
        "history": history,
        "conversation": conversation,
        "response_mode": response_mode,
        "metadata": metadata,
        "request_context": NativeModelRequestContext { end_user_reference }
    });
    if let Some(max_output_tokens) = anthropic_max_output_tokens(object, &mut report)? {
        native["execution"] = serde_json::json!({
            "model_parameters": {
                "max_output_tokens": max_output_tokens
            }
        });
    }
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

    let mut request: NativeRunRequest = serde_json::from_value(native).map_err(|_| {
        AnthropicCompatError::invalid("failed to build Native request").with_report(report.clone())
    })?;
    if let Some(beta) = context_beta {
        request.client_protocol_envelope = Some(anthropic_messages_envelope_with_beta(beta));
    }
    Ok(TranslatedNativeRunRequest { request, report })
}
