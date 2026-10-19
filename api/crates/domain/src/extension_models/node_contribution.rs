use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeContributionDependencyStatus {
    Ready,
    MissingPlugin,
    VersionMismatch,
    DisabledPlugin,
}

impl NodeContributionDependencyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::MissingPlugin => "missing_plugin",
            Self::VersionMismatch => "version_mismatch",
            Self::DisabledPlugin => "disabled_plugin",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeContributionRegistryEntry {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_unique_identifier: String,
    pub package_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub node_shell: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub schema_ui: Value,
    pub schema_version: String,
    pub output_schema: Value,
    pub contribution_checksum: String,
    pub compiled_contribution_hash: String,
    pub output_schema_snapshot: Value,
    pub side_effect_policy: String,
    pub infra_contracts: Vec<String>,
    pub required_auth: Vec<String>,
    pub visibility: String,
    pub experimental: bool,
    pub dependency_installation_kind: String,
    pub dependency_plugin_version_range: String,
    pub dependency_status: NodeContributionDependencyStatus,
}
