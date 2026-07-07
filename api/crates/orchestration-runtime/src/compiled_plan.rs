use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPlan {
    pub flow_id: Uuid,
    pub source_draft_id: String,
    pub schema_version: String,
    pub topological_order: Vec<String>,
    #[serde(default)]
    pub edges: Vec<CompiledEdge>,
    pub nodes: BTreeMap<String, CompiledNode>,
    #[serde(default)]
    pub compile_issues: Vec<CompileIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledEdge {
    pub edge_id: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledNode {
    pub node_id: String,
    pub node_type: String,
    pub alias: String,
    pub container_id: Option<String>,
    pub dependency_node_ids: Vec<String>,
    pub downstream_node_ids: Vec<String>,
    pub bindings: BTreeMap<String, CompiledBinding>,
    pub outputs: Vec<CompiledOutput>,
    pub config: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_runtime: Option<CompiledPluginRuntime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_runtime: Option<CompiledLlmRuntime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_runtime: Option<CompiledCodeRuntime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledBinding {
    pub kind: String,
    pub raw_value: serde_json::Value,
    pub selector_paths: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledOutput {
    pub key: String,
    pub title: String,
    pub value_type: String,
    pub selector: Vec<String>,
    #[serde(
        default,
        rename = "jsonSchema",
        skip_serializing_if = "Option::is_none"
    )]
    pub json_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmRoutingMode {
    FixedModel,
    FailoverQueue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LlmDistributionRule {
    #[default]
    None,
    RoundRobin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledLlmRouting {
    pub routing_mode: LlmRoutingMode,
    pub fixed_model_target: Option<serde_json::Value>,
    pub queue_template_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_snapshot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queue_targets: Vec<CompiledLlmRouteTarget>,
    #[serde(default)]
    pub distribution_rule: LlmDistributionRule,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distribution_key: Option<String>,
    #[serde(default = "default_llm_context_policy")]
    pub context_policy: serde_json::Value,
    pub stream_policy: serde_json::Value,
}

fn default_llm_context_policy() -> serde_json::Value {
    serde_json::json!({ "integration_context": "enabled" })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledLlmRouteTarget {
    pub provider_instance_id: String,
    pub provider_code: String,
    pub protocol: String,
    pub upstream_model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledLlmRuntime {
    pub provider_instance_id: String,
    pub provider_code: String,
    pub protocol: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<CompiledLlmRouting>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledCodeRuntime {
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<CompiledCodeDependency>,
    #[serde(default = "CodeIsolationProfile::quickjs_default")]
    pub isolation_profile: CodeIsolationProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeIsolationProfile {
    pub mode: String,
    pub timeout_ms: u64,
    pub memory_mb: u32,
    pub stack_kb: u32,
    pub network: String,
    pub filesystem: String,
    pub env: String,
    pub secrets: String,
    pub executor_id: String,
}

impl CodeIsolationProfile {
    pub const DEFAULT_MODE: &'static str = "vm_limited";
    pub const DEFAULT_TIMEOUT_MS: u64 = 100;
    pub const DEFAULT_MEMORY_MB: u32 = 8;
    pub const DEFAULT_STACK_KB: u32 = 256;
    pub const DEFAULT_NETWORK: &'static str = "deny";
    pub const DEFAULT_FILESYSTEM: &'static str = "deny";
    pub const DEFAULT_ENV: &'static str = "none";
    pub const DEFAULT_SECRETS: &'static str = "none";
    pub const DEFAULT_EXECUTOR_ID: &'static str = "quickjs-local";

    pub fn quickjs_default() -> Self {
        Self {
            mode: Self::DEFAULT_MODE.to_string(),
            timeout_ms: Self::DEFAULT_TIMEOUT_MS,
            memory_mb: Self::DEFAULT_MEMORY_MB,
            stack_kb: Self::DEFAULT_STACK_KB,
            network: Self::DEFAULT_NETWORK.to_string(),
            filesystem: Self::DEFAULT_FILESYSTEM.to_string(),
            env: Self::DEFAULT_ENV.to_string(),
            secrets: Self::DEFAULT_SECRETS.to_string(),
            executor_id: Self::DEFAULT_EXECUTOR_ID.to_string(),
        }
    }
}

impl Default for CodeIsolationProfile {
    fn default() -> Self {
        Self::quickjs_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeExecutorCapability {
    pub executor_id: String,
    pub supported_languages: Vec<String>,
    pub supported_modes: Vec<String>,
    pub supported_artifact_targets: Vec<String>,
    pub max_timeout_ms: u64,
    pub max_memory_mb: u32,
    pub max_stack_kb: u32,
    pub network: String,
    pub filesystem: String,
    pub env: String,
    pub secrets: String,
}

impl CodeExecutorCapability {
    pub const QUICKJS_MAX_TIMEOUT_MS: u64 = 1000;
    pub const QUICKJS_MAX_MEMORY_MB: u32 = 32;
    pub const QUICKJS_MAX_STACK_KB: u32 = 1024;

    pub fn quickjs_local() -> Self {
        Self {
            executor_id: CodeIsolationProfile::DEFAULT_EXECUTOR_ID.to_string(),
            supported_languages: vec!["javascript".to_string()],
            supported_modes: vec![CodeIsolationProfile::DEFAULT_MODE.to_string()],
            supported_artifact_targets: vec!["backend_code".to_string()],
            max_timeout_ms: Self::QUICKJS_MAX_TIMEOUT_MS,
            max_memory_mb: Self::QUICKJS_MAX_MEMORY_MB,
            max_stack_kb: Self::QUICKJS_MAX_STACK_KB,
            network: CodeIsolationProfile::DEFAULT_NETWORK.to_string(),
            filesystem: CodeIsolationProfile::DEFAULT_FILESYSTEM.to_string(),
            env: CodeIsolationProfile::DEFAULT_ENV.to_string(),
            secrets: CodeIsolationProfile::DEFAULT_SECRETS.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledCodeDependency {
    pub alias: String,
    pub target: String,
    pub artifact_path: String,
    pub artifact_hash: String,
    pub integrity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPluginRuntime {
    pub installation_id: Uuid,
    pub plugin_unique_identifier: String,
    pub package_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub node_shell: String,
    pub schema_version: String,
    pub contribution_checksum: String,
    pub compiled_contribution_hash: String,
    pub output_schema_snapshot: Vec<CompiledOutput>,
    pub side_effect_policy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileIssueCode {
    MissingProviderInstance,
    ProviderInstanceNotFound,
    ProviderInstanceNotReady,
    MissingModel,
    ModelNotAvailable,
    MissingPluginId,
    MissingPluginVersion,
    MissingContributionCode,
    MissingNodeShell,
    MissingSchemaVersion,
    MissingPluginUniqueIdentifier,
    MissingPackageId,
    MissingContributionChecksum,
    MissingCompiledContributionHash,
    MissingOutputSchemaSnapshot,
    UnsupportedPluginContributionSchemaVersion,
    MissingPluginContribution,
    PluginContributionDependencyNotReady,
    PluginContributionChecksumMismatch,
    PluginContributionOutputSchemaMismatch,
    JsDependencyImportNotEnabled,
    InvalidJsDependencyImport,
    InvalidCodeIsolationProfile,
    InvalidLlmContextSelector,
    IncompatibleLlmContextSchema,
    InvalidVisibleInternalLlmTool,
    UnresolvedNode,
    DuplicateAnswerPresentationReference,
    InvalidAnswerPresentationOrder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileIssue {
    pub node_id: String,
    pub code: CompileIssueCode,
    pub message: String,
}
