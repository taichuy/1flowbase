use domain::AiNativeOperation;
use serde_json::{Map, Value};

use crate::application_public_api::{
    native::{CompactionIntent, CompactionProfile},
    protocol_translation::{
        TranslationDecisionKind, TranslationReport, TranslationSafeRepresentation,
    },
};

use super::OpenAiCompatError;

/// The server-known endpoint that carried an OpenAI Responses request. This is
/// deliberately not inferred from request text, headers such as User-Agent,
/// token counts, or a possible response shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiResponsesEndpoint {
    Responses,
    ResponsesCompact,
}

/// Request context supplied by the authenticated HTTP ingress. Codex turn
/// metadata is accepted here only after the route has authenticated the
/// application API request; regular OpenAI `metadata` is never compaction
/// evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenAiResponsesRequestContext {
    endpoint: OpenAiResponsesEndpoint,
    captured_codex_turn_metadata: Option<Value>,
}

impl OpenAiResponsesRequestContext {
    pub fn responses() -> Self {
        Self::new(OpenAiResponsesEndpoint::Responses)
    }

    pub fn responses_compact() -> Self {
        Self::new(OpenAiResponsesEndpoint::ResponsesCompact)
    }

    pub fn new(endpoint: OpenAiResponsesEndpoint) -> Self {
        Self {
            endpoint,
            captured_codex_turn_metadata: None,
        }
    }

    /// Attach the already captured `x-codex-turn-metadata` JSON value. The
    /// caller is responsible for invoking this only after request
    /// authentication and for never treating ordinary body metadata as this
    /// trusted ingress value.
    pub fn with_captured_codex_turn_metadata(mut self, metadata: Value) -> Self {
        self.captured_codex_turn_metadata = Some(metadata);
        self
    }
}

/// A closed classifier failure. Unknown Codex implementations are deliberately
/// unsupported instead of being guessed as a local summary path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompactionIntentClassificationError {
    MalformedCodexMetadata,
    UnsupportedProfile,
    ContradictoryEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturedCodexCompactionProfile {
    LocalSummary,
    ResponsesCompact,
    ResponsesCompactionV2,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturedCodexRequestKind {
    NonCompaction,
    Compaction(CapturedCodexCompactionProfile),
}

pub(super) fn classify_response_operation(
    object: &Map<String, Value>,
    context: &OpenAiResponsesRequestContext,
    report: &mut TranslationReport,
) -> Result<AiNativeOperation, OpenAiCompatError> {
    let request_kind = context
        .captured_codex_turn_metadata
        .as_ref()
        .map(parse_captured_codex_request_kind)
        .transpose()
        .map_err(|error| classification_error(error, report))?;
    let has_compaction_trigger = input_has_compaction_trigger(object.get("input"));

    let profile = match (context.endpoint, request_kind, has_compaction_trigger) {
        (
            _,
            Some(CapturedCodexRequestKind::Compaction(CapturedCodexCompactionProfile::Unsupported)),
            _,
        ) => {
            return Err(classification_error(
                CompactionIntentClassificationError::UnsupportedProfile,
                report,
            ));
        }
        (
            OpenAiResponsesEndpoint::ResponsesCompact,
            Some(CapturedCodexRequestKind::Compaction(
                CapturedCodexCompactionProfile::LocalSummary
                | CapturedCodexCompactionProfile::ResponsesCompactionV2,
            )),
            _,
        )
        | (OpenAiResponsesEndpoint::ResponsesCompact, _, true) => {
            return Err(classification_error(
                CompactionIntentClassificationError::ContradictoryEvidence,
                report,
            ));
        }
        (OpenAiResponsesEndpoint::ResponsesCompact, _, false) => {
            Some(CompactionProfile::ResponsesCompact)
        }
        (
            OpenAiResponsesEndpoint::Responses,
            Some(CapturedCodexRequestKind::Compaction(
                CapturedCodexCompactionProfile::ResponsesCompact,
            )),
            _,
        )
        | (
            OpenAiResponsesEndpoint::Responses,
            Some(CapturedCodexRequestKind::Compaction(
                CapturedCodexCompactionProfile::LocalSummary,
            )),
            true,
        )
        | (
            OpenAiResponsesEndpoint::Responses,
            Some(CapturedCodexRequestKind::Compaction(
                CapturedCodexCompactionProfile::ResponsesCompactionV2,
            )),
            false,
        )
        | (
            OpenAiResponsesEndpoint::Responses,
            Some(CapturedCodexRequestKind::NonCompaction),
            true,
        ) => {
            return Err(classification_error(
                CompactionIntentClassificationError::ContradictoryEvidence,
                report,
            ));
        }
        (
            OpenAiResponsesEndpoint::Responses,
            Some(CapturedCodexRequestKind::Compaction(
                CapturedCodexCompactionProfile::LocalSummary,
            )),
            false,
        ) => Some(CompactionProfile::LocalSummary),
        (OpenAiResponsesEndpoint::Responses, None, true)
        | (
            OpenAiResponsesEndpoint::Responses,
            Some(CapturedCodexRequestKind::Compaction(
                CapturedCodexCompactionProfile::ResponsesCompactionV2,
            )),
            true,
        ) => Some(CompactionProfile::ResponsesCompactionV2),
        (OpenAiResponsesEndpoint::Responses, _, false) => None,
    };

    let Some(intent) = profile.map(CompactionIntent::new) else {
        return Ok(AiNativeOperation::default());
    };
    let operation = intent.execution_operation();
    record_compaction_evidence(context, has_compaction_trigger, report);
    Ok(operation)
}

fn parse_captured_codex_request_kind(
    metadata: &Value,
) -> Result<CapturedCodexRequestKind, CompactionIntentClassificationError> {
    let object = metadata
        .as_object()
        .ok_or(CompactionIntentClassificationError::MalformedCodexMetadata)?;
    let request_kind = object
        .get("request_kind")
        .and_then(Value::as_str)
        .ok_or(CompactionIntentClassificationError::MalformedCodexMetadata)?;
    if request_kind != "compaction" {
        return Ok(CapturedCodexRequestKind::NonCompaction);
    }
    let compaction = object
        .get("compaction")
        .and_then(Value::as_object)
        .ok_or(CompactionIntentClassificationError::MalformedCodexMetadata)?;
    let implementation = compaction
        .get("implementation")
        .and_then(Value::as_str)
        .ok_or(CompactionIntentClassificationError::MalformedCodexMetadata)?;
    let profile = match implementation {
        "responses" => CapturedCodexCompactionProfile::LocalSummary,
        "responses_compact" => CapturedCodexCompactionProfile::ResponsesCompact,
        "responses_compaction_v2" => CapturedCodexCompactionProfile::ResponsesCompactionV2,
        _ => CapturedCodexCompactionProfile::Unsupported,
    };
    Ok(CapturedCodexRequestKind::Compaction(profile))
}

fn input_has_compaction_trigger(input: Option<&Value>) -> bool {
    input
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(is_compaction_trigger_item))
}

pub(super) fn is_compaction_trigger_item(item: &Value) -> bool {
    item.as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
        == Some("compaction_trigger")
}

fn record_compaction_evidence(
    context: &OpenAiResponsesRequestContext,
    has_compaction_trigger: bool,
    report: &mut TranslationReport,
) {
    report.record(
        "$.ingress.endpoint",
        Some("$.execution.operation"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    if context.captured_codex_turn_metadata.is_some() {
        report.record(
            "$.ingress.x-codex-turn-metadata",
            Some("$.execution.operation"),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Redacted,
        );
    }
    if has_compaction_trigger {
        report.record(
            "$.input.<compaction_trigger>",
            Some("$.execution.operation"),
            TranslationDecisionKind::Exact,
            None,
            TranslationSafeRepresentation::Present,
        );
    }
}

fn classification_error(
    error: CompactionIntentClassificationError,
    report: &mut TranslationReport,
) -> OpenAiCompatError {
    match error {
        CompactionIntentClassificationError::MalformedCodexMetadata => {
            report.record(
                "$.ingress.x-codex-turn-metadata",
                None,
                TranslationDecisionKind::Rejected,
                Some("captured Codex turn metadata has no valid compaction shape"),
                TranslationSafeRepresentation::Present,
            );
            OpenAiCompatError::invalid(
                "x-codex-turn-metadata",
                "captured Codex turn metadata is malformed",
            )
            .with_report(report.clone())
        }
        CompactionIntentClassificationError::UnsupportedProfile => {
            report.record(
                "$.ingress.x-codex-turn-metadata.compaction.implementation",
                None,
                TranslationDecisionKind::Unsupported,
                Some("Codex compaction implementation is not supported"),
                TranslationSafeRepresentation::Present,
            );
            OpenAiCompatError::unsupported_compaction_profile().with_report(report.clone())
        }
        CompactionIntentClassificationError::ContradictoryEvidence => {
            report.record(
                "$.ingress.compaction_evidence",
                None,
                TranslationDecisionKind::Rejected,
                Some("endpoint, compaction trigger, and Codex metadata disagree"),
                TranslationSafeRepresentation::Present,
            );
            OpenAiCompatError::invalid(
                "compaction",
                "endpoint, compaction trigger, and Codex metadata disagree",
            )
            .with_report(report.clone())
        }
    }
}
