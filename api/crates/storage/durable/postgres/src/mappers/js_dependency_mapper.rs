use anyhow::Result;

pub struct StoredJsDependencyRegistryRow {
    pub installation_id: uuid::Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub alias: String,
    pub package: String,
    pub version: String,
    pub target: String,
    pub artifact_path: String,
    pub integrity: String,
    pub permission_network: String,
    pub permission_filesystem: String,
    pub permission_env: String,
}

pub struct StoredApplicationJsDependencySelectionRow {
    pub workspace_id: uuid::Uuid,
    pub application_id: uuid::Uuid,
    pub installation_id: uuid::Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub alias: String,
    pub package: String,
    pub version: String,
    pub target: String,
    pub artifact_path: String,
    pub artifact_hash: String,
    pub integrity: String,
    pub permission_network: String,
    pub permission_filesystem: String,
    pub permission_env: String,
}

pub struct PgJsDependencyMapper;

impl PgJsDependencyMapper {
    pub fn to_registry_entry(
        row: StoredJsDependencyRegistryRow,
    ) -> Result<domain::JsDependencyRegistryEntry> {
        Ok(domain::JsDependencyRegistryEntry {
            installation_id: row.installation_id,
            provider_code: row.provider_code,
            plugin_id: row.plugin_id,
            plugin_version: row.plugin_version,
            alias: row.alias,
            package: row.package,
            version: row.version,
            target: row.target,
            artifact_path: row.artifact_path,
            integrity: row.integrity,
            permissions: domain::JsDependencyPermissions {
                network: row.permission_network,
                filesystem: row.permission_filesystem,
                env: row.permission_env,
            },
        })
    }

    pub fn to_application_selection(
        row: StoredApplicationJsDependencySelectionRow,
    ) -> Result<domain::ApplicationJsDependencySelection> {
        Ok(domain::ApplicationJsDependencySelection {
            workspace_id: row.workspace_id,
            application_id: row.application_id,
            installation_id: row.installation_id,
            provider_code: row.provider_code,
            plugin_id: row.plugin_id,
            plugin_version: row.plugin_version,
            alias: row.alias,
            package: row.package,
            version: row.version,
            target: row.target,
            artifact_path: row.artifact_path,
            artifact_hash: row.artifact_hash,
            integrity: row.integrity,
            permissions: domain::JsDependencyPermissions {
                network: row.permission_network,
                filesystem: row.permission_filesystem,
                env: row.permission_env,
            },
        })
    }
}
