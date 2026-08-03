use super::*;

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
    pub client_protocol_envelope: Option<ProtocolContextEnvelope>,
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

pub(super) fn native_history(
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

pub(super) fn normalize_native_history_entry(
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

pub(super) fn native_attachments(
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

pub(super) fn normalize_native_attachment(
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

pub(super) fn optional_native_attachment_string(
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
