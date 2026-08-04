use super::*;

pub(super) fn validate_chat_root_fields(
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
    let mut residual_fields = object
        .keys()
        .filter(|field| !OPENAI_CHAT_TYPED_ROOT_FIELDS.contains(&field.as_str()))
        .collect::<Vec<_>>();
    residual_fields.sort_unstable();
    for (index, field) in residual_fields.into_iter().enumerate() {
        let retained = protocol_context_field_is_safe(field);
        report.record(
            &format!("$.<unknown>[{index}]"),
            None,
            if retained {
                TranslationDecisionKind::Exact
            } else {
                TranslationDecisionKind::Dropped
            },
            Some(if retained {
                "preserved in the OpenAI Chat protocol context residual"
            } else {
                "credential, transport, or internal fields cannot enter protocol context"
            }),
            TranslationSafeRepresentation::Redacted,
        );
    }
    Ok(())
}

pub(super) fn accept_chat_stream_options(
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

pub(super) fn validate_chat_message_fields(
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

pub(super) fn reject_chat_system_media(
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

pub(super) fn validate_openai_content_parts(
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

pub(super) fn chat_max_output_tokens(
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

pub(super) fn validate_response_transport_fields(
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
            .filter(|field| !OPENAI_RESPONSES_TYPED_ROOT_FIELDS.contains(&field.as_str()))
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
        .filter(|field| !OPENAI_RESPONSES_TYPED_ROOT_FIELDS.contains(&field.as_str()))
        .collect::<Vec<_>>();
    report.record_anonymous_unknown_fields(
        "$",
        unknown_fields,
        TranslationDecisionKind::Exact,
        "preserved in the protocol context envelope for a declared Provider profile",
        TranslationSafeRepresentation::Redacted,
    );
    Ok(())
}

pub(super) fn responses_transport_requirement(
    object: &Map<String, Value>,
) -> crate::application_public_api::native::ResponsesTransportRequirement {
    use crate::application_public_api::native::ResponsesTransportRequirement;

    let has_known_native_only_top_level_field = object.keys().any(|field| {
        matches!(
            field.as_str(),
            "response_format" | "text" | "background" | "max_tool_calls" | "truncation"
        )
    });
    let may_omit_unsupported_optional_tools = responses_may_omit_unsupported_optional_tools(object);
    let has_native_only_tools =
        object
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| {
                tools.iter().any(|tool| {
                    !(may_omit_unsupported_optional_tools
                        && responses_tool_is_unsupported_optional(tool))
                        && responses_tool_requires_native_passthrough(tool)
                })
            });
    let has_native_only_tool_choice = object
        .get("tool_choice")
        .is_some_and(responses_tool_choice_requires_native_passthrough);
    let has_native_only_input = object
        .get("input")
        .is_some_and(responses_input_requires_native_passthrough);
    let has_native_only_execution_hint = object.get("store").and_then(Value::as_bool) == Some(true)
        || object
            .get("include")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|item| item != "reasoning.encrypted_content")
            });

    if has_known_native_only_top_level_field
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

pub(super) fn responses_may_omit_unsupported_optional_tools(object: &Map<String, Value>) -> bool {
    object
        .get("tool_choice")
        .is_none_or(responses_tool_choice_allows_optional_omission)
}

pub(super) fn responses_omitted_optional_tools(object: &Map<String, Value>) -> Vec<Value> {
    if !responses_may_omit_unsupported_optional_tools(object) {
        return Vec::new();
    }
    object
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|tool| responses_tool_is_unsupported_optional(tool))
        .cloned()
        .collect()
}

pub(super) fn responses_tool_choice_allows_optional_omission(choice: &Value) -> bool {
    match choice {
        Value::String(choice) => matches!(choice.as_str(), "auto" | "none"),
        Value::Object(choice) => choice.get("type").and_then(Value::as_str) == Some("function"),
        _ => false,
    }
}

pub(super) fn responses_tool_is_unsupported_optional(tool: &Value) -> bool {
    tool.as_object()
        .and_then(|tool| tool.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "namespace" | "web_search"))
}

pub(super) fn responses_tool_requires_native_passthrough(tool: &Value) -> bool {
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

pub(super) fn responses_tool_choice_requires_native_passthrough(choice: &Value) -> bool {
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

pub(super) fn responses_input_requires_native_passthrough(input: &Value) -> bool {
    input.as_array().is_some_and(|items| {
        items
            .iter()
            .any(responses_input_item_requires_native_passthrough)
    })
}

pub(super) fn responses_input_item_requires_native_passthrough(item: &Value) -> bool {
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

pub(super) fn responses_content_requires_native_passthrough(content: &Value) -> bool {
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

pub(super) fn accept_responses_codex_metadata_hints(
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

pub(super) fn accept_responses_include_hint(
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

pub(super) fn accept_responses_parallel_tool_calls_hint(
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
                TranslationDecisionKind::Dropped,
                Some("Provider scheduling remains authoritative for Native tool calls"),
                TranslationSafeRepresentation::Present,
            );
            Ok(())
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

pub(super) fn accept_responses_store_hint(
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

pub(super) fn response_stream_mode(
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

pub(super) fn response_max_output_tokens(
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
