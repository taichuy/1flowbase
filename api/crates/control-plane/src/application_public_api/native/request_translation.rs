use plugin_framework::provider_contract::{
    NativeModelRequestContext, NativePromptBlock, NativePromptCacheControl,
    NativePromptCacheControlType, NativePromptCacheTtl,
};
use serde_json::{Map, Value};

use super::metadata::NativeRequestMetadataParseError;
use super::model_parameters::record_native_execution_receipts;
use super::{
    native_attachments, native_history, NativeExecution, NativeObject, NativeRequestMetadata,
    NativeRunRequest, NativeStreamOptions,
};
use crate::application_public_api::protocol_translation::{
    anonymous_unknown_source_paths, TranslatedNativeRunRequest, TranslationDecisionKind,
    TranslationProtocol, TranslationReport, TranslationSafeRepresentation,
};

/// Native is an adapter boundary too: it accepts only the public Native wire
/// shape and emits a non-durable field-decision receipt before the request
/// crosses into the published-run service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRequestTranslationError {
    pub code: &'static str,
    pub message: String,
    pub report: TranslationReport,
}

impl NativeRequestTranslationError {
    pub(super) fn translation_invariant(report: TranslationReport) -> Self {
        Self {
            code: "translation_invariant",
            message: "translation receipt invariant violated".to_string(),
            report,
        }
    }

    pub(super) fn with_report(
        code: &'static str,
        message: impl Into<String>,
        report: TranslationReport,
    ) -> Self {
        if report.ensure_consistent().is_err() {
            return Self::translation_invariant(report);
        }
        Self {
            code,
            message: message.into(),
            report,
        }
    }

    pub(super) fn rejected(
        code: &'static str,
        message: impl Into<String>,
        source_path: &str,
        kind: TranslationDecisionKind,
        reason: &'static str,
        effective_value: TranslationSafeRepresentation,
        mut report: TranslationReport,
    ) -> Self {
        report.record(source_path, None, kind, Some(reason), effective_value);
        Self::with_report(code, message, report)
    }
}

pub fn translate_native_run_request(
    value: Value,
) -> std::result::Result<TranslatedNativeRunRequest, NativeRequestTranslationError> {
    let mut report = TranslationReport::new(TranslationProtocol::Native);
    let object = match value.as_object() {
        Some(object) => object,
        None => {
            return Err(NativeRequestTranslationError::rejected(
                "body",
                "Native request body must be an object",
                "$",
                TranslationDecisionKind::Rejected,
                "Native request body must be an object",
                TranslationSafeRepresentation::Present,
                report,
            ));
        }
    };

    for field in object.keys().filter(|field| {
        matches!(
            field.as_str(),
            "compatibility_mode" | "client_protocol_envelope" | "user_id"
        )
    }) {
        let path = format!("$.{field}");
        match field.as_str() {
            "compatibility_mode" => {
                return Err(NativeRequestTranslationError::rejected(
                    "compatibility_mode",
                    "compatibility_mode is not supported by the Native API",
                    &path,
                    TranslationDecisionKind::Unsupported,
                    "this field has no public Native canonical owner",
                    TranslationSafeRepresentation::Redacted,
                    report,
                ));
            }
            "client_protocol_envelope" => {
                return Err(NativeRequestTranslationError::rejected(
                    "client_protocol_envelope",
                    "client_protocol_envelope is not supported by the Native API",
                    &path,
                    TranslationDecisionKind::Unsupported,
                    "this field has no public Native canonical owner",
                    TranslationSafeRepresentation::Redacted,
                    report,
                ));
            }
            "user_id" => {
                return Err(NativeRequestTranslationError::rejected(
                    "user_id",
                    "user_id is not a Native request field",
                    &path,
                    TranslationDecisionKind::Rejected,
                    "legacy user alias has no Native canonical owner",
                    TranslationSafeRepresentation::Redacted,
                    report,
                ));
            }
            _ => continue,
        }
    }
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "query"
                    | "system"
                    | "model"
                    | "inputs"
                    | "history"
                    | "attachments"
                    | "conversation"
                    | "expand_id"
                    | "response_mode"
                    | "stream_options"
                    | "execution"
                    | "metadata"
                    | "title"
                    | "compatibility_mode"
                    | "client_protocol_envelope"
                    | "user_id"
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        "$",
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Native request field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(NativeRequestTranslationError::with_report(
            "body",
            "unknown Native request field",
            report,
        ));
    }

    let request = NativeRunRequest {
        query: required_native_string(object, "query", "$.query", "$.query", &mut report)?,
        system: native_system(object, &mut report)?,
        model: optional_native_string(object, "model", "$.model", "$.model", &mut report)?,
        inputs: optional_native_object(object, "inputs", "$.inputs", "$.inputs", &mut report)?,
        history: native_history(object, &mut report)?,
        attachments: native_attachments(object, &mut report)?,
        conversation: optional_native_object(
            object,
            "conversation",
            "$.conversation",
            "$.conversation",
            &mut report,
        )?,
        expand_id: optional_native_string(
            object,
            "expand_id",
            "$.expand_id",
            "$.expand_id",
            &mut report,
        )?,
        response_mode: optional_native_string(
            object,
            "response_mode",
            "$.response_mode",
            "$.response_mode",
            &mut report,
        )?,
        stream_options: native_stream_options(object, &mut report)?,
        execution: native_execution(object, &mut report)?,
        metadata: native_metadata(object, &mut report)?,
        request_context: NativeModelRequestContext::default(),
        title: optional_native_string(object, "title", "$.title", "$.title", &mut report)?,
        client_protocol_envelope: None,
    };
    report
        .ensure_consistent()
        .map_err(|_| NativeRequestTranslationError::translation_invariant(report.clone()))?;
    Ok(TranslatedNativeRunRequest { request, report })
}

fn native_stream_options(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> std::result::Result<NativeStreamOptions, NativeRequestTranslationError> {
    let Some(value) = object.get("stream_options") else {
        report.record(
            "$.stream_options",
            Some("$.stream_options"),
            TranslationDecisionKind::Defaulted,
            Some("default Native stream options"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(NativeStreamOptions::default());
    };
    let options = serde_json::from_value::<NativeStreamOptions>(value.clone()).map_err(|_| {
        NativeRequestTranslationError::rejected(
            "stream_options",
            "stream_options must contain only a valid include_workflow_events value",
            "$.stream_options",
            TranslationDecisionKind::Rejected,
            "Native stream options must match the typed contract",
            TranslationSafeRepresentation::Present,
            report.clone(),
        )
    })?;
    report.record(
        "$.stream_options",
        Some("$.stream_options"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    Ok(options)
}

fn required_native_string(
    object: &Map<String, Value>,
    field: &'static str,
    source_path: &'static str,
    target_path: &'static str,
    report: &mut TranslationReport,
) -> std::result::Result<String, NativeRequestTranslationError> {
    match object.get(field).and_then(Value::as_str) {
        Some(value) => {
            report.record(
                source_path,
                Some(target_path),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            Ok(value.to_owned())
        }
        None => Err(NativeRequestTranslationError::rejected(
            field,
            format!("{field} must be a string"),
            source_path,
            TranslationDecisionKind::Rejected,
            "required Native text field must be a string",
            if object.contains_key(field) {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
            report.clone(),
        )),
    }
}

fn optional_native_string(
    object: &Map<String, Value>,
    field: &'static str,
    source_path: &'static str,
    target_path: &'static str,
    report: &mut TranslationReport,
) -> std::result::Result<Option<String>, NativeRequestTranslationError> {
    match object.get(field) {
        Some(value) if value.is_string() => {
            report.record(
                source_path,
                Some(target_path),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            Ok(value.as_str().map(ToOwned::to_owned))
        }
        Some(_) => Err(NativeRequestTranslationError::rejected(
            field,
            format!("{field} must be a string"),
            source_path,
            TranslationDecisionKind::Rejected,
            "optional Native text field must be a string",
            TranslationSafeRepresentation::Present,
            report.clone(),
        )),
        None => {
            report.record(
                source_path,
                Some(target_path),
                TranslationDecisionKind::Defaulted,
                Some("Native default"),
                TranslationSafeRepresentation::Defaulted,
            );
            Ok(None)
        }
    }
}

fn optional_native_object(
    object: &Map<String, Value>,
    field: &'static str,
    source_path: &'static str,
    target_path: &'static str,
    report: &mut TranslationReport,
) -> std::result::Result<NativeObject, NativeRequestTranslationError> {
    match object.get(field) {
        Some(Value::Object(values)) => {
            report.record(
                source_path,
                Some(target_path),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            Ok(NativeObject::from_map(values.clone()))
        }
        Some(_) => Err(NativeRequestTranslationError::rejected(
            field,
            format!("{field} must be an object"),
            source_path,
            TranslationDecisionKind::Rejected,
            "optional Native object field must be an object",
            TranslationSafeRepresentation::Present,
            report.clone(),
        )),
        None => {
            report.record(
                source_path,
                Some(target_path),
                TranslationDecisionKind::Defaulted,
                Some("empty Native object"),
                TranslationSafeRepresentation::Defaulted,
            );
            Ok(NativeObject::default())
        }
    }
}

fn native_metadata(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> std::result::Result<NativeRequestMetadata, NativeRequestTranslationError> {
    let Some(value) = object.get("metadata") else {
        report.record(
            "$.metadata",
            Some("$.metadata"),
            TranslationDecisionKind::Defaulted,
            Some("empty Native metadata"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(NativeRequestMetadata::default());
    };
    let Some(metadata) = value.as_object() else {
        return Err(NativeRequestTranslationError::rejected(
            "metadata",
            "metadata must be an object",
            "$.metadata",
            TranslationDecisionKind::Rejected,
            "Native metadata must be an object",
            TranslationSafeRepresentation::Present,
            report.clone(),
        ));
    };
    let metadata = NativeRequestMetadata::from_object(metadata)
        .map_err(|error| native_metadata_parse_error(error, report))?;
    report.record(
        "$.metadata",
        Some("$.metadata"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    if metadata.trace_id().is_some() {
        report.record(
            "$.metadata.trace_id",
            Some("$.metadata.trace_id"),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Redacted,
        );
    }
    Ok(metadata)
}

fn native_metadata_parse_error(
    error: NativeRequestMetadataParseError,
    report: &TranslationReport,
) -> NativeRequestTranslationError {
    let mut rejection_report = report.clone();
    rejection_report.record(
        "$.metadata",
        None,
        TranslationDecisionKind::Rejected,
        Some("Native metadata contains invalid typed fields"),
        TranslationSafeRepresentation::Present,
    );
    for source_path in error.source_paths() {
        rejection_report.record(
            source_path,
            None,
            TranslationDecisionKind::Rejected,
            Some(error.reason),
            error.effective_value,
        );
    }
    NativeRequestTranslationError::with_report("metadata", error.message, rejection_report)
}

fn native_execution(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> std::result::Result<NativeExecution, NativeRequestTranslationError> {
    let Some(value) = object.get("execution") else {
        report.record(
            "$.execution",
            Some("$.execution"),
            TranslationDecisionKind::Defaulted,
            Some("empty Native execution"),
            TranslationSafeRepresentation::Defaulted,
        );
        return Ok(NativeExecution::default());
    };
    let Some(execution) = value.as_object() else {
        return Err(NativeRequestTranslationError::rejected(
            "execution",
            "execution must be an object",
            "$.execution",
            TranslationDecisionKind::Rejected,
            "Native execution must be an object",
            TranslationSafeRepresentation::Present,
            report.clone(),
        ));
    };
    let execution = NativeExecution::from_object(execution).map_err(|error| {
        let mut rejection_report = report.clone();
        rejection_report.record(
            "$.execution",
            None,
            TranslationDecisionKind::Rejected,
            Some("Native execution contains invalid typed fields"),
            TranslationSafeRepresentation::Present,
        );
        let mut invalid_containers = Vec::new();
        for source_path in error.source_paths() {
            if source_path.starts_with("$.execution.model_parameters.") {
                invalid_containers.push("$.execution.model_parameters");
            }
            if source_path.starts_with("$.execution.model_parameters.reasoning.") {
                invalid_containers.push("$.execution.model_parameters.reasoning");
            }
        }
        invalid_containers.sort_unstable();
        invalid_containers.dedup();
        for container_path in invalid_containers {
            rejection_report.record(
                container_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("Native execution typed container contains invalid fields"),
                TranslationSafeRepresentation::Present,
            );
        }
        for source_path in error.source_paths() {
            rejection_report.record(
                source_path,
                None,
                error.decision_kind,
                Some(error.reason),
                error.effective_value,
            );
        }
        NativeRequestTranslationError::with_report(error.code, error.message, rejection_report)
    })?;
    report.record(
        "$.execution",
        Some("$.execution"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    record_native_execution_receipts(&execution, report);
    Ok(execution)
}

#[derive(Debug)]
pub(super) struct NativePromptBlockParseError {
    block_index: Option<usize>,
    fields: Vec<String>,
    reason: &'static str,
    effective_value: TranslationSafeRepresentation,
}

impl NativePromptBlockParseError {
    fn root(reason: &'static str) -> Self {
        Self {
            block_index: None,
            fields: Vec::new(),
            reason,
            effective_value: TranslationSafeRepresentation::Redacted,
        }
    }

    fn field_with_representation(
        field: impl Into<String>,
        reason: &'static str,
        effective_value: TranslationSafeRepresentation,
    ) -> Self {
        Self {
            block_index: None,
            fields: vec![field.into()],
            reason,
            effective_value,
        }
    }

    fn at_block(mut self, block_index: usize) -> Self {
        self.block_index = Some(block_index);
        self
    }

    fn unknown_fields<'a>(
        prefix: Option<&str>,
        fields: impl IntoIterator<Item = &'a String>,
        reason: &'static str,
    ) -> Self {
        let fields = anonymous_unknown_source_paths("$", fields)
            .into_iter()
            .map(|path| path.trim_start_matches("$.").to_string())
            .map(|field| match prefix {
                Some(prefix) => format!("{prefix}.{field}"),
                None => field,
            })
            .collect();
        Self {
            block_index: None,
            fields,
            reason,
            effective_value: TranslationSafeRepresentation::Present,
        }
    }

    fn source_paths(&self, root: &str) -> Vec<String> {
        let container_path = match self.block_index {
            Some(index) => format!("{root}[{index}]"),
            None => root.to_string(),
        };
        if self.fields.is_empty() {
            return vec![container_path];
        }
        self.fields
            .iter()
            .map(|field| format!("{container_path}.{field}"))
            .collect()
    }

    fn container_paths(&self, root: &str) -> Vec<String> {
        let Some(index) = self.block_index else {
            return Vec::new();
        };
        let block_path = format!("{root}[{index}]");
        let mut paths = vec![block_path.clone()];
        if self
            .fields
            .iter()
            .any(|field| field == "cache_control" || field.starts_with("cache_control."))
        {
            paths.push(format!("{block_path}.cache_control"));
        }
        paths
    }
}

impl std::fmt::Display for NativePromptBlockParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.reason)
    }
}

fn native_system(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> std::result::Result<Vec<NativePromptBlock>, NativeRequestTranslationError> {
    match object.get("system") {
        Some(Value::String(text)) => {
            report.record(
                "$.system",
                Some("$.system"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            Ok((!text.trim().is_empty())
                .then(|| NativePromptBlock::text(text.clone()))
                .into_iter()
                .collect())
        }
        Some(value @ Value::Array(_)) => {
            let blocks = parse_native_prompt_blocks(value).map_err(|error| {
                let mut rejection_report = report.clone();
                rejection_report.record(
                    "$.system",
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("Native system contains invalid prompt blocks"),
                    TranslationSafeRepresentation::Present,
                );
                let container_paths = error.container_paths("$.system");
                for container_path in &container_paths {
                    rejection_report.record(
                        container_path,
                        None,
                        TranslationDecisionKind::Rejected,
                        Some("Native system prompt container contains invalid fields"),
                        TranslationSafeRepresentation::Present,
                    );
                }
                for source_path in error.source_paths("$.system") {
                    if container_paths.contains(&source_path) {
                        continue;
                    }
                    rejection_report.record(
                        &source_path,
                        None,
                        TranslationDecisionKind::Rejected,
                        Some(error.reason),
                        error.effective_value,
                    );
                }
                NativeRequestTranslationError::with_report(
                    "system",
                    "system must be text or valid prompt blocks",
                    rejection_report,
                )
            })?;
            record_native_prompt_block_receipts(value, "$.system", report);
            report.record(
                "$.system",
                Some("$.system"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            Ok(blocks)
        }
        None => {
            report.record(
                "$.system",
                Some("$.system"),
                TranslationDecisionKind::Defaulted,
                Some("empty Native system"),
                TranslationSafeRepresentation::Defaulted,
            );
            Ok(Vec::new())
        }
        Some(_) => Err(NativeRequestTranslationError::rejected(
            "system",
            "system must be text or prompt blocks",
            "$.system",
            TranslationDecisionKind::Rejected,
            "Native system must be text or prompt blocks",
            TranslationSafeRepresentation::Present,
            report.clone(),
        )),
    }
}

pub(super) fn parse_native_prompt_blocks(
    value: &Value,
) -> std::result::Result<Vec<NativePromptBlock>, NativePromptBlockParseError> {
    match value {
        Value::String(text) => Ok((!text.trim().is_empty())
            .then(|| NativePromptBlock::text(text.clone()))
            .into_iter()
            .collect()),
        Value::Array(blocks) => blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                parse_native_prompt_block(block).map_err(|error| error.at_block(index))
            })
            .collect(),
        Value::Null => Err(NativePromptBlockParseError::root(
            "Native system must be text or prompt blocks",
        )),
        _ => Err(NativePromptBlockParseError::root(
            "Native system must be text or prompt blocks",
        )),
    }
}

fn record_native_prompt_block_receipts(
    value: &Value,
    root_path: &str,
    report: &mut TranslationReport,
) {
    let Some(blocks) = value.as_array() else {
        return;
    };
    for (index, block) in blocks.iter().enumerate() {
        let Some(object) = block.as_object() else {
            continue;
        };
        let block_path = format!("{root_path}[{index}]");
        report.record(
            &block_path,
            Some("$.system[]"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        report.record(
            &format!("{block_path}.type"),
            Some("$.system[].type"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        report.record(
            &format!("{block_path}.text"),
            Some("$.system[].text"),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        let Some(cache_control) = object.get("cache_control") else {
            report.record(
                &format!("{block_path}.cache_control"),
                Some("$.system[].cache_control"),
                TranslationDecisionKind::Defaulted,
                Some("no Native prompt cache-control"),
                TranslationSafeRepresentation::Defaulted,
            );
            continue;
        };
        report.record(
            &format!("{block_path}.cache_control"),
            Some("$.system[].cache_control"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        let Some(cache_control) = cache_control.as_object() else {
            continue;
        };
        report.record(
            &format!("{block_path}.cache_control.type"),
            Some("$.system[].cache_control.type"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        if cache_control.contains_key("ttl") {
            report.record(
                &format!("{block_path}.cache_control.ttl"),
                Some("$.system[].cache_control.ttl"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
        } else {
            report.record(
                &format!("{block_path}.cache_control.ttl"),
                Some("$.system[].cache_control.ttl"),
                TranslationDecisionKind::Defaulted,
                Some("Native prompt cache-control TTL default"),
                TranslationSafeRepresentation::Defaulted,
            );
        }
    }
}

fn parse_native_prompt_block(
    value: &Value,
) -> std::result::Result<NativePromptBlock, NativePromptBlockParseError> {
    let object = value.as_object().ok_or_else(|| {
        NativePromptBlockParseError::root("Native system prompt blocks must be objects")
    })?;
    let unknown_fields = object
        .keys()
        .filter(|field| !matches!(field.as_str(), "type" | "text" | "cache_control"))
        .collect::<Vec<_>>();
    if !unknown_fields.is_empty() {
        return Err(NativePromptBlockParseError::unknown_fields(
            None,
            unknown_fields,
            "unknown Native system prompt block field",
        ));
    }
    if object.get("type").and_then(Value::as_str) != Some("text") {
        return Err(NativePromptBlockParseError::field_with_representation(
            "type",
            "Native system prompt blocks support only text",
            if object.contains_key("type") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        ));
    }
    let text = object.get("text").and_then(Value::as_str).ok_or_else(|| {
        NativePromptBlockParseError::field_with_representation(
            "text",
            "Native system prompt blocks require text",
            if object.contains_key("text") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        )
    })?;
    if text.trim().is_empty() {
        return Err(NativePromptBlockParseError::field_with_representation(
            "text",
            "Native system prompt blocks must not be empty",
            TranslationSafeRepresentation::Present,
        ));
    }
    let cache_control = object
        .get("cache_control")
        .map(parse_native_prompt_cache_control)
        .transpose()?;
    Ok(NativePromptBlock::Text {
        text: text.to_owned(),
        cache_control,
    })
}

fn parse_native_prompt_cache_control(
    value: &Value,
) -> std::result::Result<NativePromptCacheControl, NativePromptBlockParseError> {
    let object = value.as_object().ok_or_else(|| {
        NativePromptBlockParseError::field_with_representation(
            "cache_control",
            "Native system prompt cache_control must be an object",
            TranslationSafeRepresentation::Present,
        )
    })?;
    let unknown_fields = object
        .keys()
        .filter(|field| !matches!(field.as_str(), "type" | "ttl"))
        .collect::<Vec<_>>();
    if !unknown_fields.is_empty() {
        return Err(NativePromptBlockParseError::unknown_fields(
            Some("cache_control"),
            unknown_fields,
            "unknown Native system prompt cache_control field",
        ));
    }
    if object.get("type").and_then(Value::as_str) != Some("ephemeral") {
        return Err(NativePromptBlockParseError::field_with_representation(
            "cache_control.type",
            "Native system prompt cache_control.type must be ephemeral",
            if object.contains_key("type") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        ));
    }
    let ttl = match object.get("ttl") {
        None => None,
        Some(Value::String(value)) if value == "5m" => Some(NativePromptCacheTtl::FiveMinutes),
        Some(Value::String(value)) if value == "1h" => Some(NativePromptCacheTtl::OneHour),
        Some(_) => {
            return Err(NativePromptBlockParseError::field_with_representation(
                "cache_control.ttl",
                "Native system prompt cache_control.ttl must be 5m or 1h",
                TranslationSafeRepresentation::Present,
            ));
        }
    };
    Ok(NativePromptCacheControl {
        cache_type: NativePromptCacheControlType::Ephemeral,
        ttl,
    })
}
