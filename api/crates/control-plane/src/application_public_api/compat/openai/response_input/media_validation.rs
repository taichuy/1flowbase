use super::*;

pub(in super::super) fn validate_responses_content_parts(
    content: &Value,
    message_index: usize,
    report: &mut TranslationReportBuilder,
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

pub(in super::super) fn validate_openai_supported_content_part(
    object: &Map<String, Value>,
    part_path: &str,
    part_type: &str,
    error_param: &'static str,
    report: &mut TranslationReportBuilder,
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

pub(in super::super) fn reject_unknown_openai_content_part_fields(
    object: &Map<String, Value>,
    part_path: &str,
    allowed_fields: &[&str],
    error_param: &'static str,
    report: &mut TranslationReportBuilder,
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

pub(in super::super) fn validate_openai_image_url_value(
    value: Option<&Value>,
    image_path: &str,
    error_param: &'static str,
    report: &mut TranslationReportBuilder,
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
