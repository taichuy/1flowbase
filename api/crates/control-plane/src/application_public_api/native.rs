use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
pub use orchestration_runtime::answer_projection::{
    answer_segments_from_text, answer_segments_from_value, answer_segments_value,
    AnswerProjectionSegment, AnswerProjectionSegmentKind, ANSWER_SEGMENTS_KEY,
};
use plugin_framework::provider_contract::{
    ClientProtocolEnvelope, NativeModelPromptContext, NativeModelRequestContext, NativePromptBlock,
    CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY, NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY,
    NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY,
};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::{json, Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    api_keys::ApplicationApiKeyService,
    callback_resume::ApplicationPublishedCallbackAttemptRepository,
    conversations::ApplicationPublicConversationRepository,
    mapping::ApplicationApiMappingConfig,
    protocol_translation::{
        TranslationDecisionKind, TranslationReport, TranslationSafeRepresentation,
    },
    run_service::{
        ApplicationPublishedFlowRunRepository, ApplicationPublishedRunControlRepository,
        ApplicationPublishedRunService,
    },
};
use crate::flow_run_title::build_flow_run_title;
use crate::ports::{
    ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
    ApplicationRepository, AuthRepository, CacheStore, RuntimeEventDurability, RuntimeEventStream,
};

mod compaction;
mod metadata;
mod model_parameters;

pub use compaction::{
    compaction_intent, operation_result_requirement, CompactionIntent, CompactionProfile,
    CompactionResultRequirement,
};
pub use metadata::NativeRequestMetadata;
pub(crate) use metadata::ResponsesTransportRequirement;
pub use model_parameters::{
    NativeExecution, NativeExecutionModelParameters, NativeReasoningParameters,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRunRequest {
    pub query: String,
    #[serde(
        default,
        deserialize_with = "deserialize_native_prompt_blocks",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub system: Vec<NativePromptBlock>,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub model: Option<String>,
    #[serde(default, deserialize_with = "deserialize_native_object")]
    pub inputs: NativeObject,
    #[serde(default, deserialize_with = "deserialize_native_history")]
    pub history: Vec<Value>,
    #[serde(default)]
    pub attachments: Vec<NativeAttachment>,
    #[serde(default, deserialize_with = "deserialize_native_object")]
    pub conversation: NativeObject,
    #[serde(
        rename = "expand_id",
        default,
        deserialize_with = "deserialize_optional_string_reject_null"
    )]
    pub expand_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub response_mode: Option<String>,
    #[serde(default, deserialize_with = "deserialize_native_object")]
    pub stream_options: NativeObject,
    #[serde(default)]
    pub execution: NativeExecution,
    #[serde(default)]
    pub metadata: NativeRequestMetadata,
    #[serde(default, skip_serializing_if = "NativeModelRequestContext::is_empty")]
    pub request_context: NativeModelRequestContext,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub title: Option<String>,
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub client_protocol_envelope: Option<ClientProtocolEnvelope>,
}

impl NativeRunRequest {
    pub fn system_text(&self) -> Option<String> {
        (!self.system.is_empty()).then(|| {
            self.system
                .iter()
                .map(NativePromptBlock::text_content)
                .collect::<Vec<_>>()
                .join("\n\n")
        })
    }
}

mod request_translation;

use request_translation::parse_native_prompt_blocks;
pub use request_translation::{translate_native_run_request, NativeRequestTranslationError};
fn native_history(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> std::result::Result<Vec<Value>, NativeRequestTranslationError> {
    let Some(value) = object.get("history") else {
        report.record(
            "$.history",
            Some("$.history"),
            TranslationDecisionKind::Defaulted,
            Some("empty Native history"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(Vec::new());
    };
    let Some(entries) = value.as_array() else {
        return Err(NativeRequestTranslationError::rejected(
            "history",
            "history must be an array",
            "$.history",
            TranslationDecisionKind::Rejected,
            "Native history must be an array",
            TranslationSafeRepresentation::Present,
            report.clone(),
        ));
    };
    report.record(
        "$.history",
        Some("$.history"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| normalize_native_history_entry(entry, index, report))
        .collect()
}

fn normalize_native_history_entry(
    entry: &Value,
    index: usize,
    report: &mut TranslationReport,
) -> std::result::Result<Value, NativeRequestTranslationError> {
    let entry_path = format!("$.history[{index}]");
    let Some(object) = entry.as_object() else {
        return Err(NativeRequestTranslationError::rejected(
            "history",
            "history entries must be objects",
            &entry_path,
            TranslationDecisionKind::Rejected,
            "Native history entries must be objects",
            TranslationSafeRepresentation::Present,
            report.clone(),
        ));
    };
    report.record(
        &entry_path,
        Some("$.history[]"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "role"
                    | "content"
                    | "name"
                    | "tool_call_id"
                    | "is_error"
                    | "tool_calls"
                    | "content_blocks"
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        &entry_path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Native history field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(NativeRequestTranslationError::with_report(
            "history",
            "unknown Native history field",
            report.clone(),
        ));
    }
    let role_path = format!("{entry_path}.role");
    let role = object.get("role").and_then(Value::as_str).ok_or_else(|| {
        NativeRequestTranslationError::rejected(
            "history",
            "history role must be text",
            &role_path,
            TranslationDecisionKind::Rejected,
            "Native history role must be text",
            if object.contains_key("role") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
            report.clone(),
        )
    })?;
    if !matches!(role, "system" | "user" | "assistant" | "tool") {
        return Err(NativeRequestTranslationError::rejected(
            "history",
            "unsupported Native history role",
            &role_path,
            TranslationDecisionKind::Rejected,
            "unsupported Native history role",
            TranslationSafeRepresentation::Present,
            report.clone(),
        ));
    }
    report.record(
        &role_path,
        Some("$.history[].role"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    let content_path = format!("{entry_path}.content");
    let content = object
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            NativeRequestTranslationError::rejected(
                "history",
                "history content must be text",
                &content_path,
                TranslationDecisionKind::Rejected,
                "Native history content must be text",
                if object.contains_key("content") {
                    TranslationSafeRepresentation::Present
                } else {
                    TranslationSafeRepresentation::Absent
                },
                report.clone(),
            )
        })?;
    report.record(
        &content_path,
        Some("$.history[].content"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    let mut normalized = Map::new();
    normalized.insert("role".to_string(), Value::String(role.to_string()));
    normalized.insert("content".to_string(), Value::String(content.to_string()));
    for field in ["name", "tool_call_id"] {
        if let Some(value) = object.get(field) {
            if !value.is_string() {
                return Err(NativeRequestTranslationError::rejected(
                    "history",
                    format!("history {field} must be text"),
                    &format!("{entry_path}.{field}"),
                    TranslationDecisionKind::Rejected,
                    "Native history text field must be text",
                    TranslationSafeRepresentation::Present,
                    report.clone(),
                ));
            }
            normalized.insert(field.to_string(), value.clone());
        }
    }
    if role == "tool" && !normalized.contains_key("tool_call_id") {
        return Err(NativeRequestTranslationError::rejected(
            "history",
            "tool history requires tool_call_id",
            &format!("{entry_path}.tool_call_id"),
            TranslationDecisionKind::Rejected,
            "Native tool history requires tool_call_id",
            TranslationSafeRepresentation::Absent,
            report.clone(),
        ));
    }
    if let Some(value) = object.get("is_error") {
        if !value.is_boolean() {
            return Err(NativeRequestTranslationError::rejected(
                "history",
                "history is_error must be boolean",
                &format!("{entry_path}.is_error"),
                TranslationDecisionKind::Rejected,
                "Native history is_error must be boolean",
                TranslationSafeRepresentation::Present,
                report.clone(),
            ));
        }
        normalized.insert("is_error".to_string(), value.clone());
    }
    for field in ["tool_calls", "content_blocks"] {
        if let Some(value) = object.get(field) {
            if !value.is_array() {
                return Err(NativeRequestTranslationError::rejected(
                    "history",
                    format!("history {field} must be an array"),
                    &format!("{entry_path}.{field}"),
                    TranslationDecisionKind::Rejected,
                    "Native history array field must be an array",
                    TranslationSafeRepresentation::Present,
                    report.clone(),
                ));
            }
            normalized.insert(field.to_string(), value.clone());
        }
    }
    Ok(Value::Object(normalized))
}

fn native_attachments(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> std::result::Result<Vec<NativeAttachment>, NativeRequestTranslationError> {
    let Some(value) = object.get("attachments") else {
        report.record(
            "$.attachments",
            Some("$.attachments"),
            TranslationDecisionKind::Defaulted,
            Some("empty Native attachments"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(Vec::new());
    };
    let Some(entries) = value.as_array() else {
        return Err(NativeRequestTranslationError::rejected(
            "attachments",
            "attachments must be an array",
            "$.attachments",
            TranslationDecisionKind::Rejected,
            "Native attachments must be an array",
            TranslationSafeRepresentation::Present,
            report.clone(),
        ));
    };
    report.record(
        "$.attachments",
        Some("$.attachments"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| normalize_native_attachment(entry, index, report))
        .collect()
}

fn normalize_native_attachment(
    entry: &Value,
    index: usize,
    report: &mut TranslationReport,
) -> std::result::Result<NativeAttachment, NativeRequestTranslationError> {
    let entry_path = format!("$.attachments[{index}]");
    let Some(object) = entry.as_object() else {
        return Err(NativeRequestTranslationError::rejected(
            "attachments",
            "attachments entries must be objects",
            &entry_path,
            TranslationDecisionKind::Rejected,
            "Native attachment entries must be objects",
            TranslationSafeRepresentation::Present,
            report.clone(),
        ));
    };
    report.record(
        &entry_path,
        Some("$.attachments[]"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "source" | "value" | "name" | "mime_type" | "metadata"
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        &entry_path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Native attachment field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(NativeRequestTranslationError::with_report(
            "attachments",
            "unknown Native attachment field",
            report.clone(),
        ));
    }
    let source_path = format!("{entry_path}.source");
    let source = match object.get("source").and_then(Value::as_str) {
        Some("upload_file_id") => NativeAttachmentSource::UploadFileId,
        Some("url") => NativeAttachmentSource::Url,
        Some("base64") => NativeAttachmentSource::Base64,
        Some(_) => {
            return Err(NativeRequestTranslationError::rejected(
                "attachments",
                "unsupported Native attachment source",
                &source_path,
                TranslationDecisionKind::Rejected,
                "unsupported Native attachment source",
                TranslationSafeRepresentation::Present,
                report.clone(),
            ));
        }
        None => {
            return Err(NativeRequestTranslationError::rejected(
                "attachments",
                "attachment source must be text",
                &source_path,
                TranslationDecisionKind::Rejected,
                "Native attachment source must be text",
                if object.contains_key("source") {
                    TranslationSafeRepresentation::Present
                } else {
                    TranslationSafeRepresentation::Absent
                },
                report.clone(),
            ));
        }
    };
    report.record(
        &source_path,
        Some("$.attachments[].source"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Present,
    );
    let value_path = format!("{entry_path}.value");
    let value = object.get("value").and_then(Value::as_str).ok_or_else(|| {
        NativeRequestTranslationError::rejected(
            "attachments",
            "attachment value must be text",
            &value_path,
            TranslationDecisionKind::Rejected,
            "Native attachment value must be text",
            if object.contains_key("value") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
            report.clone(),
        )
    })?;
    report.record(
        &value_path,
        Some("$.attachments[].value"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    let name = optional_native_attachment_string(object, "name", &entry_path, report)?;
    let mime_type = optional_native_attachment_string(object, "mime_type", &entry_path, report)?;
    let metadata_path = format!("{entry_path}.metadata");
    let metadata = match object.get("metadata") {
        Some(Value::Object(metadata)) => {
            report.record(
                &metadata_path,
                Some("$.attachments[].metadata"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            NativeObject::from_map(metadata.clone())
        }
        Some(_) => {
            return Err(NativeRequestTranslationError::rejected(
                "attachments",
                "attachment metadata must be an object",
                &metadata_path,
                TranslationDecisionKind::Rejected,
                "Native attachment metadata must be an object",
                TranslationSafeRepresentation::Present,
                report.clone(),
            ));
        }
        None => {
            report.record(
                &metadata_path,
                Some("$.attachments[].metadata"),
                TranslationDecisionKind::Defaulted,
                Some("empty attachment metadata"),
                TranslationSafeRepresentation::Defaulted,
            );
            NativeObject::default()
        }
    };
    Ok(NativeAttachment {
        source,
        value: value.to_owned(),
        name,
        mime_type,
        metadata,
    })
}

fn optional_native_attachment_string(
    object: &Map<String, Value>,
    field: &'static str,
    entry_path: &str,
    report: &mut TranslationReport,
) -> std::result::Result<Option<String>, NativeRequestTranslationError> {
    let source_path = format!("{entry_path}.{field}");
    match object.get(field) {
        Some(Value::String(value)) => {
            report.record(
                &source_path,
                Some(&format!("$.attachments[].{field}")),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            Ok(Some(value.clone()))
        }
        Some(_) => Err(NativeRequestTranslationError::rejected(
            "attachments",
            format!("attachment {field} must be text"),
            &source_path,
            TranslationDecisionKind::Rejected,
            "Native attachment optional text fields must be text",
            TranslationSafeRepresentation::Present,
            report.clone(),
        )),
        None => {
            report.record(
                &source_path,
                Some(&format!("$.attachments[].{field}")),
                TranslationDecisionKind::Defaulted,
                Some("no attachment text value"),
                TranslationSafeRepresentation::Defaulted,
            );
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NativeObject(Map<String, Value>);

impl NativeObject {
    pub(crate) fn from_map(values: Map<String, Value>) -> Self {
        Self(values)
    }

    pub fn into_value(self) -> Value {
        Value::Object(self.0)
    }

    pub fn as_value(&self) -> Value {
        Value::Object(self.0.clone())
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn string(&self, key: &str) -> Option<String> {
        string_field(self, key)
    }

    pub fn insert_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.0.insert(key.into(), Value::String(value.into()));
    }
}

impl std::ops::Index<&str> for NativeObject {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        self.0.index(index)
    }
}

impl<'de> Deserialize<'de> for NativeObject {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Object(object) => Ok(Self(object)),
            Value::Null => Err(de::Error::custom("expected object, found null")),
            _ => Err(de::Error::custom("expected object")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAttachmentSource {
    UploadFileId,
    Url,
    Base64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAttachment {
    pub source: NativeAttachmentSource,
    pub value: String,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub mime_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_native_object")]
    pub metadata: NativeObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeRunResult {
    pub id: Uuid,
    pub application_id: Uuid,
    pub api_key_id: Uuid,
    pub publication_version_id: Uuid,
    pub status: NativeRunStatus,
    pub node_input_payload: Value,
    pub metadata: Value,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub answer_segments: Option<Vec<AnswerProjectionSegment>>,
    #[serde(default)]
    pub required_action: Option<NativeRequiredAction>,
    #[serde(default)]
    pub tool_calls: Option<Value>,
    #[serde(default)]
    pub usage: Option<NativeUsage>,
    #[serde(default)]
    pub error: Option<NativeError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_terminal: Option<orchestration_runtime::execution_state::NativeOperationTerminal>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRunStatus {
    Created,
    Queued,
    Running,
    Waiting,
    Succeeded,
    Incomplete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeRequiredAction {
    pub action_type: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeUsage {
    #[serde(default)]
    pub prompt_tokens: Option<u64>,
    #[serde(default)]
    pub completion_tokens: Option<u64>,
    #[serde(default)]
    pub total_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_hit_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_miss_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeMappedInput {
    pub node_input_payload: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeInputMappingError {
    SelectorCollision { selector: String },
    InvalidSelector { selector: String },
    InvalidPromptContext,
    InvalidSystemPrompt,
    InvalidRequestContext,
    InvalidAttachments,
}

pub struct NativeInputMapper;

impl NativeInputMapper {
    pub fn map(
        request: &NativeRunRequest,
        mapping: &ApplicationApiMappingConfig,
    ) -> std::result::Result<NativeMappedInput, NativeInputMappingError> {
        let mut node_input_payload = Value::Object(Map::new());
        let input = &mapping.input;

        write_selector(
            &mut node_input_payload,
            &input.query_target,
            Value::String(request.query.clone()),
        )?;
        if let (Some(model), Some(model_target)) = (&request.model, &input.model_target) {
            write_selector(
                &mut node_input_payload,
                model_target,
                Value::String(model.clone()),
            )?;
        }
        write_optional_selector(
            &mut node_input_payload,
            input.inputs_target.as_deref(),
            request.inputs.as_value(),
        )?;
        write_selector(
            &mut node_input_payload,
            &operation_target(input)?,
            serde_json::to_value(request.execution.execution_operation())
                .expect("canonical AI Native operation must serialize"),
        )?;
        if input.inputs_target.is_none() {
            let (start_selector, _) = input.query_target.rsplit_once('.').ok_or_else(|| {
                NativeInputMappingError::InvalidSelector {
                    selector: input.query_target.clone(),
                }
            })?;
            for field in ["tools", "tool_choice"] {
                if let Some(value) = request.inputs.get(field) {
                    write_selector(
                        &mut node_input_payload,
                        &format!("{start_selector}.{field}"),
                        value.clone(),
                    )?;
                }
            }
        }
        let (system, history) = split_system_context_from_history(request)?;
        let native_model_prompt_context = NativeModelPromptContext {
            system: system.clone(),
            messages: history.clone(),
        };
        if !native_model_prompt_context.is_empty() {
            write_selector(
                &mut node_input_payload,
                NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY,
                serde_json::to_value(native_model_prompt_context)
                    .map_err(|_| NativeInputMappingError::InvalidPromptContext)?,
            )?;
        }
        write_optional_selector(
            &mut node_input_payload,
            input.history_target.as_deref(),
            Value::Array(history),
        )?;
        write_optional_selector(
            &mut node_input_payload,
            system_target(input).as_deref(),
            serde_json::to_value(system)
                .map_err(|_| NativeInputMappingError::InvalidSystemPrompt)?,
        )?;
        write_optional_selector(
            &mut node_input_payload,
            input.attachments_target.as_deref(),
            serde_json::to_value(&request.attachments)
                .map_err(|_| NativeInputMappingError::InvalidAttachments)?,
        )?;
        if let Some(envelope) = &request.client_protocol_envelope {
            write_selector(
                &mut node_input_payload,
                CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY,
                client_protocol_envelope_payload(envelope),
            )?;
        }
        if !request.request_context.is_empty() {
            write_selector(
                &mut node_input_payload,
                NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY,
                serde_json::to_value(&request.request_context)
                    .map_err(|_| NativeInputMappingError::InvalidRequestContext)?,
            )?;
        }

        Ok(NativeMappedInput {
            node_input_payload,
            metadata: build_run_metadata(request),
        })
    }
}

fn operation_target(
    input: &super::mapping::ApplicationApiMappingInput,
) -> std::result::Result<String, NativeInputMappingError> {
    if let Some(inputs_target) = &input.inputs_target {
        return Ok(format!("{inputs_target}.operation"));
    }
    let (start_selector, _) = input.query_target.rsplit_once('.').ok_or_else(|| {
        NativeInputMappingError::InvalidSelector {
            selector: input.query_target.clone(),
        }
    })?;
    Ok(format!("{start_selector}.operation"))
}

fn client_protocol_envelope_payload(envelope: &ClientProtocolEnvelope) -> Value {
    json!({
        "source_protocol": &envelope.source_protocol,
        "policy": &envelope.policy,
        "headers": &envelope.headers,
    })
}

fn split_system_context_from_history(
    request: &NativeRunRequest,
) -> std::result::Result<(Vec<NativePromptBlock>, Vec<Value>), NativeInputMappingError> {
    let mut system_blocks = request.system.clone();
    let mut history = Vec::new();

    for message in &request.history {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or(NativeInputMappingError::InvalidPromptContext)?;
        let content_value = message
            .get("content")
            .ok_or(NativeInputMappingError::InvalidPromptContext)?;
        if role == "system" {
            system_blocks.extend(native_system_content_blocks(content_value)?);
            continue;
        }
        let content = content_value
            .as_str()
            .ok_or(NativeInputMappingError::InvalidPromptContext)?;
        if !matches!(role, "user" | "assistant" | "tool") {
            return Err(NativeInputMappingError::InvalidPromptContext);
        }
        let mut normalized = Map::new();
        normalized.insert("role".to_string(), Value::String(role.to_owned()));
        normalized.insert("content".to_string(), Value::String(content.to_owned()));
        for field in [
            "name",
            "tool_call_id",
            "is_error",
            "tool_calls",
            "content_blocks",
        ] {
            if let Some(value) = message.get(field) {
                normalized.insert(field.to_string(), value.clone());
            }
        }
        history.push(Value::Object(normalized));
    }

    Ok((system_blocks, history))
}

fn native_system_content_blocks(
    value: &Value,
) -> std::result::Result<Vec<NativePromptBlock>, NativeInputMappingError> {
    parse_native_prompt_blocks(value).map_err(|_| NativeInputMappingError::InvalidSystemPrompt)
}

fn system_target(input: &super::mapping::ApplicationApiMappingInput) -> Option<String> {
    if let Some(history_target) = input.history_target.as_deref() {
        if let Some(prefix) = history_target.strip_suffix(".history") {
            return Some(format!("{prefix}.system"));
        }
    }

    input
        .inputs_target
        .as_deref()
        .map(|target| format!("{target}.system"))
}

#[derive(Debug, Clone)]
pub struct CreateNativeRunCommand {
    pub bearer_token: String,
    pub request: NativeRunRequest,
}

#[derive(Debug, Clone)]
pub struct GetNativeRunCommand {
    pub bearer_token: String,
    pub run_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct GetNativeRunByProviderResponseIdCommand {
    pub bearer_token: String,
    pub provider_response_id: String,
}

#[derive(Debug, Clone)]
pub struct CancelNativeRunCommand {
    pub bearer_token: String,
    pub run_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRunValidationError {
    NotAuthenticated,
    ApplicationNotPublished,
    Forbidden,
    NotFound,
    InvalidMapping,
    InvalidToolResults(String),
    InvalidState,
    IdempotencyConflict,
}

pub struct ApplicationNativeRunService<R> {
    repository: R,
    last_used_cache: Option<Arc<dyn CacheStore>>,
    runtime_event_stream: Option<Arc<dyn RuntimeEventStream>>,
}

impl<R> ApplicationNativeRunService<R>
where
    R: ApplicationRepository
        + ApiKeyRepository
        + AuthRepository
        + ApplicationPublicationRepository
        + ApplicationCompiledPlanRepository
        + ApplicationPublishedFlowRunRepository
        + ApplicationPublishedRunControlRepository
        + ApplicationPublishedCallbackAttemptRepository
        + ApplicationPublicConversationRepository
        + Clone,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            last_used_cache: None,
            runtime_event_stream: None,
        }
    }

    pub fn with_last_used_cache(mut self, cache: Arc<dyn CacheStore>) -> Self {
        self.last_used_cache = Some(cache);
        self
    }

    pub fn with_runtime_event_stream(mut self, stream: Arc<dyn RuntimeEventStream>) -> Self {
        self.runtime_event_stream = Some(stream);
        self
    }

    pub async fn create_native_run(
        &self,
        command: CreateNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let run = self
            .published_run_service()
            .start_native_run(command)
            .await?;

        Ok(run)
    }

    pub async fn get_native_run(
        &self,
        command: GetNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| NativeRunValidationError::NotAuthenticated)?;
        let flow_run = self
            .repository
            .get_published_flow_run(command.run_id)
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
            .ok_or(NativeRunValidationError::NotFound)?;

        if !published_run_belongs_to_actor(&flow_run, actor.application_id, actor.api_key_id) {
            return Err(NativeRunValidationError::Forbidden);
        }

        self.project_published_native_run(actor.application_id, flow_run)
            .await
    }

    pub async fn get_native_run_by_provider_response_id(
        &self,
        command: GetNativeRunByProviderResponseIdCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| NativeRunValidationError::NotAuthenticated)?;
        let provider_response_id = command.provider_response_id.trim();
        if provider_response_id.is_empty() {
            return Err(NativeRunValidationError::NotFound);
        }
        let flow_run = self
            .repository
            .find_published_flow_run_by_provider_response_id(
                actor.application_id,
                actor.api_key_id,
                provider_response_id,
            )
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
            .ok_or(NativeRunValidationError::NotFound)?;
        self.project_published_native_run(actor.application_id, flow_run)
            .await
    }

    async fn project_published_native_run(
        &self,
        application_id: Uuid,
        flow_run: domain::FlowRunRecord,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let metadata = durable_metadata_from_flow_run(&flow_run);
        let initial_run = super::run_service::native_result_from_flow_run(&flow_run, metadata);
        if let Some(stream_state) = self
            .repository
            .get_published_run_stream_state(application_id, flow_run.id)
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
        {
            return Ok(super::run_service::native_result_from_run_stream_state(
                &initial_run,
                &stream_state,
            ));
        }

        Ok(initial_run)
    }

    pub async fn cancel_native_run(
        &self,
        command: CancelNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| NativeRunValidationError::NotAuthenticated)?;

        let flow_run = self
            .repository
            .get_published_flow_run(command.run_id)
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
            .ok_or(NativeRunValidationError::NotFound)?;
        if !published_run_belongs_to_actor(&flow_run, actor.application_id, actor.api_key_id) {
            return Err(NativeRunValidationError::Forbidden);
        }

        let cancelled = self
            .published_run_service()
            .cancel_published_run(&actor, &flow_run)
            .await?;
        if cancelled.status == domain::FlowRunStatus::Cancelled {
            self.project_committed_cancellation_terminal(&cancelled)
                .await;
            let completed_at = cancelled
                .finished_at
                .unwrap_or_else(OffsetDateTime::now_utc);
            let cancelled_callback_tasks = self
                .repository
                .cancel_published_pending_callback_tasks_for_run(cancelled.id, completed_at)
                .await
                .map_err(|_| NativeRunValidationError::InvalidState)?;
            for callback_task in cancelled_callback_tasks {
                self.repository
                    .append_published_run_event(&crate::ports::AppendRunEventInput {
                        flow_run_id: cancelled.id,
                        node_run_id: Some(callback_task.node_run_id),
                        event_type: "public_run_callback_cancelled".to_string(),
                        payload: json!({
                            "callback_task_id": callback_task.id,
                            "callback_kind": callback_task.callback_kind,
                        }),
                    })
                    .await
                    .map_err(|_| NativeRunValidationError::InvalidMapping)?;
            }
            let cancelled_attempts = self
                .repository
                .cancel_published_callback_resume_attempts_for_run(cancelled.id, completed_at)
                .await
                .map_err(|_| NativeRunValidationError::InvalidState)?;
            for attempt in cancelled_attempts {
                self.repository
                    .append_published_run_event(&crate::ports::AppendRunEventInput {
                        flow_run_id: cancelled.id,
                        node_run_id: None,
                        event_type: "public_run_resume_cancelled".to_string(),
                        payload: json!({
                            "callback_task_id": attempt.callback_task_id,
                            "resume_attempt_id": attempt.id,
                        }),
                    })
                    .await
                    .map_err(|_| NativeRunValidationError::InvalidMapping)?;
            }
        }

        Ok(super::run_service::native_result_from_flow_run(
            &cancelled,
            durable_metadata_from_flow_run(&cancelled),
        ))
    }

    fn api_key_service(&self) -> ApplicationApiKeyService<R> {
        let service = ApplicationApiKeyService::new(self.repository.clone());
        match &self.last_used_cache {
            Some(cache) => service.with_last_used_cache(cache.clone()),
            None => service,
        }
    }

    fn published_run_service(&self) -> ApplicationPublishedRunService<R> {
        let service = ApplicationPublishedRunService::new(self.repository.clone());
        match &self.last_used_cache {
            Some(cache) => service.with_last_used_cache(cache.clone()),
            None => service,
        }
    }

    async fn project_committed_cancellation_terminal(&self, flow_run: &domain::FlowRunRecord) {
        let Some(stream) = &self.runtime_event_stream else {
            return;
        };

        // The durable terminal has already won inside `cancel_published_run`.
        // This projection only closes an open live stream; it must not create a
        // second durable terminal record.
        let mut terminal_event =
            crate::orchestration_runtime::debug_stream_events::flow_cancelled(flow_run.id);
        terminal_event.persist_required = false;
        terminal_event.durability = RuntimeEventDurability::Ephemeral;
        if let Err(error) = stream
            .append_terminal_if_missing_and_close(flow_run.id, terminal_event)
            .await
        {
            tracing::warn!(
                flow_run_id = %flow_run.id,
                application_id = %flow_run.application_id,
                error = %error,
                "failed to project committed public cancellation terminal to runtime event stream"
            );
        }
    }
}

#[async_trait]
pub trait NativeRunRepository: Send + Sync {
    async fn create_native_run_result(&self, run: &NativeRunResult) -> Result<NativeRunResult>;
    async fn get_native_run_result(&self, run_id: Uuid) -> Result<Option<NativeRunResult>>;
}

fn deserialize_native_object<'de, D>(deserializer: D) -> std::result::Result<NativeObject, D::Error>
where
    D: Deserializer<'de>,
{
    NativeObject::deserialize(deserializer)
}

fn deserialize_optional_string_reject_null<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(value) => Ok(Some(value)),
        Value::Null => Err(de::Error::custom("expected string, found null")),
        _ => Err(de::Error::custom("expected string")),
    }
}

fn deserialize_native_prompt_blocks<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<NativePromptBlock>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    parse_native_prompt_blocks(&value).map_err(de::Error::custom)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeHistoryEntry {
    role: String,
    content: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tool_call_id: Option<String>,
    #[serde(default)]
    is_error: Option<bool>,
    #[serde(default)]
    tool_calls: Option<Vec<Value>>,
    #[serde(default)]
    content_blocks: Option<Vec<Value>>,
}

fn deserialize_native_history<'de, D>(deserializer: D) -> std::result::Result<Vec<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    let entries = Vec::<NativeHistoryEntry>::deserialize(deserializer)?;
    entries
        .into_iter()
        .map(|entry| {
            if !matches!(
                entry.role.as_str(),
                "system" | "user" | "assistant" | "tool"
            ) {
                return Err(de::Error::custom("unsupported Native history role"));
            }
            if entry.role == "tool" && entry.tool_call_id.is_none() {
                return Err(de::Error::custom(
                    "Native tool history requires tool_call_id",
                ));
            }
            let mut message = Map::new();
            message.insert("role".to_string(), Value::String(entry.role));
            message.insert("content".to_string(), Value::String(entry.content));
            if let Some(name) = entry.name {
                message.insert("name".to_string(), Value::String(name));
            }
            if let Some(tool_call_id) = entry.tool_call_id {
                message.insert("tool_call_id".to_string(), Value::String(tool_call_id));
            }
            if let Some(is_error) = entry.is_error {
                message.insert("is_error".to_string(), Value::Bool(is_error));
            }
            if let Some(tool_calls) = entry.tool_calls {
                message.insert("tool_calls".to_string(), Value::Array(tool_calls));
            }
            if let Some(content_blocks) = entry.content_blocks {
                message.insert("content_blocks".to_string(), Value::Array(content_blocks));
            }
            Ok(Value::Object(message))
        })
        .collect()
}

fn build_run_metadata(request: &NativeRunRequest) -> Value {
    let idempotency_key = request.execution.idempotency_key().map(ToOwned::to_owned);
    let external_user = request
        .expand_id
        .clone()
        .or_else(|| string_field(&request.conversation, "user"));
    let external_conversation_id = string_field(&request.conversation, "id");
    let external_trace_id = request.metadata.trace_id().map(ToOwned::to_owned);
    let title = build_flow_run_title(request.title.as_deref(), &request.query);

    json!({
        "model": request.model,
        "execution": request.execution.as_value(),
        "metadata": request.metadata.as_value(),
        "title": title,
        "expand_id": external_user,
        "idempotency_key": idempotency_key,
        "external_user": external_user,
        "external_conversation_id": external_conversation_id,
        "external_trace_id": external_trace_id,
        "request": {
            "conversation": request.conversation.as_value(),
            "response_mode": request.response_mode,
            "stream_options": request.stream_options.as_value()
        }
    })
}

pub(super) fn durable_metadata_from_flow_run(flow_run: &domain::FlowRunRecord) -> Value {
    json!({
        "title": flow_run.title,
        "expand_id": flow_run.external_user,
        "external_user": flow_run.external_user,
        "external_conversation_id": flow_run.external_conversation_id,
        "external_trace_id": flow_run.external_trace_id,
        "idempotency_key": flow_run.idempotency_key,
        "request": {
            "conversation": {
                "id": flow_run.external_conversation_id,
                "user": flow_run.external_user,
            }
        }
    })
}

fn published_run_belongs_to_actor(
    flow_run: &domain::FlowRunRecord,
    application_id: Uuid,
    api_key_id: Uuid,
) -> bool {
    flow_run.run_mode == domain::FlowRunMode::PublishedApiRun
        && flow_run.application_id == application_id
        && flow_run.api_key_id == Some(api_key_id)
}

fn string_field(object: &NativeObject, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn write_optional_selector(
    root: &mut Value,
    selector: Option<&str>,
    value: Value,
) -> std::result::Result<(), NativeInputMappingError> {
    let Some(selector) = selector else {
        return Ok(());
    };
    write_selector(root, selector, value)
}

pub(crate) fn write_selector(
    root: &mut Value,
    selector: &str,
    value: Value,
) -> std::result::Result<(), NativeInputMappingError> {
    let parts = selector.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(NativeInputMappingError::InvalidSelector {
            selector: selector.to_string(),
        });
    }

    let mut cursor = root;
    for part in parts.iter().take(parts.len() - 1) {
        let object =
            cursor
                .as_object_mut()
                .ok_or_else(|| NativeInputMappingError::SelectorCollision {
                    selector: selector.to_string(),
                })?;
        cursor = object
            .entry((*part).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }

    let leaf = parts[parts.len() - 1];
    let object =
        cursor
            .as_object_mut()
            .ok_or_else(|| NativeInputMappingError::SelectorCollision {
                selector: selector.to_string(),
            })?;
    if let Some(existing) = object.get_mut(leaf) {
        if let (Some(existing), Value::Object(next)) = (existing.as_object_mut(), value) {
            for (key, value) in next {
                if existing.contains_key(&key) {
                    return Err(NativeInputMappingError::SelectorCollision {
                        selector: format!("{selector}.{key}"),
                    });
                }
                existing.insert(key, value);
            }
            return Ok(());
        }

        return Err(NativeInputMappingError::SelectorCollision {
            selector: selector.to_string(),
        });
    }
    object.insert(leaf.to_string(), value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::application_public_api::mapping::{
        ApplicationApiMappingInput, ApplicationApiMappingOutput,
    };

    fn request_with_model(model: &str) -> NativeRunRequest {
        serde_json::from_value(json!({
            "query": "hello",
            "model": model,
            "execution": {
                "idempotency_key": "idem-1"
            },
            "metadata": {
                "trace_id": "trace-1"
            }
        }))
        .unwrap()
    }

    #[test]
    fn mapper_rejects_selector_collisions() {
        let mapping = ApplicationApiMappingConfig {
            input: ApplicationApiMappingInput {
                query_target: "start.query".into(),
                model_target: Some("start.query".into()),
                inputs_target: None,
                history_target: None,
                attachments_target: None,
            },
            output: ApplicationApiMappingOutput::default(),
            extension: None,
        };

        let error =
            NativeInputMapper::map(&request_with_model("any/provider"), &mapping).unwrap_err();

        assert_eq!(
            error,
            NativeInputMappingError::SelectorCollision {
                selector: "start.query".into()
            }
        );
    }

    #[test]
    fn mapper_preserves_model_metadata_when_model_target_is_null() {
        let mapping = ApplicationApiMappingConfig {
            input: ApplicationApiMappingInput {
                query_target: "start.query".into(),
                model_target: None,
                inputs_target: None,
                history_target: None,
                attachments_target: None,
            },
            output: ApplicationApiMappingOutput::default(),
            extension: None,
        };

        let mapped =
            NativeInputMapper::map(&request_with_model("unlisted-model"), &mapping).unwrap();

        assert!(mapped.node_input_payload["start"].get("model").is_none());
        assert_eq!(mapped.metadata["model"], json!("unlisted-model"));
        assert_eq!(mapped.metadata["idempotency_key"], json!("idem-1"));
        assert_eq!(mapped.metadata["external_trace_id"], json!("trace-1"));
    }

    #[test]
    fn mapper_keeps_requested_model_in_the_existing_start_model_builtin() {
        let mapped = NativeInputMapper::map(
            &request_with_model("provider/requested-model"),
            &ApplicationApiMappingConfig::default_native(),
        )
        .unwrap();

        assert_eq!(
            mapped.node_input_payload["node-start"]["model"],
            json!("provider/requested-model")
        );
        assert!(mapped.node_input_payload["node-start"]
            .get("requested_model")
            .is_none());
    }

    #[test]
    fn mapper_places_tool_registry_under_default_start_input() {
        let request: NativeRunRequest = serde_json::from_value(json!({
            "query": "hello",
            "inputs": {
                "tools": [
                    {
                        "name": "read_file",
                        "source": "openai_compatible",
                        "input_schema": {
                            "type": "object"
                        }
                    }
                ],
                "tool_choice": "auto"
            }
        }))
        .unwrap();

        let mapped =
            NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
                .unwrap();

        assert_eq!(
            mapped.node_input_payload["node-start"]["tools"][0]["name"],
            json!("read_file")
        );
        assert_eq!(
            mapped.node_input_payload["node-start"]["tool_choice"],
            json!("auto")
        );
        assert!(mapped.node_input_payload["node-start"]
            .get("compatibility")
            .is_none());
    }

    #[test]
    fn mapper_places_native_tools_under_query_start_when_inputs_target_is_absent() {
        let request: NativeRunRequest = serde_json::from_value(json!({
            "query": "hello",
            "inputs": {
                "tools": [{"name": "read_file", "input_schema": {"type": "object"}}],
                "tool_choice": "auto"
            }
        }))
        .unwrap();
        let mapping = ApplicationApiMappingConfig {
            input: ApplicationApiMappingInput {
                query_target: "node-start.query".into(),
                model_target: None,
                inputs_target: None,
                history_target: Some("node-start.history".into()),
                attachments_target: None,
            },
            output: ApplicationApiMappingOutput::default(),
            extension: None,
        };

        let mapped = NativeInputMapper::map(&request, &mapping).unwrap();

        assert_eq!(
            mapped.node_input_payload["node-start"]["tools"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            mapped.node_input_payload["node-start"]["tool_choice"],
            "auto"
        );
    }

    #[test]
    fn mapper_materializes_empty_typed_start_context() {
        let request: NativeRunRequest = serde_json::from_value(json!({
            "query": "hello"
        }))
        .unwrap();

        let mapped =
            NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
                .unwrap();

        assert_eq!(mapped.node_input_payload["node-start"]["system"], json!([]));
        assert_eq!(
            mapped.node_input_payload["node-start"]["operation"],
            json!({"kind": "generate", "profile": "standard"})
        );
        assert_eq!(
            mapped.node_input_payload["node-start"]["history"],
            json!([])
        );
    }

    #[test]
    fn mapper_materializes_only_the_safe_canonical_operation_view() {
        let request: NativeRunRequest = serde_json::from_value(json!({
            "query": "compact",
            "execution": {
                "operation": {
                    "kind": "compact",
                    "profile": "responses_compaction_v2"
                }
            }
        }))
        .unwrap();

        let mapped =
            NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
                .unwrap();

        assert_eq!(
            mapped.node_input_payload["node-start"]["operation"],
            json!({"kind": "compact", "profile": "responses_compaction_v2"})
        );
        assert_eq!(
            mapped.node_input_payload["node-start"]["operation"]
                .as_object()
                .map(|operation| operation.len()),
            Some(2)
        );
    }

    #[test]
    fn mapper_promotes_system_context_out_of_native_history() {
        let request: NativeRunRequest = serde_json::from_value(json!({
            "query": "hello",
            "system": "Use the request system.",
            "history": [
                { "role": "system", "content": "Use the legacy history system." },
                { "role": "user", "content": "Earlier question" }
            ]
        }))
        .unwrap();

        let mapped =
            NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
                .unwrap();

        assert_eq!(
            mapped.node_input_payload["node-start"]["system"],
            json!([
                { "type": "text", "text": "Use the request system." },
                { "type": "text", "text": "Use the legacy history system." }
            ])
        );
        assert_eq!(
            mapped.node_input_payload["node-start"]["history"],
            json!([{ "role": "user", "content": "Earlier question" }])
        );
        assert_eq!(
            mapped.node_input_payload[NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY],
            json!({
                "system": [
                    { "type": "text", "text": "Use the request system." },
                    { "type": "text", "text": "Use the legacy history system." }
                ],
                "messages": [
                    { "role": "user", "content": "Earlier question" }
                ]
            })
        );
    }

    #[test]
    fn mapper_rebuilds_history_without_unknown_raw_fields() {
        let sentinel = "D2-NATIVE-MAPPER-RAW-HISTORY-MUST-NOT-REACH-MODEL";
        let mut request: NativeRunRequest = serde_json::from_value(json!({
            "query": "hello"
        }))
        .unwrap();
        request.history.push(json!({
            "role": "assistant",
            "content": "prior answer",
            "content_blocks": [
                {
                    "type": "image_url",
                    "image_url": {"url": "https://example.invalid/image.png"}
                }
            ],
            "raw_provider_body": sentinel
        }));

        let mapped =
            NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
                .unwrap();

        let mapped_history = &mapped.node_input_payload["node-start"]["history"];
        assert_eq!(
            mapped_history,
            &json!([
                {
                    "role": "assistant",
                    "content": "prior answer",
                    "content_blocks": [
                        {
                            "type": "image_url",
                            "image_url": {"url": "https://example.invalid/image.png"}
                        }
                    ]
                }
            ])
        );
        assert!(!serde_json::to_string(&mapped.node_input_payload)
            .expect("mapped Native input should serialize")
            .contains(sentinel));
    }
}
