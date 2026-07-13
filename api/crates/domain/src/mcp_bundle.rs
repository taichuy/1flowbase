use serde::{Deserialize, Serialize};

use crate::{McpInstanceStatus, McpRiskLevel, McpToolStatus};

pub const MCP_BUNDLE_SCHEMA_VERSION: &str = "1flowbase.mcp.bundle/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpBundleFileKind {
    Tool,
    Instance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpBundleFile {
    pub path: String,
    pub kind: McpBundleFileKind,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleManifest {
    pub schema_version: String,
    pub organization: String,
    pub bundle_id: String,
    pub bundle_version: String,
    pub locale: String,
    pub minimum_host_version: String,
    pub exported_from_system_version: String,
    pub exported_at: String,
    pub files: Vec<McpBundleFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleTool {
    pub tool_id: String,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub interface_id: String,
    #[serde(default)]
    pub parameter_schema_snapshot: serde_json::Value,
    #[serde(default)]
    pub result_schema_snapshot: serde_json::Value,
    #[serde(default)]
    pub input_mapping: serde_json::Value,
    #[serde(default)]
    pub output_mapping: serde_json::Value,
    pub permission_code_snapshot: Option<String>,
    pub risk_level_snapshot: McpRiskLevel,
    pub status: McpToolStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleGroup {
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleToolBinding {
    pub group_path: String,
    pub tool_id: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleInstanceDiscoveryPolicy {
    pub list_default_limit: i32,
    pub list_max_depth: i32,
    pub list_regex_enabled: bool,
    pub list_regex_max_length: i32,
    pub list_return_fields: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleInstance {
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: McpInstanceStatus,
    pub default_entry_path: String,
    #[serde(default)]
    pub groups: Vec<McpBundleGroup>,
    #[serde(default)]
    pub bindings: Vec<McpBundleToolBinding>,
    pub discovery_policy: McpBundleInstanceDiscoveryPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpBundlePackage {
    pub manifest: McpBundleManifest,
    pub tools: Vec<McpBundleTool>,
    pub instances: Vec<McpBundleInstance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpBundleVersionStatus {
    SameSystemVersion,
    ExportedFromOlderSystem,
    ExportedFromNewerSystem,
    UnknownSystemVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpBundleItemReport {
    pub id: String,
    pub result: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundlePreview {
    pub manifest: McpBundleManifest,
    pub current_system_version: String,
    pub version_status: McpBundleVersionStatus,
    pub tools: Vec<McpBundleItemReport>,
    pub instances: Vec<McpBundleItemReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleImportReport {
    pub manifest: McpBundleManifest,
    pub current_system_version: String,
    pub version_status: McpBundleVersionStatus,
    pub status: String,
    pub tools: Vec<McpBundleItemReport>,
    pub instances: Vec<McpBundleItemReport>,
}
