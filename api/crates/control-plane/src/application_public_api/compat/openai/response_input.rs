use super::*;

pub(super) fn validate_responses_input(
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

pub(super) fn validate_native_responses_input(
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

pub(super) fn validate_native_mcp_approval_continuation(
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

pub(super) fn validate_responses_input_items(
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

pub(super) fn reject_responses_system_media(
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

pub(super) fn validate_responses_content_parts(
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

pub(super) fn validate_openai_supported_content_part(
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

pub(super) fn reject_unknown_openai_content_part_fields(
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

pub(super) fn validate_openai_image_url_value(
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

pub(super) fn responses_input_to_native_run_input(
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

pub(super) fn responses_native_input_to_run_input(input: &Value) -> ResponsesInputMapping {
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

pub(super) fn responses_end_with_reconstructable_tool_output(
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

pub(super) fn openai_chat_history_tool_calls(tool_calls: &Value) -> Value {
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

pub(super) fn openai_chat_history_tool_call_id(tool_call_id: &str) -> String {
    decode_openai_callback_tool_call_id(tool_call_id)
        .map(|(_, original_tool_call_id)| original_tool_call_id)
        .unwrap_or_else(|| tool_call_id.to_string())
}

pub(super) fn parse_openai_tool_arguments(arguments: &Value) -> Value {
    match arguments {
        Value::String(arguments) => {
            serde_json::from_str(arguments).unwrap_or_else(|_| Value::String(arguments.clone()))
        }
        arguments => arguments.clone(),
    }
}

pub(super) fn responses_previous_history(
    previous: Option<&OpenAiPreviousResponseContext>,
) -> Vec<Value> {
    previous
        .and_then(|previous| previous.answer.as_ref())
        .map(|answer| vec![json!({ "role": "assistant", "content": answer })])
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResponsesInputMapping {
    pub(super) query: String,
    pub(super) history: Vec<Value>,
    pub(super) system_parts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ResponsesInputMessage {
    role: String,
    content: String,
    content_blocks: Option<Value>,
}

pub(super) fn responses_input_message(
    item: &Value,
) -> Result<ResponsesInputMessage, OpenAiCompatError> {
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
pub(super) struct OpenAiMappedContent {
    pub(super) text: String,
    pub(super) content_blocks: Option<Value>,
}

impl OpenAiMappedContent {
    pub(super) fn trim(&self) -> &str {
        self.text.trim()
    }
}

pub(super) fn openai_message_content(
    message: &Value,
) -> Result<OpenAiMappedContent, OpenAiCompatError> {
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

pub(super) fn openai_content(content: &Value) -> Result<OpenAiMappedContent, OpenAiCompatError> {
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

pub(super) fn escape_openai_json_nul_characters(text: &str) -> String {
    text.replace('\0', "\\u0000")
}

pub(super) fn openai_image_content_block(part: &Value) -> Option<Value> {
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
