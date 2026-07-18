use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::application_public_api::run_service::GenerateExecutionProfile;

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
    pub fn execution_operation(&self) -> NativeExecutionOperation {
        match self.profile {
            CompactionProfile::LocalSummary => {
                NativeExecutionOperation::Generate(GenerateExecutionProfile::LocalSummary)
            }
            CompactionProfile::ResponsesCompact => {
                NativeExecutionOperation::Compact(RemoteCompactionProfile::ResponsesCompact)
            }
            CompactionProfile::ResponsesCompactionV2 => {
                NativeExecutionOperation::Compact(RemoteCompactionProfile::ResponsesCompactionV2)
            }
        }
    }
}

/// The two profiles that may consume a published compact operation binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteCompactionProfile {
    ResponsesCompact,
    ResponsesCompactionV2,
}

impl RemoteCompactionProfile {
    pub fn compaction_profile(self) -> CompactionProfile {
        match self {
            Self::ResponsesCompact => CompactionProfile::ResponsesCompact,
            Self::ResponsesCompactionV2 => CompactionProfile::ResponsesCompactionV2,
        }
    }
}

/// The operation selected at the Native request boundary. Route resolution
/// consumes this value directly: local compaction remains Generate with its
/// existing LocalSummary profile, while remote compaction is Compact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeExecutionOperation {
    Generate(GenerateExecutionProfile),
    Compact(RemoteCompactionProfile),
}

impl Default for NativeExecutionOperation {
    fn default() -> Self {
        Self::Generate(GenerateExecutionProfile::Standard)
    }
}

impl NativeExecutionOperation {
    pub fn generate_profile(&self) -> Option<GenerateExecutionProfile> {
        match self {
            Self::Generate(profile) => Some(*profile),
            Self::Compact(_) => None,
        }
    }

    pub fn compaction_intent(&self) -> Option<CompactionIntent> {
        match self {
            Self::Generate(GenerateExecutionProfile::Standard) => None,
            Self::Generate(GenerateExecutionProfile::LocalSummary) => {
                Some(CompactionIntent::new(CompactionProfile::LocalSummary))
            }
            Self::Compact(profile) => Some(CompactionIntent::new(profile.compaction_profile())),
        }
    }

    pub fn result_requirement(&self) -> CompactionResultRequirement {
        self.compaction_intent()
            .as_ref()
            .map(CompactionIntent::result_requirement)
            .unwrap_or(CompactionResultRequirement::Generate)
    }

    pub(crate) fn from_value(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("kind")?.as_str()?;
        let profile = object.get("profile")?.as_str()?;
        match (kind, profile) {
            ("generate", "standard") => Some(Self::Generate(GenerateExecutionProfile::Standard)),
            ("generate", "local_summary") => {
                Some(Self::Generate(GenerateExecutionProfile::LocalSummary))
            }
            ("compact", "responses_compact") => {
                Some(Self::Compact(RemoteCompactionProfile::ResponsesCompact))
            }
            ("compact", "responses_compaction_v2") => Some(Self::Compact(
                RemoteCompactionProfile::ResponsesCompactionV2,
            )),
            _ => None,
        }
    }

    pub(crate) fn as_value(&self) -> Value {
        let (kind, profile) = match self {
            Self::Generate(GenerateExecutionProfile::Standard) => ("generate", "standard"),
            Self::Generate(GenerateExecutionProfile::LocalSummary) => ("generate", "local_summary"),
            Self::Compact(RemoteCompactionProfile::ResponsesCompact) => {
                ("compact", "responses_compact")
            }
            Self::Compact(RemoteCompactionProfile::ResponsesCompactionV2) => {
                ("compact", "responses_compaction_v2")
            }
        };
        json!({ "kind": kind, "profile": profile })
    }
}
