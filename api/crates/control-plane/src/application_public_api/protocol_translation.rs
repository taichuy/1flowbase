use serde::{Deserialize, Serialize};

use super::native::NativeRunRequest;

/// A protocol adapter receipt. It records how wire fields were handled without
/// retaining request content, credentials, headers, raw bodies, or hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationReport {
    pub protocol: TranslationProtocol,
    pub decisions: Vec<TranslationDecision>,
    #[serde(skip)]
    invariant_error: Option<TranslationReportInvariantError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationProtocol {
    Native,
    OpenAiChat,
    OpenAiResponses,
    AnthropicMessages,
}

impl TranslationProtocol {
    pub const fn compatibility_mode(self) -> &'static str {
        match self {
            Self::Native => "native-v1",
            Self::AnthropicMessages => "anthropic-messages-v1",
            Self::OpenAiChat => "openai-chat-completions-v1",
            Self::OpenAiResponses => "openai-responses-v1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationDecision {
    pub source_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
    pub kind: TranslationDecisionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub effective_value: TranslationSafeRepresentation,
}

/// A translation adapter attempted to make two incompatible decisions for the
/// same wire location. It is an internal invariant error: callers must return
/// an adapter failure rather than expose a receipt whose first decision won by
/// accident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationReportInvariantError {
    source_path: String,
    existing: TranslationDecision,
    attempted: TranslationDecision,
}

impl TranslationReportInvariantError {
    pub fn source_path(&self) -> &str {
        &self.source_path
    }
}

impl std::fmt::Display for TranslationReportInvariantError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("translation receipt source path received conflicting decisions")
    }
}

impl std::error::Error for TranslationReportInvariantError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationDecisionKind {
    Exact,
    Normalized,
    Defaulted,
    Rejected,
    Unsupported,
    Dropped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationSafeRepresentation {
    Present,
    Absent,
    Defaulted,
    Redacted,
}

/// The adapter owns this pair until the command crosses into the Native
/// service. The receipt must not be persisted with the command or its result.
#[derive(Debug, Clone, PartialEq)]
pub struct TranslatedNativeRunRequest {
    pub request: NativeRunRequest,
    pub report: TranslationReport,
}

impl TranslationReport {
    pub fn new(protocol: TranslationProtocol) -> Self {
        Self {
            protocol,
            decisions: Vec::new(),
            invariant_error: None,
        }
    }

    pub fn record(
        &mut self,
        source_path: &str,
        target_path: Option<&str>,
        kind: TranslationDecisionKind,
        reason: Option<&str>,
        effective_value: TranslationSafeRepresentation,
    ) {
        if self.invariant_error.is_some() {
            return;
        }
        let decision = TranslationDecision {
            source_path: source_path.to_string(),
            target_path: target_path.map(ToOwned::to_owned),
            kind,
            reason: reason.map(ToOwned::to_owned),
            effective_value,
        };
        if let Some(existing) = self
            .decisions
            .iter_mut()
            .find(|existing| existing.source_path == source_path)
        {
            if *existing != decision {
                self.invariant_error = Some(TranslationReportInvariantError {
                    source_path: source_path.to_string(),
                    existing: existing.clone(),
                    attempted: decision,
                });
            }
            return;
        }
        self.decisions.push(decision);
    }

    /// Finalize a report at the external-protocol boundary. Record calls are
    /// deliberately ergonomic inside translators; this turns any duplicate
    /// conflict into a typed, propagatable adapter invariant before the report
    /// can cross that boundary.
    pub fn ensure_consistent(&self) -> Result<(), TranslationReportInvariantError> {
        self.invariant_error.clone().map_or(Ok(()), Err)
    }

    /// Preserve the fact that each unexpected wire key was present while never
    /// retaining the key itself in a non-durable receipt. Sorting makes the
    /// anonymous ordinal stable regardless of JSON insertion order.
    pub(crate) fn record_anonymous_unknown_fields<'a>(
        &mut self,
        parent_path: &str,
        fields: impl IntoIterator<Item = &'a String>,
        kind: TranslationDecisionKind,
        reason: &'static str,
        effective_value: TranslationSafeRepresentation,
    ) -> usize {
        let source_paths = anonymous_unknown_source_paths(parent_path, fields);
        for source_path in &source_paths {
            self.record(source_path, None, kind, Some(reason), effective_value);
        }
        source_paths.len()
    }

    pub fn has_decision(&self, source_path: &str, kind: TranslationDecisionKind) -> bool {
        self.decisions
            .iter()
            .any(|decision| decision.source_path == source_path && decision.kind == kind)
    }
}

pub(crate) fn anonymous_unknown_source_paths<'a>(
    parent_path: &str,
    fields: impl IntoIterator<Item = &'a String>,
) -> Vec<String> {
    let mut fields = fields.into_iter().map(String::as_str).collect::<Vec<_>>();
    fields.sort_unstable();
    fields
        .iter()
        .enumerate()
        .map(|(index, _)| format!("{parent_path}.<unknown>[{index}]"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_protocol_has_one_canonical_versioned_compatibility_mode() {
        assert_eq!(
            TranslationProtocol::Native.compatibility_mode(),
            "native-v1"
        );
        assert_eq!(
            TranslationProtocol::AnthropicMessages.compatibility_mode(),
            "anthropic-messages-v1"
        );
        assert_eq!(
            TranslationProtocol::OpenAiChat.compatibility_mode(),
            "openai-chat-completions-v1"
        );
        assert_eq!(
            TranslationProtocol::OpenAiResponses.compatibility_mode(),
            "openai-responses-v1"
        );
    }

    #[test]
    fn accepts_an_idempotent_repeat_for_the_same_source_path() {
        let mut report = TranslationReport::new(TranslationProtocol::AnthropicMessages);
        report.record(
            "$.messages[0].content",
            Some("$.query"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        report.record(
            "$.messages[0].content",
            Some("$.query"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );

        assert_eq!(report.decisions.len(), 1);
        assert_eq!(
            report.decisions[0].kind,
            TranslationDecisionKind::Normalized
        );
    }

    #[test]
    fn surfaces_conflicting_duplicate_source_path_decisions_as_a_translation_invariant() {
        let mut report = TranslationReport::new(TranslationProtocol::Native);
        report.record(
            "$.execution.compatibility_mode",
            None,
            TranslationDecisionKind::Rejected,
            Some("legacy mode is not public Native input"),
            TranslationSafeRepresentation::Redacted,
        );
        report.record(
            "$.execution.compatibility_mode",
            Some("$.execution"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );

        let error = report
            .ensure_consistent()
            .expect_err("a conflicting source decision must become a recoverable invariant error");
        assert_eq!(error.source_path(), "$.execution.compatibility_mode");
        assert_eq!(report.decisions.len(), 1);
    }
}
