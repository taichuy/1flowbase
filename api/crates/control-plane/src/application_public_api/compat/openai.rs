use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::application_public_api::callback_tool_ids::decode_openai_callback_tool_call_id;

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
    fn translation_invariant(report: TranslationReport) -> Self {
        Self {
            message: "translation receipt invariant violated".to_string(),
            error_type: "server_error".to_string(),
            param: None,
            code: "translation_invariant".to_string(),
            report,
        }
    }

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

    fn unsupported_compaction_profile() -> Self {
        Self {
            message: "Codex compaction profile is not supported by this endpoint".to_string(),
            error_type: "invalid_request_error".to_string(),
            param: Some("x-codex-turn-metadata.compaction.implementation".to_string()),
            code: "unsupported_compaction_profile".to_string(),
            report: TranslationReport::new(TranslationProtocol::OpenAiResponses),
        }
    }

    fn with_report(mut self, report: TranslationReport) -> Self {
        if report.ensure_consistent().is_err() {
            return Self::translation_invariant(report);
        }
        self.report = report;
        self
    }
}

mod compaction;
mod request_translation;

use compaction::classify_response_operation;
pub use compaction::{OpenAiResponsesEndpoint, OpenAiResponsesRequestContext};
pub use request_translation::{
    translate_chat_completion_request, translate_response_request,
    translate_response_request_with_context, translate_response_request_with_context_and_previous,
};

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
    accept_chat_stream_options(object, report)?;
    if let Some(field) = object.keys().find(|field| {
        matches!(
            field.as_str(),
            "audio"
                | "modalities"
                | "function_call"
                | "parallel_tool_calls"
                | "response_format"
                | "temperature"
                | "top_p"
                | "presence_penalty"
                | "frequency_penalty"
                | "seed"
                | "stop"
        )
    }) {
        let path = format!("$.{field}");
        report.record(
            &path,
            None,
            TranslationDecisionKind::Unsupported,
            Some("this field has no current canonical owner"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::unsupported(field).with_report(report.clone()));
    }
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "model"
                    | "messages"
                    | "stream"
                    | "user"
                    | "metadata"
                    | "max_completion_tokens"
                    | "max_tokens"
                    | "audio"
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
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        "$",
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown OpenAI Chat field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            OpenAiCompatError::invalid("body", "unknown OpenAI Chat field")
                .with_report(report.clone()),
        );
    }
    Ok(())
}

fn accept_chat_stream_options(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(value) = object.get("stream_options") else {
        return Ok(());
    };
    let Some(options) = value.as_object() else {
        report.record(
            "$.stream_options",
            None,
            TranslationDecisionKind::Rejected,
            Some("stream_options must be an object"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::invalid(
            "stream_options",
            "stream_options must be an object",
        )
        .with_report(report.clone()));
    };
    if options.keys().any(|field| field != "include_usage")
        || options
            .get("include_usage")
            .is_some_and(|value| !value.is_boolean())
    {
        report.record(
            "$.stream_options",
            None,
            TranslationDecisionKind::Rejected,
            Some("only boolean include_usage is supported in stream_options"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::invalid(
            "stream_options",
            "only boolean include_usage is supported in stream_options",
        )
        .with_report(report.clone()));
    }
    report.record(
        "$.stream_options",
        None,
        TranslationDecisionKind::Dropped,
        Some("compatible streams already project usage when available"),
        TranslationSafeRepresentation::Present,
    );
    Ok(())
}

fn validate_chat_message_fields(
    message: &Value,
    index: usize,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let message_path = format!("$.messages[{index}]");
    let Some(object) = message.as_object() else {
        report.record(
            &message_path,
            None,
            TranslationDecisionKind::Rejected,
            Some("OpenAI Chat messages must be objects"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            OpenAiCompatError::invalid("messages", "message must be an object")
                .with_report(report.clone()),
        );
    };
    report.record(
        &message_path,
        Some("$.query,$.history,$.system"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "role" | "content" | "tool_calls" | "tool_call_id" | "name"
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        &message_path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown OpenAI Chat message field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            OpenAiCompatError::invalid("messages", "unknown OpenAI Chat message field")
                .with_report(report.clone()),
        );
    }
    let role_path = format!("$.messages[{index}].role");
    let Some(role) = object.get("role").and_then(Value::as_str) else {
        report.record(
            &role_path,
            None,
            TranslationDecisionKind::Rejected,
            Some("OpenAI Chat message role must be text"),
            if object.contains_key("role") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        );
        return Err(
            OpenAiCompatError::invalid("messages", "message role is required")
                .with_report(report.clone()),
        );
    };
    if !matches!(role, "system" | "developer" | "user" | "assistant" | "tool") {
        report.record(
            &role_path,
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
    report.record(
        &role_path,
        Some("$.query,$.history,$.system"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Present,
    );
    if !object.contains_key("content") && !object.contains_key("tool_calls") {
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
    if object.get("content").is_none_or(Value::is_null) && object.contains_key("tool_calls") {
        report.record(
            &format!("$.messages[{index}].content"),
            Some("$.history"),
            TranslationDecisionKind::Defaulted,
            Some("assistant tool-call messages may omit text content"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(());
    }
    let content = object.get("content").expect("validated content exists");
    if matches!(content, Value::String(_) | Value::Array(_)) {
        report.record(
            &format!("$.messages[{index}].content"),
            Some("$.query,$.history,$.system"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
    }
    if matches!(role, "system" | "developer") {
        reject_chat_system_media(content, index, report)?;
    }
    validate_openai_content_parts(content, index, report)
}

fn reject_chat_system_media(
    content: &Value,
    message_index: usize,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(parts) = content.as_array() else {
        return Ok(());
    };
    for (part_index, part) in parts.iter().enumerate() {
        let Some(part) = part.as_object() else {
            continue;
        };
        if !matches!(
            part.get("type").and_then(Value::as_str),
            Some("image_url" | "input_image")
        ) {
            continue;
        }
        let type_path = format!("$.messages[{message_index}].content[{part_index}].type");
        report.record(
            &type_path,
            None,
            TranslationDecisionKind::Unsupported,
            Some("system and developer media has no current canonical owner"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::unsupported("messages").with_report(report.clone()));
    }
    Ok(())
}

fn validate_openai_content_parts(
    content: &Value,
    message_index: usize,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(parts) = content.as_array() else {
        if content.is_string() {
            return Ok(());
        }
        let path = format!("$.messages[{message_index}].content");
        report.record(
            &path,
            None,
            TranslationDecisionKind::Rejected,
            Some("Chat message content must be text or content parts"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::invalid(
            "messages",
            "content must be text or content parts",
        )
        .with_report(report.clone()));
    };
    for (part_index, part) in parts.iter().enumerate() {
        let part_path = format!("$.messages[{message_index}].content[{part_index}]");
        let Some(object) = part.as_object() else {
            report.record(
                &part_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("Chat content parts must be objects"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                OpenAiCompatError::invalid("messages", "content part must be an object")
                    .with_report(report.clone()),
            );
        };
        report.record(
            &part_path,
            Some("$.query,$.history"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
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
                    object, &part_path, part_type, "messages", report,
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
                    if object.contains_key("type") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
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

fn validate_response_transport_fields(
    object: &Map<String, Value>,
    transport_requirement: crate::application_public_api::native::ResponsesTransportRequirement,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    if transport_requirement
        == crate::application_public_api::native::ResponsesTransportRequirement::NativePassthrough
    {
        for field in [
            "store",
            "parallel_tool_calls",
            "include",
            "prompt_cache_key",
            "client_metadata",
            "response_format",
            "text",
            "background",
            "max_tool_calls",
            "truncation",
        ] {
            if object.contains_key(field) {
                report.record(
                    &format!("$.{field}"),
                    None,
                    TranslationDecisionKind::Exact,
                    Some("preserved only in native Responses provider transport"),
                    TranslationSafeRepresentation::Redacted,
                );
            }
        }
        let unknown_fields = object
            .keys()
            .filter(|field| {
                !matches!(
                    field.as_str(),
                    "model"
                        | "input"
                        | "instructions"
                        | "stream"
                        | "user"
                        | "metadata"
                        | "max_output_tokens"
                        | "store"
                        | "previous_response_id"
                        | "tools"
                        | "tool_choice"
                        | "parallel_tool_calls"
                        | "response_format"
                        | "text"
                        | "reasoning"
                        | "background"
                        | "include"
                        | "prompt_cache_key"
                        | "client_metadata"
                        | "max_tool_calls"
                        | "truncation"
                )
            })
            .collect::<Vec<_>>();
        report.record_anonymous_unknown_fields(
            "$",
            unknown_fields,
            TranslationDecisionKind::Exact,
            "preserved only in native Responses provider transport",
            TranslationSafeRepresentation::Redacted,
        );
        return Ok(());
    }
    accept_responses_store_hint(object, report)?;
    accept_responses_parallel_tool_calls_hint(object, report)?;
    accept_responses_include_hint(object, report)?;
    accept_responses_codex_metadata_hints(object, report)?;
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "model"
                    | "input"
                    | "instructions"
                    | "stream"
                    | "user"
                    | "metadata"
                    | "max_output_tokens"
                    | "store"
                    | "previous_response_id"
                    | "tools"
                    | "tool_choice"
                    | "parallel_tool_calls"
                    | "response_format"
                    | "text"
                    | "reasoning"
                    | "background"
                    | "include"
                    | "prompt_cache_key"
                    | "client_metadata"
                    | "max_tool_calls"
                    | "truncation"
            )
        })
        .collect::<Vec<_>>();
    report.record_anonymous_unknown_fields(
        "$",
        unknown_fields,
        TranslationDecisionKind::Exact,
        "preserved only in native Responses provider transport",
        TranslationSafeRepresentation::Redacted,
    );
    Ok(())
}

fn responses_transport_requirement(
    object: &Map<String, Value>,
) -> crate::application_public_api::native::ResponsesTransportRequirement {
    use crate::application_public_api::native::ResponsesTransportRequirement;

    let has_native_only_top_level_extension = object.keys().any(|field| {
        !matches!(
            field.as_str(),
            "model"
                | "input"
                | "instructions"
                | "stream"
                | "user"
                | "metadata"
                | "max_output_tokens"
                | "store"
                | "previous_response_id"
                | "tools"
                | "tool_choice"
                | "parallel_tool_calls"
                | "reasoning"
                | "include"
                | "prompt_cache_key"
                | "client_metadata"
        )
    });
    let has_native_only_tools = object
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| tools.iter().any(responses_tool_requires_native_passthrough));
    let has_native_only_tool_choice = object
        .get("tool_choice")
        .is_some_and(responses_tool_choice_requires_native_passthrough);
    let has_native_only_input = object
        .get("input")
        .is_some_and(responses_input_requires_native_passthrough);
    let has_native_only_execution_hint = object.get("store").and_then(Value::as_bool) == Some(true)
        || object.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true)
        || object.get("include").is_some();

    if has_native_only_top_level_extension
        || has_native_only_tools
        || has_native_only_tool_choice
        || has_native_only_input
        || has_native_only_execution_hint
    {
        ResponsesTransportRequirement::NativePassthrough
    } else {
        ResponsesTransportRequirement::SemanticCompatible
    }
}

fn responses_tool_requires_native_passthrough(tool: &Value) -> bool {
    let Some(tool) = tool.as_object() else {
        return false;
    };
    tool.get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind != "function")
        || !tool
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| !name.trim().is_empty())
        || tool.keys().any(|field| {
            !matches!(
                field.as_str(),
                "type" | "name" | "description" | "parameters" | "strict"
            )
        })
        || tool.get("strict").and_then(Value::as_bool) == Some(true)
}

fn responses_tool_choice_requires_native_passthrough(choice: &Value) -> bool {
    match choice {
        Value::String(choice) => !matches!(choice.as_str(), "auto" | "none" | "required"),
        Value::Object(choice) => {
            choice
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind != "function")
                || !choice
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| !name.trim().is_empty())
                || choice
                    .keys()
                    .any(|field| !matches!(field.as_str(), "type" | "name"))
        }
        _ => false,
    }
}

fn responses_input_requires_native_passthrough(input: &Value) -> bool {
    input.as_array().is_some_and(|items| {
        items
            .iter()
            .any(responses_input_item_requires_native_passthrough)
    })
}

fn responses_input_item_requires_native_passthrough(item: &Value) -> bool {
    let Some(item) = item.as_object() else {
        return false;
    };
    match item.get("type").and_then(Value::as_str) {
        None | Some("message") => {
            item.keys()
                .any(|field| !matches!(field.as_str(), "type" | "role" | "content"))
                || item
                    .get("content")
                    .is_some_and(responses_content_requires_native_passthrough)
        }
        Some("function_call") => item
            .keys()
            .any(|field| !matches!(field.as_str(), "type" | "call_id" | "name" | "arguments")),
        Some("function_call_output") => item
            .keys()
            .any(|field| !matches!(field.as_str(), "type" | "call_id" | "output")),
        Some("reasoning") => item.keys().any(|field| {
            !matches!(
                field.as_str(),
                "type" | "summary" | "content" | "encrypted_content"
            )
        }),
        Some("compaction_trigger") => item.keys().any(|field| field != "type"),
        Some(_) => true,
    }
}

fn responses_content_requires_native_passthrough(content: &Value) -> bool {
    content.as_array().is_some_and(|parts| {
        parts.iter().any(|part| {
            let Some(part) = part.as_object() else {
                return false;
            };
            match part.get("type").and_then(Value::as_str) {
                Some("text" | "input_text" | "output_text") => part
                    .keys()
                    .any(|field| !matches!(field.as_str(), "type" | "text")),
                Some("image_url") => part
                    .keys()
                    .any(|field| !matches!(field.as_str(), "type" | "image_url")),
                Some("input_image") => part
                    .keys()
                    .any(|field| !matches!(field.as_str(), "type" | "image_url" | "detail")),
                Some(_) => true,
                None => false,
            }
        })
    })
}

fn accept_responses_codex_metadata_hints(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    if let Some(value) = object.get("prompt_cache_key") {
        if !value.is_string() {
            report.record(
                "$.prompt_cache_key",
                None,
                TranslationDecisionKind::Rejected,
                Some("prompt_cache_key must be text"),
                TranslationSafeRepresentation::Present,
            );
            return Err(OpenAiCompatError::invalid(
                "prompt_cache_key",
                "prompt_cache_key must be text",
            )
            .with_report(report.clone()));
        }
        report.record(
            "$.prompt_cache_key",
            None,
            TranslationDecisionKind::Dropped,
            Some("provider cache affinity has no Native run semantic"),
            TranslationSafeRepresentation::Redacted,
        );
    }
    if let Some(value) = object.get("client_metadata") {
        let Some(metadata) = value.as_object() else {
            report.record(
                "$.client_metadata",
                None,
                TranslationDecisionKind::Rejected,
                Some("client_metadata must be an object of text values"),
                TranslationSafeRepresentation::Present,
            );
            return Err(OpenAiCompatError::invalid(
                "client_metadata",
                "client_metadata must be an object of text values",
            )
            .with_report(report.clone()));
        };
        if metadata.values().any(|value| !value.is_string()) {
            report.record(
                "$.client_metadata",
                None,
                TranslationDecisionKind::Rejected,
                Some("client_metadata must be an object of text values"),
                TranslationSafeRepresentation::Present,
            );
            return Err(OpenAiCompatError::invalid(
                "client_metadata",
                "client_metadata must be an object of text values",
            )
            .with_report(report.clone()));
        }
        report.record(
            "$.client_metadata",
            None,
            TranslationDecisionKind::Dropped,
            Some("Codex diagnostic metadata has no Native run semantic"),
            TranslationSafeRepresentation::Redacted,
        );
    }
    Ok(())
}

fn accept_responses_include_hint(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(value) = object.get("include") else {
        return Ok(());
    };
    let Some(items) = value.as_array() else {
        report.record(
            "$.include",
            None,
            TranslationDecisionKind::Rejected,
            Some("include must be an array of strings"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            OpenAiCompatError::invalid("include", "include must be an array of strings")
                .with_report(report.clone()),
        );
    };
    if items.iter().any(|item| !item.is_string()) {
        report.record(
            "$.include",
            None,
            TranslationDecisionKind::Rejected,
            Some("include must be an array of strings"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            OpenAiCompatError::invalid("include", "include must be an array of strings")
                .with_report(report.clone()),
        );
    }
    if items
        .iter()
        .filter_map(Value::as_str)
        .any(|item| item != "reasoning.encrypted_content")
    {
        report.record(
            "$.include",
            None,
            TranslationDecisionKind::Unsupported,
            Some("only reasoning.encrypted_content is a recognized optional include hint"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::unsupported("include").with_report(report.clone()));
    }
    report.record(
        "$.include",
        None,
        TranslationDecisionKind::Dropped,
        Some("Native responses do not expose encrypted reasoning content"),
        TranslationSafeRepresentation::Present,
    );
    Ok(())
}

fn accept_responses_parallel_tool_calls_hint(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(value) = object.get("parallel_tool_calls") else {
        return Ok(());
    };
    match value.as_bool() {
        Some(false) => {
            report.record(
                "$.parallel_tool_calls",
                None,
                TranslationDecisionKind::Dropped,
                Some("published models do not advertise parallel tool calls"),
                TranslationSafeRepresentation::Present,
            );
            Ok(())
        }
        Some(true) => {
            report.record(
                "$.parallel_tool_calls",
                None,
                TranslationDecisionKind::Unsupported,
                Some("Native execution cannot promise parallel tool calls"),
                TranslationSafeRepresentation::Present,
            );
            Err(OpenAiCompatError::unsupported("parallel_tool_calls").with_report(report.clone()))
        }
        None => {
            report.record(
                "$.parallel_tool_calls",
                None,
                TranslationDecisionKind::Rejected,
                Some("parallel_tool_calls must be a boolean"),
                TranslationSafeRepresentation::Present,
            );
            Err(OpenAiCompatError::invalid(
                "parallel_tool_calls",
                "parallel_tool_calls must be a boolean",
            )
            .with_report(report.clone()))
        }
    }
}

fn accept_responses_store_hint(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(value) = object.get("store") else {
        return Ok(());
    };
    match value.as_bool() {
        Some(false) => {
            report.record(
                "$.store",
                None,
                TranslationDecisionKind::Dropped,
                Some("store=false requests no OpenAI server-side storage"),
                TranslationSafeRepresentation::Present,
            );
            Ok(())
        }
        Some(true) => {
            report.record(
                "$.store",
                None,
                TranslationDecisionKind::Unsupported,
                Some("OpenAI server-side storage is not a Native run semantic"),
                TranslationSafeRepresentation::Present,
            );
            Err(OpenAiCompatError::unsupported("store").with_report(report.clone()))
        }
        None => {
            report.record(
                "$.store",
                None,
                TranslationDecisionKind::Rejected,
                Some("store must be a boolean"),
                TranslationSafeRepresentation::Present,
            );
            Err(
                OpenAiCompatError::invalid("store", "store must be a boolean")
                    .with_report(report.clone()),
            )
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenAiToolMapping {
    ChatCompletions,
    ResponsesSemantic,
    ResponsesNative,
}

fn openai_inputs(
    object: &Map<String, Value>,
    tool_mapping: OpenAiToolMapping,
    report: &mut TranslationReport,
) -> Result<crate::application_public_api::native::NativeObject, OpenAiCompatError> {
    let mut inputs = Map::new();
    if let Some(value) = object.get("tools") {
        let tools = value.as_array().ok_or_else(|| {
            OpenAiCompatError::invalid("tools", "tools must be an array")
                .with_report(report.clone())
        })?;
        let mut normalized = Vec::with_capacity(tools.len());
        for (index, tool) in tools.iter().enumerate() {
            let tool = tool.as_object().ok_or_else(|| {
                OpenAiCompatError::invalid("tools", "tool definitions must be objects")
                    .with_report(report.clone())
            })?;
            if tool_mapping == OpenAiToolMapping::ResponsesNative {
                report.record(
                    &format!("$.tools[{index}]"),
                    None,
                    TranslationDecisionKind::Exact,
                    Some("preserved only in native Responses provider transport"),
                    TranslationSafeRepresentation::Redacted,
                );
                continue;
            }
            let function = if tool_mapping == OpenAiToolMapping::ChatCompletions {
                if tool.get("type").and_then(Value::as_str) != Some("function") {
                    return Err(OpenAiCompatError::invalid(
                        "tools",
                        "only function tools are supported",
                    )
                    .with_report(report.clone()));
                }
                tool.get("function")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        OpenAiCompatError::invalid("tools", "function tool payload is required")
                            .with_report(report.clone())
                    })?
            } else {
                tool
            };
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    OpenAiCompatError::invalid("tools", "tool name is required")
                        .with_report(report.clone())
                })?;
            let input_schema = function
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
            let mut native_tool = json!({
                "name": name,
                "input_schema": input_schema,
                "source": "client"
            });
            if let Some(description) = function.get("description").and_then(Value::as_str) {
                native_tool["description"] = Value::String(description.to_string());
            }
            normalized.push(native_tool);
            report.record(
                &format!("$.tools[{index}]"),
                Some("$.inputs.tools[]"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
        }
        if tool_mapping == OpenAiToolMapping::ResponsesNative {
            report.record(
                "$.tools",
                None,
                TranslationDecisionKind::Exact,
                Some("preserved only in native Responses provider transport"),
                TranslationSafeRepresentation::Redacted,
            );
        } else {
            report.record(
                "$.tools",
                Some("$.inputs.tools"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            inputs.insert("tools".to_string(), Value::Array(normalized));
        }
    }
    if let Some(choice) = object.get("tool_choice") {
        if tool_mapping == OpenAiToolMapping::ResponsesNative {
            report.record(
                "$.tool_choice",
                None,
                TranslationDecisionKind::Exact,
                Some("preserved only in native Responses provider transport"),
                TranslationSafeRepresentation::Redacted,
            );
            return Ok(crate::application_public_api::native::NativeObject::from_map(inputs));
        }
        let normalized = match choice {
            Value::String(choice) if matches!(choice.as_str(), "auto" | "none" | "required") => {
                json!({ "type": choice })
            }
            Value::Object(choice) if tool_mapping == OpenAiToolMapping::ChatCompletions => {
                let name = choice
                    .get("function")
                    .and_then(Value::as_object)
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        OpenAiCompatError::invalid("tool_choice", "tool_choice name is required")
                            .with_report(report.clone())
                    })?;
                json!({ "type": "tool", "name": name })
            }
            Value::Object(choice) => {
                let name = choice.get("name").and_then(Value::as_str).ok_or_else(|| {
                    OpenAiCompatError::invalid("tool_choice", "tool_choice name is required")
                        .with_report(report.clone())
                })?;
                json!({ "type": "tool", "name": name })
            }
            _ => {
                return Err(
                    OpenAiCompatError::invalid("tool_choice", "unsupported tool_choice")
                        .with_report(report.clone()),
                );
            }
        };
        report.record(
            "$.tool_choice",
            Some("$.inputs.tool_choice"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        inputs.insert("tool_choice".to_string(), normalized);
    }
    Ok(crate::application_public_api::native::NativeObject::from_map(inputs))
}

fn openai_reasoning(
    object: &Map<String, Value>,
    chat_completions: bool,
    report: &mut TranslationReport,
) -> Result<
    Option<crate::application_public_api::native::NativeReasoningParameters>,
    OpenAiCompatError,
> {
    let (path, effort) = if chat_completions {
        ("$.reasoning_effort", object.get("reasoning_effort"))
    } else {
        let reasoning = match object.get("reasoning") {
            Some(Value::Object(reasoning)) => Some(reasoning),
            Some(Value::Null) => {
                report.record(
                    "$.reasoning",
                    None,
                    TranslationDecisionKind::Dropped,
                    Some("null reasoning is equivalent to an absent optional parameter"),
                    TranslationSafeRepresentation::Absent,
                );
                None
            }
            Some(_) => {
                return Err(
                    OpenAiCompatError::invalid("reasoning", "reasoning must be an object")
                        .with_report(report.clone()),
                );
            }
            None => None,
        };
        (
            "$.reasoning.effort",
            reasoning.and_then(|value| value.get("effort")),
        )
    };
    let Some(effort) = effort else {
        return Ok(None);
    };
    let effort = effort
        .as_str()
        .filter(|value| matches!(*value, "minimal" | "low" | "medium" | "high" | "xhigh"))
        .ok_or_else(|| {
            OpenAiCompatError::invalid("reasoning", "unsupported reasoning effort")
                .with_report(report.clone())
        })?;
    report.record(
        path,
        Some("$.execution.model_parameters.reasoning.effort"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    Ok(Some(
        crate::application_public_api::native::NativeReasoningParameters::with_enabled_budget_and_effort(
            true,
            None,
            Some(effort),
        ),
    ))
}

fn validate_responses_input(
    input: &Value,
    is_v2_compaction: bool,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    validate_responses_input_items(input, is_v2_compaction, report).map_err(|error| {
        if !report.has_decision("$.input", TranslationDecisionKind::Rejected) {
            report.record(
                "$.input",
                None,
                TranslationDecisionKind::Rejected,
                Some("Responses input contains invalid items"),
                TranslationSafeRepresentation::Present,
            );
        }
        error.with_report(report.clone())
    })
}

fn validate_native_responses_input(
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
    let items = input.as_array().ok_or_else(|| {
        OpenAiCompatError::invalid("input", "input must be text or an array")
            .with_report(report.clone())
    })?;
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("$.input[{index}]");
        let object = item.as_object().ok_or_else(|| {
            OpenAiCompatError::invalid("input", "input items must be objects")
                .with_report(report.clone())
        })?;
        if object.get("type").is_some_and(|value| !value.is_string()) {
            return Err(
                OpenAiCompatError::invalid("input", "input item type must be text")
                    .with_report(report.clone()),
            );
        }
        report.record(
            &item_path,
            None,
            TranslationDecisionKind::Exact,
            Some("preserved only in native Responses provider transport"),
            TranslationSafeRepresentation::Redacted,
        );
    }
    report.record(
        "$.input",
        None,
        TranslationDecisionKind::Exact,
        Some("preserved only in native Responses provider transport"),
        TranslationSafeRepresentation::Redacted,
    );
    Ok(())
}

fn validate_native_mcp_approval_continuation(
    input: &Value,
    previous_response: Option<&OpenAiPreviousResponseContext>,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(items) = input.as_array() else {
        return Ok(());
    };
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            continue;
        };
        if object.get("type").and_then(Value::as_str) != Some("mcp_approval_response") {
            continue;
        }
        let path = format!("$.input[{index}]");
        if previous_response.is_none() {
            report.record(
                &path,
                None,
                TranslationDecisionKind::Rejected,
                Some("MCP approval response requires provider continuation"),
                TranslationSafeRepresentation::Present,
            );
            return Err(OpenAiCompatError::invalid(
                "previous_response_id",
                "mcp_approval_response requires previous_response_id",
            )
            .with_report(report.clone()));
        }
        if !object
            .get("approval_request_id")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(OpenAiCompatError::invalid(
                "input",
                "mcp_approval_response approval_request_id is required",
            )
            .with_report(report.clone()));
        }
        if !object.get("approve").is_some_and(Value::is_boolean) {
            return Err(OpenAiCompatError::invalid(
                "input",
                "mcp_approval_response approve must be a boolean",
            )
            .with_report(report.clone()));
        }
    }
    Ok(())
}

fn validate_responses_input_items(
    input: &Value,
    is_v2_compaction: bool,
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
    let Some(items) = input.as_array() else {
        report.record(
            "$.input",
            None,
            TranslationDecisionKind::Rejected,
            Some("Responses input must be text or messages"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            OpenAiCompatError::invalid("input", "input must be text or messages")
                .with_report(report.clone()),
        );
    };
    let reconstructable_tool_continuation = responses_end_with_reconstructable_tool_output(items)?;
    let mut has_user_message = false;
    let mut has_compaction_trigger = false;
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("$.input[{index}]");
        let Some(object) = item.as_object() else {
            report.record(
                &item_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("Responses input items must be objects"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                OpenAiCompatError::invalid("input", "input message must be an object")
                    .with_report(report.clone()),
            );
        };
        let type_path = format!("$.input[{index}].type");
        let item_type = match object.get("type") {
            Some(Value::String(item_type)) => Some(item_type.as_str()),
            Some(_) => {
                report.record(
                    &type_path,
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("Responses input item type must be text"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(
                    OpenAiCompatError::invalid("input", "input item type must be text")
                        .with_report(report.clone()),
                );
            }
            None => {
                report.record(
                    &type_path,
                    Some("$.input[].type"),
                    TranslationDecisionKind::Defaulted,
                    Some("message is the default Responses input item type"),
                    TranslationSafeRepresentation::Defaulted,
                );
                None
            }
        };
        if item_type == Some("compaction_trigger") && is_v2_compaction {
            let unknown_fields = object
                .keys()
                .filter(|field| field.as_str() != "type")
                .collect::<Vec<_>>();
            if report.record_anonymous_unknown_fields(
                &item_path,
                unknown_fields,
                TranslationDecisionKind::Rejected,
                "unknown V2 compaction trigger field",
                TranslationSafeRepresentation::Present,
            ) > 0
            {
                return Err(OpenAiCompatError::invalid(
                    "input",
                    "unknown V2 compaction trigger field",
                )
                .with_report(report.clone()));
            }
            report.record(
                &item_path,
                Some("$.execution.operation"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Present,
            );
            report.record(
                &type_path,
                Some("$.execution.operation"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Present,
            );
            has_compaction_trigger = true;
            continue;
        }
        if matches!(
            item_type,
            Some("function_call") | Some("function_call_output") | Some("reasoning")
        ) {
            let required_fields: &[&str] = match item_type {
                Some("function_call") => &["call_id", "name", "arguments"],
                Some("function_call_output") => &["call_id", "output"],
                Some("reasoning") => &[],
                _ => unreachable!(),
            };
            for field in required_fields {
                if !object.contains_key(*field) {
                    report.record(
                        &format!("{item_path}.{field}"),
                        None,
                        TranslationDecisionKind::Rejected,
                        Some("required Responses continuation field is missing"),
                        TranslationSafeRepresentation::Absent,
                    );
                    return Err(OpenAiCompatError::invalid(
                        "input",
                        format!("{field} is required"),
                    )
                    .with_report(report.clone()));
                }
            }
            report.record(
                &item_path,
                Some("$.history"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            report.record(
                &type_path,
                Some("$.history"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
            continue;
        }
        report.record(
            &item_path,
            Some("$.query,$.history"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        if !matches!(item_type, None | Some("message")) {
            let kind = if matches!(item_type, Some("item_reference")) {
                TranslationDecisionKind::Unsupported
            } else {
                TranslationDecisionKind::Rejected
            };
            report.record(
                &type_path,
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
        if item_type.is_some() {
            report.record(
                &type_path,
                Some("$.input[].type"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Present,
            );
        }
        let unknown_fields = object
            .keys()
            .filter(|field| !matches!(field.as_str(), "type" | "role" | "content"))
            .collect::<Vec<_>>();
        if report.record_anonymous_unknown_fields(
            &item_path,
            unknown_fields,
            TranslationDecisionKind::Rejected,
            "unknown Responses input field",
            TranslationSafeRepresentation::Present,
        ) > 0
        {
            return Err(
                OpenAiCompatError::invalid("input", "unknown Responses input field")
                    .with_report(report.clone()),
            );
        }
        let role_path = format!("$.input[{index}].role");
        let role_was_explicit = object.contains_key("role");
        let role = match object.get("role") {
            Some(Value::String(role)) => role.as_str(),
            Some(_) => {
                report.record(
                    &role_path,
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("Responses message role must be text"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(
                    OpenAiCompatError::invalid("input", "message role must be text")
                        .with_report(report.clone()),
                );
            }
            None => {
                report.record(
                    &role_path,
                    Some("$.query,$.history"),
                    TranslationDecisionKind::Defaulted,
                    Some("user is the default Responses message role"),
                    TranslationSafeRepresentation::Defaulted,
                );
                "user"
            }
        };
        if !matches!(role, "system" | "developer" | "user" | "assistant") {
            report.record(
                &role_path,
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
        if role_was_explicit {
            report.record(
                &role_path,
                Some(if matches!(role, "system" | "developer") {
                    "$.system"
                } else {
                    "$.query,$.history"
                }),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
        }
        has_user_message |= role == "user";
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
        let content = object.get("content").expect("content exists");
        if matches!(content, Value::String(_) | Value::Array(_)) {
            report.record(
                &content_path,
                Some(if matches!(role, "system" | "developer") {
                    "$.system"
                } else {
                    "$.query,$.history"
                }),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
        }
        if matches!(role, "system" | "developer") {
            reject_responses_system_media(content, index, report)?;
        }
        validate_responses_content_parts(content, index, report)?;
    }
    if !has_user_message
        && !reconstructable_tool_continuation
        && !(is_v2_compaction && has_compaction_trigger)
    {
        report.record(
            "$.input",
            None,
            TranslationDecisionKind::Rejected,
            Some("Responses input requires a user message"),
            TranslationSafeRepresentation::Redacted,
        );
        return Err(
            OpenAiCompatError::invalid("input", "user input is required")
                .with_report(report.clone()),
        );
    }
    report.record(
        "$.input",
        Some("$.query,$.history"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    Ok(())
}

fn reject_responses_system_media(
    content: &Value,
    message_index: usize,
    report: &mut TranslationReport,
) -> Result<(), OpenAiCompatError> {
    let Some(parts) = content.as_array() else {
        return Ok(());
    };
    for (part_index, part) in parts.iter().enumerate() {
        let Some(part) = part.as_object() else {
            continue;
        };
        if !matches!(
            part.get("type").and_then(Value::as_str),
            Some("image_url" | "input_image")
        ) {
            continue;
        }
        let type_path = format!("$.input[{message_index}].content[{part_index}].type");
        report.record(
            &type_path,
            None,
            TranslationDecisionKind::Unsupported,
            Some("Responses system and developer media has no current canonical owner"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::unsupported("input").with_report(report.clone()));
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
        let part_path = format!("$.input[{message_index}].content[{part_index}]");
        let type_path = format!("$.input[{message_index}].content[{part_index}].type");
        let Some(object) = part.as_object() else {
            report.record(
                &part_path,
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
        report.record(
            &part_path,
            Some("$.query,$.history"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
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
                    object, &part_path, part_type, "input", report,
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
                    if object.contains_key("type") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
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
                    if object.contains_key("text") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
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
    let unknown_fields = object
        .keys()
        .filter(|field| !allowed_fields.contains(&field.as_str()) && field.as_str() != "file_id")
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        part_path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown OpenAI content-part field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            OpenAiCompatError::invalid(error_param, "unknown OpenAI content-part field")
                .with_report(report.clone()),
        );
    }
    if object.contains_key("file_id") && !allowed_fields.contains(&"file_id") {
        report.record(
            &format!("{part_path}.file_id"),
            None,
            TranslationDecisionKind::Unsupported,
            Some("file-backed image input has no current canonical owner"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::unsupported(error_param).with_report(report.clone()));
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
            report.record(
                image_path,
                Some("$.history[].content_blocks[].image_url.url"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            let unknown_fields = image
                .keys()
                .filter(|field| !matches!(field.as_str(), "url" | "detail"))
                .collect::<Vec<_>>();
            if report.record_anonymous_unknown_fields(
                image_path,
                unknown_fields,
                TranslationDecisionKind::Rejected,
                "unknown OpenAI image_url field",
                TranslationSafeRepresentation::Present,
            ) > 0
            {
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
                    if image.contains_key("url") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
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

fn responses_input_to_native_run_input(
    input: &Value,
    is_v2_compaction: bool,
) -> Result<ResponsesInputMapping, OpenAiCompatError> {
    if let Some(text) = input.as_str() {
        return Ok(ResponsesInputMapping {
            query: text.to_string(),
            history: Vec::new(),
            system_parts: Vec::new(),
        });
    }

    let items = input
        .as_array()
        .ok_or_else(|| OpenAiCompatError::invalid("input", "input must be text or messages"))?;
    let reconstructable_tool_continuation = responses_end_with_reconstructable_tool_output(items)?;
    let last_user_index = if reconstructable_tool_continuation {
        None
    } else {
        items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                (!is_v2_compaction || !compaction::is_compaction_trigger_item(item))
                    .then_some((index, item))
            })
            .filter(|(_, item)| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    None | Some("message")
                ) && item.get("role").and_then(Value::as_str).unwrap_or("user") == "user"
            })
            .map(|(index, _)| index)
            .next_back()
    };
    if last_user_index.is_none() && !reconstructable_tool_continuation {
        if is_v2_compaction {
            return Ok(ResponsesInputMapping {
                query: String::new(),
                history: Vec::new(),
                system_parts: Vec::new(),
            });
        }
        return Err(OpenAiCompatError::invalid(
            "input",
            "user input is required",
        ));
    }

    let mut history: Vec<Value> = Vec::new();
    let mut system_parts = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if is_v2_compaction && compaction::is_compaction_trigger_item(item) {
            continue;
        }
        match item.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                let tool_call = json!({
                    "id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                    "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
                    "arguments": item.get("arguments").map(parse_openai_tool_arguments).unwrap_or_else(|| json!({})),
                });
                if let Some(existing) = history.last_mut().filter(|entry| {
                    entry.get("role").and_then(Value::as_str) == Some("assistant")
                        && entry.get("tool_calls").is_some_and(Value::is_array)
                }) {
                    existing["tool_calls"]
                        .as_array_mut()
                        .expect("validated tool_calls array")
                        .push(tool_call);
                } else {
                    history.push(json!({
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [tool_call]
                    }));
                }
                continue;
            }
            Some("function_call_output") => {
                let content = match item.get("output") {
                    Some(Value::String(output)) => output.clone(),
                    Some(output) => output.to_string(),
                    None => String::new(),
                };
                history.push(json!({
                    "role": "tool",
                    "content": content,
                    "tool_call_id": item.get("call_id").and_then(Value::as_str).unwrap_or_default()
                }));
                continue;
            }
            Some("reasoning") => {
                history.push(json!({
                    "role": "assistant",
                    "content": "",
                    "reasoning": item.clone()
                }));
                continue;
            }
            _ => {}
        }
        let message = responses_input_message(item)?;
        if Some(index) == last_user_index {
            if let Some(content_blocks) = message.content_blocks {
                history.push(serde_json::json!({
                    "role": message.role,
                    "content": message.content.clone(),
                    "content_blocks": content_blocks,
                }));
            }
            return Ok(ResponsesInputMapping {
                query: message.content,
                history,
                system_parts,
            });
        }

        if matches!(message.role.as_str(), "system" | "developer") {
            if !message.content.trim().is_empty() {
                system_parts.push(message.content);
            }
            continue;
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

    if reconstructable_tool_continuation {
        return Ok(ResponsesInputMapping {
            query: String::new(),
            history,
            system_parts,
        });
    }

    Err(OpenAiCompatError::invalid(
        "input",
        "user input is required",
    ))
}

fn responses_native_input_to_run_input(input: &Value) -> ResponsesInputMapping {
    if let Some(text) = input.as_str() {
        return ResponsesInputMapping {
            query: text.to_string(),
            history: Vec::new(),
            system_parts: Vec::new(),
        };
    }
    let mut messages = input
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| {
            matches!(
                item.get("type").and_then(Value::as_str),
                None | Some("message")
            )
        })
        .filter_map(|item| responses_input_message(item).ok())
        .collect::<Vec<_>>();
    let last_user_index = messages.iter().rposition(|message| message.role == "user");
    let query = last_user_index
        .map(|index| messages.remove(index).content)
        .unwrap_or_default();
    let mut history = Vec::new();
    let mut system_parts = Vec::new();
    for message in messages {
        if matches!(message.role.as_str(), "system" | "developer") {
            if !message.content.trim().is_empty() {
                system_parts.push(message.content);
            }
            continue;
        }
        let mut entry = json!({
            "role": message.role,
            "content": message.content,
        });
        if let Some(content_blocks) = message.content_blocks {
            entry["content_blocks"] = content_blocks;
        }
        history.push(entry);
    }
    ResponsesInputMapping {
        query,
        history,
        system_parts,
    }
}

fn responses_end_with_reconstructable_tool_output(
    items: &[Value],
) -> Result<bool, OpenAiCompatError> {
    if items
        .last()
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
        != Some("function_call_output")
    {
        return Ok(false);
    }
    let call_ids = items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .filter_map(|item| item.get("call_id").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>();
    let outputs_are_paired = items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call_output"))
        .all(|item| {
            item.get("call_id")
                .and_then(Value::as_str)
                .is_some_and(|call_id| call_ids.contains(call_id))
        });
    if call_ids.is_empty() || !outputs_are_paired {
        return Err(OpenAiCompatError::invalid(
            "input",
            "function_call_output requires a matching function_call",
        ));
    }
    Ok(true)
}

fn openai_chat_history_tool_calls(tool_calls: &Value) -> Value {
    Value::Array(
        tool_calls
            .as_array()
            .into_iter()
            .flatten()
            .map(|tool_call| {
                let id = tool_call
                    .get("id")
                    .and_then(Value::as_str)
                    .map(openai_chat_history_tool_call_id)
                    .unwrap_or_default();
                let function = tool_call.get("function").and_then(Value::as_object);
                let name = function
                    .and_then(|value| value.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let arguments = function
                    .and_then(|value| value.get("arguments"))
                    .map(parse_openai_tool_arguments)
                    .unwrap_or_else(|| json!({}));
                json!({ "id": id, "name": name, "arguments": arguments })
            })
            .collect(),
    )
}

fn openai_chat_history_tool_call_id(tool_call_id: &str) -> String {
    decode_openai_callback_tool_call_id(tool_call_id)
        .map(|(_, original_tool_call_id)| original_tool_call_id)
        .unwrap_or_else(|| tool_call_id.to_string())
}

fn parse_openai_tool_arguments(arguments: &Value) -> Value {
    match arguments {
        Value::String(arguments) => {
            serde_json::from_str(arguments).unwrap_or_else(|_| Value::String(arguments.clone()))
        }
        arguments => arguments.clone(),
    }
}

fn responses_previous_history(previous: Option<&OpenAiPreviousResponseContext>) -> Vec<Value> {
    previous
        .and_then(|previous| previous.answer.as_ref())
        .map(|answer| vec![json!({ "role": "assistant", "content": answer })])
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq)]
struct ResponsesInputMapping {
    query: String,
    history: Vec<Value>,
    system_parts: Vec<String>,
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
            text: escape_openai_json_nul_characters(text),
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
                    let value = escape_openai_json_nul_characters(value);
                    text.push_str(&value);
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

fn escape_openai_json_nul_characters(text: &str) -> String {
    text.replace('\0', "\\u0000")
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
    fn ac_001_chat_tools_map_to_native_inputs() {
        let translated = translate_chat_completion_request(json!({
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
        .expect("Chat tools should map to Native inputs");

        assert_eq!(
            translated.request.inputs.as_value()["tools"][0]["name"],
            "read_file"
        );
        assert_eq!(
            translated.request.inputs.as_value()["tool_choice"]["type"],
            "auto"
        );
    }

    #[test]
    fn ac_001_chat_callback_tool_history_maps_to_native() {
        let external_tool_call_id = "calltask_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa_call_weather_lookup";

        let translated = translate_chat_completion_request(json!({
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
        .expect("callback history should map to Native");
        assert_eq!(
            translated.request.history[1]["tool_calls"][0]["id"],
            "call_weather_lookup"
        );
        assert_eq!(
            translated.request.history[2]["tool_call_id"],
            "call_weather_lookup"
        );
    }

    #[test]
    fn ac_001_chat_provider_native_tool_ids_are_preserved() {
        let translated = translate_chat_completion_request(json!({
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
        .expect("provider-native tool ids should be preserved");
        assert_eq!(
            translated.request.history[1]["tool_calls"][0]["id"],
            "calltask_not-a-valid-callback"
        );
        assert_eq!(
            translated.request.history[2]["tool_call_id"],
            "provider_native_call"
        );
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
        assert_eq!(request.metadata.trace_id(), Some("trace-responses"));
    }

    #[test]
    fn codex_store_false_is_a_dropped_no_storage_hint() {
        let translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "store": false
        }))
        .expect("store=false should not require OpenAI server-side storage");

        assert!(translated
            .report
            .has_decision("$.store", TranslationDecisionKind::Dropped));
    }

    #[test]
    fn codex_parallel_tool_calls_false_matches_published_capability() {
        let translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "parallel_tool_calls": false
        }))
        .expect("parallel_tool_calls=false matches the published model capability");

        assert!(translated
            .report
            .has_decision("$.parallel_tool_calls", TranslationDecisionKind::Dropped));
    }

    #[test]
    fn d4_ac_016_reasoning_encrypted_content_include_is_exact_native_transport() {
        let mut translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "include": ["reasoning.encrypted_content"]
        }))
        .expect("encrypted reasoning include should remain in native Responses transport");

        assert!(translated
            .report
            .has_decision("$.include", TranslationDecisionKind::Exact));
        let payload = translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("include should remain in ephemeral provider transport");
        assert_eq!(
            payload.wire_body()["include"][0],
            "reasoning.encrypted_content"
        );
    }

    #[test]
    fn codex_null_reasoning_is_an_absent_optional_parameter() {
        let translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "reasoning": null
        }))
        .expect("Codex null reasoning should be equivalent to an absent optional parameter");

        assert!(translated
            .report
            .has_decision("$.reasoning", TranslationDecisionKind::Dropped));
        assert!(translated
            .request
            .execution
            .model_parameters()
            .and_then(|parameters| parameters.reasoning())
            .is_none());
    }

    #[test]
    fn codex_cache_and_client_metadata_are_typed_optional_hints() {
        let translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "prompt_cache_key": "thread-1",
            "client_metadata": {
                "session_id": "session-1",
                "thread_id": "thread-1"
            }
        }))
        .expect("Codex cache and diagnostic metadata are optional Native hints");

        assert!(translated
            .report
            .has_decision("$.prompt_cache_key", TranslationDecisionKind::Dropped));
        assert!(translated
            .report
            .has_decision("$.client_metadata", TranslationDecisionKind::Dropped));
    }

    #[test]
    fn codex_metadata_hints_retain_their_wire_types() {
        for (field, value) in [
            ("prompt_cache_key", json!(42)),
            ("client_metadata", json!({"session_id": 42})),
        ] {
            let mut request = json!({"model": "1flowbase", "input": "hello"});
            request[field] = value;
            let error = translate_response_request(request)
                .expect_err("Codex metadata hint wire types must remain explicit");
            assert_eq!(error.param.as_deref(), Some(field));
            assert!(error
                .report
                .has_decision(&format!("$.{field}"), TranslationDecisionKind::Rejected));
        }
    }

    #[test]
    fn d4_ac_016_unknown_include_remains_exact_in_native_transport() {
        let mut translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "include": ["message.output_text"]
        }))
        .expect("unknown include projections should remain in native Responses transport");

        assert!(translated
            .report
            .has_decision("$.include", TranslationDecisionKind::Exact));
        let payload = translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("include should remain in ephemeral provider transport");
        assert_eq!(payload.wire_body()["include"][0], "message.output_text");
    }

    #[test]
    fn d4_ac_016_untyped_include_remains_exact_in_native_transport() {
        let mut translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "include": "reasoning.encrypted_content"
        }))
        .expect("untyped include should remain opaque in native Responses transport");

        assert!(translated
            .report
            .has_decision("$.include", TranslationDecisionKind::Exact));
        let payload = translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("untyped include should remain in ephemeral provider transport");
        assert_eq!(
            payload.wire_body()["include"],
            "reasoning.encrypted_content"
        );
    }

    #[test]
    fn d4_ac_016_parallel_tool_calls_true_remains_exact_in_native_transport() {
        let mut translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "parallel_tool_calls": true
        }))
        .expect("parallel tool calls should remain in native Responses transport");

        assert!(translated
            .report
            .has_decision("$.parallel_tool_calls", TranslationDecisionKind::Exact));
        let payload = translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("parallel tool calls should remain in ephemeral provider transport");
        assert_eq!(payload.wire_body()["parallel_tool_calls"], true);
    }

    #[test]
    fn responses_parallel_tool_calls_requires_a_boolean() {
        let error = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hello",
            "parallel_tool_calls": "false"
        }))
        .expect_err("parallel_tool_calls must retain its boolean wire type");

        assert_eq!(error.param.as_deref(), Some("parallel_tool_calls"));
        assert_eq!(error.code, "invalid_request");
        assert!(error
            .report
            .has_decision("$.parallel_tool_calls", TranslationDecisionKind::Rejected));
    }

    #[test]
    fn opencode_chat_stream_options_include_usage_is_a_dropped_hint() {
        let translated = translate_chat_completion_request(json!({
            "model": "1flowbase",
            "stream": true,
            "stream_options": { "include_usage": true },
            "messages": [{ "role": "user", "content": "hello" }]
        }))
        .expect("compatible streaming already projects usage when available");

        assert!(translated
            .report
            .has_decision("$.stream_options", TranslationDecisionKind::Dropped));
    }

    #[test]
    fn stale_chat_tool_output_escapes_nul_before_native_history() {
        let translated = translate_chat_completion_request(json!({
            "model": "1flowbase",
            "messages": [
                { "role": "user", "content": "run command" },
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_shell",
                        "type": "function",
                        "function": { "name": "shell", "arguments": "{}" }
                    }]
                },
                { "role": "tool", "tool_call_id": "call_shell", "content": "STDERR:\n\0after" },
                { "role": "user", "content": "continue" }
            ]
        }))
        .expect("NUL tool history should remain representable in PostgreSQL JSON");

        assert_eq!(
            translated.request.history[2]["content"],
            json!("STDERR:\n\\u0000after")
        );
    }

    #[test]
    fn ac_002_responses_tools_map_to_native_inputs() {
        let translated = translate_response_request(json!({
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
        .expect("Responses tools should map to Native inputs");
        assert_eq!(
            translated.request.inputs.as_value()["tools"][0]["name"],
            "shell"
        );
        assert_eq!(
            translated.request.metadata.responses_transport_requirement(),
            crate::application_public_api::native::ResponsesTransportRequirement::SemanticCompatible
        );
    }

    #[test]
    fn d4_ac_001_responses_classifier_marks_opaque_tools_choices_items_and_extensions_native() {
        for request in [
            json!({
                "model": "1flowbase",
                "input": "hi",
                "tools": [{"type": "web_search_preview"}]
            }),
            json!({
                "model": "1flowbase",
                "input": "hi",
                "tool_choice": {"type": "hosted_tool", "name": "search"}
            }),
            json!({
                "model": "1flowbase",
                "input": [{"type": "item_reference", "id": "item_1"}]
            }),
            json!({
                "model": "1flowbase",
                "input": "hi",
                "future_responses_extension": {"opaque": true}
            }),
            json!({
                "model": "1flowbase",
                "input": "hi",
                "parallel_tool_calls": true
            }),
            json!({
                "model": "1flowbase",
                "input": "hi",
                "store": true
            }),
            json!({
                "model": "1flowbase",
                "input": "hi",
                "include": ["reasoning.encrypted_content"]
            }),
        ] {
            assert_eq!(
                responses_transport_requirement(
                    request.as_object().expect("fixture is a Responses object")
                ),
                crate::application_public_api::native::ResponsesTransportRequirement::NativePassthrough
            );
        }
    }

    #[test]
    fn d4_ac_016_native_responses_translation_retains_real_wire_payload_only_in_sidecar() {
        const SECRET: &str = "Bearer transport-secret-canary";
        let mut translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "hi",
            "tools": [{
                "type": "mcp",
                "server_url": "https://mcp.example.test",
                "authorization": SECRET
            }],
            "future_responses_extension": {"opaque": true}
        }))
        .expect("native Responses request should retain its provider wire body");

        let payload = translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("native request should carry an ephemeral transport sidecar");
        let summary = translated
            .request
            .metadata
            .provider_transport_summary_value()
            .expect("durable metadata should retain only a transport summary");
        assert_eq!(summary["protocol"], "openai_responses");
        assert_eq!(summary["storage"], "ephemeral");
        assert!(summary["digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:")));
        assert!(!summary.to_string().contains(SECRET));
        assert_eq!(
            payload.wire_body()["future_responses_extension"]["opaque"],
            true
        );
        assert_eq!(payload.wire_body()["tools"][0]["authorization"], SECRET);
        assert!(!format!("{payload:?}").contains(SECRET));
        assert!(!serde_json::to_string(&translated.request)
            .expect("Native request should serialize")
            .contains(SECRET));
    }

    #[test]
    fn d4_ac_016_native_responses_keeps_opaque_input_item_without_fabricating_history() {
        let mut translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": [{"type": "item_reference", "id": "item_1"}]
        }))
        .expect("native Responses input item should bypass semantic reconstruction");

        assert!(translated.request.query.is_empty());
        assert!(translated.request.history.is_empty());
        let payload = translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("opaque input should remain in ephemeral transport");
        assert_eq!(payload.wire_body()["input"][0]["id"], "item_1");
    }

    #[test]
    fn d5_ac_004_hosted_tools_stay_out_of_gateway_tool_execution_inputs() {
        let mut translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": "search",
            "tools": [
                {"type": "web_search", "external_web_access": false},
                {"type": "code_interpreter", "container": {"type": "auto"}},
                {"type": "image_generation", "quality": "high"}
            ]
        }))
        .expect("hosted tools should use native Responses transport");

        assert!(translated.request.inputs.as_value().get("tools").is_none());
        assert!(translated.request.history.is_empty());
        let payload = translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("hosted tools should remain in ephemeral provider transport");
        assert_eq!(payload.wire_body()["tools"][0]["type"], "web_search");
        assert_eq!(payload.wire_body()["tools"][1]["type"], "code_interpreter");
        assert_eq!(payload.wire_body()["tools"][2]["type"], "image_generation");
    }

    #[test]
    fn d6_ac_003_orphan_mcp_approval_response_is_rejected_before_run_creation() {
        let error = translate_response_request(json!({
            "model": "1flowbase",
            "input": [{
                "type": "mcp_approval_response",
                "approval_request_id": "approval_provider_owned",
                "approve": true
            }]
        }))
        .expect_err("MCP approval response must name a provider continuation");

        assert_eq!(error.code, "invalid_request");
        assert_eq!(error.param.as_deref(), Some("previous_response_id"));
    }

    #[test]
    fn d6_ac_001_mcp_approval_response_remains_opaque_with_provider_continuation() {
        let mut translated = translate_response_request_with_context_and_previous(
            json!({
                "model": "1flowbase",
                "previous_response_id": "resp_provider_owned",
                "input": [{
                    "type": "mcp_approval_response",
                    "approval_request_id": "approval_provider_owned",
                    "approve": false,
                    "future_extension": {"opaque": true}
                }]
            }),
            OpenAiResponsesRequestContext::responses(),
            Some(OpenAiPreviousResponseContext {
                response_id: "resp_provider_owned".to_string(),
                external_user: None,
                external_conversation_id: None,
                answer: None,
            }),
        )
        .expect("MCP approval response should continue through the native provider lane");

        assert!(translated.request.history.is_empty());
        let payload = translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("MCP approval response should remain ephemeral");
        assert_eq!(
            payload.wire_body()["input"][0]["approval_request_id"],
            "approval_provider_owned"
        );
        assert_eq!(payload.wire_body()["input"][0]["approve"], false);
        assert_eq!(
            payload.wire_body()["input"][0]["future_extension"]["opaque"],
            true
        );
    }

    #[test]
    fn ac_002_responses_function_calls_map_to_native_history() {
        let translated = translate_response_request(json!({
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
        .expect("Responses function calls should map to Native history");
        assert_eq!(
            translated.request.history[1]["tool_calls"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(translated.request.history[2]["tool_call_id"], "call_a");
    }

    #[test]
    fn ac_008_responses_reconstructable_tool_output_can_start_a_new_turn() {
        let translated = translate_response_request(json!({
            "model": "1flowbase",
            "input": [
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "查库存"}]},
                {"type": "function_call", "call_id": "call_inventory", "name": "lookup", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "call_inventory", "output": "7"}
            ]
        }))
        .expect("paired Responses tool history should start a new turn without invented input");

        assert_eq!(translated.request.query, "");
        assert_eq!(
            translated.request.history[1]["tool_calls"][0]["id"],
            "call_inventory"
        );
        assert_eq!(
            translated.request.history[2]["tool_call_id"],
            "call_inventory"
        );
    }

    #[test]
    fn ac_008_responses_orphan_tool_output_is_rejected() {
        let error = translate_response_request(json!({
            "model": "1flowbase",
            "input": [
                {"type": "function_call_output", "call_id": "call_orphan", "output": "7"}
            ]
        }))
        .expect_err("orphan Responses tool output must not invent a function call");

        assert_eq!(error.code, "invalid_request");
        assert!(error.message.contains("matching function_call"));
    }

    #[test]
    fn ac_003_native_responses_replay_preserves_opaque_item_identity_without_semantic_history() {
        let mut translated = translate_response_request(json!({
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
        .expect("opaque Responses replay items should use native provider transport");

        assert!(translated
            .request
            .history
            .iter()
            .all(|item| item.get("reasoning").is_none()));
        let payload = translated
            .request
            .metadata
            .take_provider_transport_payload()
            .expect("opaque replay identity should remain in ephemeral provider transport");
        assert_eq!(payload.wire_body()["input"][1]["id"], "rs_1");
        assert_eq!(payload.wire_body()["input"][3]["id"], "fc_1");
    }

    #[test]
    fn ac_002_previous_response_id_is_accepted_for_route_context_resolution() {
        let translated = translate_response_request(json!({
            "model": "deepseek-v4-flash",
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "Continue"}]}],
            "previous_response_id": "resp_11111111-1111-1111-1111-111111111111"
        }))
        .expect("previous_response_id should be accepted");
        assert_eq!(translated.request.query, "Continue");
    }
}
