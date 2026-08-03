use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProviderInvocationResult {
    pub final_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ProviderToolCall>,
    #[serde(default)]
    pub mcp_calls: Vec<ProviderMcpCall>,
    #[serde(default)]
    pub usage: ProviderUsage,
    pub finish_reason: Option<ProviderFinishReason>,
    #[serde(default)]
    pub provider_metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRuntimeErrorKind {
    AuthFailed,
    EndpointUnreachable,
    ModelNotFound,
    ProviderAffinityMismatch,
    ProviderTransportUnavailable,
    SemanticCapabilityUnsupported,
    RateLimited,
    ProviderUpstreamError,
    ProviderInvalidResponse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRuntimeError {
    pub kind: ProviderRuntimeErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_details: Option<Value>,
}

impl ProviderRuntimeError {
    pub fn new(kind: ProviderRuntimeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            provider_summary: None,
            provider_details: None,
        }
    }

    pub fn with_provider_summary(mut self, provider_summary: impl Into<String>) -> Self {
        self.provider_summary = Some(provider_summary.into());
        self
    }

    pub fn with_provider_details(mut self, provider_details: Value) -> Self {
        self.provider_details = Some(provider_details);
        self
    }

    pub fn normalize<M>(code: &str, message: M, provider_summary: Option<&str>) -> Self
    where
        M: Into<String>,
    {
        let message = message.into();
        let haystack = format!("{code} {message}").to_ascii_lowercase();
        let kind = if haystack.contains("auth")
            || haystack.contains("api_key")
            || haystack.contains("unauthorized")
            || haystack.contains("forbidden")
            || haystack.contains("401")
        {
            ProviderRuntimeErrorKind::AuthFailed
        } else if haystack.contains("rate")
            || haystack.contains("quota")
            || haystack.contains("too_many")
            || haystack.contains("429")
        {
            ProviderRuntimeErrorKind::RateLimited
        } else if (haystack.contains("model") && haystack.contains("not found"))
            || haystack.contains("unknown_model")
            || haystack.contains("model_not_found")
        {
            ProviderRuntimeErrorKind::ModelNotFound
        } else if haystack.contains("timeout")
            || haystack.contains("connect")
            || haystack.contains("unreachable")
            || haystack.contains("refused")
            || haystack.contains("dns")
            || haystack.contains("503")
        {
            ProviderRuntimeErrorKind::EndpointUnreachable
        } else {
            ProviderRuntimeErrorKind::ProviderInvalidResponse
        };

        let mut error = Self::new(kind, message);
        if let Some(summary) = provider_summary {
            error.provider_summary = Some(summary.to_string());
        }
        error
    }
}

impl fmt::Display for ProviderRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // RuntimeContract errors intentionally preserve upstream provider details.
        // Do not collapse, redact, or localize this display path in host code.
        match &self.provider_summary {
            Some(summary) => write!(f, "{:?}: {} ({summary})", self.kind, self.message),
            None => write!(f, "{:?}: {}", self.kind, self.message),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderCountTokensError {
    Unsupported { capabilities: Vec<&'static str> },
    InvalidContract { message: String },
    Runtime { error: ProviderRuntimeError },
}

impl fmt::Display for ProviderCountTokensError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { capabilities } => write!(
                f,
                "provider does not declare required CountTokens capabilities: {}",
                capabilities.join(", ")
            ),
            Self::InvalidContract { message } => {
                write!(f, "provider CountTokens contract is invalid: {message}")
            }
            Self::Runtime { error } => write!(f, "provider CountTokens runtime error: {error}"),
        }
    }
}

impl std::error::Error for ProviderCountTokensError {}

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderCompactError {
    Unsupported {
        profile: ProviderCompactProfile,
        capabilities: Vec<&'static str>,
    },
    InvalidContract {
        message: String,
    },
    Runtime {
        error: ProviderRuntimeError,
    },
}

impl fmt::Display for ProviderCompactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported {
                profile,
                capabilities,
            } => write!(
                f,
                "provider does not declare required Compact capabilities for {}: {}",
                profile.as_str(),
                capabilities.join(", ")
            ),
            Self::InvalidContract { message } => {
                write!(f, "provider Compact contract is invalid: {message}")
            }
            Self::Runtime { error } => write!(f, "provider Compact runtime error: {error}"),
        }
    }
}

impl std::error::Error for ProviderCompactError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderStreamEvent {
    NativeEvent {
        protocol: String,
        event: Value,
    },
    TextDelta {
        delta: String,
    },
    ReasoningDelta {
        delta: String,
    },
    ReasoningSignatureDelta {
        signature: String,
    },
    ToolCallDelta {
        call_id: String,
        delta: Value,
    },
    ToolCallCommit {
        call: ProviderToolCall,
    },
    McpCallDelta {
        call_id: String,
        delta: Value,
    },
    McpCallCommit {
        call: ProviderMcpCall,
    },
    OutputItem {
        phase: ProviderOutputItemPhase,
        output_index: usize,
        #[serde(deserialize_with = "deserialize_provider_output_item")]
        item: Value,
    },
    UsageDelta {
        usage: ProviderUsage,
    },
    UsageSnapshot {
        usage: ProviderUsage,
    },
    Finish {
        reason: ProviderFinishReason,
    },
    Error {
        error: ProviderRuntimeError,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderRuntimeLine {
    NativeEvent {
        protocol: String,
        event: Value,
    },
    TextDelta {
        delta: String,
    },
    ReasoningDelta {
        delta: String,
    },
    ReasoningSignatureDelta {
        signature: String,
    },
    ToolCallDelta {
        call_id: String,
        delta: Value,
    },
    ToolCallCommit {
        call: ProviderToolCall,
    },
    McpCallDelta {
        call_id: String,
        delta: Value,
    },
    McpCallCommit {
        call: ProviderMcpCall,
    },
    OutputItem {
        phase: ProviderOutputItemPhase,
        output_index: usize,
        #[serde(deserialize_with = "deserialize_provider_output_item")]
        item: Value,
    },
    UsageDelta {
        usage: ProviderUsage,
    },
    UsageSnapshot {
        usage: ProviderUsage,
    },
    Finish {
        reason: ProviderFinishReason,
    },
    Error {
        error: ProviderRuntimeError,
    },
    Result {
        result: ProviderInvocationResult,
    },
}

impl ProviderRuntimeLine {
    pub fn into_stream_event(self) -> Option<ProviderStreamEvent> {
        match self {
            Self::NativeEvent { protocol, event } => {
                Some(ProviderStreamEvent::NativeEvent { protocol, event })
            }
            Self::TextDelta { delta } => Some(ProviderStreamEvent::TextDelta { delta }),
            Self::ReasoningDelta { delta } => Some(ProviderStreamEvent::ReasoningDelta { delta }),
            Self::ReasoningSignatureDelta { signature } => {
                Some(ProviderStreamEvent::ReasoningSignatureDelta { signature })
            }
            Self::ToolCallDelta { call_id, delta } => {
                Some(ProviderStreamEvent::ToolCallDelta { call_id, delta })
            }
            Self::ToolCallCommit { call } => Some(ProviderStreamEvent::ToolCallCommit { call }),
            Self::McpCallDelta { call_id, delta } => {
                Some(ProviderStreamEvent::McpCallDelta { call_id, delta })
            }
            Self::McpCallCommit { call } => Some(ProviderStreamEvent::McpCallCommit { call }),
            Self::OutputItem {
                phase,
                output_index,
                item,
            } => Some(ProviderStreamEvent::OutputItem {
                phase,
                output_index,
                item,
            }),
            Self::UsageDelta { usage } => Some(ProviderStreamEvent::UsageDelta { usage }),
            Self::UsageSnapshot { usage } => Some(ProviderStreamEvent::UsageSnapshot { usage }),
            Self::Finish { reason } => Some(ProviderStreamEvent::Finish { reason }),
            Self::Error { error } => Some(ProviderStreamEvent::Error { error }),
            Self::Result { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOutputItemPhase {
    Added,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderOutputItemValidationError {
    #[error("provider output item must be an object")]
    NotObject,
    #[error("provider output item id must be a non-empty string")]
    InvalidId,
    #[error("provider output item type is not supported by the typed Responses projection")]
    InvalidType,
}

pub fn validate_provider_output_item(
    item: &Value,
) -> Result<(), ProviderOutputItemValidationError> {
    let object = item
        .as_object()
        .ok_or(ProviderOutputItemValidationError::NotObject)?;
    if !matches!(
        object.get("id").and_then(Value::as_str),
        Some(id) if !id.trim().is_empty()
    ) {
        return Err(ProviderOutputItemValidationError::InvalidId);
    }
    if !matches!(
        object.get("type").and_then(Value::as_str),
        Some(
            "tool_search_call"
                | "tool_search_output"
                | "additional_tools"
                | "file_search_call"
                | "program"
                | "shell_call"
                | "mcp_list_tools"
                | "mcp_call"
                | "mcp_approval_request"
        )
    ) {
        return Err(ProviderOutputItemValidationError::InvalidType);
    }
    Ok(())
}

fn deserialize_provider_output_item<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let item = Value::deserialize(deserializer)?;
    validate_provider_output_item(&item).map_err(serde::de::Error::custom)?;
    Ok(item)
}
