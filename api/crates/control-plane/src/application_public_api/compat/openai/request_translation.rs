use std::num::NonZeroU64;

use plugin_framework::provider_contract::{NativeModelRequestContext, NativePromptBlock};
use serde_json::{Map, Value};

use super::{
    chat_max_output_tokens, openai_message_content, reject_unknown_chat_fields,
    reject_unknown_response_fields, response_max_output_tokens, response_stream_mode,
    responses_input_to_native_run_input, system_from_parts, validate_chat_message_fields,
    validate_responses_input, OpenAiCompatError,
};
use crate::application_public_api::native::{
    NativeExecution, NativeObject, NativeRequestMetadata, NativeRunRequest,
};
use crate::application_public_api::protocol_translation::{
    TranslatedNativeRunRequest, TranslationDecisionKind, TranslationProtocol, TranslationReport,
    TranslationSafeRepresentation,
};

pub fn translate_chat_completion_request(
    request: Value,
) -> Result<TranslatedNativeRunRequest, OpenAiCompatError> {
    let mut report = TranslationReport::new(TranslationProtocol::OpenAiChat);
    let object = openai_request_object(&request, &mut report)?;
    reject_unknown_chat_fields(object, &mut report)?;
    let model = required_openai_string(object, "model", &mut report)?;
    let messages = required_openai_array(object, "messages", &mut report)?;

    for (index, message) in messages.iter().enumerate() {
        validate_chat_message_fields(message, index, &mut report).map_err(|error| {
            report.record(
                "$.messages",
                None,
                TranslationDecisionKind::Rejected,
                Some("Chat messages contain an invalid message"),
                TranslationSafeRepresentation::Present,
            );
            error.with_report(report.clone())
        })?;
    }
    let last_user_index = messages
        .iter()
        .rposition(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .ok_or_else(|| {
            reject_openai_required_field(
                &mut report,
                "messages",
                "user message is required",
                TranslationSafeRepresentation::Present,
            )
        })?;
    report.record(
        "$.messages",
        Some("$.query,$.history,$.system"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    let mut system_parts = Vec::new();
    let mut history = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .expect("validated Chat message has a role");
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
            report.record(
                "$.stream",
                None,
                TranslationDecisionKind::Rejected,
                Some("stream must be a boolean"),
                TranslationSafeRepresentation::Present,
            );
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
    let conversation = openai_conversation(object, &mut report)?;
    let metadata = openai_metadata(object, &mut report)?;
    let execution = native_execution(chat_max_output_tokens(object, &mut report)?);
    let request = NativeRunRequest {
        query,
        system: system_from_parts(system_parts)
            .map(NativePromptBlock::text)
            .into_iter()
            .collect(),
        model: Some(model),
        inputs: NativeObject::default(),
        history,
        attachments: Vec::new(),
        conversation,
        expand_id: None,
        response_mode,
        stream_options: NativeObject::default(),
        execution,
        metadata,
        request_context: NativeModelRequestContext::default(),
        title: None,
        client_protocol_envelope: None,
    };
    report
        .ensure_consistent()
        .map_err(|_| OpenAiCompatError::translation_invariant(report.clone()))?;
    Ok(TranslatedNativeRunRequest { request, report })
}

pub fn translate_response_request(
    request: Value,
) -> Result<TranslatedNativeRunRequest, OpenAiCompatError> {
    let mut report = TranslationReport::new(TranslationProtocol::OpenAiResponses);
    let object = openai_request_object(&request, &mut report)?;
    reject_unknown_response_fields(object, &mut report)?;
    let model = required_openai_string(object, "model", &mut report)?;
    let input = required_openai_value(object, "input", &mut report)?;
    validate_responses_input(input, &mut report)?;
    let input_mapping = responses_input_to_native_run_input(input)
        .map_err(|error| error.with_report(report.clone()))?;
    let query = input_mapping.query;
    let history = input_mapping.history;
    let instructions = match object.get("instructions") {
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
            report.record(
                "$.instructions",
                None,
                TranslationDecisionKind::Rejected,
                Some("instructions must be text"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                OpenAiCompatError::invalid("instructions", "instructions must be text")
                    .with_report(report),
            );
        }
    };
    let system = system_from_parts(
        instructions
            .into_iter()
            .chain(input_mapping.system_parts)
            .collect(),
    );

    let response_mode = response_stream_mode(object, &mut report)?;
    let conversation = openai_conversation(object, &mut report)?;
    let metadata = openai_metadata(object, &mut report)?;
    let execution = native_execution(response_max_output_tokens(object, &mut report)?);
    let request = NativeRunRequest {
        query,
        system: system.map(NativePromptBlock::text).into_iter().collect(),
        model: Some(model),
        inputs: NativeObject::default(),
        history,
        attachments: Vec::new(),
        conversation,
        expand_id: None,
        response_mode,
        stream_options: NativeObject::default(),
        execution,
        metadata,
        request_context: NativeModelRequestContext::default(),
        title: None,
        client_protocol_envelope: None,
    };
    report
        .ensure_consistent()
        .map_err(|_| OpenAiCompatError::translation_invariant(report.clone()))?;
    Ok(TranslatedNativeRunRequest { request, report })
}

fn openai_request_object<'a>(
    request: &'a Value,
    report: &mut TranslationReport,
) -> Result<&'a Map<String, Value>, OpenAiCompatError> {
    let Some(object) = request.as_object() else {
        report.record(
            "$.body",
            None,
            TranslationDecisionKind::Rejected,
            Some("request body must be an object"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            OpenAiCompatError::invalid("body", "request body must be an object")
                .with_report(report.clone()),
        );
    };
    Ok(object)
}

fn required_openai_string(
    object: &Map<String, Value>,
    field: &'static str,
    report: &mut TranslationReport,
) -> Result<String, OpenAiCompatError> {
    match object.get(field) {
        Some(Value::String(value)) => {
            report.record(
                &format!("$.{field}"),
                Some(&format!("$.{field}")),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Present,
            );
            Ok(value.clone())
        }
        Some(_) => Err(reject_openai_required_field(
            report,
            field,
            &format!("{field} is required and must be text"),
            TranslationSafeRepresentation::Present,
        )),
        None => Err(reject_openai_required_field(
            report,
            field,
            &format!("{field} is required and must be text"),
            TranslationSafeRepresentation::Absent,
        )),
    }
}

fn required_openai_array<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
    report: &mut TranslationReport,
) -> Result<&'a Vec<Value>, OpenAiCompatError> {
    match object.get(field) {
        Some(Value::Array(values)) => Ok(values),
        Some(_) => Err(reject_openai_required_field(
            report,
            field,
            &format!("{field} is required"),
            TranslationSafeRepresentation::Present,
        )),
        None => Err(reject_openai_required_field(
            report,
            field,
            &format!("{field} is required"),
            TranslationSafeRepresentation::Absent,
        )),
    }
}

fn required_openai_value<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
    report: &mut TranslationReport,
) -> Result<&'a Value, OpenAiCompatError> {
    object.get(field).ok_or_else(|| {
        reject_openai_required_field(
            report,
            field,
            &format!("{field} is required"),
            TranslationSafeRepresentation::Absent,
        )
    })
}

fn reject_openai_required_field(
    report: &mut TranslationReport,
    field: &'static str,
    reason: &str,
    effective_value: TranslationSafeRepresentation,
) -> OpenAiCompatError {
    report.record(
        &format!("$.{field}"),
        None,
        TranslationDecisionKind::Rejected,
        Some(reason),
        effective_value,
    );
    OpenAiCompatError::invalid(field, reason).with_report(report.clone())
}

fn openai_conversation(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<NativeObject, OpenAiCompatError> {
    let mut conversation = NativeObject::default();
    match object.get("user") {
        Some(Value::String(user)) if !user.trim().is_empty() => {
            report.record(
                "$.user",
                Some("$.conversation.user"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            conversation.insert_string("user", user.trim());
        }
        Some(_) => {
            report.record(
                "$.user",
                None,
                TranslationDecisionKind::Rejected,
                Some("user must be non-empty text"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                OpenAiCompatError::invalid("user", "user must be non-empty text")
                    .with_report(report.clone()),
            );
        }
        None => report.record(
            "$.user",
            Some("$.conversation.user"),
            TranslationDecisionKind::Defaulted,
            Some("no external user"),
            TranslationSafeRepresentation::Defaulted,
        ),
    }
    Ok(conversation)
}

fn openai_metadata(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<NativeRequestMetadata, OpenAiCompatError> {
    let Some(metadata) = object.get("metadata") else {
        report.record(
            "$.metadata",
            Some("$.metadata"),
            TranslationDecisionKind::Defaulted,
            Some("empty metadata"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(NativeRequestMetadata::default());
    };
    let Some(metadata) = metadata.as_object() else {
        report.record(
            "$.metadata",
            None,
            TranslationDecisionKind::Rejected,
            Some("metadata must be an object"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            OpenAiCompatError::invalid("metadata", "metadata must be an object")
                .with_report(report.clone()),
        );
    };
    report.record(
        "$.metadata",
        Some("$.metadata"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    let unknown_fields = metadata
        .keys()
        .filter(|field| field.as_str() != "trace_id")
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        "$.metadata",
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "OpenAI metadata field has no canonical owner",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            OpenAiCompatError::invalid("metadata", "unsupported OpenAI metadata field")
                .with_report(report.clone()),
        );
    }
    if let Some(value) = metadata.get("trace_id") {
        if !value.is_string() {
            report.record(
                "$.metadata.trace_id",
                None,
                TranslationDecisionKind::Rejected,
                Some("OpenAI metadata trace_id must be text"),
                TranslationSafeRepresentation::Present,
            );
            return Err(OpenAiCompatError::invalid(
                "metadata",
                "OpenAI metadata trace_id must be text",
            )
            .with_report(report.clone()));
        }
        report.record(
            "$.metadata.trace_id",
            Some("$.metadata.trace_id"),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        return Ok(NativeRequestMetadata::with_trace_id(Some(
            value.as_str().expect("validated text metadata").to_owned(),
        )));
    }
    Ok(NativeRequestMetadata::default())
}

fn native_execution(max_output_tokens: Option<u64>) -> NativeExecution {
    max_output_tokens
        .and_then(NonZeroU64::new)
        .map(NativeExecution::with_max_output_tokens)
        .unwrap_or_default()
}
