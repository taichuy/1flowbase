use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiNativeGenerateProfile {
    Standard,
    LocalSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiNativeCompactProfile {
    ResponsesCompact,
    ResponsesCompactionV2,
}

/// The canonical operation selected at the AI Native request boundary.
///
/// Its serialized form is deliberately limited to the workflow-safe
/// `{kind, profile}` view. Transport bodies, credentials, routing affinity,
/// and other sealed execution data have no representation in this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiNativeOperation {
    Generate(AiNativeGenerateProfile),
    CountTokens,
    Compact(AiNativeCompactProfile),
}

impl Default for AiNativeOperation {
    fn default() -> Self {
        Self::Generate(AiNativeGenerateProfile::Standard)
    }
}

impl AiNativeOperation {
    pub fn generate(profile: AiNativeGenerateProfile) -> Self {
        Self::Generate(profile)
    }

    pub fn compact(profile: AiNativeCompactProfile) -> Self {
        Self::Compact(profile)
    }

    pub fn kind(self) -> &'static str {
        match self {
            Self::Generate(_) => "generate",
            Self::CountTokens => "count_tokens",
            Self::Compact(_) => "compact",
        }
    }

    pub fn profile(self) -> Option<&'static str> {
        match self {
            Self::Generate(AiNativeGenerateProfile::Standard) => Some("standard"),
            Self::Generate(AiNativeGenerateProfile::LocalSummary) => Some("local_summary"),
            Self::CountTokens => None,
            Self::Compact(AiNativeCompactProfile::ResponsesCompact) => Some("responses_compact"),
            Self::Compact(AiNativeCompactProfile::ResponsesCompactionV2) => {
                Some("responses_compaction_v2")
            }
        }
    }
}

impl Serialize for AiNativeOperation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut envelope = serializer.serialize_struct("AiNativeOperation", 2)?;
        envelope.serialize_field("kind", self.kind())?;
        envelope.serialize_field("profile", &self.profile())?;
        envelope.end()
    }
}

impl<'de> Deserialize<'de> for AiNativeOperation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct OperationEnvelope {
            kind: String,
            profile: Value,
        }

        let envelope = OperationEnvelope::deserialize(deserializer)?;
        let profile = envelope.profile.as_str();
        match (envelope.kind.as_str(), profile) {
            ("generate", Some("standard")) => Ok(Self::Generate(AiNativeGenerateProfile::Standard)),
            ("generate", Some("local_summary")) => {
                Ok(Self::Generate(AiNativeGenerateProfile::LocalSummary))
            }
            ("count_tokens", None) if envelope.profile.is_null() => Ok(Self::CountTokens),
            ("compact", Some("responses_compact")) => {
                Ok(Self::Compact(AiNativeCompactProfile::ResponsesCompact))
            }
            ("compact", Some("responses_compaction_v2")) => {
                Ok(Self::Compact(AiNativeCompactProfile::ResponsesCompactionV2))
            }
            ("generate" | "count_tokens" | "compact", _) => Err(de::Error::custom(format!(
                "unknown AI Native operation profile for kind {}",
                envelope.kind
            ))),
            _ => Err(de::Error::custom(format!(
                "unknown AI Native operation kind {}",
                envelope.kind
            ))),
        }
    }
}
