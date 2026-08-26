use anyhow::{anyhow, Result};
use domain::{NodeContributionDependencyStatus, NodeContributionRegistryEntry};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoredNodeContributionRegistryRow {
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
    pub infra_contracts: Value,
    pub required_auth: Value,
    pub visibility: String,
    pub experimental: bool,
    pub dependency_installation_kind: String,
    pub dependency_plugin_version_range: String,
    pub dependency_status: String,
}

pub struct PgNodeContributionMapper;

impl PgNodeContributionMapper {
    pub fn to_registry_entry(
        row: StoredNodeContributionRegistryRow,
    ) -> Result<NodeContributionRegistryEntry> {
        Ok(NodeContributionRegistryEntry {
            installation_id: row.installation_id,
            provider_code: row.provider_code,
            plugin_unique_identifier: row.plugin_unique_identifier,
            package_id: row.package_id,
            plugin_id: row.plugin_id,
            plugin_version: row.plugin_version,
            contribution_code: row.contribution_code,
            node_shell: row.node_shell,
            category: row.category,
            title: row.title,
            description: row.description,
            icon: row.icon,
            schema_ui: row.schema_ui,
            schema_version: row.schema_version,
            output_schema: row.output_schema,
            contribution_checksum: row.contribution_checksum,
            compiled_contribution_hash: row.compiled_contribution_hash,
            output_schema_snapshot: row.output_schema_snapshot,
            side_effect_policy: row.side_effect_policy,
            infra_contracts: parse_string_array(row.infra_contracts)?,
            required_auth: parse_string_array(row.required_auth)?,
            visibility: row.visibility,
            experimental: row.experimental,
            dependency_installation_kind: row.dependency_installation_kind,
            dependency_plugin_version_range: row.dependency_plugin_version_range,
            dependency_status: parse_dependency_status(&row.dependency_status)?,
        })
    }
}

fn parse_string_array(value: Value) -> Result<Vec<String>> {
    let items = value
        .as_array()
        .ok_or_else(|| anyhow!("expected json array of strings"))?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("expected string array item"))
        })
        .collect()
}

pub fn parse_dependency_status(value: &str) -> Result<NodeContributionDependencyStatus> {
    match value {
        "ready" => Ok(NodeContributionDependencyStatus::Ready),
        "missing_plugin" => Ok(NodeContributionDependencyStatus::MissingPlugin),
        "version_mismatch" => Ok(NodeContributionDependencyStatus::VersionMismatch),
        "disabled_plugin" => Ok(NodeContributionDependencyStatus::DisabledPlugin),
        _ => Err(anyhow!(
            "unknown node contribution dependency_status: {value}"
        )),
    }
}
