use serde::{Deserialize, Serialize};

use crate::{McpInstanceStatus, McpRiskLevel, McpToolExecutionTarget, McpToolStatus};

pub const MCP_BUNDLE_SCHEMA_VERSION: &str = "1flowbase.mcp.bundle/v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpBundleFileKind {
    Tool,
    Instance,
    Connection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct McpBundleTool {
    pub tool_id: String,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub execution_target: McpToolExecutionTarget,
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

impl<'de> Deserialize<'de> for McpBundleTool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            tool_id: String,
            name: String,
            short_description: String,
            full_description: String,
            execution_target: Option<McpToolExecutionTarget>,
            // @field-contract-compat source=mcp_bundle/v1 alias=interface_id remove_by=2027-07-14
            interface_id: Option<String>,
            #[serde(default)]
            parameter_schema_snapshot: serde_json::Value,
            #[serde(default)]
            result_schema_snapshot: serde_json::Value,
            #[serde(default)]
            input_mapping: serde_json::Value,
            #[serde(default)]
            output_mapping: serde_json::Value,
            permission_code_snapshot: Option<String>,
            risk_level_snapshot: McpRiskLevel,
            status: McpToolStatus,
        }
        let wire = Wire::deserialize(deserializer)?;
        let execution_target = match (wire.execution_target, wire.interface_id) {
            (Some(target), None) => target,
            (None, Some(interface_id)) => McpToolExecutionTarget::InterfaceWrapper { interface_id },
            _ => {
                return Err(serde::de::Error::custom(
                    "bundle tool requires one execution target",
                ))
            }
        };
        Ok(Self {
            tool_id: wire.tool_id,
            name: wire.name,
            short_description: wire.short_description,
            full_description: wire.full_description,
            execution_target,
            parameter_schema_snapshot: wire.parameter_schema_snapshot,
            result_schema_snapshot: wire.result_schema_snapshot,
            input_mapping: wire.input_mapping,
            output_mapping: wire.output_mapping,
            permission_code_snapshot: wire.permission_code_snapshot,
            risk_level_snapshot: wire.risk_level_snapshot,
            status: wire.status,
        })
    }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleUpstreamConnection {
    pub connection_id: uuid::Uuid,
    pub name: String,
    pub endpoint: String,
    pub transport: crate::McpUpstreamTransport,
    pub auth_type: crate::McpUpstreamAuthType,
    pub custom_header_name: Option<String>,
    pub status: crate::McpUpstreamConnectionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpBundlePackage {
    pub manifest: McpBundleManifest,
    pub tools: Vec<McpBundleTool>,
    pub instances: Vec<McpBundleInstance>,
    pub connections: Vec<McpBundleUpstreamConnection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpBundleVersionStatus {
    SameSystemVersion,
    ExportedFromOlderSystem,
    ExportedFromNewerSystem,
    UnknownSystemVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpBundleItemEffect {
    Create,
    AlreadyPresent,
    Conflict,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpBundleEffectSummary {
    pub changes: usize,
    pub already_present: usize,
    pub conflicts: usize,
    pub unavailable: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpBundleItemReport {
    pub id: String,
    pub effect: McpBundleItemEffect,
    pub result: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundlePreview {
    pub manifest: McpBundleManifest,
    pub current_system_version: String,
    pub version_status: McpBundleVersionStatus,
    pub effect_summary: McpBundleEffectSummary,
    pub tools: Vec<McpBundleItemReport>,
    pub instances: Vec<McpBundleItemReport>,
    pub connections: Vec<McpBundleItemReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleImportReport {
    pub manifest: McpBundleManifest,
    pub current_system_version: String,
    pub version_status: McpBundleVersionStatus,
    pub status: String,
    pub effect_summary: McpBundleEffectSummary,
    pub tools: Vec<McpBundleItemReport>,
    pub instances: Vec<McpBundleItemReport>,
    pub connections: Vec<McpBundleItemReport>,
}
