use std::num::NonZeroU64;

use plugin_framework::provider_contract::NativeModelRequestContext;
use serde_json::{Map, Value};

use super::{
    anthropic_current_user_text_content, anthropic_history_entries, anthropic_history_tool_results,
    anthropic_inputs, anthropic_max_output_tokens, anthropic_metadata, anthropic_reasoning,
    anthropic_response_mode, anthropic_system_content_parts, metadata_conversation,
    normalize_anthropic_model_for_native, query_media_content_blocks,
    record_anthropic_context_management_decision, record_anthropic_system_decision,
    reject_legacy_anthropic_control, validate_anthropic_message, validate_anthropic_root_fields,
    AnthropicCompatError, AnthropicContextWindowRequest, ANTHROPIC_TYPED_ROOT_FIELDS,
};
use crate::application_public_api::client_protocol_envelope::{
    capture_client_protocol_body, ClientProtocolIngressPolicy,
};
use crate::application_public_api::native::{
    NativeExecution, NativeRequestMetadata, NativeRunRequest, NativeStreamOptions,
};
use crate::application_public_api::protocol_translation::{
    TranslatedNativeRunRequest, TranslationDecisionKind, TranslationProtocol, TranslationReport,
    TranslationSafeRepresentation,
};

pub fn translate_messages_request(
    request: Value,
) -> Result<TranslatedNativeRunRequest, AnthropicCompatError> {
    translate_messages_request_with_context_window(request, None)
}

pub fn translate_messages_request_with_context_window(
    request: Value,
    ingress_context_window: Option<AnthropicContextWindowRequest>,
) -> Result<TranslatedNativeRunRequest, AnthropicCompatError> {
    let mut report = TranslationReport::new(TranslationProtocol::AnthropicMessages);
    let object = anthropic_request_object(&request, &mut report)?;
    validate_anthropic_root_fields(object, &mut report)?;
    record_anthropic_context_management_decision(object.get("context_management"), &mut report)?;
    let protocol_context = capture_client_protocol_body(
        ClientProtocolIngressPolicy::AnthropicMessages,
        object,
        ANTHROPIC_TYPED_ROOT_FIELDS,
    );
    let model = required_anthropic_string(object, "model", &mut report)?;
    let (model, model_context_window) = normalize_anthropic_model_for_native(&model);
    let requested_context_window = model_context_window
        .or(ingress_context_window)
        .map(AnthropicContextWindowRequest::tokens)
        .and_then(NonZeroU64::new);
    let messages = required_anthropic_array(object, "messages", &mut report)?;

    let system_parts =
        anthropic_system_content_parts(object.get("system"), &mut report).map_err(|error| {
            if !report.has_decision("$.system", TranslationDecisionKind::Rejected) {
                report.record(
                    "$.system",
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("Anthropic system contains invalid prompt blocks"),
                    TranslationSafeRepresentation::Present,
                );
            }
            error.with_report(report.clone())
        })?;
    reject_legacy_anthropic_control(&system_parts, "$.system", &mut report)?;
    record_anthropic_system_decision(object.get("system"), &mut report);
    for (index, message) in messages.iter().enumerate() {
        validate_anthropic_message(message, index, &mut report).map_err(|error| {
            report.record(
                "$.messages",
                None,
                TranslationDecisionKind::Rejected,
                Some("Anthropic messages contain an invalid message"),
                TranslationSafeRepresentation::Present,
            );
            error.with_report(report.clone())
        })?;
    }
    let last_user_index = messages
        .iter()
        .rposition(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .ok_or_else(|| {
            reject_anthropic_required_field(
                &mut report,
                "messages",
                "user message is required",
                TranslationSafeRepresentation::Present,
            )
        })?;
    report.record(
        "$.messages",
        Some("$.query,$.history"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
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
        history.extend(
            anthropic_history_entries(role, content_value)
                .map_err(|error| error.with_report(report.clone()))?,
        );
    }
    history.extend(anthropic_history_tool_results(latest_user_content));
    if let Some(content_blocks) = latest_user_media_blocks {
        history.push(serde_json::json!({
            "role": "user",
            "content": "",
            "content_blocks": content_blocks
        }));
    }

    let metadata = anthropic_metadata(object, &mut report)?;
    let conversation = metadata_conversation(&metadata);
    let native_metadata = NativeRequestMetadata::with_trace_id(metadata.trace_id.clone());
    let response_mode = anthropic_response_mode(object, &mut report)?;
    let max_output_tokens =
        anthropic_max_output_tokens(object, &mut report)?.and_then(NonZeroU64::new);
    let reasoning = anthropic_reasoning(object, &mut report)?;
    let execution = NativeExecution::with_model_parameters(
        max_output_tokens,
        requested_context_window,
        reasoning,
    );
    let request = NativeRunRequest {
        query,
        system: system_parts,
        model: Some(model),
        inputs: anthropic_inputs(object, &mut report)?,
        history,
        attachments: Vec::new(),
        conversation,
        expand_id: None,
        response_mode,
        stream_options: NativeStreamOptions::default(),
        execution,
        metadata: native_metadata,
        request_context: NativeModelRequestContext {
            end_user_reference: metadata.user_id,
        },
        title: None,
        client_protocol_envelope: protocol_context,
    };
    report
        .ensure_consistent()
        .map_err(|_| AnthropicCompatError::translation_invariant(report.clone()))?;
    Ok(TranslatedNativeRunRequest { request, report })
}

fn anthropic_request_object<'a>(
    request: &'a Value,
    report: &mut TranslationReport,
) -> Result<&'a Map<String, Value>, AnthropicCompatError> {
    let Some(object) = request.as_object() else {
        report.record(
            "$.body",
            None,
            TranslationDecisionKind::Rejected,
            Some("request body must be an object"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            AnthropicCompatError::invalid("request body must be an object")
                .with_report(report.clone()),
        );
    };
    Ok(object)
}

fn required_anthropic_string(
    object: &Map<String, Value>,
    field: &'static str,
    report: &mut TranslationReport,
) -> Result<String, AnthropicCompatError> {
    match object.get(field) {
        Some(Value::String(value)) => {
            report.record(
                &format!("$.{field}"),
                Some(&format!("$.{field}")),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
            Ok(value.clone())
        }
        Some(_) => Err(reject_anthropic_required_field(
            report,
            field,
            &format!("{field} is required and must be text"),
            TranslationSafeRepresentation::Present,
        )),
        None => Err(reject_anthropic_required_field(
            report,
            field,
            &format!("{field} is required and must be text"),
            TranslationSafeRepresentation::Absent,
        )),
    }
}

fn required_anthropic_array<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
    report: &mut TranslationReport,
) -> Result<&'a Vec<Value>, AnthropicCompatError> {
    match object.get(field) {
        Some(Value::Array(values)) => Ok(values),
        Some(_) => Err(reject_anthropic_required_field(
            report,
            field,
            &format!("{field} is required"),
            TranslationSafeRepresentation::Present,
        )),
        None => Err(reject_anthropic_required_field(
            report,
            field,
            &format!("{field} is required"),
            TranslationSafeRepresentation::Absent,
        )),
    }
}

fn reject_anthropic_required_field(
    report: &mut TranslationReport,
    field: &'static str,
    reason: &str,
    effective_value: TranslationSafeRepresentation,
) -> AnthropicCompatError {
    report.record(
        &format!("$.{field}"),
        None,
        TranslationDecisionKind::Rejected,
        Some(reason),
        effective_value,
    );
    AnthropicCompatError::invalid(reason).with_report(report.clone())
}
