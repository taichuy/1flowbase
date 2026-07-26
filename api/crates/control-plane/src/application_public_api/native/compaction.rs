use domain::{AiNativeCompactProfile, AiNativeGenerateProfile, AiNativeOperation};
use serde::{Deserialize, Serialize};

/// The closed set of compaction profiles understood by the published
/// application API. A profile selects both the execution route and the
/// response contract; callers cannot combine those concerns arbitrarily.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionProfile {
    LocalSummary,
    ResponsesCompact,
    ResponsesCompactionV2,
}

impl CompactionProfile {
    pub fn result_requirement(self) -> CompactionResultRequirement {
        match self {
            Self::LocalSummary => CompactionResultRequirement::Generate,
            Self::ResponsesCompact => CompactionResultRequirement::ResponseItems,
            Self::ResponsesCompactionV2 => {
                CompactionResultRequirement::CompletedOpaqueCompactionItem
            }
        }
    }
}

/// The result shape that the later route and renderer must enforce for a
/// compaction operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionResultRequirement {
    /// Local summary compaction is rendered through the ordinary Generate
    /// result path.
    Generate,
    /// Legacy `/responses/compact` returns the provider's `ResponseItem[]`.
    ResponseItems,
    /// V2 accepts exactly one provider-produced `compaction` item and a
    /// completed response. Its `encrypted_content` is opaque and must cross
    /// the renderer unchanged; it cannot be synthesized from a Generate,
    /// Code, or static JSON result.
    CompletedOpaqueCompactionItem,
}

/// Canonical intent carried by a Native execution operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionIntent {
    profile: CompactionProfile,
}

impl CompactionIntent {
    pub fn new(profile: CompactionProfile) -> Self {
        Self { profile }
    }

    pub fn profile(&self) -> CompactionProfile {
        self.profile
    }

    pub fn result_requirement(&self) -> CompactionResultRequirement {
        self.profile.result_requirement()
    }

    /// D-006 keeps local summary on the existing Generate route. Only the two
    /// remote profiles select a Compact operation and therefore consume a
    /// compact operation binding.
    pub fn execution_operation(&self) -> AiNativeOperation {
        match self.profile {
            CompactionProfile::LocalSummary => {
                AiNativeOperation::Generate(AiNativeGenerateProfile::LocalSummary)
            }
            CompactionProfile::ResponsesCompact => {
                AiNativeOperation::Compact(AiNativeCompactProfile::ResponsesCompact)
            }
            CompactionProfile::ResponsesCompactionV2 => {
                AiNativeOperation::Compact(AiNativeCompactProfile::ResponsesCompactionV2)
            }
        }
    }
}

pub fn compaction_intent(operation: AiNativeOperation) -> Option<CompactionIntent> {
    match operation {
        AiNativeOperation::Generate(AiNativeGenerateProfile::Standard)
        | AiNativeOperation::CountTokens => None,
        AiNativeOperation::Generate(AiNativeGenerateProfile::LocalSummary) => {
            Some(CompactionIntent::new(CompactionProfile::LocalSummary))
        }
        AiNativeOperation::Compact(AiNativeCompactProfile::ResponsesCompact) => {
            Some(CompactionIntent::new(CompactionProfile::ResponsesCompact))
        }
        AiNativeOperation::Compact(AiNativeCompactProfile::ResponsesCompactionV2) => Some(
            CompactionIntent::new(CompactionProfile::ResponsesCompactionV2),
        ),
    }
}

pub fn operation_result_requirement(operation: AiNativeOperation) -> CompactionResultRequirement {
    compaction_intent(operation)
        .as_ref()
        .map(CompactionIntent::result_requirement)
        .unwrap_or(CompactionResultRequirement::Generate)
}
