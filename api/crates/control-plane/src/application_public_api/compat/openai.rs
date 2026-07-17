use serde_json::{json, Map, Value};
use uuid::Uuid;

pub use crate::application_public_api::model_catalog::{
    extract_agent_model_catalog_from_start_node as extract_model_list_from_start_node,
    AgentModelDescriptor as OpenAiCompatibleModel,
};
use crate::application_public_api::native::NativeRunRequest;
use crate::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationProtocol, TranslationReport, TranslationSafeRepresentation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiCompatError {
    pub message: String,
    pub error_type: String,
    pub param: Option<String>,
    pub code: String,
    pub report: TranslationReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiPreviousResponseContext {
    pub response_id: String,
    pub external_user: Option<String>,
    pub external_conversation_id: Option<String>,
    pub answer: Option<String>,
}

impl OpenAiCompatError {
    fn invalid(param: &'static str, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: "invalid_request_error".to_string(),
            param: Some(param.to_string()),
            code: "invalid_request".to_string(),
            report: TranslationReport::new(TranslationProtocol::OpenAiChat),
        }
    }

    fn unsupported(param: impl AsRef<str>) -> Self {
        let param = param.as_ref();
        Self {
            message: format!("{param} is not supported by this endpoint"),
            error_type: "invalid_request_error".to_string(),
            param: Some(param.to_string()),
            code: "unsupported_feature".to_string(),
            report: TranslationReport::new(TranslationProtocol::OpenAiChat),
        }
    }

    fn with_report(mut self, report: TranslationReport) -> Self {
        self.report = report;
        self
    }
}

mod request_translation;

pub use request_translation::{translate_chat_completion_request, translate_response_request};

pub fn map_chat_completion_request(request: Value) -> Result<NativeRunRequest, OpenAiCompatError> {
    translate_chat_completion_request(request).map(|translated| translated.request)
}
pub fn map_response_request(
    request: Value,
    _previous_response: Option<OpenAiPreviousResponseContext>,
) -> Result<NativeRunRequest, OpenAiCompatError> {
    translate_response_request(request).map(|translated| translated.request)
}

fn system_from_parts(parts: Vec<String>) -> Option<String> {
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

pub fn response_id_from_run_id(run_id: Uuid) -> String {
    format!("resp_{run_id}")
}

pub fn run_id_from_response_id(response_id: &str) -> Result<Uuid, OpenAiCompatError> {
    let run_id = response_id
        .strip_prefix("resp_")
        .ok_or_else(|| OpenAiCompatError::invalid("previous_response_id", "invalid response id"))?;
    Uuid::parse_str(run_id)
        .map_err(|_| OpenAiCompatError::invalid("previous_response_id", "invalid response id"))
}

fn reject_unknown_chat_fields(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    for field in object.keys() {
        if matches!(
            field.as_str(),
            "model"
                | "messages"
                | "stream"
                | "user"
                | "metadata"
                | "max_completion_tokens"
                | "max_tokens"
        ) {
            continue;
        }
        let path = format!("$.{field}");
        if matches!(
            field.as_str(),
            "audio"
                | "modalities"
                | "tools"
                | "tool_choice"
                | "function_call"
                | "parallel_tool_calls"
                | "response_format"
                | "reasoning_effort"
                | "temperature"
                | "top_p"
                | "presence_penalty"
                | "frequency_penalty"
                | "seed"
                | "stop"
                | "stream_options"
        ) {
            report.record(
                &path,
                None,
                TranslationDecisionKind::Unsupported,
                Some("this field has no current canonical owner"),
                TranslationSafeRepresentation::Redacted,
            );
            return Err(OpenAiCompatError::unsupported(field).with_report(report.clone()));
        }
        report.record(
            &path,
            None,
            TranslationDecisionKind::Rejected,
            Some("unknown OpenAI Chat field"),
            TranslationSafeRepresentation::Redacted,
        );
        return Err(
            OpenAiCompatError::invalid("body", "unknown OpenAI Chat field")
                .with_report(report.clone()),
        );
    }
    Ok(())
}

fn validate_chat_message_fields(
    message: &Value,
    index: usize,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let object = message
        .as_object()
        .ok_or_else(|| OpenAiCompatError::invalid("messages", "message must be an object"))?;
    for field in object.keys() {
        let path = format!("$.messages[{index}].{field}");
        if matches!(field.as_str(), "role" | "content") {
            continue;
        }
        let kind = if matches!(field.as_str(), "tool_calls" | "tool_call_id" | "name") {
            TranslationDecisionKind::Unsupported
        } else {
            TranslationDecisionKind::Rejected
        };
        report.record(
            &path,
            None,
            kind,
            Some(if kind == TranslationDecisionKind::Unsupported {
                "tool and named-message semantics have no current canonical owner"
            } else {
                "unknown OpenAI Chat message field"
            }),
            TranslationSafeRepresentation::Redacted,
        );
        let error = if kind == TranslationDecisionKind::Unsupported {
            OpenAiCompatError::unsupported("messages")
        } else {
            OpenAiCompatError::invalid("messages", "unknown OpenAI Chat message field")
        };
        return Err(error.with_report(report.clone()));
    }
    let role = object
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| OpenAiCompatError::invalid("messages", "message role is required"))?;
    if !matches!(role, "system" | "developer" | "user" | "assistant") {
        let path = format!("$.messages[{index}].role");
        report.record(
            &path,
            None,
            TranslationDecisionKind::Rejected,
            Some("unsupported OpenAI Chat message role"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            OpenAiCompatError::invalid("messages", "unsupported message role")
                .with_report(report.clone()),
        );
    }
    if !object.contains_key("content") {
        let path = format!("$.messages[{index}].content");
        report.record(
            &path,
            None,
            TranslationDecisionKind::Rejected,
            Some("message content is required"),
            TranslationSafeRepresentation::Absent,
        );
        return Err(
            OpenAiCompatError::invalid("messages", "message content is required")
                .with_report(report.clone()),
        );
    }
    validate_openai_content_parts(
        object.get("content").expect("content exists"),
        index,
        report,
    )
}

fn validate_openai_content_parts(
    content: &Value,
    message_index: usize,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(parts) = content.as_array() else {
        return Ok(());
    };
    for (part_index, part) in parts.iter().enumerate() {
        let object = part.as_object().ok_or_else(|| {
            OpenAiCompatError::invalid("messages", "content part must be an object")
        })?;
        let type_path = format!("$.messages[{message_index}].content[{part_index}].type");
        let part_type = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match part_type {
            "text" | "input_text" | "output_text" | "image_url" | "input_image" => {
                report.record(
                    &type_path,
                    Some("$.query,$.history"),
                    TranslationDecisionKind::Normalized,
                    None,
                    TranslationSafeRepresentation::Present,
                );
                validate_openai_supported_content_part(
                    object,
                    &format!("$.messages[{message_index}].content[{part_index}]"),
                    part_type,
                    "messages",
                    report,
                )?;
            }
            "input_audio" | "file" | "input_file" => {
                report.record(
                    &type_path,
                    None,
                    TranslationDecisionKind::Unsupported,
                    Some("multimodal input has no current canonical owner"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(OpenAiCompatError::unsupported("messages").with_report(report.clone()));
            }
            _ => {
                report.record(
                    &type_path,
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("unknown OpenAI Chat content type"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(
                    OpenAiCompatError::invalid("messages", "unknown content type")
                        .with_report(report.clone()),
                );
            }
        }
    }
    Ok(())
}

fn chat_max_output_tokens(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<Option<u64>, OpenAiCompatError> {
    let max_completion_tokens = object.get("max_completion_tokens");
    let max_tokens = object.get("max_tokens");
    if max_completion_tokens.is_some() && max_tokens.is_some() {
        report.record(
            "$.max_completion_tokens",
            None,
            TranslationDecisionKind::Rejected,
            Some("only one output token limit may be supplied"),
            TranslationSafeRepresentation::Present,
        );
        report.record(
            "$.max_tokens",
            None,
            TranslationDecisionKind::Rejected,
            Some("only one output token limit may be supplied"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::invalid(
            "max_completion_tokens",
            "max_completion_tokens and max_tokens cannot both be supplied",
        )
        .with_report(report.clone()));
    }
    let Some((source_path, value)) = max_completion_tokens
        .map(|value| ("$.max_completion_tokens", value))
        .or_else(|| max_tokens.map(|value| ("$.max_tokens", value)))
    else {
        report.record(
            "$.max_completion_tokens",
            Some("$.execution.model_parameters.max_output_tokens"),
            TranslationDecisionKind::Defaulted,
            Some("provider default output limit"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(None);
    };
    let Some(max_output_tokens) = value.as_u64().filter(|value| *value > 0) else {
        report.record(
            source_path,
            None,
            TranslationDecisionKind::Rejected,
            Some("output token limit must be a positive integer"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::invalid(
            "max_completion_tokens",
            "output token limit must be a positive integer",
        )
        .with_report(report.clone()));
    };
    report.record(
        source_path,
        Some("$.execution.model_parameters.max_output_tokens"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Present,
    );
    Ok(Some(max_output_tokens))
}

fn reject_unknown_response_fields(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    for field in object.keys() {
        if matches!(
            field.as_str(),
            "model"
                | "input"
                | "instructions"
                | "stream"
                | "user"
                | "metadata"
                | "max_output_tokens"
                | "store"
        ) {
            if field == "store" {
                report.record(
                    "$.store",
                    None,
                    TranslationDecisionKind::Dropped,
                    Some("OpenAI server-side storage is not a Native run semantic"),
                    TranslationSafeRepresentation::Present,
                );
            }
            continue;
        }
        let path = format!("$.{field}");
        let kind = if matches!(
            field.as_str(),
            "previous_response_id"
                | "tools"
                | "tool_choice"
                | "parallel_tool_calls"
                | "response_format"
                | "text"
                | "reasoning"
                | "background"
                | "include"
                | "max_tool_calls"
                | "truncation"
        ) {
            TranslationDecisionKind::Unsupported
        } else {
            TranslationDecisionKind::Rejected
        };
        report.record(
            &path,
            None,
            kind,
            Some(if kind == TranslationDecisionKind::Unsupported {
                "this field has no current canonical owner"
            } else {
                "unknown OpenAI Responses field"
            }),
            TranslationSafeRepresentation::Redacted,
        );
        let error = if kind == TranslationDecisionKind::Unsupported {
            OpenAiCompatError::unsupported(field)
        } else {
            OpenAiCompatError::invalid("body", "unknown OpenAI Responses field")
        };
        return Err(error.with_report(report.clone()));
    }
    Ok(())
}

fn response_stream_mode(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<Option<String>, OpenAiCompatError> {
    match object.get("stream") {
        Some(Value::Bool(true)) => {
            report.record(
                "$.stream",
                Some("$.response_mode"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
            Ok(Some("streaming".to_string()))
        }
        Some(Value::Bool(false)) => {
            report.record(
                "$.stream",
                Some("$.response_mode"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
            Ok(None)
        }
        Some(_) => {
            report.record(
                "$.stream",
                None,
                TranslationDecisionKind::Rejected,
                Some("stream must be a boolean"),
                TranslationSafeRepresentation::Present,
            );
            Err(
                OpenAiCompatError::invalid("stream", "stream must be a boolean")
                    .with_report(report.clone()),
            )
        }
        None => {
            report.record(
                "$.stream",
                Some("$.response_mode"),
                TranslationDecisionKind::Defaulted,
                Some("blocking is the default response mode"),
                TranslationSafeRepresentation::Defaulted,
            );
            Ok(None)
        }
    }
}

fn response_metadata(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<Value, OpenAiCompatError> {
    match object.get("metadata") {
        Some(Value::Object(_)) => {
            report.record(
                "$.metadata",
                Some("$.metadata"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            Ok(object.get("metadata").cloned().unwrap_or_else(|| json!({})))
        }
        Some(_) => {
            report.record(
                "$.metadata",
                None,
                TranslationDecisionKind::Rejected,
                Some("metadata must be an object"),
                TranslationSafeRepresentation::Present,
            );
            Err(
                OpenAiCompatError::invalid("metadata", "metadata must be an object")
                    .with_report(report.clone()),
            )
        }
        None => {
            report.record(
                "$.metadata",
                Some("$.metadata"),
                TranslationDecisionKind::Defaulted,
                Some("empty metadata"),
                TranslationSafeRepresentation::Defaulted,
            );
            Ok(json!({}))
        }
    }
}

fn response_max_output_tokens(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<Option<u64>, OpenAiCompatError> {
    let Some(value) = object.get("max_output_tokens") else {
        report.record(
            "$.max_output_tokens",
            Some("$.execution.model_parameters.max_output_tokens"),
            TranslationDecisionKind::Defaulted,
            Some("provider default output limit"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(None);
    };
    let Some(max_output_tokens) = value.as_u64().filter(|value| *value > 0) else {
        report.record(
            "$.max_output_tokens",
            None,
            TranslationDecisionKind::Rejected,
            Some("max_output_tokens must be a positive integer"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::invalid(
            "max_output_tokens",
            "max_output_tokens must be a positive integer",
        )
        .with_report(report.clone()));
    };
    report.record(
        "$.max_output_tokens",
        Some("$.execution.model_parameters.max_output_tokens"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    Ok(Some(max_output_tokens))
}

fn validate_responses_input(
    input: &Value,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    if input.is_string() {
        report.record(
            "$.input",
            Some("$.query"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        return Ok(());
    }
    let items = input
        .as_array()
        .ok_or_else(|| OpenAiCompatError::invalid("input", "input must be text or messages"))?;
    report.record(
        "$.input",
        Some("$.query,$.history"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    for (index, item) in items.iter().enumerate() {
        let object = item.as_object().ok_or_else(|| {
            OpenAiCompatError::invalid("input", "input message must be an object")
        })?;
        let item_type = object.get("type").and_then(Value::as_str);
        if !matches!(item_type, None | Some("message")) {
            let path = format!("$.input[{index}].type");
            let kind = if matches!(
                item_type,
                Some("function_call")
                    | Some("function_call_output")
                    | Some("reasoning")
                    | Some("item_reference")
            ) {
                TranslationDecisionKind::Unsupported
            } else {
                TranslationDecisionKind::Rejected
            };
            report.record(
                &path,
                None,
                kind,
                Some("Responses continuation and tool items have no current canonical owner"),
                TranslationSafeRepresentation::Present,
            );
            let error = if kind == TranslationDecisionKind::Unsupported {
                OpenAiCompatError::unsupported("input")
            } else {
                OpenAiCompatError::invalid("input", "unknown Responses input item type")
            };
            return Err(error.with_report(report.clone()));
        }
        for field in object.keys() {
            if matches!(field.as_str(), "type" | "role" | "content") {
                continue;
            }
            let path = format!("$.input[{index}].{field}");
            report.record(
                &path,
                None,
                TranslationDecisionKind::Rejected,
                Some("unknown Responses input field"),
                TranslationSafeRepresentation::Redacted,
            );
            return Err(
                OpenAiCompatError::invalid("input", "unknown Responses input field")
                    .with_report(report.clone()),
            );
        }
        let role = object.get("role").and_then(Value::as_str).unwrap_or("user");
        if !matches!(role, "system" | "developer" | "user" | "assistant") {
            let path = format!("$.input[{index}].role");
            report.record(
                &path,
                None,
                TranslationDecisionKind::Rejected,
                Some("unsupported Responses message role"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                OpenAiCompatError::invalid("input", "unsupported message role")
                    .with_report(report.clone()),
            );
        }
        if !object.contains_key("content") {
            let path = format!("$.input[{index}].content");
            report.record(
                &path,
                None,
                TranslationDecisionKind::Rejected,
                Some("input content is required"),
                TranslationSafeRepresentation::Absent,
            );
            return Err(
                OpenAiCompatError::invalid("input", "input content is required")
                    .with_report(report.clone()),
            );
        }
        let content_path = format!("$.input[{index}].content");
        report.record(
            &content_path,
            Some("$.query,$.history"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        validate_responses_content_parts(
            object.get("content").expect("content exists"),
            index,
            report,
        )?;
    }
    Ok(())
}

fn validate_responses_content_parts(
    content: &Value,
    message_index: usize,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(parts) = content.as_array() else {
        if content.is_string() {
            return Ok(());
        }
        let path = format!("$.input[{message_index}].content");
        report.record(
            &path,
            None,
            TranslationDecisionKind::Rejected,
            Some("input content must be text or content parts"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::invalid(
            "input",
            "input content must be text or content parts",
        )
        .with_report(report.clone()));
    };

    for (part_index, part) in parts.iter().enumerate() {
        let type_path = format!("$.input[{message_index}].content[{part_index}].type");
        let Some(object) = part.as_object() else {
            report.record(
                &type_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("input content part must be an object"),
                TranslationSafeRepresentation::Present,
            );
            return Err(OpenAiCompatError::invalid(
                "input",
                "input content part must be an object",
            )
            .with_report(report.clone()));
        };
        let part_type = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match part_type {
            "text" | "input_text" | "output_text" | "image_url" | "input_image" => {
                report.record(
                    &type_path,
                    Some("$.query,$.history"),
                    TranslationDecisionKind::Normalized,
                    None,
                    TranslationSafeRepresentation::Present,
                );
                validate_openai_supported_content_part(
                    object,
                    &format!("$.input[{message_index}].content[{part_index}]"),
                    part_type,
                    "input",
                    report,
                )?;
            }
            "input_audio" | "file" | "input_file" => {
                report.record(
                    &type_path,
                    None,
                    TranslationDecisionKind::Unsupported,
                    Some("multimodal input has no current canonical owner"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(OpenAiCompatError::unsupported("input").with_report(report.clone()));
            }
            _ => {
                report.record(
                    &type_path,
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("unknown OpenAI Responses content type"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(OpenAiCompatError::invalid(
                    "input",
                    "unknown OpenAI Responses content type",
                )
                .with_report(report.clone()));
            }
        }
    }
    Ok(())
}

fn validate_openai_supported_content_part(
    object: &Map<String, Value>,
    part_path: &str,
    part_type: &str,
    error_param: &'static str,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    match part_type {
        "text" | "input_text" | "output_text" => {
            reject_unknown_openai_content_part_fields(
                object,
                part_path,
                &["type", "text"],
                error_param,
                report,
            )?;
            let text_path = format!("{part_path}.text");
            if !object.get("text").is_some_and(Value::is_string) {
                report.record(
                    &text_path,
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("text content parts require text"),
                    TranslationSafeRepresentation::Absent,
                );
                return Err(OpenAiCompatError::invalid(
                    error_param,
                    "text content part requires text",
                )
                .with_report(report.clone()));
            }
            report.record(
                &text_path,
                Some("$.query,$.history"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
        }
        "image_url" => {
            reject_unknown_openai_content_part_fields(
                object,
                part_path,
                &["type", "image_url"],
                error_param,
                report,
            )?;
            validate_openai_image_url_value(
                object.get("image_url"),
                &format!("{part_path}.image_url"),
                error_param,
                report,
            )?;
        }
        "input_image" => {
            reject_unknown_openai_content_part_fields(
                object,
                part_path,
                &["type", "image_url", "detail"],
                error_param,
                report,
            )?;
            validate_openai_image_url_value(
                object.get("image_url"),
                &format!("{part_path}.image_url"),
                error_param,
                report,
            )?;
            if let Some(detail) = object.get("detail") {
                let detail_path = format!("{part_path}.detail");
                if !detail.is_string() {
                    report.record(
                        &detail_path,
                        None,
                        TranslationDecisionKind::Rejected,
                        Some("image detail must be text"),
                        TranslationSafeRepresentation::Present,
                    );
                    return Err(OpenAiCompatError::invalid(
                        error_param,
                        "image detail must be text",
                    )
                    .with_report(report.clone()));
                }
                report.record(
                    &detail_path,
                    Some("$.history[].content_blocks[].image_url.detail"),
                    TranslationDecisionKind::Normalized,
                    None,
                    TranslationSafeRepresentation::Present,
                );
            } else {
                report.record(
                    &format!("{part_path}.detail"),
                    Some("$.history[].content_blocks[].image_url.detail"),
                    TranslationDecisionKind::Defaulted,
                    Some("no image detail supplied"),
                    TranslationSafeRepresentation::Defaulted,
                );
            }
        }
        _ => unreachable!("caller validates the OpenAI content part type"),
    }
    Ok(())
}

fn reject_unknown_openai_content_part_fields(
    object: &Map<String, Value>,
    part_path: &str,
    allowed_fields: &[&str],
    error_param: &'static str,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    for field in object.keys() {
        if allowed_fields.contains(&field.as_str()) {
            continue;
        }
        let path = format!("{part_path}.{field}");
        let kind = if field == "file_id" {
            TranslationDecisionKind::Unsupported
        } else {
            TranslationDecisionKind::Rejected
        };
        report.record(
            &path,
            None,
            kind,
            Some(if kind == TranslationDecisionKind::Unsupported {
                "file-backed image input has no current canonical owner"
            } else {
                "unknown OpenAI content-part field"
            }),
            TranslationSafeRepresentation::Redacted,
        );
        let error = if kind == TranslationDecisionKind::Unsupported {
            OpenAiCompatError::unsupported(error_param)
        } else {
            OpenAiCompatError::invalid(error_param, "unknown OpenAI content-part field")
        };
        return Err(error.with_report(report.clone()));
    }
    Ok(())
}

fn validate_openai_image_url_value(
    value: Option<&Value>,
    image_path: &str,
    error_param: &'static str,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(value) = value else {
        report.record(
            image_path,
            None,
            TranslationDecisionKind::Rejected,
            Some("image content part requires image_url"),
            TranslationSafeRepresentation::Absent,
        );
        return Err(OpenAiCompatError::invalid(
            error_param,
            "image content part requires image_url",
        )
        .with_report(report.clone()));
    };
    match value {
        Value::String(_) => report.record(
            image_path,
            Some("$.history[].content_blocks[].image_url.url"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        ),
        Value::Object(image) => {
            for field in image.keys() {
                if matches!(field.as_str(), "url" | "detail") {
                    continue;
                }
                let path = format!("{image_path}.{field}");
                report.record(
                    &path,
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("unknown OpenAI image_url field"),
                    TranslationSafeRepresentation::Redacted,
                );
                return Err(OpenAiCompatError::invalid(
                    error_param,
                    "unknown OpenAI image_url field",
                )
                .with_report(report.clone()));
            }
            if !image.get("url").is_some_and(Value::is_string) {
                report.record(
                    &format!("{image_path}.url"),
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("image_url.url must be text"),
                    TranslationSafeRepresentation::Absent,
                );
                return Err(
                    OpenAiCompatError::invalid(error_param, "image_url.url must be text")
                        .with_report(report.clone()),
                );
            }
            report.record(
                &format!("{image_path}.url"),
                Some("$.history[].content_blocks[].image_url.url"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            if let Some(detail) = image.get("detail") {
                if !detail.is_string() {
                    report.record(
                        &format!("{image_path}.detail"),
                        None,
                        TranslationDecisionKind::Rejected,
                        Some("image_url.detail must be text"),
                        TranslationSafeRepresentation::Present,
                    );
                    return Err(OpenAiCompatError::invalid(
                        error_param,
                        "image_url.detail must be text",
                    )
                    .with_report(report.clone()));
                }
                report.record(
                    &format!("{image_path}.detail"),
                    Some("$.history[].content_blocks[].image_url.detail"),
                    TranslationDecisionKind::Exact,
                    None,
                    TranslationSafeRepresentation::Present,
                );
            } else {
                report.record(
                    &format!("{image_path}.detail"),
                    Some("$.history[].content_blocks[].image_url.detail"),
                    TranslationDecisionKind::Defaulted,
                    Some("no image detail supplied"),
                    TranslationSafeRepresentation::Defaulted,
                );
            }
        }
        _ => {
            report.record(
                image_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("image_url must be text or an object"),
                TranslationSafeRepresentation::Present,
            );
            return Err(OpenAiCompatError::invalid(
                error_param,
                "image_url must be text or an object",
            )
            .with_report(report.clone()));
        }
    }
    Ok(())
}

fn responses_conversation(object: &Map<String, Value>) -> Value {
    let mut conversation = Map::new();
    if let Some(user) = object
        .get("user")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        conversation.insert("user".to_string(), Value::String(user.to_string()));
    }
    Value::Object(conversation)
}

fn responses_input_to_query_and_history(
    input: &Value,
) -> Result<(String, Vec<Value>), OpenAiCompatError> {
    if let Some(text) = input.as_str() {
        return Ok((text.to_string(), Vec::new()));
    }

    let items = input
        .as_array()
        .ok_or_else(|| OpenAiCompatError::invalid("input", "input must be text or messages"))?;
    let last_user_index = items
        .iter()
        .rposition(|item| item.get("role").and_then(Value::as_str) == Some("user"))
        .ok_or_else(|| OpenAiCompatError::invalid("input", "user input is required"))?;

    let mut history = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let message = responses_input_message(item)?;
        if index == last_user_index {
            if let Some(content_blocks) = message.content_blocks {
                history.push(serde_json::json!({
                    "role": message.role,
                    "content": message.content.clone(),
                    "content_blocks": content_blocks,
                }));
            }
            return Ok((message.content, history));
        }

        let mut history_entry = serde_json::json!({
            "role": message.role,
            "content": message.content,
        });
        if let Some(content_blocks) = message.content_blocks {
            history_entry["content_blocks"] = content_blocks;
        }
        history.push(history_entry);
    }

    Err(OpenAiCompatError::invalid(
        "input",
        "user input is required",
    ))
}

#[derive(Debug, Clone, PartialEq)]
struct ResponsesInputMessage {
    role: String,
    content: String,
    content_blocks: Option<Value>,
}

fn responses_input_message(item: &Value) -> Result<ResponsesInputMessage, OpenAiCompatError> {
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
struct OpenAiMappedContent {
    text: String,
    content_blocks: Option<Value>,
}

impl OpenAiMappedContent {
    fn trim(&self) -> &str {
        self.text.trim()
    }
}

fn openai_message_content(message: &Value) -> Result<OpenAiMappedContent, OpenAiCompatError> {
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

fn openai_content(content: &Value) -> Result<OpenAiMappedContent, OpenAiCompatError> {
    if let Some(text) = content.as_str() {
        return Ok(OpenAiMappedContent {
            text: text.to_string(),
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

fn openai_image_content_block(part: &Value) -> Option<Value> {
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::application_public_api::model_catalog::{
        AgentModelCapabilities, AgentModelReasoning,
    };

    #[test]
    fn extracts_start_node_model_list_from_strings_and_objects() {
        let document = json!({
            "graph": {
                "nodes": [
                    {
                        "id": "node-start",
                        "type": "start",
                        "config": {
                            "model_list": [
                                {
                                    "id": "qwen3.6-35b-a3b",
                                    "name": "Qwen 3.6 35B",
                                    "context_window": 128000,
                                    "auto_compact_token_limit": 110000
                                },
                                "deepseek-v4-flash",
                                {"id": "deepseek-v4-flash", "name": "Duplicate"}
                            ]
                        }
                    }
                ]
            }
        });

        assert_eq!(
            extract_model_list_from_start_node(&document),
            vec![
                OpenAiCompatibleModel {
                    id: "qwen3.6-35b-a3b".into(),
                    name: Some("Qwen 3.6 35B".into()),
                    context_window: Some(128000),
                    max_context_window: None,
                    max_output_tokens: None,
                    auto_compact_token_limit: Some(110000),
                    capabilities: AgentModelCapabilities::default(),
                    reasoning: None,
                },
                OpenAiCompatibleModel {
                    id: "deepseek-v4-flash".into(),
                    name: None,
                    context_window: None,
                    max_context_window: None,
                    max_output_tokens: None,
                    auto_compact_token_limit: None,
                    capabilities: AgentModelCapabilities::default(),
                    reasoning: None,
                },
            ]
        );
    }

    #[test]
    fn extracts_default_model_when_start_node_has_no_model_list() {
        let document = json!({
            "graph": {
                "nodes": [
                    {
                        "id": "node-start",
                        "type": "start",
                        "config": {
                            "input_fields": []
                        }
                    }
                ]
            }
        });

        assert_eq!(
            extract_model_list_from_start_node(&document),
            vec![OpenAiCompatibleModel {
                id: "1flowbase".into(),
                name: Some("1flowbase".into()),
                context_window: Some(257000),
                max_context_window: Some(128000),
                max_output_tokens: Some(32000),
                auto_compact_token_limit: Some(218450),
                capabilities: AgentModelCapabilities {
                    reasoning: true,
                    tool_call: true,
                    multimodal: true,
                    structured_output: true,
                },
                reasoning: Some(AgentModelReasoning {
                    default_effort: Some("medium".into()),
                    supported_efforts: vec![
                        "minimal".into(),
                        "low".into(),
                        "medium".into(),
                        "high".into(),
                        "xhigh".into(),
                    ],
                }),
            }]
        );
    }

    #[test]
    fn d2_ac_007_chat_tools_have_an_unsupported_receipt() {
        let error = translate_chat_completion_request(json!({
            "model": "deepseek-v4-flash",
            "messages": [
                { "role": "user", "content": "say hello" }
            ],
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "description": "Read a file",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" }
                            }
                        }
                    }
                }
            ],
            "tool_choice": "auto"
        }))
        .expect_err("tools have no D2 canonical owner");

        assert_eq!(error.param.as_deref(), Some("tools"));
        assert!(error
            .report
            .has_decision("$.tools", TranslationDecisionKind::Unsupported));
    }

    #[test]
    fn d2_ac_007_chat_callback_tool_ids_have_an_unsupported_receipt() {
        let external_tool_call_id = "calltask_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa_call_weather_lookup";

        let error = translate_chat_completion_request(json!({
            "model": "deepseek-v4-flash",
            "messages": [
                { "role": "user", "content": "first question" },
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {
                            "id": external_tool_call_id,
                            "type": "function",
                            "function": {
                                "name": "lookup_weather",
                                "arguments": "{}"
                            }
                        }
                    ]
                },
                {
                    "role": "tool",
                    "tool_call_id": external_tool_call_id,
                    "content": "{\"temperature\":21}"
                },
                { "role": "assistant", "content": "old answer" },
                { "role": "user", "content": "next question" }
            ]
        }))
        .expect_err("callback tool semantics have no D2 canonical owner");

        assert_eq!(error.param.as_deref(), Some("messages"));
        assert!(error.report.has_decision(
            "$.messages[1].tool_calls",
            TranslationDecisionKind::Unsupported
        ));
    }

    #[test]
    fn d2_ac_007_chat_unrecognized_tool_ids_have_an_unsupported_receipt() {
        let error = translate_chat_completion_request(json!({
            "model": "deepseek-v4-flash",
            "messages": [
                { "role": "user", "content": "first question" },
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {
                            "id": "calltask_not-a-valid-callback",
                            "type": "function",
                            "function": {
                                "name": "lookup_weather",
                                "arguments": "{}"
                            }
                        }
                    ]
                },
                {
                    "role": "tool",
                    "tool_call_id": "provider_native_call",
                    "content": "{\"temperature\":21}"
                },
                { "role": "user", "content": "next question" }
            ]
        }))
        .expect_err("tool message semantics have no D2 canonical owner");

        assert_eq!(error.param.as_deref(), Some("messages"));
        assert!(error.report.has_decision(
            "$.messages[1].tool_calls",
            TranslationDecisionKind::Unsupported
        ));
    }

    #[test]
    fn d2_ac_007_legacy_function_call_has_an_unsupported_receipt() {
        let error = translate_chat_completion_request(json!({
            "model": "deepseek-v4-flash",
            "messages": [
                { "role": "user", "content": "say hello" }
            ],
            "function_call": { "name": "read_file" }
        }))
        .expect_err("function_call has no D2 canonical owner");

        assert_eq!(error.param.as_deref(), Some("function_call"));
        assert!(error
            .report
            .has_decision("$.function_call", TranslationDecisionKind::Unsupported));
    }

    #[test]
    fn maps_responses_text_input_into_native_run() {
        let request = map_response_request(
            json!({
                "model": "deepseek-v4-flash",
                "input": "Summarize the incident",
                "user": "external-user-1",
                "metadata": {"trace_id": "trace-responses"},
                "stream": true
            }),
            None,
        )
        .unwrap();

        assert_eq!(request.query, "Summarize the incident");
        assert_eq!(request.model.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(request.response_mode.as_deref(), Some("streaming"));
        assert_eq!(request.conversation["user"], json!("external-user-1"));
        assert_eq!(request.metadata["trace_id"], json!("trace-responses"));
    }

    #[test]
    fn d2_ac_007_responses_tools_have_an_unsupported_receipt() {
        let error = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hi",
            "tools": [
                {
                    "type": "function",
                    "name": "shell",
                    "description": "Run a command",
                    "parameters": {
                        "type": "object",
                        "properties": { "command": { "type": "array" } }
                    },
                    "strict": false
                }
            ]
        }))
        .expect_err("Responses tools have no D2 canonical owner");

        assert_eq!(error.param.as_deref(), Some("tools"));
        assert!(error
            .report
            .has_decision("$.tools", TranslationDecisionKind::Unsupported));
    }

    #[test]
    fn d2_ac_007_responses_function_calls_have_an_unsupported_receipt() {
        let error = translate_response_request(json!({
                "model": "1flowbase",
                "input": [
                    {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "查代码"}]},
                    {"type": "function_call", "call_id": "call_a", "name": "shell", "arguments": "{}"},
                    {"type": "function_call", "call_id": "call_b", "name": "shell", "arguments": "{}"},
                    {"type": "function_call_output", "call_id": "call_a", "output": "a-result"},
                    {"type": "function_call_output", "call_id": "call_b", "output": "b-result"},
                    {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "继续"}]}
                ]
            }))
        .expect_err("Responses function calls have no D2 canonical owner");

        assert_eq!(error.param.as_deref(), Some("input"));
        assert!(error
            .report
            .has_decision("$.input[1].type", TranslationDecisionKind::Unsupported));
    }

    #[test]
    fn d2_ac_007_responses_replay_items_have_an_unsupported_receipt() {
        let error = translate_response_request(json!({
                "model": "1flowbase",
                "input": [
                    {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "看图"}]},
                    {"type": "reasoning", "id": "rs_1", "summary": [], "content": [{"type": "reasoning_text", "text": "想一想"}], "encrypted_content": null},
                    {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "先查目录"}]},
                    {"type": "function_call", "id": "fc_1", "call_id": "call_shell_1", "name": "shell", "arguments": "{\"command\":[\"ls\"]}"},
                    {"type": "function_call_output", "call_id": "call_shell_1", "output": "uploads\nweb"},
                    {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "继续找导航栏代码"}]}
                ]
            }))
        .expect_err("Responses replay items have no D2 canonical owner");

        assert_eq!(error.param.as_deref(), Some("input"));
        assert!(error
            .report
            .has_decision("$.input[1].type", TranslationDecisionKind::Unsupported));
    }

    #[test]
    fn d2_ac_007_previous_response_id_has_an_unsupported_receipt() {
        let error = translate_response_request(json!({
            "model": "deepseek-v4-flash",
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "Continue"}]}],
            "previous_response_id": "resp_11111111-1111-1111-1111-111111111111"
        }))
        .expect_err("previous_response_id has no D2 canonical owner");

        assert_eq!(error.param.as_deref(), Some("previous_response_id"));
        assert!(error.report.has_decision(
            "$.previous_response_id",
            TranslationDecisionKind::Unsupported
        ));
    }
}
