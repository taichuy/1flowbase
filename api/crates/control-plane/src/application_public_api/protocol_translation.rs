use serde::{Deserialize, Serialize};

use super::native::NativeRunRequest;

/// A protocol adapter receipt. It records how wire fields were handled without
/// retaining request content, credentials, headers, raw bodies, or hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationReport {
    pub protocol: TranslationProtocol,
    pub decisions: Vec<TranslationDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationProtocol {
    Native,
    OpenAiChat,
    OpenAiResponses,
    AnthropicMessages,
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
                panic!("translation receipt source path decided more than once: {source_path}");
            }
            return;
        }
        self.decisions.push(decision);
    }

    pub fn has_decision(&self, source_path: &str, kind: TranslationDecisionKind) -> bool {
        self.decisions
            .iter()
            .any(|decision| decision.source_path == source_path && decision.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    #[should_panic(expected = "translation receipt source path decided more than once")]
    fn rejects_conflicting_duplicate_source_path_decisions() {
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
    }
}
