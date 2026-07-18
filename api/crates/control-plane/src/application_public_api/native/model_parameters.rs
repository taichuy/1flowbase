use std::num::NonZeroU64;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::NativeObject;
use crate::application_public_api::protocol_translation::{
    anonymous_unknown_source_paths, TranslationDecisionKind, TranslationReport,
    TranslationSafeRepresentation,
};

const MODEL_PARAMETERS_PATH: &str = "$.execution.model_parameters";
const REASONING_PATH: &str = "$.execution.model_parameters.reasoning";

/// Native execution preserves opaque execution options while giving model
/// parameters one typed owner at the protocol edge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NativeExecution {
    opaque: NativeObject,
    idempotency_key: Option<String>,
    model_parameters: Option<NativeExecutionModelParameters>,
}

impl NativeExecution {
    pub(super) fn from_object(
        object: &Map<String, Value>,
    ) -> Result<Self, NativeExecutionParseError> {
        let mut opaque = Map::new();
        let mut idempotency_key = None;
        let mut model_parameters = None;
        for (field, value) in object {
            match field.as_str() {
                "idempotency_key" => {
                    idempotency_key = Some(
                        value
                            .as_str()
                            .ok_or_else(NativeExecutionParseError::idempotency_key)?
                            .to_owned(),
                    );
                }
                "model_parameters" => {
                    model_parameters = Some(NativeExecutionModelParameters::from_value(value)?);
                }
                "compatibility_mode" => {
                    return Err(NativeExecutionParseError::compatibility_mode());
                }
                _ => {
                    opaque.insert(field.clone(), value.clone());
                }
            }
        }
        Ok(Self {
            opaque: NativeObject::from_map(opaque),
            idempotency_key,
            model_parameters,
        })
    }

    pub(crate) fn with_max_output_tokens(max_output_tokens: NonZeroU64) -> Self {
        Self {
            opaque: NativeObject::default(),
            idempotency_key: None,
            model_parameters: Some(NativeExecutionModelParameters::with_max_output_tokens(
                max_output_tokens,
            )),
        }
    }

    pub(crate) fn model_parameters(&self) -> Option<&NativeExecutionModelParameters> {
        self.model_parameters.as_ref()
    }

    pub(crate) fn idempotency_key(&self) -> Option<&str> {
        self.idempotency_key.as_deref()
    }

    pub(crate) fn as_value(&self) -> Value {
        let mut execution = object_from_native_object(&self.opaque);
        if let Some(idempotency_key) = &self.idempotency_key {
            execution.insert(
                "idempotency_key".to_string(),
                Value::String(idempotency_key.clone()),
            );
        }
        if let Some(model_parameters) = &self.model_parameters {
            execution.insert(
                "model_parameters".to_string(),
                model_parameters.canonical_value(),
            );
        }
        Value::Object(execution)
    }

    pub(crate) fn fingerprint_value(&self) -> Value {
        let mut execution = object_from_native_object(&self.opaque);
        if let Some(idempotency_key) = &self.idempotency_key {
            execution.insert(
                "idempotency_key".to_string(),
                Value::String(idempotency_key.clone()),
            );
        }
        if let Some(model_parameters) = &self.model_parameters {
            execution.insert(
                "model_parameters".to_string(),
                model_parameters.fingerprint_value(),
            );
        }
        Value::Object(execution)
    }
}

impl Serialize for NativeExecution {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NativeExecution {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| de::Error::custom("expected object"))?;
        Self::from_object(object).map_err(de::Error::custom)
    }
}

impl std::ops::Index<&str> for NativeExecution {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        self.opaque.index(index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionModelParameters {
    max_output_tokens: Option<NonZeroU64>,
    reasoning: Option<NativeReasoningParameters>,
}

impl NativeExecutionModelParameters {
    fn from_value(value: &Value) -> Result<Self, NativeModelParameterParseError> {
        let object = value.as_object().ok_or_else(|| {
            NativeModelParameterParseError::present(
                MODEL_PARAMETERS_PATH,
                "Native model_parameters must be an object",
            )
        })?;
        let unknown_fields = object
            .keys()
            .filter(|field| !matches!(field.as_str(), "max_output_tokens" | "reasoning"))
            .collect::<Vec<_>>();
        if !unknown_fields.is_empty() {
            return Err(NativeModelParameterParseError::unknown_fields(
                MODEL_PARAMETERS_PATH,
                unknown_fields,
                "unknown Native model parameter",
            ));
        }
        let max_output_tokens = match object.get("max_output_tokens") {
            Some(value) => {
                NonZeroU64::new(value.as_u64().unwrap_or_default()).ok_or_else(|| {
                    NativeModelParameterParseError::present(
                        format!("{MODEL_PARAMETERS_PATH}.max_output_tokens"),
                        "max_output_tokens must be a positive integer",
                    )
                })?
            }
            None => return Ok(Self::without_max_output_tokens(object.get("reasoning"))?),
        };
        Ok(Self {
            max_output_tokens: Some(max_output_tokens),
            reasoning: object
                .get("reasoning")
                .map(NativeReasoningParameters::from_value)
                .transpose()?,
        })
    }

    fn without_max_output_tokens(
        reasoning: Option<&Value>,
    ) -> Result<Self, NativeModelParameterParseError> {
        Ok(Self {
            max_output_tokens: None,
            reasoning: reasoning
                .map(NativeReasoningParameters::from_value)
                .transpose()?,
        })
    }

    fn with_max_output_tokens(max_output_tokens: NonZeroU64) -> Self {
        Self {
            max_output_tokens: Some(max_output_tokens),
            reasoning: None,
        }
    }

    pub(crate) fn max_output_tokens(&self) -> Option<u64> {
        self.max_output_tokens.map(NonZeroU64::get)
    }

    pub(crate) fn reasoning(&self) -> Option<&NativeReasoningParameters> {
        self.reasoning.as_ref()
    }

    pub(crate) fn canonical_value(&self) -> Value {
        self.value_with_effort(NativeReasoningEffort::normalized)
    }

    fn fingerprint_value(&self) -> Value {
        self.value_with_effort(NativeReasoningEffort::wire_spelling)
    }

    fn value_with_effort(&self, effort_value: fn(&NativeReasoningEffort) -> &str) -> Value {
        let mut model_parameters = Map::new();
        if let Some(max_output_tokens) = self.max_output_tokens() {
            model_parameters.insert(
                "max_output_tokens".to_string(),
                Value::Number(max_output_tokens.into()),
            );
        }
        if let Some(reasoning) = &self.reasoning {
            model_parameters.insert(
                "reasoning".to_string(),
                reasoning.value_with_effort(effort_value),
            );
        }
        Value::Object(model_parameters)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeReasoningParameters {
    enabled: Option<bool>,
    effort: Option<NativeReasoningEffort>,
    budget_tokens: Option<NonZeroU64>,
}

impl NativeReasoningParameters {
    fn from_value(value: &Value) -> Result<Self, NativeModelParameterParseError> {
        let object = value.as_object().ok_or_else(|| {
            NativeModelParameterParseError::present(
                REASONING_PATH,
                "Native reasoning parameters must be an object",
            )
        })?;
        let unknown_fields = object
            .keys()
            .filter(|field| !matches!(field.as_str(), "enabled" | "effort" | "budget_tokens"))
            .collect::<Vec<_>>();
        if !unknown_fields.is_empty() {
            return Err(NativeModelParameterParseError::unknown_fields(
                REASONING_PATH,
                unknown_fields,
                "unknown Native reasoning parameter",
            ));
        }
        let enabled = match object.get("enabled") {
            Some(value) => Some(value.as_bool().ok_or_else(|| {
                NativeModelParameterParseError::present(
                    format!("{REASONING_PATH}.enabled"),
                    "reasoning.enabled must be a boolean",
                )
            })?),
            None => None,
        };
        let effort = match object.get("effort") {
            Some(value) => {
                let value = value.as_str().ok_or_else(|| {
                    NativeModelParameterParseError::present(
                        format!("{REASONING_PATH}.effort"),
                        "reasoning.effort must be a supported non-empty string",
                    )
                })?;
                Some(NativeReasoningEffort::parse(value).ok_or_else(|| {
                    NativeModelParameterParseError::present(
                        format!("{REASONING_PATH}.effort"),
                        "reasoning.effort must be a supported non-empty string",
                    )
                })?)
            }
            None => None,
        };
        let budget_tokens = match object.get("budget_tokens") {
            Some(value) => Some(
                NonZeroU64::new(value.as_u64().unwrap_or_default()).ok_or_else(|| {
                    NativeModelParameterParseError::present(
                        format!("{REASONING_PATH}.budget_tokens"),
                        "reasoning.budget_tokens must be a positive integer",
                    )
                })?,
            ),
            None => None,
        };
        Ok(Self {
            enabled,
            effort,
            budget_tokens,
        })
    }

    pub(crate) fn effective_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    pub(crate) fn effort(&self) -> Option<&str> {
        self.effort.as_ref().map(NativeReasoningEffort::normalized)
    }

    pub(crate) fn budget_tokens(&self) -> Option<u64> {
        self.budget_tokens.map(NonZeroU64::get)
    }

    fn value_with_effort(&self, effort_value: fn(&NativeReasoningEffort) -> &str) -> Value {
        let mut reasoning = Map::new();
        reasoning.insert("enabled".to_string(), Value::Bool(self.effective_enabled()));
        if let Some(effort) = &self.effort {
            reasoning.insert(
                "effort".to_string(),
                Value::String(effort_value(effort).to_string()),
            );
        }
        if let Some(budget_tokens) = self.budget_tokens() {
            reasoning.insert(
                "budget_tokens".to_string(),
                Value::Number(budget_tokens.into()),
            );
        }
        Value::Object(reasoning)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeReasoningEffort {
    wire_spelling: String,
    normalized: NativeReasoningEffortKind,
}

impl NativeReasoningEffort {
    fn parse(wire_spelling: &str) -> Option<Self> {
        let normalized = NativeReasoningEffortKind::parse(wire_spelling.trim())?;
        Some(Self {
            wire_spelling: wire_spelling.to_string(),
            normalized,
        })
    }

    fn normalized(&self) -> &str {
        self.normalized.as_str()
    }

    fn wire_spelling(&self) -> &str {
        &self.wire_spelling
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeReasoningEffortKind {
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

impl NativeReasoningEffortKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::Xhigh),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
        }
    }
}

#[derive(Debug)]
pub(super) struct NativeModelParameterParseError {
    source_paths: Vec<String>,
    pub(super) reason: &'static str,
    pub(super) effective_value: TranslationSafeRepresentation,
}

#[derive(Debug)]
pub(super) struct NativeExecutionParseError {
    source_paths: Vec<String>,
    pub(super) code: &'static str,
    pub(super) message: &'static str,
    pub(super) reason: &'static str,
    pub(super) decision_kind: TranslationDecisionKind,
    pub(super) effective_value: TranslationSafeRepresentation,
}

impl NativeExecutionParseError {
    fn idempotency_key() -> Self {
        Self {
            source_paths: vec!["$.execution.idempotency_key".to_string()],
            code: "invalid_idempotency_key",
            message: "idempotency_key must be a string",
            reason: "Native execution idempotency_key must be a string",
            decision_kind: TranslationDecisionKind::Rejected,
            effective_value: TranslationSafeRepresentation::Present,
        }
    }

    fn compatibility_mode() -> Self {
        Self {
            source_paths: vec!["$.execution.compatibility_mode".to_string()],
            code: "compatibility_mode",
            message: "execution compatibility_mode is not supported by the Native API",
            reason: "execution compatibility_mode has no Native canonical owner",
            decision_kind: TranslationDecisionKind::Unsupported,
            effective_value: TranslationSafeRepresentation::Redacted,
        }
    }

    pub(super) fn source_paths(&self) -> &[String] {
        &self.source_paths
    }
}

impl std::fmt::Display for NativeExecutionParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.reason)
    }
}

impl From<NativeModelParameterParseError> for NativeExecutionParseError {
    fn from(error: NativeModelParameterParseError) -> Self {
        Self {
            source_paths: error.source_paths,
            code: "invalid_model_parameters",
            message: "invalid model parameters",
            reason: error.reason,
            decision_kind: TranslationDecisionKind::Rejected,
            effective_value: error.effective_value,
        }
    }
}

impl NativeModelParameterParseError {
    fn present(source_path: impl Into<String>, reason: &'static str) -> Self {
        Self {
            source_paths: vec![source_path.into()],
            reason,
            effective_value: TranslationSafeRepresentation::Present,
        }
    }

    fn unknown_fields<'a>(
        parent_path: &str,
        fields: impl IntoIterator<Item = &'a String>,
        reason: &'static str,
    ) -> Self {
        Self {
            source_paths: anonymous_unknown_source_paths(parent_path, fields),
            reason,
            effective_value: TranslationSafeRepresentation::Present,
        }
    }
}

impl std::fmt::Display for NativeModelParameterParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.reason)
    }
}

pub(super) fn record_native_execution_receipts(
    execution: &NativeExecution,
    report: &mut TranslationReport,
) {
    if execution.idempotency_key.is_some() {
        report.record(
            "$.execution.idempotency_key",
            Some("$.execution.idempotency_key"),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Redacted,
        );
    }
    let Some(model_parameters) = execution.model_parameters() else {
        return;
    };
    record_native_model_parameter_receipts(model_parameters, report);
}

fn record_native_model_parameter_receipts(
    model_parameters: &NativeExecutionModelParameters,
    report: &mut TranslationReport,
) {
    report.record(
        MODEL_PARAMETERS_PATH,
        Some(MODEL_PARAMETERS_PATH),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    if model_parameters.max_output_tokens().is_some() {
        let path = format!("{MODEL_PARAMETERS_PATH}.max_output_tokens");
        report.record(
            &path,
            Some(&path),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Present,
        );
    }
    let Some(reasoning) = model_parameters.reasoning() else {
        return;
    };
    report.record(
        REASONING_PATH,
        Some(REASONING_PATH),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    let enabled_path = format!("{REASONING_PATH}.enabled");
    if reasoning.enabled.is_some() {
        report.record(
            &enabled_path,
            Some(&enabled_path),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Present,
        );
    } else {
        report.record(
            &enabled_path,
            Some(&enabled_path),
            TranslationDecisionKind::Defaulted,
            Some("reasoning.enabled defaults to true"),
            TranslationSafeRepresentation::Defaulted,
        );
    }
    if reasoning.effort.is_some() {
        let path = format!("{REASONING_PATH}.effort");
        report.record(
            &path,
            Some(&path),
            TranslationDecisionKind::Normalized,
            Some("reasoning effort is normalized before runtime use"),
            TranslationSafeRepresentation::Present,
        );
    }
    if reasoning.budget_tokens().is_some() {
        let path = format!("{REASONING_PATH}.budget_tokens");
        report.record(
            &path,
            Some(&path),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Present,
        );
    }
}

fn object_from_native_object(object: &NativeObject) -> Map<String, Value> {
    object.as_value().as_object().cloned().unwrap_or_default()
}
