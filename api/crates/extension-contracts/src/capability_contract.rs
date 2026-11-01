//! Existing capability stdio envelope, shared by the Host and author SDK.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStdioMethod {
    ValidateConfig,
    ResolveDynamicOptions,
    ResolveOutputSchema,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityStdioRequest {
    pub method: CapabilityStdioMethod,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityStdioResponse {
    pub ok: bool,
    #[serde(default)]
    pub result: Value,
    #[serde(default)]
    pub error: Option<CapabilityStdioError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityStdioError {
    pub message: String,
    #[serde(default)]
    pub provider_summary: Option<String>,
}

/// Existing unary capability transport frame bound, shared with SDK workers.
pub const CAPABILITY_STDIO_FRAME_BYTES: u64 = 1024 * 1024;

/// Host-selected policy for one managed stdio execution, never user body input.
/// Optional memory matches the existing PluginRuntimeLimits policy (no guessed worker cap).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityExecutionBudget {
    pub timeout_ms: u64,
    pub memory_bytes: Option<u64>,
    pub max_output_bytes: u64,
}
