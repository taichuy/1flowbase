use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ExtensionContractError;

mod count_tokens;
pub use count_tokens::*;

mod generate_projection;
use generate_projection::project_generate_reasoning_history;
pub use generate_projection::*;

mod external_context;
pub use external_context::*;

mod media;
pub use media::*;

pub const CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY: &str = "__client_protocol_envelope";
pub const NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY: &str = "__native_model_prompt_context";
pub const NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY: &str = "__native_model_request_context";
/// Host-owned protocol-independent model tools, separate from mapped business inputs.
pub const NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY: &str = "__native_model_tool_context";
pub const CURRENT_PROVIDER_CONTRACT: &str = "1flowbase.provider/v2";
/// Host-owned outer stdio request field. Never copied into typed input or supplier bodies.
pub const PROVIDER_HOST_CAPABILITIES_FIELD: &str = "host_capabilities";
/// Opt-in to the additive ProtocolObservation event family; absent means legacy events only.
pub const PROVIDER_PROTOCOL_OBSERVATION_CAPABILITY: &str = "protocol_observation_v1";
pub const PROVIDER_CONFIGURATION_VALIDATION_CAPABILITY: &str = "config.validate";
pub const PROVIDER_MODEL_LISTING_CAPABILITY: &str = "models.list";
pub const PROVIDER_COUNT_TOKENS_CAPABILITY: &str = "count_tokens";
pub const PROVIDER_USAGE_WINDOWS_CAPABILITY: &str = "usage.rate_limit_windows";
pub const PROVIDER_RESET_CREDITS_CAPABILITY: &str = "reset_credits";
pub const PROVIDER_COMPACT_RESPONSES_COMPACT_CAPABILITY: &str = "compact.responses_compact";
pub const PROVIDER_COMPACT_RESPONSES_COMPACTION_V2_CAPABILITY: &str =
    "compact.responses_compaction_v2";
pub const PROVIDER_RESPONSES_NATIVE_PASSTHROUGH_CAPABILITY: &str = "responses.native_passthrough";
pub const PROVIDER_REASONING_OUTPUT_SUPPORTED_CAPABILITY: &str = "reasoning_output_supported";
pub const PROVIDER_REASONING_HISTORY_INPUT_SUPPORTED_CAPABILITY: &str =
    "reasoning_history_input_supported";
pub const PROVIDER_NATIVE_CONTINUATION_SUPPORTED_CAPABILITY: &str = "native_continuation_supported";
pub const PROVIDER_MESSAGE_BLOCKS_REASONING_HISTORY_V1_CAPABILITY: &str =
    "message_blocks.reasoning_history.v1";
pub const PROVIDER_MESSAGE_BLOCKS_REDACTED_REASONING_HISTORY_V1_CAPABILITY: &str =
    "message_blocks.redacted_reasoning_history.v1";
pub const PROVIDER_PROTOCOL_CONTEXT_CONSUME_ANTHROPIC_MESSAGES_V1_CAPABILITY: &str =
    "protocol_context.consume.anthropic_messages.v1";
pub const PROVIDER_PROTOCOL_CONTEXT_CONSUME_OPENAI_CHAT_V1_CAPABILITY: &str =
    "protocol_context.consume.openai_chat.v1";
pub const PROVIDER_PROTOCOL_CONTEXT_CONSUME_OPENAI_RESPONSES_V1_CAPABILITY: &str =
    "protocol_context.consume.openai_responses.v1";
pub const PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V1_CAPABILITY: &str =
    "protocol_context.restore.anthropic_messages.v1";
pub const PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V2_CAPABILITY: &str =
    "protocol_context.restore.anthropic_messages.v2";
pub const PROVIDER_PROTOCOL_CONTEXT_RESTORE_OPENAI_CHAT_V1_CAPABILITY: &str =
    "protocol_context.restore.openai_chat.v1";
pub const PROVIDER_PROTOCOL_CONTEXT_RESTORE_OPENAI_RESPONSES_V1_CAPABILITY: &str =
    "protocol_context.restore.openai_responses.v1";
pub const PROVIDER_NETWORK_EGRESS_HANDOFF_V1_CAPABILITY: &str = "network_egress_handoff/v1";
pub const PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY: &str =
    "1flowbase_generate_translation";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelDiscoveryMode {
    Static,
    Dynamic,
    Hybrid,
}

impl TryFrom<&str> for ModelDiscoveryMode {
    type Error = ExtensionContractError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "static" => Ok(Self::Static),
            "dynamic" => Ok(Self::Dynamic),
            "hybrid" => Ok(Self::Hybrid),
            other => Err(ExtensionContractError::invalid_provider_contract(format!(
                "unsupported model discovery mode: {other}"
            ))),
        }
    }
}

impl TryFrom<String> for ModelDiscoveryMode {
    type Error = ExtensionContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderModelSource {
    Static,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStdioMethod {
    Validate,
    ListModels,
    Invoke,
    Balance,
    Auth,
    Usage,
    ResetCredit,
    SelectDistribution,
    TransportSession,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderStdioRequest {
    pub method: ProviderStdioMethod,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderStdioError {
    pub kind: ProviderRuntimeErrorKind,
    pub message: String,
    #[serde(default)]
    pub provider_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_details: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderStdioResponse {
    pub ok: bool,
    #[serde(default)]
    pub result: Value,
    #[serde(default)]
    pub error: Option<ProviderStdioError>,
}

/// Provider services that are opt-in so the host can render and invoke only
/// the operations a package has explicitly committed to support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOperationalCapability {
    ConfigurationValidation,
    ModelListing,
    UsageWindows,
    ResetCredits,
}

impl ProviderOperationalCapability {
    pub const fn manifest_capability(self) -> &'static str {
        match self {
            Self::ConfigurationValidation => PROVIDER_CONFIGURATION_VALIDATION_CAPABILITY,
            Self::ModelListing => PROVIDER_MODEL_LISTING_CAPABILITY,
            Self::UsageWindows => PROVIDER_USAGE_WINDOWS_CAPABILITY,
            Self::ResetCredits => PROVIDER_RESET_CREDITS_CAPABILITY,
        }
    }

    pub fn declared_by(self, runtime_capabilities: &[String]) -> bool {
        runtime_capabilities
            .iter()
            .any(|capability| capability == self.manifest_capability())
    }
}

/// A provider-owned authentication action. The host owns only dispatch and
/// encrypted persistence; provider-specific OAuth and token semantics stay in
/// the runtime extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderAuthOperation {
    Begin { action: String },
    Poll,
    Submit { value: String },
    Cancel,
    Maintain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderAuthRuntimeInput {
    #[serde(default)]
    pub provider_config: Value,
    pub operation: ProviderAuthOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthStatus {
    Pending,
    Authorized,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthUserActionKind {
    DeviceCode,
    PasteCallbackUrl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderAuthUserAction {
    pub kind: ProviderAuthUserActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_interval_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderAuthResult {
    pub status: ProviderAuthStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_action: Option<ProviderAuthUserAction>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub managed_secret_patch: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderAuthActionManifest {
    pub code: String,
    pub label: String,
    #[serde(default)]
    pub user_action_kinds: Vec<ProviderAuthUserActionKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderAuthManifest {
    #[serde(default)]
    pub actions: Vec<ProviderAuthActionManifest>,
    #[serde(default)]
    pub managed_secret_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderBalanceInfo {
    pub currency: String,
    pub total_balance: String,
    #[serde(default)]
    pub granted_balance: Option<String>,
    #[serde(default)]
    pub topped_up_balance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderBalanceResult {
    pub is_available: bool,
    #[serde(default)]
    pub balance_infos: Vec<ProviderBalanceInfo>,
    #[serde(default = "empty_provider_metadata")]
    pub provider_metadata: Value,
}

/// A normalized provider account usage window. The provider owns upstream
/// labels and payloads; the host only understands duration and percentage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderUsageWindow {
    pub limit_window_seconds: u64,
    pub used_percent: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderUsageWindowsResult {
    pub windows: Vec<ProviderUsageWindow>,
    pub queried_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderResetCreditOperation {
    Count,
    Consume { idempotency_key: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResetCreditRuntimeInput {
    #[serde(default)]
    pub provider_config: Value,
    pub operation: ProviderResetCreditOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderResetCreditResult {
    Count { available_count: u32 },
    Consumed,
}

fn empty_provider_metadata() -> Value {
    serde_json::json!({})
}

fn is_empty_provider_metadata(value: &Value) -> bool {
    value.as_object().is_some_and(|object| object.is_empty())
}

impl Default for ProviderBalanceResult {
    fn default() -> Self {
        Self {
            is_available: false,
            balance_infos: Vec::new(),
            provider_metadata: empty_provider_metadata(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginFormOption {
    pub label: String,
    pub value: Value,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginFormCondition {
    pub field: String,
    pub operator: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub values: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginFormFieldSchema {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub control: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub order: Option<i32>,
    #[serde(default)]
    pub advanced: Option<bool>,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub send_mode: Option<String>,
    #[serde(default)]
    pub enabled_by_default: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub default_value: Option<Value>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub step: Option<f64>,
    #[serde(default)]
    pub precision: Option<u32>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub options: Vec<PluginFormOption>,
    #[serde(default)]
    pub visible_when: Vec<PluginFormCondition>,
    #[serde(default)]
    pub disabled_when: Vec<PluginFormCondition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginFormSchema {
    pub schema_version: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub fields: Vec<PluginFormFieldSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProviderUsage {
    pub input_tokens: Option<u64>,
    pub input_cache_hit_tokens: Option<u64>,
    pub input_cache_miss_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    /// Provider-reported cache writes grouped by TTL in seconds; absent is not an inferred TTL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_by_ttl_seconds: Option<std::collections::BTreeMap<String, u64>>,
    pub total_tokens: Option<u64>,
}

impl ProviderUsage {
    pub fn total_tokens(&self) -> Option<u64> {
        if let Some(value) = self.total_tokens {
            return Some(value);
        }

        let mut total = 0_u64;
        let mut has_value = false;
        for segment in [self.input_tokens, self.output_tokens, self.reasoning_tokens]
            .into_iter()
            .flatten()
        {
            has_value = true;
            total += segment;
        }

        has_value.then_some(total)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFinishReason {
    Stop,
    Length,
    ToolCall,
    McpCall,
    ContentFilter,
    Error,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_call: bool,
    pub mcp: bool,
    pub multimodal: bool,
    pub structured_output: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderModelDescriptor {
    pub model_id: String,
    pub display_name: String,
    pub source: ProviderModelSource,
    pub supports_streaming: bool,
    pub supports_tool_call: bool,
    pub supports_multimodal: bool,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub provider_metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderToolCall {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(
        default = "empty_provider_metadata",
        skip_serializing_if = "is_empty_provider_metadata"
    )]
    pub provider_metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderMcpCall {
    pub id: String,
    pub server: String,
    pub method: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativePromptCacheControlType {
    Ephemeral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativePromptCacheTtl {
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    OneHour,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativePromptCacheControl {
    #[serde(rename = "type")]
    pub cache_type: NativePromptCacheControlType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<NativePromptCacheTtl>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativePromptBlock {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_control: Option<NativePromptCacheControl>,
    },
}

impl NativePromptBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            cache_control: None,
        }
    }

    pub fn text_content(&self) -> &str {
        match self {
            Self::Text { text, .. } => text,
        }
    }

    pub fn has_cache_control(&self) -> bool {
        match self {
            Self::Text { cache_control, .. } => cache_control.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NativeModelRequestContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_user_reference: Option<String>,
}

impl NativeModelRequestContext {
    pub fn is_empty(&self) -> bool {
        self.end_user_reference.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NativeModelPromptContext {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system: Vec<NativePromptBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<Value>,
}

impl NativeModelPromptContext {
    pub fn is_empty(&self) -> bool {
        self.system.is_empty() && self.messages.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProviderInvocationContractVersion {
    #[default]
    #[serde(rename = "1flowbase.provider/v2")]
    Current,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderInvocationCapability {
    CountTokens,
    #[serde(rename = "compact.responses_compact")]
    CompactResponsesCompact,
    #[serde(rename = "compact.responses_compaction_v2")]
    CompactResponsesCompactionV2,
    #[serde(rename = "responses.native_passthrough")]
    ResponsesNativePassthrough,
    #[serde(rename = "responses.native_output.v1")]
    ResponsesNativeOutputV1,
    ReasoningOutputSupported,
    ReasoningHistoryInputSupported,
    NativeContinuationSupported,
    SystemPromptBlocks,
    SystemPromptCacheControl,
    EndUserReference,
    #[serde(rename = "message_blocks.reasoning_history.v1")]
    MessageBlocksReasoningHistoryV1,
    #[serde(rename = "message_blocks.redacted_reasoning_history.v1")]
    MessageBlocksRedactedReasoningHistoryV1,
    ProtocolContext,
    #[serde(rename = "network_egress_handoff/v1")]
    NetworkEgressHandoffV1,
}

impl ProviderInvocationCapability {
    /// The exact capability name declared by a provider package manifest.
    pub fn manifest_capability_name(self) -> &'static str {
        match self {
            Self::CountTokens => PROVIDER_COUNT_TOKENS_CAPABILITY,
            Self::CompactResponsesCompact => PROVIDER_COMPACT_RESPONSES_COMPACT_CAPABILITY,
            Self::CompactResponsesCompactionV2 => {
                PROVIDER_COMPACT_RESPONSES_COMPACTION_V2_CAPABILITY
            }
            Self::ResponsesNativePassthrough => PROVIDER_RESPONSES_NATIVE_PASSTHROUGH_CAPABILITY,
            Self::ResponsesNativeOutputV1 => "responses.native_output.v1",
            Self::ReasoningOutputSupported => PROVIDER_REASONING_OUTPUT_SUPPORTED_CAPABILITY,
            Self::ReasoningHistoryInputSupported => {
                PROVIDER_REASONING_HISTORY_INPUT_SUPPORTED_CAPABILITY
            }
            Self::NativeContinuationSupported => PROVIDER_NATIVE_CONTINUATION_SUPPORTED_CAPABILITY,
            Self::SystemPromptBlocks => "system_prompt_blocks",
            Self::SystemPromptCacheControl => "system_prompt_cache_control",
            Self::EndUserReference => "end_user_reference",
            Self::MessageBlocksReasoningHistoryV1 => {
                PROVIDER_MESSAGE_BLOCKS_REASONING_HISTORY_V1_CAPABILITY
            }
            Self::MessageBlocksRedactedReasoningHistoryV1 => {
                PROVIDER_MESSAGE_BLOCKS_REDACTED_REASONING_HISTORY_V1_CAPABILITY
            }
            // Kept as an input compatibility marker for host call sites. It is stripped before
            // provider projection and is not a package manifest capability.
            Self::ProtocolContext => "protocol_context",
            Self::NetworkEgressHandoffV1 => PROVIDER_NETWORK_EGRESS_HANDOFF_V1_CAPABILITY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderNetworkEgressMode {
    RequiredHttpProxy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderNetworkEgressContext {
    pub mode: ProviderNetworkEgressMode,
    pub http_proxy_url: String,
    pub expires_at: String,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCompactProfile {
    ResponsesCompact,
    ResponsesCompactionV2,
}

impl ProviderCompactProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResponsesCompact => "responses_compact",
            Self::ResponsesCompactionV2 => "responses_compaction_v2",
        }
    }

    pub fn required_capability(self) -> ProviderInvocationCapability {
        match self {
            Self::ResponsesCompact => ProviderInvocationCapability::CompactResponsesCompact,
            Self::ResponsesCompactionV2 => {
                ProviderInvocationCapability::CompactResponsesCompactionV2
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderMessage {
    pub role: ProviderMessageRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SourceProtocolRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authentication: Option<ProtocolAuthenticationPresentation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolAuthenticationPresentation {
    AuthorizationBearer,
    XApiKey,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProtocolContextEnvelope {
    pub source_protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_request: Option<SourceProtocolRequest>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub body: BTreeMap<String, Value>,
}

pub const PROTOCOL_CONTEXT_VALUE_TYPE: &str = "protocol_context";

pub fn protocol_context_envelope_json_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["source_protocol"],
        "properties": {
            "source_protocol": { "type": "string", "minLength": 1 },
            "source_request": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "authentication": {
                        "type": "string",
                        "enum": ["authorization_bearer", "x_api_key"]
                    },
                    "body": { "type": "object" }
                }
            },
            "query": {
                "type": "object",
                "additionalProperties": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "headers": {
                "type": "object",
                "additionalProperties": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "body": {
                "type": "object",
                "additionalProperties": true
            }
        }
    })
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderNativeTransport {
    pub protocol: String,
    pub wire_body: Value,
    pub digest: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderClientTransport {
    Http,
    Websocket,
}

impl std::fmt::Debug for ProviderNativeTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderNativeTransport")
            .field("protocol", &self.protocol)
            .field("digest", &self.digest)
            .field("size_bytes", &self.size_bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProviderInvocationInput {
    #[serde(default, skip_serializing_if = "is_generate_provider_wire_operation")]
    pub operation: ProviderWireOperation,
    pub contract_version: ProviderInvocationContractVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProviderCompactProfile>,
    pub provider_instance_id: String,
    pub provider_code: String,
    pub protocol: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    #[serde(default)]
    pub provider_config: Value,
    #[serde(default)]
    pub messages: Vec<ProviderMessage>,
    #[serde(default)]
    pub system: Vec<NativePromptBlock>,
    #[serde(default, skip_serializing_if = "NativeModelRequestContext::is_empty")]
    pub request_context: NativeModelRequestContext,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required_capabilities: BTreeSet<ProviderInvocationCapability>,
    #[serde(default)]
    pub tools: Vec<Value>,
    #[serde(default)]
    pub mcp_bindings: Vec<Value>,
    pub response_format: Option<Value>,
    #[serde(default)]
    pub model_parameters: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_protocol_envelope: Option<ProtocolContextEnvelope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_transport: Option<ProviderNativeTransport>,
    /// Host-observed ingress transport, never derived from client-supplied headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_transport: Option<ProviderClientTransport>,
    #[serde(default)]
    pub trace_context: BTreeMap<String, String>,
    #[serde(default)]
    pub run_context: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderWireOperation {
    #[default]
    Generate,
    CountTokens,
    Compact,
}

fn is_generate_provider_wire_operation(operation: &ProviderWireOperation) -> bool {
    *operation == ProviderWireOperation::Generate
}

/// The closed result set for remote compaction. V2 exposes only the
/// provider-produced opaque value; callers must not decode or replace it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result_type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderCompactResult {
    ResponseItems {
        operation: ProviderWireOperation,
        profile: ProviderCompactProfile,
        response_items: Vec<Value>,
    },
    CompletedOpaqueCompactionItem {
        operation: ProviderWireOperation,
        profile: ProviderCompactProfile,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        response_id: Option<String>,
        compaction_item: Value,
        encrypted_content: String,
    },
}

impl ProviderCompactResult {
    pub fn operation(&self) -> ProviderWireOperation {
        match self {
            Self::ResponseItems { operation, .. }
            | Self::CompletedOpaqueCompactionItem { operation, .. } => *operation,
        }
    }

    pub fn profile(&self) -> ProviderCompactProfile {
        match self {
            Self::ResponseItems { profile, .. }
            | Self::CompletedOpaqueCompactionItem { profile, .. } => *profile,
        }
    }

    pub fn satisfies_profile(&self, expected_profile: ProviderCompactProfile) -> bool {
        self.operation() == ProviderWireOperation::Compact
            && self.profile() == expected_profile
            && match (expected_profile, self) {
                (
                    ProviderCompactProfile::ResponsesCompact,
                    Self::ResponseItems { response_items, .. },
                ) => response_items.iter().all(Value::is_object),
                (
                    ProviderCompactProfile::ResponsesCompactionV2,
                    Self::CompletedOpaqueCompactionItem {
                        compaction_item,
                        encrypted_content,
                        ..
                    },
                ) => compaction_item.as_object().is_some_and(|item| {
                    item.get("type").and_then(Value::as_str) == Some("compaction")
                        && item.get("encrypted_content").and_then(Value::as_str)
                            == Some(encrypted_content.as_str())
                }),
                _ => false,
            }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderWireAudit {
    pub operation: ProviderWireOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProviderCompactProfile>,
    pub contract_version: ProviderInvocationContractVersion,
    pub message_count: u32,
    pub system_block_count: u32,
    pub tool_count: u32,
    pub mcp_binding_count: u32,
    pub model_parameter_count: u32,
    pub trace_context_entry_count: u32,
    pub run_context_entry_count: u32,
    pub counts_capped: bool,
    pub has_previous_response_id: bool,
    pub has_request_context: bool,
    pub has_response_format: bool,
    pub has_client_protocol_envelope: bool,
    pub has_native_transport: bool,
    pub required_capabilities: BTreeSet<ProviderInvocationCapability>,
}

mod protocol_context;
pub use protocol_context::validate_protocol_context_envelope;
use protocol_context::{bounded_wire_count, project_protocol_context_envelope};

fn provider_invocation_capability_name(capability: &ProviderInvocationCapability) -> &'static str {
    capability.manifest_capability_name()
}

pub fn semantic_required_capabilities(
    system: &[NativePromptBlock],
    request_context: &NativeModelRequestContext,
) -> BTreeSet<ProviderInvocationCapability> {
    let mut capabilities = BTreeSet::new();
    if system.iter().any(NativePromptBlock::has_cache_control) {
        capabilities.insert(ProviderInvocationCapability::SystemPromptBlocks);
        capabilities.insert(ProviderInvocationCapability::SystemPromptCacheControl);
    }
    if request_context.end_user_reference.is_some() {
        capabilities.insert(ProviderInvocationCapability::EndUserReference);
    }
    capabilities
}

pub fn message_block_required_capabilities(
    messages: &[ProviderMessage],
) -> Result<BTreeSet<ProviderInvocationCapability>, String> {
    let mut capabilities = BTreeSet::new();
    for (message_index, message) in messages.iter().enumerate() {
        let Some(content_blocks) = message.content_blocks.as_ref() else {
            continue;
        };
        let blocks = content_blocks
            .as_array()
            .ok_or_else(|| format!("messages[{message_index}].content_blocks must be an array"))?;
        for (block_index, block) in blocks.iter().enumerate() {
            let block_type = block
                .as_object()
                .and_then(|object| object.get("type"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "messages[{message_index}].content_blocks[{block_index}].type must be a string"
                    )
                })?;
            match block_type {
                "text" | "image" | "image_url" | "document" | "tool_use" | "tool_result" => {}
                "reasoning" => {
                    capabilities
                        .insert(ProviderInvocationCapability::MessageBlocksReasoningHistoryV1);
                }
                "reasoning_redacted" => {
                    capabilities.insert(
                        ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1,
                    );
                }
                unknown => {
                    return Err(format!(
                        "messages[{message_index}].content_blocks[{block_index}] has unsupported canonical block type: {unknown}"
                    ));
                }
            }
        }
    }
    Ok(capabilities)
}

fn undeclared_provider_capabilities(
    required_capabilities: &BTreeSet<ProviderInvocationCapability>,
    declared_capabilities: &[String],
) -> Vec<ProviderInvocationCapability> {
    let declared_capabilities = declared_capabilities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    required_capabilities
        .iter()
        .filter(|capability| {
            !declared_capabilities.contains(provider_invocation_capability_name(capability))
        })
        .copied()
        .collect()
}

mod runtime;
pub use runtime::*;

mod recovery;
pub use recovery::*;

mod transport_session;
pub use transport_session::*;

#[path = "provider_contract/invocation_input.rs"]
mod invocation_input;
