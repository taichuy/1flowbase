use plugin_framework::provider_contract::{
    NativePromptBlock, NativePromptCacheControl, NativePromptCacheControlType, NativePromptCacheTtl,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::application_public_api::client_protocol_envelope::{
    protocol_context_field_is_safe, ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE,
};
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
const ANTHROPIC_TYPED_ROOT_FIELDS: &[&str] = &[
    "model",
    "messages",
    "max_tokens",
    "system",
    "stream",
    "metadata",
    "tools",
    "tool_choice",
    "container",
    "mcp_servers",
    "thinking",
    "output_config",
    "service_tier",
    "temperature",
    "top_k",
    "top_p",
    "stop_sequences",
    "stream_options",
];

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

fn validate_anthropic_root_fields(
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

fn record_anthropic_context_management_decision(
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

fn validate_anthropic_reasoning_block(
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

fn validate_anthropic_tool_use_block(
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

fn validate_anthropic_tool_result_block(
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

fn anthropic_inputs(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<NativeObject, AnthropicCompatError> {
    let mut inputs = Map::new();
    if let Some(value) = object.get("tools") {
        let tools = value.as_array().ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                "$.tools",
                "tools must be an array",
                TranslationSafeRepresentation::Present,
            )
        })?;
        let mut normalized = Vec::with_capacity(tools.len());
        for (index, tool) in tools.iter().enumerate() {
            normalized.push(normalize_anthropic_tool(tool, index, report)?);
        }
        report.record(
            "$.tools",
            Some("$.inputs.tools"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        inputs.insert("tools".to_string(), Value::Array(normalized));
    }
    if let Some(value) = object.get("tool_choice") {
        let normalized = normalize_anthropic_tool_choice(value, report)?;
        report.record(
            "$.tool_choice",
            Some("$.inputs.tool_choice"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        inputs.insert("tool_choice".to_string(), normalized);
    }
    Ok(NativeObject::from_map(inputs))
}

fn anthropic_reasoning(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<
    Option<crate::application_public_api::native::NativeReasoningParameters>,
    AnthropicCompatError,
> {
    let mut mode = crate::application_public_api::native::NativeReasoningMode::Enabled;
    let mut budget_tokens = None;
    let mut has_reasoning = false;
    if let Some(value) = object.get("thinking") {
        let thinking = value.as_object().ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                "$.thinking",
                "thinking must be an object",
                TranslationSafeRepresentation::Present,
            )
        })?;
        let unknown_fields = thinking
            .keys()
            .filter(|field| !matches!(field.as_str(), "type" | "budget_tokens" | "display"))
            .collect::<Vec<_>>();
        let unknown_field_names = unknown_fields
            .iter()
            .map(|field| field.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if report.record_anonymous_unknown_fields(
            "$.thinking",
            unknown_fields,
            TranslationDecisionKind::Rejected,
            "unknown Anthropic thinking field",
            TranslationSafeRepresentation::Present,
        ) > 0
        {
            return Err(AnthropicCompatError::invalid(format!(
                "unknown Anthropic thinking field: {unknown_field_names}"
            ))
            .with_report(report.clone()));
        }
        let thinking_type = thinking
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                reject_anthropic_nested_field(
                    report,
                    "$.thinking.type",
                    "thinking type must be text",
                    if thinking.contains_key("type") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
                )
            })?;
        mode = match thinking_type {
            "adaptive" => crate::application_public_api::native::NativeReasoningMode::Adaptive,
            "enabled" => crate::application_public_api::native::NativeReasoningMode::Enabled,
            "disabled" => crate::application_public_api::native::NativeReasoningMode::Disabled,
            _ => {
                return Err(reject_anthropic_nested_field(
                    report,
                    "$.thinking.type",
                    "unknown Anthropic thinking type",
                    TranslationSafeRepresentation::Present,
                ));
            }
        };
        budget_tokens = thinking
            .get("budget_tokens")
            .map(|value| {
                value
                    .as_u64()
                    .and_then(std::num::NonZeroU64::new)
                    .ok_or_else(|| {
                        reject_anthropic_nested_field(
                            report,
                            "$.thinking.budget_tokens",
                            "thinking budget_tokens must be a positive integer",
                            TranslationSafeRepresentation::Present,
                        )
                    })
            })
            .transpose()?;
        if let Some(display) = thinking.get("display") {
            if !display.is_string() {
                return Err(reject_anthropic_nested_field(
                    report,
                    "$.thinking.display",
                    "thinking display must be text",
                    TranslationSafeRepresentation::Present,
                ));
            }
            report.record(
                "$.thinking.display",
                None,
                TranslationDecisionKind::Dropped,
                Some("Native reasoning visibility follows runtime event semantics"),
                TranslationSafeRepresentation::Present,
            );
        }
        report.record(
            "$.thinking",
            Some("$.execution.model_parameters.reasoning"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        has_reasoning = true;
    }

    let mut effort = None;
    if let Some(value) = object.get("output_config") {
        let output_config = value.as_object().ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                "$.output_config",
                "output_config must be an object",
                TranslationSafeRepresentation::Present,
            )
        })?;
        let unknown_fields = output_config
            .keys()
            .filter(|field| !matches!(field.as_str(), "effort" | "format"))
            .collect::<Vec<_>>();
        if report.record_anonymous_unknown_fields(
            "$.output_config",
            unknown_fields,
            TranslationDecisionKind::Rejected,
            "unknown Anthropic output_config field",
            TranslationSafeRepresentation::Present,
        ) > 0
        {
            return Err(
                AnthropicCompatError::invalid("unknown Anthropic output_config field")
                    .with_report(report.clone()),
            );
        }
        if output_config.contains_key("format") {
            report.record(
                "$.output_config.format",
                None,
                TranslationDecisionKind::Unsupported,
                Some("structured output format has no current Native owner"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                AnthropicCompatError::unsupported("output_config").with_report(report.clone())
            );
        }
        if let Some(value) = output_config.get("effort") {
            let value = value.as_str().ok_or_else(|| {
                reject_anthropic_nested_field(
                    report,
                    "$.output_config.effort",
                    "output_config effort must be text",
                    TranslationSafeRepresentation::Present,
                )
            })?;
            effort = Some(match value {
                "minimal" | "low" | "medium" | "high" | "xhigh" => value.to_string(),
                "max" => "xhigh".to_string(),
                _ => {
                    return Err(reject_anthropic_nested_field(
                        report,
                        "$.output_config.effort",
                        "unknown Anthropic output effort",
                        TranslationSafeRepresentation::Present,
                    ));
                }
            });
            report.record(
                "$.output_config.effort",
                Some("$.execution.model_parameters.reasoning.effort"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
            has_reasoning = true;
        }
        report.record(
            "$.output_config",
            effort
                .as_ref()
                .map(|_| "$.execution.model_parameters.reasoning"),
            if effort.is_some() {
                TranslationDecisionKind::Normalized
            } else {
                TranslationDecisionKind::Dropped
            },
            effort
                .is_none()
                .then_some("empty output_config has no Native effect"),
            TranslationSafeRepresentation::Present,
        );
    }

    if !has_reasoning {
        return Ok(None);
    }
    Ok(Some(
        crate::application_public_api::native::NativeReasoningParameters::with_mode_budget_and_effort(
            mode,
            budget_tokens,
            effort.as_deref(),
        ),
    ))
}

fn normalize_anthropic_tool(
    tool: &Value,
    index: usize,
    report: &mut TranslationReport,
) -> Result<Value, AnthropicCompatError> {
    let path = format!("$.tools[{index}]");
    let object = tool.as_object().ok_or_else(|| {
        reject_anthropic_nested_field(
            report,
            &path,
            "tool definitions must be objects",
            TranslationSafeRepresentation::Present,
        )
    })?;
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "name" | "description" | "input_schema" | "cache_control"
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        &path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic tool field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic tool field")
                .with_report(report.clone()),
        );
    }
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                &format!("{path}.name"),
                "tool name must be non-empty text",
                if object.contains_key("name") {
                    TranslationSafeRepresentation::Present
                } else {
                    TranslationSafeRepresentation::Absent
                },
            )
        })?;
    let input_schema = object
        .get("input_schema")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                &format!("{path}.input_schema"),
                "tool input_schema must be an object",
                if object.contains_key("input_schema") {
                    TranslationSafeRepresentation::Present
                } else {
                    TranslationSafeRepresentation::Absent
                },
            )
        })?;
    let mut normalized = Map::new();
    normalized.insert("name".to_string(), Value::String(name.to_string()));
    normalized.insert(
        "source".to_string(),
        Value::String("anthropic_compatible".to_string()),
    );
    if let Some(description) = object.get("description").and_then(Value::as_str) {
        normalized.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
    normalized.insert("input_schema".to_string(), input_schema);
    if object.contains_key("cache_control") {
        report.record(
            &format!("{path}.cache_control"),
            None,
            TranslationDecisionKind::Dropped,
            Some("tool cache hints do not affect Native tool semantics"),
            TranslationSafeRepresentation::Present,
        );
    }
    Ok(Value::Object(normalized))
}

fn normalize_anthropic_tool_choice(
    value: &Value,
    report: &mut TranslationReport,
) -> Result<Value, AnthropicCompatError> {
    let object = value.as_object().ok_or_else(|| {
        reject_anthropic_nested_field(
            report,
            "$.tool_choice",
            "tool_choice must be an object",
            TranslationSafeRepresentation::Present,
        )
    })?;
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "type" | "name" | "disable_parallel_tool_use"
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        "$.tool_choice",
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic tool_choice field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic tool_choice field")
                .with_report(report.clone()),
        );
    }
    if object
        .get("disable_parallel_tool_use")
        .and_then(Value::as_bool)
        == Some(true)
    {
        report.record(
            "$.tool_choice.disable_parallel_tool_use",
            None,
            TranslationDecisionKind::Unsupported,
            Some("Native tool choice does not yet constrain parallel calls"),
            TranslationSafeRepresentation::Present,
        );
        return Err(AnthropicCompatError::unsupported("tool_choice").with_report(report.clone()));
    }
    if object.contains_key("disable_parallel_tool_use") {
        report.record(
            "$.tool_choice.disable_parallel_tool_use",
            None,
            TranslationDecisionKind::Dropped,
            Some("false preserves Native parallel tool defaults"),
            TranslationSafeRepresentation::Present,
        );
    }
    match object.get("type").and_then(Value::as_str) {
        Some("auto") => Ok(json!("auto")),
        Some("any") => Ok(json!("required")),
        Some("none") => Ok(json!("none")),
        Some("tool") => object
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|name| json!({ "name": name }))
            .ok_or_else(|| {
                reject_anthropic_nested_field(
                    report,
                    "$.tool_choice.name",
                    "tool_choice name must be non-empty text",
                    if object.contains_key("name") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
                )
            }),
        _ => Err(reject_anthropic_nested_field(
            report,
            "$.tool_choice.type",
            "unknown Anthropic tool_choice type",
            if object.contains_key("type") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        )),
    }
}

fn anthropic_history_entries(
    role: &str,
    content: &Value,
) -> Result<Vec<Value>, AnthropicCompatError> {
    let text = anthropic_history_text_content(content)?;
    if role == "assistant" {
        let mut message = json!({ "role": "assistant", "content": text });
        let reasoning_blocks = anthropic_history_reasoning_blocks(content);
        if !reasoning_blocks.is_empty() {
            message["content_blocks"] = Value::Array(reasoning_blocks);
        }
        let tool_calls = anthropic_history_tool_calls(content);
        if !tool_calls.is_empty() {
            message["tool_calls"] = Value::Array(tool_calls);
        }
        return Ok(vec![message]);
    }

    let mut messages = anthropic_history_tool_results(content);
    let media = query_media_content_blocks(content);
    if !text.trim().is_empty() || media.is_some() || messages.is_empty() {
        let mut message = json!({ "role": role, "content": text });
        if let Some(media) = media {
            message["content_blocks"] = media;
        }
        messages.push(message);
    }
    Ok(messages)
}

fn anthropic_history_reasoning_blocks(content: &Value) -> Vec<Value> {
    let Some(blocks) = content.as_array() else {
        return Vec::new();
    };
    if !blocks.iter().any(|block| {
        matches!(
            block.get("type").and_then(Value::as_str),
            Some("thinking" | "redacted_thinking")
        )
    }) {
        return Vec::new();
    }
    blocks
        .iter()
        .filter_map(|block| match block.get("type").and_then(Value::as_str) {
            Some("text") => canonical_anthropic_text_block(block),
            Some("thinking") => {
                let mut reasoning = json!({
                    "type": "reasoning",
                    "text": block.get("thinking")?.as_str()?,
                });
                if let Some(signature) = block.get("signature").and_then(Value::as_str) {
                    reasoning["signature"] = Value::String(signature.to_string());
                }
                Some(reasoning)
            }
            Some("redacted_thinking") => Some(json!({
                "type": "reasoning_redacted",
                "data": block.get("data")?.as_str()?,
            })),
            _ => None,
        })
        .collect()
}

fn anthropic_history_text_content(content: &Value) -> Result<String, AnthropicCompatError> {
    if let Some(text) = content.as_str() {
        return Ok(text.to_string());
    }
    let blocks = content
        .as_array()
        .ok_or_else(|| AnthropicCompatError::invalid("content must be text"))?;
    Ok(blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn anthropic_history_tool_calls(content: &Value) -> Vec<Value> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|block| {
            Some(json!({
                "id": block.get("id")?.as_str()?,
                "name": block.get("name")?.as_str()?,
                "arguments": block.get("input").cloned().unwrap_or_else(|| json!({})),
            }))
        })
        .collect()
}

fn anthropic_history_tool_results(content: &Value) -> Vec<Value> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
        .filter_map(|block| {
            let tool_call_id = block.get("tool_use_id")?.as_str()?;
            let mut message = json!({
                "role": "tool",
                "content": anthropic_tool_result_text(block),
                "tool_call_id": tool_call_id,
            });
            if let Some(is_error) = block.get("is_error").and_then(Value::as_bool) {
                message["is_error"] = Value::Bool(is_error);
            }
            let content_blocks = anthropic_tool_result_content_blocks("tool", block);
            if !content_blocks.is_empty() {
                message["content_blocks"] = Value::Array(content_blocks);
            }
            Some(message)
        })
        .collect()
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
    fn tools_have_a_native_translation_receipt() {
        let translated = translate_messages_request(json!({
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
        .expect("tools have a Native owner");

        assert!(translated
            .report
            .has_decision("$.tools", TranslationDecisionKind::Normalized));
        assert_eq!(
            translated.request.inputs.as_value()["tools"][0]["name"],
            "read_file"
        );
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
    fn thinking_content_has_a_native_reasoning_receipt() {
        let translated = translate_messages_request(json!({
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
        .expect("thinking content has a Native reasoning owner");

        assert_eq!(
            translated.request.history[1]["content_blocks"][0],
            json!({"type": "reasoning", "text": "private reasoning"})
        );
        assert!(translated.report.has_decision(
            "$.messages[1].content[0].type",
            TranslationDecisionKind::Normalized
        ));
    }

    #[test]
    fn tool_result_history_maps_to_native_tool_messages() {
        let translated = translate_messages_request(json!({
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
        .expect("tool result history has a Native owner");

        assert_eq!(translated.request.history[1]["role"], "assistant");
        assert_eq!(
            translated.request.history[1]["tool_calls"][0]["id"],
            "toolu_read"
        );
        assert_eq!(translated.request.history[2]["role"], "tool");
        assert_eq!(translated.request.history[2]["tool_call_id"], "toolu_read");
        assert_eq!(
            translated.request.history[2]["content_blocks"][0]["type"],
            "image"
        );
    }

    #[test]
    fn latest_tool_result_maps_to_native_query_for_callback_routing() {
        let translated = translate_messages_request(json!({
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
        .expect("latest tool result has a Native representation");

        assert_eq!(translated.request.history[0]["role"], "assistant");
        assert_eq!(
            translated.request.history[0]["tool_calls"][0]["id"],
            "toolu_read"
        );
        assert_eq!(
            translated.request.query,
            "-rw-r--r-- 1 Lw 197121 17907 Jun 12 15:25 uploads/test-01.png"
        );
    }
}
