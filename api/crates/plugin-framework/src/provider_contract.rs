use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::PluginFrameworkError;

pub const CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY: &str = "__client_protocol_envelope";
pub const NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY: &str = "__native_model_prompt_context";
pub const NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY: &str = "__native_model_request_context";
pub const CURRENT_PROVIDER_CONTRACT: &str = "1flowbase.provider/v2";
pub const PROVIDER_COUNT_TOKENS_CAPABILITY: &str = "count_tokens";
pub const PROVIDER_COMPACT_RESPONSES_COMPACT_CAPABILITY: &str = "compact.responses_compact";
pub const PROVIDER_COMPACT_RESPONSES_COMPACTION_V2_CAPABILITY: &str =
    "compact.responses_compaction_v2";
pub const PROVIDER_RESPONSES_NATIVE_PASSTHROUGH_CAPABILITY: &str = "responses.native_passthrough";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelDiscoveryMode {
    Static,
    Dynamic,
    Hybrid,
}

impl TryFrom<&str> for ModelDiscoveryMode {
    type Error = PluginFrameworkError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "static" => Ok(Self::Static),
            "dynamic" => Ok(Self::Dynamic),
            "hybrid" => Ok(Self::Hybrid),
            other => Err(PluginFrameworkError::invalid_provider_contract(format!(
                "unsupported model discovery mode: {other}"
            ))),
        }
    }
}

impl TryFrom<String> for ModelDiscoveryMode {
    type Error = PluginFrameworkError;

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
    SystemPromptBlocks,
    SystemPromptCacheControl,
    EndUserReference,
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
            Self::SystemPromptBlocks => "system_prompt_blocks",
            Self::SystemPromptCacheControl => "system_prompt_cache_control",
            Self::EndUserReference => "end_user_reference",
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClientProtocolEnvelope {
    pub source_protocol: String,
    pub policy: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderNativeTransport {
    pub protocol: String,
    pub wire_body: Value,
    pub digest: String,
    pub size_bytes: u64,
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
    pub client_protocol_envelope: Option<ClientProtocolEnvelope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_transport: Option<ProviderNativeTransport>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCountTokensInput {
    pub operation: ProviderWireOperation,
    pub contract_version: ProviderInvocationContractVersion,
    pub provider_instance_id: String,
    pub provider_code: String,
    pub protocol: String,
    pub model: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_protocol_envelope: Option<ClientProtocolEnvelope>,
}

impl Default for ProviderCountTokensInput {
    fn default() -> Self {
        Self {
            operation: ProviderWireOperation::CountTokens,
            contract_version: ProviderInvocationContractVersion::default(),
            provider_instance_id: String::new(),
            provider_code: String::new(),
            protocol: String::new(),
            model: String::new(),
            provider_config: Value::Null,
            messages: Vec::new(),
            system: Vec::new(),
            request_context: NativeModelRequestContext::default(),
            required_capabilities: BTreeSet::new(),
            client_protocol_envelope: None,
        }
    }
}

impl ProviderCountTokensInput {
    pub fn required_capabilities(&self) -> BTreeSet<ProviderInvocationCapability> {
        let mut capabilities = self.required_capabilities.clone();
        capabilities.insert(ProviderInvocationCapability::CountTokens);
        capabilities.extend(semantic_required_capabilities(
            &self.system,
            &self.request_context,
        ));
        capabilities
    }

    // Runtime errors intentionally keep the complete upstream diagnostics payload.
    #[allow(clippy::result_large_err)]
    pub fn to_current_provider_wire_value(
        &self,
        declared_capabilities: &[String],
    ) -> Result<Value, ProviderCountTokensError> {
        if self.operation != ProviderWireOperation::CountTokens {
            return Err(ProviderCountTokensError::InvalidContract {
                message: "CountTokens input must declare operation=count_tokens".to_string(),
            });
        }
        let unsupported =
            undeclared_provider_capabilities(&self.required_capabilities(), declared_capabilities);
        if !unsupported.is_empty() {
            return Err(ProviderCountTokensError::Unsupported {
                capabilities: unsupported
                    .iter()
                    .map(provider_invocation_capability_name)
                    .collect(),
            });
        }

        serde_json::to_value(self).map_err(|error| ProviderCountTokensError::InvalidContract {
            message: error.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCountTokensResult {
    pub operation: ProviderWireOperation,
    pub input_tokens: u64,
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

impl ProviderInvocationInput {
    pub fn system_text(&self) -> Option<String> {
        (!self.system.is_empty()).then(|| {
            self.system
                .iter()
                .map(NativePromptBlock::text_content)
                .collect::<Vec<_>>()
                .join("\n\n")
        })
    }

    pub fn synchronize_required_capabilities(&mut self) {
        self.required_capabilities
            .extend(self.derived_required_capabilities());
    }

    pub fn semantic_required_capabilities(&self) -> BTreeSet<ProviderInvocationCapability> {
        semantic_required_capabilities(&self.system, &self.request_context)
    }

    pub fn compact_profile(&self) -> Result<ProviderCompactProfile, String> {
        if self.operation != ProviderWireOperation::Compact {
            return Err("Compact input must declare operation=compact".to_string());
        }
        self.profile
            .ok_or_else(|| "Compact input must declare a compact profile".to_string())
    }

    pub fn to_current_provider_wire_value(
        &self,
        declared_capabilities: &[String],
    ) -> Result<Value, PluginFrameworkError> {
        let invocation = self
            .prepared_current_provider_invocation()
            .map_err(PluginFrameworkError::invalid_provider_contract)?;
        let unsupported = undeclared_provider_capabilities(
            &invocation.required_capabilities,
            declared_capabilities,
        );

        if !unsupported.is_empty() {
            return Err(PluginFrameworkError::invalid_provider_contract(format!(
                "current provider contract is missing required capabilities: {}",
                unsupported
                    .iter()
                    .map(provider_invocation_capability_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        serde_json::to_value(invocation)
            .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
    }

    // Compact errors preserve the same typed upstream diagnostics contract.
    #[allow(clippy::result_large_err)]
    pub fn to_current_provider_compact_wire_value(
        &self,
        declared_capabilities: &[String],
    ) -> Result<Value, ProviderCompactError> {
        let profile = self
            .compact_profile()
            .map_err(|message| ProviderCompactError::InvalidContract { message })?;
        let invocation = self
            .prepared_current_provider_invocation()
            .map_err(|message| ProviderCompactError::InvalidContract { message })?;
        let unsupported = undeclared_provider_capabilities(
            &invocation.required_capabilities,
            declared_capabilities,
        );

        if !unsupported.is_empty() {
            return Err(ProviderCompactError::Unsupported {
                profile,
                capabilities: unsupported
                    .iter()
                    .map(provider_invocation_capability_name)
                    .collect(),
            });
        }

        serde_json::to_value(invocation).map_err(|error| ProviderCompactError::InvalidContract {
            message: error.to_string(),
        })
    }

    pub fn wire_audit(&self) -> ProviderWireAudit {
        let lengths = [
            self.messages.len(),
            self.system.len(),
            self.tools.len(),
            self.mcp_bindings.len(),
            self.model_parameters.len(),
            self.trace_context.len(),
            self.run_context.len(),
        ];
        let mut required_capabilities = self.required_capabilities.clone();
        required_capabilities.extend(self.derived_required_capabilities());

        ProviderWireAudit {
            operation: self.operation,
            profile: self.profile,
            contract_version: self.contract_version,
            message_count: bounded_wire_count(self.messages.len()),
            system_block_count: bounded_wire_count(self.system.len()),
            tool_count: bounded_wire_count(self.tools.len()),
            mcp_binding_count: bounded_wire_count(self.mcp_bindings.len()),
            model_parameter_count: bounded_wire_count(self.model_parameters.len()),
            trace_context_entry_count: bounded_wire_count(self.trace_context.len()),
            run_context_entry_count: bounded_wire_count(self.run_context.len()),
            counts_capped: lengths.iter().any(|length| u32::try_from(*length).is_err()),
            has_previous_response_id: self.previous_response_id.is_some(),
            has_request_context: !self.request_context.is_empty(),
            has_response_format: self.response_format.is_some(),
            has_client_protocol_envelope: self.client_protocol_envelope.is_some(),
            has_native_transport: self.native_transport.is_some(),
            required_capabilities,
        }
    }

    fn prepared_current_provider_invocation(&self) -> Result<Self, String> {
        self.validate_current_provider_operation()?;
        let mut invocation = self.clone();
        invocation.synchronize_required_capabilities();
        Ok(invocation)
    }

    fn derived_required_capabilities(&self) -> BTreeSet<ProviderInvocationCapability> {
        let mut capabilities = self.semantic_required_capabilities();
        if self.operation == ProviderWireOperation::Compact {
            if let Some(profile) = self.profile {
                capabilities.insert(profile.required_capability());
            }
        }
        capabilities
    }

    fn validate_current_provider_operation(&self) -> Result<(), String> {
        let claimed_compact_capability = self.required_capabilities.iter().find(|capability| {
            matches!(
                capability,
                ProviderInvocationCapability::CompactResponsesCompact
                    | ProviderInvocationCapability::CompactResponsesCompactionV2
            )
        });

        match (self.operation, self.profile) {
            (ProviderWireOperation::Generate, None) => {
                if let Some(capability) = claimed_compact_capability {
                    return Err(format!(
                        "Generate input must not claim Compact capability {}",
                        provider_invocation_capability_name(capability)
                    ));
                }
                Ok(())
            }
            (ProviderWireOperation::Generate, Some(_)) => {
                Err("Generate input must not declare a compact profile".to_string())
            }
            (ProviderWireOperation::CountTokens, _) => Err(
                "CountTokens input must use ProviderCountTokensInput instead of ProviderInvocationInput"
                    .to_string(),
            ),
            (ProviderWireOperation::Compact, Some(profile)) => {
                if let Some(capability) = self.required_capabilities.iter().find(|capability| {
                    matches!(
                        capability,
                        ProviderInvocationCapability::CompactResponsesCompact
                            | ProviderInvocationCapability::CompactResponsesCompactionV2
                    ) && **capability != profile.required_capability()
                })
                {
                    return Err(format!(
                        "Compact input profile {} must not claim capability {}",
                        profile.as_str(),
                        provider_invocation_capability_name(capability)
                    ));
                }
                Ok(())
            }
            (ProviderWireOperation::Compact, None) => {
                Err("Compact input must declare a compact profile".to_string())
            }
        }
    }
}

fn bounded_wire_count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

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
