use plugin_framework::provider_contract::{
    NativePromptBlock, NativePromptCacheControl, NativePromptCacheControlType, NativePromptCacheTtl,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::application_public_api::client_protocol_envelope::ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE;
use crate::application_public_api::native::{NativeObject, NativeRunRequest};
use crate::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationProtocol, TranslationReport, TranslationSafeRepresentation,
};

const CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of the conversation so far";
const CLAUDE_CODE_PARTIAL_COMPACT_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of the RECENT portion of the conversation";
const CLAUDE_CODE_CONTEXT_CONTINUATION_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of this conversation. This summary will be placed at the start of a continuing session";
const CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX: &str =
    "The user stepped away and is coming back. Write exactly 1-3 short sentences.";
const CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER: &str = "Next: the concrete next step.";
const CLAUDE_CODE_COMPACT_RESUME_MARKER: &str =
    "This session is being continued from a previous conversation that ran out of context.";
const CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER: &str =
    "The summary below covers the earlier portion of the conversation.";
const CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER: &str =
    "If you need specific details from before compaction";
const CLAUDE_CODE_SESSION_TITLE_SYSTEM_MARKER: &str = "Generate a concise, sentence-case title";
const CLAUDE_CODE_SESSION_TITLE_JSON_MARKER: &str = "Return JSON with a single \"title\" field";
const ANTHROPIC_CONTEXT_1M_MODEL_SUFFIX: &str = "[1m]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicCompatError {
    pub message: String,
    pub error_type: String,
    pub report: TranslationReport,
}

impl AnthropicCompatError {
    fn translation_invariant(report: TranslationReport) -> Self {
        Self {
            message: "translation receipt invariant violated".to_string(),
            error_type: "api_error".to_string(),
            report,
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: "invalid_request".to_string(),
            report: TranslationReport::new(TranslationProtocol::AnthropicMessages),
        }
    }

    fn unsupported(param: impl AsRef<str>) -> Self {
        let param = param.as_ref();
        Self {
            message: format!("{param} is not supported by this endpoint"),
            error_type: "unsupported_feature".to_string(),
            report: TranslationReport::new(TranslationProtocol::AnthropicMessages),
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

pub fn map_messages_request(request: Value) -> Result<NativeRunRequest, AnthropicCompatError> {
    translate_messages_request(request).map(|translated| translated.request)
}

mod request_translation;

pub use request_translation::translate_messages_request;

fn reject_unknown_anthropic_fields(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    for field in object.keys().filter(|field| {
        matches!(
            field.as_str(),
            "context_management"
                | "tools"
                | "tool_choice"
                | "container"
                | "mcp_servers"
                | "thinking"
                | "output_config"
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
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "model"
                    | "messages"
                    | "max_tokens"
                    | "system"
                    | "stream"
                    | "metadata"
                    | "context_management"
                    | "tools"
                    | "tool_choice"
                    | "container"
                    | "mcp_servers"
                    | "thinking"
                    | "output_config"
                    | "service_tier"
                    | "temperature"
                    | "top_k"
                    | "top_p"
                    | "stop_sequences"
                    | "stream_options"
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        "$",
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic Messages field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic Messages field")
                .with_report(report.clone()),
        );
    }
    Ok(())
}

fn record_anthropic_system_decision(value: Option<&Value>, report: &mut TranslationReport) {
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

fn reject_legacy_anthropic_control(
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

fn reject_legacy_anthropic_control_text(
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

fn validate_anthropic_message(
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
    if !matches!(role, "user" | "assistant") {
        report.record(
            &role_path,
            None,
            TranslationDecisionKind::Rejected,
            Some("unsupported Anthropic message role"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            AnthropicCompatError::invalid("unsupported message role").with_report(report.clone())
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

fn validate_anthropic_content_blocks(
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
            "tool_result" | "tool_use" | "server_tool_use" | "computer_use" | "thinking"
            | "redacted_thinking" => {
                report.record(
                    &path,
                    None,
                    TranslationDecisionKind::Unsupported,
                    Some("tool and reasoning content has no current canonical owner"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(
                    AnthropicCompatError::unsupported("messages").with_report(report.clone())
                );
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

fn validate_anthropic_supported_content_block(
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
            TranslationDecisionKind::Unsupported,
            None,
            report,
        )?;
        return Err(AnthropicCompatError::unsupported("messages").with_report(report.clone()));
    }
    report.record(
        &format!("{block_path}.cache_control"),
        None,
        TranslationDecisionKind::Defaulted,
        Some("no content-block cache control"),
        TranslationSafeRepresentation::Defaulted,
    );
    Ok(())
}

fn reject_unknown_anthropic_content_block_fields(
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
    for field in object
        .keys()
        .filter(|field| unsupported_fields.contains(&field.as_str()))
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

fn validate_anthropic_media_source(
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

fn validate_anthropic_cache_control(
    value: &Value,
    cache_path: &str,
    kind: TranslationDecisionKind,
    target_path: Option<&str>,
    report: &mut TranslationReport,
) -> Result<(), AnthropicCompatError> {
    let unsupported_reason = (kind == TranslationDecisionKind::Unsupported)
        .then_some("message content cache control has no current canonical owner");
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
        unsupported_reason,
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
        unsupported_reason,
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
            unsupported_reason,
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

fn reject_anthropic_nested_field(
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

fn anthropic_response_mode(
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

#[derive(Debug, Default)]
struct AnthropicRequestMetadata {
    user_id: Option<String>,
    expand_id: Option<String>,
    session_id: Option<String>,
    trace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeCodeUserIdentity {
    #[serde(default)]
    account_uuid: Option<String>,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
}

fn anthropic_metadata(
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

fn anthropic_max_output_tokens(
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

fn normalize_anthropic_model_for_native(model: &str) -> (String, Option<&'static str>) {
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
        return (native_model, Some(ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE));
    }

    (model.to_string(), None)
}

fn anthropic_system_content_parts(
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

fn native_anthropic_system_text_block(object: &Map<String, Value>) -> NativePromptBlock {
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

fn validate_anthropic_system_text_block(
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

fn push_system_part(parts: &mut Vec<NativePromptBlock>, content: &str) {
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

fn claude_code_system_control_kind(system_parts: &[NativePromptBlock]) -> Option<&'static str> {
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

fn metadata_conversation(metadata: &AnthropicRequestMetadata) -> NativeObject {
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

fn metadata_user_from_user_id(
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

fn claude_code_session_id_from_identity(identity: &str) -> Option<String> {
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

fn anthropic_text_content(content: &Value) -> Result<String, AnthropicCompatError> {
    if let Some(text) = content.as_str() {
        return Ok(text.to_string());
    }
    let blocks = content
        .as_array()
        .ok_or_else(|| AnthropicCompatError::invalid("content must be text"))?;
    let mut text = String::new();
    for block in blocks {
        let block_type = block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match block_type {
            "text" => {
                if let Some(value) = block.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(value);
                }
            }
            "tool_result" => {
                let value = anthropic_tool_result_text(block);
                if !value.is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&value);
                }
            }
            "tool_use" | "server_tool_use" => {
                if block
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == "computer")
                {
                    return Err(AnthropicCompatError::unsupported("computer_use"));
                }
            }
            "computer_use" => {
                return Err(AnthropicCompatError::unsupported("computer_use"));
            }
            "thinking" | "redacted_thinking" => {}
            "image" | "document" => {}
            _ => return Err(AnthropicCompatError::unsupported("messages")),
        }
    }
    Ok(text)
}

fn anthropic_current_user_text_content(content: &Value) -> Result<String, AnthropicCompatError> {
    if let Some(text) = content.as_str() {
        return Ok(text.to_string());
    }
    let blocks = content
        .as_array()
        .ok_or_else(|| AnthropicCompatError::invalid("content must be text"))?;
    if !anthropic_blocks_have_visible_user_text(blocks) {
        return anthropic_text_content(content);
    }

    let mut text = String::new();
    for block in blocks {
        let block_type = block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match block_type {
            "text" => {
                if let Some(value) = block.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(value);
                }
            }
            "tool_result" => {}
            "tool_use" | "server_tool_use" => {
                if block
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == "computer")
                {
                    return Err(AnthropicCompatError::unsupported("computer_use"));
                }
            }
            "computer_use" => {
                return Err(AnthropicCompatError::unsupported("computer_use"));
            }
            "thinking" | "redacted_thinking" => {}
            "image" | "document" => {}
            _ => return Err(AnthropicCompatError::unsupported("messages")),
        }
    }
    Ok(text)
}

fn anthropic_blocks_have_visible_user_text(blocks: &[Value]) -> bool {
    blocks.iter().any(|block| {
        block
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|block_type| block_type == "text")
            && block
                .get("text")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.trim().is_empty())
    })
}

pub fn anthropic_content_is_tool_result_only(content: &Value) -> bool {
    let Some(blocks) = content.as_array() else {
        return false;
    };
    let mut has_tool_result = false;
    for block in blocks {
        match block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "tool_result" => has_tool_result = true,
            "thinking" | "redacted_thinking" => {}
            "text" => {
                if block
                    .get("text")
                    .and_then(Value::as_str)
                    .is_some_and(|text| !text.trim().is_empty())
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    has_tool_result
}

fn history_content_blocks(_role: &str, content: &Value) -> Option<Value> {
    let blocks = content.as_array()?;
    let has_media_blocks = blocks.iter().any(anthropic_history_block_has_media);
    let mut mapped_blocks = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("thinking" | "redacted_thinking") => {}
            Some("text") if has_media_blocks => {
                if let Some(text_block) = canonical_anthropic_text_block(block) {
                    mapped_blocks.push(text_block);
                }
            }
            Some("text") => {}
            Some("image" | "document") => {
                if let Some(media_block) = canonical_anthropic_media_block(block) {
                    mapped_blocks.push(media_block);
                }
            }
            Some("tool_result") if has_media_blocks => {
                mapped_blocks.extend(anthropic_tool_result_content_blocks(_role, block));
            }
            _ => {}
        }
    }
    (!mapped_blocks.is_empty()).then_some(Value::Array(mapped_blocks))
}

fn query_media_content_blocks(content: &Value) -> Option<Value> {
    let blocks = content.as_array()?;
    let media_blocks = blocks
        .iter()
        .filter(|block| {
            matches!(
                block.get("type").and_then(Value::as_str),
                Some("image" | "document")
            )
        })
        .filter_map(canonical_anthropic_media_block)
        .collect::<Vec<_>>();
    (!media_blocks.is_empty()).then_some(Value::Array(media_blocks))
}

fn anthropic_tool_result_text(block: &Value) -> String {
    let Some(content) = block.get("content") else {
        return String::new();
    };
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    if let Some(blocks) = content.as_array() {
        let text = blocks
            .iter()
            .filter_map(|entry| entry.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if blocks
            .iter()
            .all(|entry| entry.get("type").and_then(Value::as_str) == Some("text"))
            || !text.trim().is_empty()
        {
            return text;
        }
        if blocks.iter().any(anthropic_content_block_is_media) {
            return String::new();
        }
        return content.to_string();
    }
    content.to_string()
}

fn anthropic_history_block_has_media(block: &Value) -> bool {
    match block.get("type").and_then(Value::as_str) {
        Some("image" | "document") => true,
        Some("tool_result") => block
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|blocks| blocks.iter().any(anthropic_content_block_is_media)),
        _ => false,
    }
}

fn anthropic_content_block_is_media(block: &Value) -> bool {
    matches!(
        block.get("type").and_then(Value::as_str),
        Some("image" | "document")
    )
}

fn canonical_anthropic_text_block(block: &Value) -> Option<Value> {
    let text = block.get("text")?.as_str()?.trim();
    (!text.is_empty()).then(|| json!({ "type": "text", "text": text }))
}

fn anthropic_tool_result_content_blocks(_role: &str, block: &Value) -> Vec<Value> {
    block
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|entry| match entry.get("type").and_then(Value::as_str) {
                    Some("text") => canonical_anthropic_text_block(entry),
                    Some("image" | "document") => canonical_anthropic_media_block(entry),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn canonical_anthropic_media_block(block: &Value) -> Option<Value> {
    let object = block.as_object()?;
    let block_type = object.get("type")?.as_str()?;
    if !matches!(block_type, "image" | "document") {
        return None;
    }
    let source = object.get("source")?.as_object()?;
    let source_type = source.get("type")?.as_str()?;
    let mut canonical_source = Map::new();
    canonical_source.insert("type".to_string(), Value::String(source_type.to_string()));
    match source_type {
        "base64" => {
            canonical_source.insert(
                "media_type".to_string(),
                Value::String(source.get("media_type")?.as_str()?.to_string()),
            );
            canonical_source.insert(
                "data".to_string(),
                Value::String(source.get("data")?.as_str()?.to_string()),
            );
        }
        "url" => {
            canonical_source.insert(
                "url".to_string(),
                Value::String(source.get("url")?.as_str()?.to_string()),
            );
        }
        "text" if block_type == "document" => {
            if let Some(media_type) = source.get("media_type").and_then(Value::as_str) {
                canonical_source.insert(
                    "media_type".to_string(),
                    Value::String(media_type.to_string()),
                );
            }
            canonical_source.insert(
                "data".to_string(),
                Value::String(source.get("data")?.as_str()?.to_string()),
            );
        }
        _ => return None,
    }
    Some(json!({
        "type": block_type,
        "source": Value::Object(canonical_source),
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn d2_ac_007_tools_have_an_unsupported_receipt() {
        let error = translate_messages_request(json!({
            "model": "claude-compatible",
            "messages": [
                { "role": "user", "content": "say hello" }
            ],
            "tools": [
                {
                    "name": "read_file",
                    "description": "Read a file",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string" }
                        }
                    }
                }
            ],
            "tool_choice": { "type": "auto" }
        }))
        .expect_err("tools have no D2 canonical owner");

        assert_eq!(error.error_type, "unsupported_feature");
        assert!(error
            .report
            .has_decision("$.tools", TranslationDecisionKind::Unsupported));
    }

    #[test]
    fn prompt_markers_are_not_interpreted_as_system_context() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {
                    "role": "user",
                    "content": "<system-reminder>internal tools</system-reminder>\n\nhi？"
                }
            ]
        }))
        .unwrap();

        assert_eq!(
            request.query,
            "<system-reminder>internal tools</system-reminder>\n\nhi？"
        );
        assert!(request.system_text().is_none());
        assert!(request.history.is_empty());
    }

    #[test]
    fn maps_top_level_system_content_blocks_into_native_system() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "system": [
                {
                    "type": "text",
                    "text": "Use Claude Code project instructions.",
                    "cache_control": { "type": "ephemeral" }
                },
                {
                    "type": "text",
                    "text": "Preserve repository safety rules."
                }
            ],
            "messages": [
                { "role": "user", "content": "hi？" }
            ]
        }))
        .unwrap();

        assert_eq!(
            request.system_text().as_deref(),
            Some("Use Claude Code project instructions.\n\nPreserve repository safety rules.")
        );
        assert_eq!(request.query, "hi？");
    }

    #[test]
    fn prompt_markers_are_not_interpreted_in_history() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {
                    "role": "user",
                    "content": "<system-reminder>available tools</system-reminder>\n\nhi？"
                },
                {
                    "role": "assistant",
                    "content": "<think>private reasoning</think>嗨，有什么需要我帮忙的？"
                },
                {
                    "role": "user",
                    "content": "uploads/agent-flow-preview-debug.png 描述一下这幅图说什么？"
                }
            ]
        }))
        .unwrap();

        assert_eq!(
            request.query,
            "uploads/agent-flow-preview-debug.png 描述一下这幅图说什么？"
        );
        assert_eq!(
            request.history,
            vec![
                json!({"role": "user", "content": "<system-reminder>available tools</system-reminder>\n\nhi？"}),
                json!({"role": "assistant", "content": "<think>private reasoning</think>嗨，有什么需要我帮忙的？"}),
            ]
        );
        assert!(request.system_text().is_none());
    }

    #[test]
    fn duplicate_turns_are_preserved_without_prompt_marker_heuristics() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "Describe image"},
                {"role": "assistant", "content": "old draft"},
                {"role": "user", "content": "Describe image"},
                {"role": "assistant", "content": "<think>retry</think>final draft"},
                {"role": "user", "content": "Continue"}
            ]
        }))
        .unwrap();

        assert_eq!(request.query, "Continue");
        assert_eq!(
            request.history,
            vec![
                json!({"role": "user", "content": "Describe image"}),
                json!({"role": "assistant", "content": "old draft"}),
                json!({"role": "user", "content": "Describe image"}),
                json!({"role": "assistant", "content": "<think>retry</think>final draft"}),
            ]
        );
    }

    #[test]
    fn replayed_current_user_turn_keeps_prior_history() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "Describe image"},
                {"role": "assistant", "content": "old image answer"},
                {"role": "user", "content": "Describe image"}
            ]
        }))
        .unwrap();

        assert_eq!(request.query, "Describe image");
        assert_eq!(
            request.history,
            vec![
                json!({"role": "user", "content": "Describe image"}),
                json!({"role": "assistant", "content": "old image answer"}),
            ]
        );
    }

    #[test]
    fn latest_media_is_preserved_without_replay_heuristics() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "Describe image"},
                {"role": "assistant", "content": "old image answer"},
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "Describe image"},
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "aW1hZ2U="
                            }
                        }
                    ]
                }
            ]
        }))
        .unwrap();

        assert_eq!(request.query, "Describe image");
        assert_eq!(request.history.len(), 3);
        assert_eq!(request.history[2]["role"], json!("user"));
        assert_eq!(request.history[2]["content"], json!(""));
        assert_eq!(
            request.history[2]["content_blocks"][0]["type"],
            json!("image")
        );
    }

    #[test]
    fn beautified_marker_text_is_not_interpreted() {
        let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "hi？"},
                {
                    "role": "assistant",
                    "content": "<think>draft</think>嗨！\n\n---\n\n下面是美化后内容\n\n你好，有需要我随时帮你。"
                },
                {"role": "user", "content": "继续"}
            ]
        }))
        .unwrap();

        assert_eq!(
            request.history,
            vec![
                json!({"role": "user", "content": "hi？"}),
                json!({"role": "assistant", "content": "<think>draft</think>嗨！\n\n---\n\n下面是美化后内容\n\n你好，有需要我随时帮你。"}),
            ]
        );
    }

    #[test]
    fn d2_ac_007_thinking_content_has_an_unsupported_receipt() {
        let error = translate_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "<system-reminder>private Claude Code context</system-reminder>\n\nhi？"
                        }
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        {"type": "thinking", "thinking": "private reasoning"},
                        {"type": "text", "text": "<think>draft</think>你好"}
                    ]
                },
                {"role": "user", "content": "继续"}
            ]
        }))
        .expect_err("thinking content has no D2 canonical owner");

        assert_eq!(error.error_type, "unsupported_feature");
        assert!(error.report.has_decision(
            "$.messages[1].content[0].type",
            TranslationDecisionKind::Unsupported
        ));
    }

    #[test]
    fn d2_ac_007_tool_result_history_has_an_unsupported_receipt() {
        let error = translate_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "describe image"},
                {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "toolu_read",
                        "name": "Read",
                        "input": {"file_path": "uploads/agent-flow-preview-debug.png"}
                    }]
                },
                {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "toolu_read",
                        "content": [{
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "aW1hZ2U="
                            }
                        }]
                    }]
                },
                {"role": "user", "content": "next question"}
            ]
        }))
        .expect_err("tool result history has no D2 canonical owner");

        assert_eq!(error.error_type, "unsupported_feature");
        assert!(error.report.has_decision(
            "$.messages[1].content[0].type",
            TranslationDecisionKind::Unsupported
        ));
    }

    #[test]
    fn d2_ac_007_latest_tool_result_has_an_unsupported_receipt() {
        let error = translate_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "toolu_read",
                        "name": "Read",
                        "input": {"file_path": "uploads/test-01.png"}
                    }]
                },
                {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "toolu_read",
                        "content": "-rw-r--r-- 1 Lw 197121 17907 Jun 12 15:25 uploads/test-01.png"
                    }]
                }
            ]
        }))
        .expect_err("tool result continuation has no D2 canonical owner");

        assert_eq!(error.error_type, "unsupported_feature");
        assert!(error.report.has_decision(
            "$.messages[0].content[0].type",
            TranslationDecisionKind::Unsupported
        ));
    }
}
