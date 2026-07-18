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
}

/// The operation selected at the Native request boundary. Route resolution
/// owns the subsequent dispatch; this type only records the explicit intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "intent", rename_all = "snake_case")]
pub enum NativeExecutionOperation {
    Generate,
    Compact(CompactionIntent),
}

impl Default for NativeExecutionOperation {
    fn default() -> Self {
        Self::Generate
    }
}

impl NativeExecutionOperation {
    pub fn compaction_intent(&self) -> Option<&CompactionIntent> {
        match self {
            Self::Generate => None,
            Self::Compact(intent) => Some(intent),
        }
    }

    pub fn result_requirement(&self) -> CompactionResultRequirement {
        self.compaction_intent()
            .map(CompactionIntent::result_requirement)
            .unwrap_or(CompactionResultRequirement::Generate)
    }

    pub fn is_generate(&self) -> bool {
        matches!(self, Self::Generate)
    }
}
