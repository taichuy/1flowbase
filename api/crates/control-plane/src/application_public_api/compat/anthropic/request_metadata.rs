use super::*;

#[derive(Debug, Default)]
pub(super) struct AnthropicRequestMetadata {
    pub(super) user_id: Option<String>,
    expand_id: Option<String>,
    session_id: Option<String>,
    pub(super) trace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ClaudeCodeUserIdentity {
    #[serde(default)]
    account_uuid: Option<String>,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
}

pub(super) fn anthropic_metadata(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<AnthropicRequestMetadata, AnthropicCompatError> {
    match object.get("metadata") {
        Some(Value::Object(metadata)) => {
            report.record(
                "$.metadata",
                Some("$.metadata,$.conversation,$.request_context.end_user_reference"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            let mut normalized = AnthropicRequestMetadata::default();
            let unknown_fields = metadata
                .keys()
                .filter(|field| {
                    !matches!(
                        field.as_str(),
                        "user_id" | "expand_id" | "session_id" | "trace_id"
                    )
                })
                .collect::<Vec<_>>();
            if report.record_anonymous_unknown_fields(
                "$.metadata",
                unknown_fields,
                TranslationDecisionKind::Rejected,
                "Anthropic metadata field has no canonical owner",
                TranslationSafeRepresentation::Present,
            ) > 0
            {
                return Err(
                    AnthropicCompatError::invalid("unsupported Anthropic metadata field")
                        .with_report(report.clone()),
                );
            }
            for (field, value) in metadata {
                let path = format!("$.metadata.{field}");
                let target_path = match field.as_str() {
                    "user_id" => "$.request_context.end_user_reference,$.conversation",
                    "expand_id" => "$.conversation.user",
                    "session_id" => "$.conversation.id",
                    "trace_id" => "$.metadata.trace_id",
                    _ => continue,
                };
                let Some(text) = value.as_str() else {
                    report.record(
                        &path,
                        None,
                        TranslationDecisionKind::Rejected,
                        Some("Anthropic metadata fields must be text"),
                        TranslationSafeRepresentation::Present,
                    );
                    return Err(AnthropicCompatError::invalid(
                        "Anthropic metadata fields must be text",
                    )
                    .with_report(report.clone()));
                };
                report.record(
                    &path,
                    Some(target_path),
                    TranslationDecisionKind::Normalized,
                    None,
                    TranslationSafeRepresentation::Redacted,
                );
                let text = text.trim();
                let text = (!text.is_empty()).then(|| text.to_owned());
                match field.as_str() {
                    "user_id" => normalized.user_id = text,
                    "expand_id" => normalized.expand_id = text,
                    "session_id" => normalized.session_id = text,
                    "trace_id" => normalized.trace_id = text,
                    _ => continue,
                }
            }
            Ok(normalized)
        }
        Some(_) => {
            report.record(
                "$.metadata",
                None,
                TranslationDecisionKind::Rejected,
                Some("metadata must be an object"),
                TranslationSafeRepresentation::Present,
            );
            Err(AnthropicCompatError::invalid("metadata must be an object")
                .with_report(report.clone()))
        }
        None => {
            report.record(
                "$.metadata",
                Some("$.metadata"),
                TranslationDecisionKind::Defaulted,
                Some("empty metadata"),
                TranslationSafeRepresentation::Defaulted,
            );
            Ok(AnthropicRequestMetadata::default())
        }
    }
}

pub(super) fn anthropic_max_output_tokens(
    object: &serde_json::Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<Option<u64>, AnthropicCompatError> {
    let Some(value) = object.get("max_tokens") else {
        report.record(
            "$.max_tokens",
            Some("$.execution.model_parameters.max_output_tokens"),
            TranslationDecisionKind::Defaulted,
            Some("provider default output limit"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(None);
    };
    let Some(max_output_tokens) = value.as_u64().filter(|value| *value > 0) else {
        report.record(
            "$.max_tokens",
            None,
            TranslationDecisionKind::Rejected,
            Some("max_tokens must be a positive integer"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            AnthropicCompatError::invalid("max_tokens must be a positive integer")
                .with_report(report.clone()),
        );
    };
    report.record(
        "$.max_tokens",
        Some("$.execution.model_parameters.max_output_tokens"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Present,
    );
    Ok(Some(max_output_tokens))
}

pub(super) fn normalize_anthropic_model_for_native(
    model: &str,
) -> (String, Option<AnthropicContextWindowRequest>) {
    let trimmed_end = model.trim_end();
    let suffix_start = trimmed_end
        .len()
        .saturating_sub(ANTHROPIC_CONTEXT_1M_MODEL_SUFFIX.len());
    let has_one_m_suffix = trimmed_end
        .get(suffix_start..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(ANTHROPIC_CONTEXT_1M_MODEL_SUFFIX));

    if has_one_m_suffix {
        let native_model = trimmed_end
            .get(..suffix_start)
            .unwrap_or(trimmed_end)
            .to_string();
        return (
            native_model,
            Some(AnthropicContextWindowRequest::OneMillion),
        );
    }

    (model.to_string(), None)
}

pub(super) fn anthropic_system_content_parts(
    value: Option<&Value>,
    report: &mut TranslationReport,
) -> Result<Vec<NativePromptBlock>, AnthropicCompatError> {
    let mut parts = Vec::new();
    match value {
        Some(Value::String(text)) => push_system_part(&mut parts, text),
        Some(Value::Array(blocks)) => {
            for (index, block) in blocks.iter().enumerate() {
                match block {
                    Value::String(text) => {
                        report.record(
                            &format!("$.system[{index}]"),
                            Some("$.system"),
                            TranslationDecisionKind::Normalized,
                            None,
                            TranslationSafeRepresentation::Redacted,
                        );
                        push_system_part(&mut parts, text)
                    }
                    Value::Object(object) => {
                        let path = format!("$.system[{index}]");
                        report.record(
                            &path,
                            Some("$.system"),
                            TranslationDecisionKind::Normalized,
                            None,
                            TranslationSafeRepresentation::Redacted,
                        );
                        validate_anthropic_system_text_block(object, &path, report)?;
                        let block = native_anthropic_system_text_block(object);
                        if !block.text_content().trim().is_empty() {
                            parts.push(block);
                        }
                    }
                    _ => {
                        return Err(reject_anthropic_nested_field(
                            report,
                            &format!("$.system[{index}]"),
                            "system prompt blocks must be text or objects",
                            TranslationSafeRepresentation::Present,
                        ));
                    }
                }
            }
        }
        None => {}
        _ => {
            return Err(reject_anthropic_nested_field(
                report,
                "$.system",
                "system must be text or text blocks",
                TranslationSafeRepresentation::Present,
            ))
        }
    }
    Ok(parts)
}

pub(super) fn native_anthropic_system_text_block(object: &Map<String, Value>) -> NativePromptBlock {
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .expect("validated Anthropic system text blocks always contain text")
        .to_owned();
    let cache_control =
        object
            .get("cache_control")
            .and_then(Value::as_object)
            .map(|cache_control| NativePromptCacheControl {
                cache_type: NativePromptCacheControlType::Ephemeral,
                ttl: match cache_control.get("ttl").and_then(Value::as_str) {
                    Some("5m") => Some(NativePromptCacheTtl::FiveMinutes),
                    Some("1h") => Some(NativePromptCacheTtl::OneHour),
                    None => None,
                    Some(_) => {
                        unreachable!("validated Anthropic cache control has a supported TTL")
                    }
                },
            });
    NativePromptBlock::Text {
        text,
        cache_control,
    }
}

pub(super) fn validate_anthropic_system_text_block(
    object: &Map<String, Value>,
    block_path: &str,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    let unknown_fields = object
        .keys()
        .filter(|field| !matches!(field.as_str(), "type" | "text" | "cache_control"))
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        block_path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic system block field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic system block field")
                .with_report(report.clone()),
        );
    }
    let type_path = format!("{block_path}.type");
    if object.get("type").and_then(Value::as_str) != Some("text") {
        return Err(reject_anthropic_nested_field(
            report,
            &type_path,
            "Anthropic system blocks support only text",
            if object.contains_key("type") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        ));
    }
    report.record(
        &type_path,
        Some("$.system"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Present,
    );
    let text_path = format!("{block_path}.text");
    if !object.get("text").is_some_and(Value::is_string) {
        return Err(reject_anthropic_nested_field(
            report,
            &text_path,
            "Anthropic system text blocks require text",
            if object.contains_key("text") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        ));
    }
    report.record(
        &text_path,
        Some("$.system"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    if let Some(cache_control) = object.get("cache_control") {
        validate_anthropic_cache_control(
            cache_control,
            &format!("{block_path}.cache_control"),
            TranslationDecisionKind::Exact,
            Some("$.system"),
            report,
        )?;
    } else {
        report.record(
            &format!("{block_path}.cache_control"),
            Some("$.system"),
            TranslationDecisionKind::Defaulted,
            Some("no system cache control"),
            TranslationSafeRepresentation::Defaulted,
        );
    }
    Ok(())
}

pub(super) fn push_system_part(parts: &mut Vec<NativePromptBlock>, content: &str) {
    let content = content.trim();
    if content.is_empty() {
        return;
    }
    parts.push(NativePromptBlock::text(content));
}

pub fn claude_code_control_kind(content: &str) -> Option<&'static str> {
    if content.contains(CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX)
        || content.contains(CLAUDE_CODE_PARTIAL_COMPACT_SUMMARY_PROMPT_PREFIX)
        || content.contains(CLAUDE_CODE_CONTEXT_CONTINUATION_SUMMARY_PROMPT_PREFIX)
    {
        return Some("compact_summary");
    }
    if content.contains(CLAUDE_CODE_COMPACT_RESUME_MARKER)
        && (content.contains(CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER)
            || content.contains(CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER))
    {
        return Some("compact_resume");
    }
    if content.contains(CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX)
        && content.contains(CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER)
    {
        return Some("away_summary");
    }
    None
}

pub(super) fn claude_code_system_control_kind(
    system_parts: &[NativePromptBlock],
) -> Option<&'static str> {
    system_parts
        .iter()
        .any(|part| {
            part.text_content()
                .contains(CLAUDE_CODE_SESSION_TITLE_SYSTEM_MARKER)
                && part
                    .text_content()
                    .contains(CLAUDE_CODE_SESSION_TITLE_JSON_MARKER)
        })
        .then_some("session_title")
}

pub(super) fn metadata_conversation(metadata: &AnthropicRequestMetadata) -> NativeObject {
    let mut conversation = NativeObject::default();
    let user_id = metadata.user_id.as_deref();
    let user_id_payload =
        user_id.and_then(|value| serde_json::from_str::<ClaudeCodeUserIdentity>(value).ok());
    if let Some(user) = metadata
        .expand_id
        .clone()
        .or_else(|| metadata_user_from_user_id(user_id, user_id_payload.as_ref()))
    {
        conversation.insert_string("user", user);
    }
    if let Some(session_id) = user_id_payload
        .as_ref()
        .and_then(|payload| payload.session_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| metadata.session_id.clone())
        .or_else(|| user_id.and_then(claude_code_session_id_from_identity))
    {
        conversation.insert_string("id", session_id);
    }
    conversation
}

pub(super) fn metadata_user_from_user_id(
    user_id: Option<&str>,
    payload: Option<&ClaudeCodeUserIdentity>,
) -> Option<String> {
    payload
        .and_then(|payload| {
            payload
                .account_uuid
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    payload
                        .device_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                })
        })
        .or(user_id)
        .map(ToOwned::to_owned)
}

pub(super) fn claude_code_session_id_from_identity(identity: &str) -> Option<String> {
    let marker = "_session_";
    let start = identity.rfind(marker)? + marker.len();
    let candidate = identity[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
        .collect::<String>();
    Uuid::parse_str(&candidate)
        .ok()
        .map(|session_id| session_id.to_string())
}
