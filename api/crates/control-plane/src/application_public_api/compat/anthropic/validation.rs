use super::*;

pub(super) fn validate_anthropic_root_fields(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    if let Some(field) = object.keys().find(|field| {
        matches!(
            field.as_str(),
            "container"
                | "mcp_servers"
                | "service_tier"
                | "temperature"
                | "top_k"
                | "top_p"
                | "stop_sequences"
                | "stream_options"
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
        return Err(AnthropicCompatError::unsupported(field).with_report(report.clone()));
    }
    let mut residual_fields = object
        .keys()
        .filter(|field| {
            field.as_str() != "context_management"
                && !ANTHROPIC_TYPED_ROOT_FIELDS.contains(&field.as_str())
        })
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
                "preserved in the Anthropic protocol context residual"
            } else {
                "credential, transport, or internal fields cannot enter protocol context"
            }),
            TranslationSafeRepresentation::Redacted,
        );
    }
    Ok(())
}

pub(super) fn record_anthropic_system_decision(
    value: Option<&Value>,
    report: &mut TranslationReport,
) {
    let (kind, reason, effective_value) = match value {
        Some(_) => (
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        ),
        None => (
            TranslationDecisionKind::Defaulted,
            Some("no system prompt"),
            TranslationSafeRepresentation::Defaulted,
        ),
    };
    report.record("$.system", Some("$.system"), kind, reason, effective_value);
}

pub(super) fn record_anthropic_context_management_decision(
    value: Option<&Value>,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    let Some(value) = value else {
        return Ok(());
    };
    if !value.is_object() {
        return Err(reject_anthropic_nested_field(
            report,
            "$.context_management",
            "context_management must be an object",
            TranslationSafeRepresentation::Present,
        ));
    }
    report.record(
        "$.context_management",
        Some("$.client_protocol_envelope.body.context_management"),
        TranslationDecisionKind::Exact,
        Some("preserved only for matching Anthropic protocol projection"),
        TranslationSafeRepresentation::Redacted,
    );
    Ok(())
}

pub(super) fn reject_legacy_anthropic_control(
    system_parts: &[NativePromptBlock],
    source_path: &str,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    if claude_code_system_control_kind(system_parts).is_none() {
        return Ok(());
    }
    report.record(
        source_path,
        None,
        TranslationDecisionKind::Unsupported,
        Some("Claude Code prompt-marker control has no current canonical owner"),
        TranslationSafeRepresentation::Redacted,
    );
    Err(AnthropicCompatError::unsupported("system").with_report(report.clone()))
}

pub(super) fn reject_legacy_anthropic_control_text(
    content: &str,
    source_path: &str,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    if claude_code_control_kind(content).is_none() {
        return Ok(());
    }
    report.record(
        source_path,
        None,
        TranslationDecisionKind::Unsupported,
        Some("Claude Code prompt-marker control has no current canonical owner"),
        TranslationSafeRepresentation::Redacted,
    );
    Err(AnthropicCompatError::unsupported("messages").with_report(report.clone()))
}

pub(super) fn validate_anthropic_message(
    message: &Value,
    index: usize,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    let message_path = format!("$.messages[{index}]");
    let Some(object) = message.as_object() else {
        report.record(
            &message_path,
            None,
            TranslationDecisionKind::Rejected,
            Some("Anthropic messages must be objects"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            AnthropicCompatError::invalid("message must be an object").with_report(report.clone())
        );
    };
    report.record(
        &message_path,
        Some("$.query,$.history"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    let unknown_fields = object
        .keys()
        .filter(|field| !matches!(field.as_str(), "role" | "content"))
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        &message_path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic message field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic message field")
                .with_report(report.clone()),
        );
    }

    let role_path = format!("$.messages[{index}].role");
    let Some(role) = object.get("role").and_then(Value::as_str) else {
        report.record(
            &role_path,
            None,
            TranslationDecisionKind::Rejected,
            Some("Anthropic message role must be text"),
            if object.contains_key("role") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        );
        return Err(
            AnthropicCompatError::invalid("message role is required").with_report(report.clone())
        );
    };
    if !matches!(role, "system" | "user" | "assistant") {
        report.record(
            &role_path,
            None,
            TranslationDecisionKind::Rejected,
            Some("unsupported Anthropic message role"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            AnthropicCompatError::invalid(format!("unsupported message role: {role}"))
                .with_report(report.clone()),
        );
    }
    report.record(
        &role_path,
        Some("$.query,$.history"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Present,
    );

    let content = object.get("content").ok_or_else(|| {
        let path = format!("$.messages[{index}].content");
        report.record(
            &path,
            None,
            TranslationDecisionKind::Rejected,
            Some("message content is required"),
            TranslationSafeRepresentation::Absent,
        );
        AnthropicCompatError::invalid("message content is required").with_report(report.clone())
    })?;
    let content_path = format!("$.messages[{index}].content");
    validate_anthropic_content_blocks(content, index, report).map_err(|error| {
        if report
            .decisions
            .iter()
            .all(|decision| decision.source_path != content_path)
        {
            report.record(
                &content_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("Anthropic message content contains invalid blocks"),
                TranslationSafeRepresentation::Present,
            );
        }
        error.with_report(report.clone())
    })?;
    let content_text = anthropic_text_content(content).map_err(|error| {
        if report
            .decisions
            .iter()
            .all(|decision| decision.source_path != content_path)
        {
            report.record(
                &content_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("Anthropic message content cannot be translated"),
                TranslationSafeRepresentation::Present,
            );
        }
        error.with_report(report.clone())
    })?;
    reject_legacy_anthropic_control_text(&content_text, &content_path, report)?;
    report.record(
        &content_path,
        Some("$.query,$.history"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    Ok(())
}

pub(super) fn validate_anthropic_content_blocks(
    content: &Value,
    message_index: usize,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    let Some(blocks) = content.as_array() else {
        if content.is_string() {
            return Ok(());
        }
        let path = format!("$.messages[{message_index}].content");
        report.record(
            &path,
            None,
            TranslationDecisionKind::Rejected,
            Some("message content must be text or content blocks"),
            TranslationSafeRepresentation::Present,
        );
        return Err(AnthropicCompatError::invalid(
            "message content must be text or content blocks",
        )
        .with_report(report.clone()));
    };

    for (block_index, block) in blocks.iter().enumerate() {
        let block_path = format!("$.messages[{message_index}].content[{block_index}]");
        let Some(object) = block.as_object() else {
            report.record(
                &block_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("Anthropic content blocks must be objects"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                AnthropicCompatError::invalid("content block must be an object")
                    .with_report(report.clone()),
            );
        };
        report.record(
            &block_path,
            Some("$.query,$.history"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        let path = format!("$.messages[{message_index}].content[{block_index}].type");
        let block_type = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match block_type {
            "text" | "image" | "document" => {
                report.record(
                    &path,
                    Some("$.query,$.history"),
                    TranslationDecisionKind::Normalized,
                    None,
                    TranslationSafeRepresentation::Present,
                );
                validate_anthropic_supported_content_block(
                    object,
                    &block_path,
                    block_type,
                    report,
                )?;
            }
            "tool_use" => {
                validate_anthropic_tool_use_block(object, &block_path, report)?;
                report.record(
                    &path,
                    Some("$.history[].tool_calls"),
                    TranslationDecisionKind::Normalized,
                    None,
                    TranslationSafeRepresentation::Present,
                );
            }
            "tool_result" => {
                validate_anthropic_tool_result_block(object, &block_path, report)?;
                report.record(
                    &path,
                    Some("$.history[]"),
                    TranslationDecisionKind::Normalized,
                    None,
                    TranslationSafeRepresentation::Present,
                );
            }
            "thinking" | "redacted_thinking" => {
                validate_anthropic_reasoning_block(object, &block_path, block_type, report)?;
                report.record(
                    &path,
                    Some("$.history[].content_blocks"),
                    TranslationDecisionKind::Normalized,
                    None,
                    TranslationSafeRepresentation::Present,
                );
            }
            "server_tool_use" | "computer_use" => {
                report.record(
                    &path,
                    None,
                    TranslationDecisionKind::Unsupported,
                    Some("server-managed tool content has no current canonical owner"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(AnthropicCompatError::unsupported(format!(
                    "messages content block {block_type}"
                ))
                .with_report(report.clone()));
            }
            _ => {
                report.record(
                    &path,
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("unknown Anthropic content block type"),
                    if object.contains_key("type") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
                );
                return Err(
                    AnthropicCompatError::invalid("unknown Anthropic content block type")
                        .with_report(report.clone()),
                );
            }
        }
    }
    Ok(())
}

pub(super) fn validate_anthropic_reasoning_block(
    object: &Map<String, Value>,
    block_path: &str,
    block_type: &str,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    let (allowed_fields, required_field, error_message): (&[&str], &str, &str) =
        if block_type == "thinking" {
            (
                &["type", "thinking", "signature"],
                "thinking",
                "thinking block must contain text",
            )
        } else {
            (
                &["type", "data"],
                "data",
                "redacted_thinking block must contain data",
            )
        };
    reject_unknown_anthropic_content_block_fields(object, block_path, allowed_fields, &[], report)?;
    if !object.get(required_field).is_some_and(Value::is_string) {
        return Err(reject_anthropic_nested_field(
            report,
            &format!("{block_path}.{required_field}"),
            error_message,
            if object.contains_key(required_field) {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        ));
    }
    if object
        .get("signature")
        .is_some_and(|signature| !signature.is_string())
    {
        return Err(reject_anthropic_nested_field(
            report,
            &format!("{block_path}.signature"),
            "thinking signature must be text",
            TranslationSafeRepresentation::Present,
        ));
    }
    Ok(())
}

pub(super) fn validate_anthropic_tool_use_block(
    object: &Map<String, Value>,
    block_path: &str,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    if object.get("name").and_then(Value::as_str) == Some("computer") {
        report.record(
            &format!("{block_path}.type"),
            None,
            TranslationDecisionKind::Unsupported,
            Some("computer tool use has no current Native owner"),
            TranslationSafeRepresentation::Present,
        );
        return Err(AnthropicCompatError::unsupported("computer_use").with_report(report.clone()));
    }
    reject_unknown_anthropic_content_block_fields(
        object,
        block_path,
        &["type", "id", "name", "input", "cache_control"],
        &[],
        report,
    )?;
    for field in ["id", "name"] {
        let path = format!("{block_path}.{field}");
        if !object
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(reject_anthropic_nested_field(
                report,
                &path,
                if field == "id" {
                    "tool_use id must be non-empty text"
                } else {
                    "tool_use name must be non-empty text"
                },
                if object.contains_key(field) {
                    TranslationSafeRepresentation::Present
                } else {
                    TranslationSafeRepresentation::Absent
                },
            ));
        }
    }
    if !object.get("input").is_some_and(Value::is_object) {
        return Err(reject_anthropic_nested_field(
            report,
            &format!("{block_path}.input"),
            "tool_use input must be an object",
            if object.contains_key("input") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        ));
    }
    if let Some(cache_control) = object.get("cache_control") {
        validate_anthropic_cache_control(
            cache_control,
            &format!("{block_path}.cache_control"),
            TranslationDecisionKind::Dropped,
            None,
            report,
        )?;
    }
    Ok(())
}

pub(super) fn validate_anthropic_tool_result_block(
    object: &Map<String, Value>,
    block_path: &str,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    reject_unknown_anthropic_content_block_fields(
        object,
        block_path,
        &[
            "type",
            "tool_use_id",
            "content",
            "is_error",
            "cache_control",
        ],
        &[],
        report,
    )?;
    let id_path = format!("{block_path}.tool_use_id");
    if !object
        .get("tool_use_id")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(reject_anthropic_nested_field(
            report,
            &id_path,
            "tool_result tool_use_id must be non-empty text",
            if object.contains_key("tool_use_id") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        ));
    }
    if let Some(is_error) = object.get("is_error") {
        if !is_error.is_boolean() {
            return Err(reject_anthropic_nested_field(
                report,
                &format!("{block_path}.is_error"),
                "tool_result is_error must be boolean",
                TranslationSafeRepresentation::Present,
            ));
        }
    }
    if let Some(cache_control) = object.get("cache_control") {
        validate_anthropic_cache_control(
            cache_control,
            &format!("{block_path}.cache_control"),
            TranslationDecisionKind::Dropped,
            None,
            report,
        )?;
    }
    Ok(())
}

pub(super) fn validate_anthropic_supported_content_block(
    object: &Map<String, Value>,
    block_path: &str,
    block_type: &str,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    match block_type {
        "text" => {
            reject_unknown_anthropic_content_block_fields(
                object,
                block_path,
                &["type", "text", "cache_control"],
                &[],
                report,
            )?;
            let text_path = format!("{block_path}.text");
            if !object.get("text").is_some_and(Value::is_string) {
                return Err(reject_anthropic_nested_field(
                    report,
                    &text_path,
                    "text content blocks require text",
                    if object.contains_key("text") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
                ));
            }
            report.record(
                &text_path,
                Some("$.query,$.history"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
        }
        "image" | "document" => {
            let unsupported_document_fields = if block_type == "document" {
                &["title", "context", "citations"][..]
            } else {
                &[][..]
            };
            reject_unknown_anthropic_content_block_fields(
                object,
                block_path,
                &["type", "source", "cache_control"],
                unsupported_document_fields,
                report,
            )?;
            validate_anthropic_media_source(
                object.get("source"),
                &format!("{block_path}.source"),
                block_type == "document",
                report,
            )?;
        }
        _ => unreachable!("caller validates supported Anthropic content block types"),
    }
    if let Some(cache_control) = object.get("cache_control") {
        validate_anthropic_cache_control(
            cache_control,
            &format!("{block_path}.cache_control"),
            TranslationDecisionKind::Dropped,
            None,
            report,
        )?;
    } else {
        report.record(
            &format!("{block_path}.cache_control"),
            None,
            TranslationDecisionKind::Defaulted,
            Some("no content-block cache control"),
            TranslationSafeRepresentation::Defaulted,
        );
    }
    Ok(())
}

pub(super) fn reject_unknown_anthropic_content_block_fields(
    object: &Map<String, Value>,
    block_path: &str,
    allowed_fields: &[&str],
    unsupported_fields: &[&str],
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !allowed_fields.contains(&field.as_str())
                && !unsupported_fields.contains(&field.as_str())
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        block_path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic content-block field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic content-block field")
                .with_report(report.clone()),
        );
    }
    if let Some(field) = object
        .keys()
        .find(|field| unsupported_fields.contains(&field.as_str()))
    {
        let path = format!("{block_path}.{field}");
        report.record(
            &path,
            None,
            TranslationDecisionKind::Unsupported,
            Some("this Anthropic content-block field has no current canonical owner"),
            TranslationSafeRepresentation::Present,
        );
        return Err(AnthropicCompatError::unsupported("messages").with_report(report.clone()));
    }
    Ok(())
}

pub(super) fn validate_anthropic_media_source(
    value: Option<&Value>,
    source_path: &str,
    allows_text_source: bool,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    let Some(source) = value.and_then(Value::as_object) else {
        return Err(reject_anthropic_nested_field(
            report,
            source_path,
            "media content blocks require an object source",
            if value.is_some() {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        ));
    };
    report.record(
        source_path,
        Some("$.history[].content_blocks[].source"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    let type_path = format!("{source_path}.type");
    let source_type = source
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (required_fields, allowed_fields) = match source_type {
        "base64" => (
            &["media_type", "data"][..],
            &["type", "media_type", "data"][..],
        ),
        "url" => (&["url"][..], &["type", "url"][..]),
        "text" if allows_text_source => (&["data"][..], &["type", "media_type", "data"][..]),
        "file" | "content" => {
            report.record(
                &type_path,
                None,
                TranslationDecisionKind::Unsupported,
                Some("this Anthropic media source has no current canonical owner"),
                TranslationSafeRepresentation::Present,
            );
            return Err(AnthropicCompatError::unsupported("messages").with_report(report.clone()));
        }
        _ => {
            return Err(reject_anthropic_nested_field(
                report,
                &type_path,
                "unknown Anthropic media source type",
                if source.contains_key("type") {
                    TranslationSafeRepresentation::Present
                } else {
                    TranslationSafeRepresentation::Absent
                },
            ));
        }
    };
    report.record(
        &type_path,
        Some("$.history[].content_blocks[].source.type"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Present,
    );
    let unknown_fields = source
        .keys()
        .filter(|field| !allowed_fields.contains(&field.as_str()))
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        source_path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic media source field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic media source field")
                .with_report(report.clone()),
        );
    }
    for field in required_fields {
        let path = format!("{source_path}.{field}");
        if !source.get(*field).is_some_and(Value::is_string) {
            return Err(reject_anthropic_nested_field(
                report,
                &path,
                "Anthropic media source field must be text",
                if source.contains_key(*field) {
                    TranslationSafeRepresentation::Present
                } else {
                    TranslationSafeRepresentation::Absent
                },
            ));
        }
        report.record(
            &path,
            Some("$.history[].content_blocks[].source"),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Redacted,
        );
    }
    if source_type == "text" {
        if let Some(media_type) = source.get("media_type") {
            if !media_type.is_string() {
                return Err(reject_anthropic_nested_field(
                    report,
                    &format!("{source_path}.media_type"),
                    "Anthropic media source media_type must be text",
                    TranslationSafeRepresentation::Present,
                ));
            }
            report.record(
                &format!("{source_path}.media_type"),
                Some("$.history[].content_blocks[].source.media_type"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Present,
            );
        } else {
            report.record(
                &format!("{source_path}.media_type"),
                Some("$.history[].content_blocks[].source.media_type"),
                TranslationDecisionKind::Defaulted,
                Some("no document media type supplied"),
                TranslationSafeRepresentation::Defaulted,
            );
        }
    }
    Ok(())
}

pub(super) fn validate_anthropic_cache_control(
    value: &Value,
    cache_path: &str,
    kind: TranslationDecisionKind,
    target_path: Option<&str>,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    let decision_reason = match kind {
        TranslationDecisionKind::Unsupported => {
            Some("message content cache control has no current canonical owner")
        }
        TranslationDecisionKind::Dropped => {
            Some("cache hint is omitted while Native retains the content")
        }
        _ => None,
    };
    let Some(cache_control) = value.as_object() else {
        return Err(reject_anthropic_nested_field(
            report,
            cache_path,
            "cache_control must be an object",
            TranslationSafeRepresentation::Present,
        ));
    };
    report.record(
        cache_path,
        target_path,
        kind,
        decision_reason,
        TranslationSafeRepresentation::Present,
    );
    let unknown_fields = cache_control
        .keys()
        .filter(|field| !matches!(field.as_str(), "type" | "ttl"))
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        cache_path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic cache_control field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic cache_control field")
                .with_report(report.clone()),
        );
    }
    let type_path = format!("{cache_path}.type");
    if cache_control.get("type").and_then(Value::as_str) != Some("ephemeral") {
        return Err(reject_anthropic_nested_field(
            report,
            &type_path,
            "cache_control.type must be ephemeral",
            if cache_control.contains_key("type") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        ));
    }
    report.record(
        &type_path,
        target_path,
        kind,
        decision_reason,
        TranslationSafeRepresentation::Present,
    );
    if let Some(ttl) = cache_control.get("ttl") {
        let ttl_path = format!("{cache_path}.ttl");
        if !matches!(ttl.as_str(), Some("5m" | "1h")) {
            return Err(reject_anthropic_nested_field(
                report,
                &ttl_path,
                "cache_control.ttl must be 5m or 1h",
                TranslationSafeRepresentation::Present,
            ));
        }
        report.record(
            &ttl_path,
            target_path,
            kind,
            decision_reason,
            TranslationSafeRepresentation::Present,
        );
    } else {
        report.record(
            &format!("{cache_path}.ttl"),
            target_path,
            TranslationDecisionKind::Defaulted,
            Some("default cache TTL"),
            TranslationSafeRepresentation::Defaulted,
        );
    }
    Ok(())
}

pub(super) fn reject_anthropic_nested_field(
    report: &mut TranslationReport,
    source_path: &str,
    reason: &'static str,
    effective_value: TranslationSafeRepresentation,
) -> AnthropicCompatError {
    report.record(
        source_path,
        None,
        TranslationDecisionKind::Rejected,
        Some(reason),
        effective_value,
    );
    AnthropicCompatError::invalid(reason).with_report(report.clone())
}

pub(super) fn anthropic_response_mode(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<Option<String>, AnthropicCompatError> {
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
            Err(AnthropicCompatError::invalid("stream must be a boolean")
                .with_report(report.clone()))
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
