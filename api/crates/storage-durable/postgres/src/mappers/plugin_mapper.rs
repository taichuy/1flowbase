use anyhow::{anyhow, Result};
use domain::{
    PluginArtifactInstanceRecord, PluginArtifactInstanceStatus, PluginAssignmentRecord,
    PluginAvailabilityStatus, PluginDesiredState, PluginInstallationRecord,
    PluginPackageCatalogProjectionRecord, PluginPackageCatalogProjectionStatus,
    PluginRuntimeStatus, PluginTaskKind, PluginTaskRecord, PluginTaskStatus,
    PluginVerificationStatus,
};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoredPluginInstallationRow {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub category: String,
    pub organization: String,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contract_version: String,
    pub protocol: String,
    pub display_name: String,
    pub source_kind: String,
    pub trust_level: String,
    pub verification_status: String,
    pub desired_state: String,
    pub expected_checksum: Option<String>,
    pub signature_status: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub legacy_manifest_compatibility: Option<String>,
    pub metadata_json: serde_json::Value,
    pub is_system_reserved: bool,
    pub created_by: Uuid,
    pub updated_by: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredPluginAssignmentRow {
    pub id: Uuid,
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub assigned_by: Uuid,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredPluginArtifactInstanceRow {
    pub node_id: String,
    pub installation_id: Uuid,
    pub local_version: Option<String>,
    pub local_checksum: Option<String>,
    pub local_path: Option<String>,
    pub package_path: Option<String>,
    pub manifest_fingerprint: Option<String>,
    pub artifact_status: String,
    pub runtime_status: String,
    pub availability_status: String,
    pub checked_at: OffsetDateTime,
    pub last_error: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct StoredPluginTaskRow {
    pub id: Uuid,
    pub installation_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub provider_code: String,
    pub task_kind: String,
    pub status: String,
    pub status_message: Option<String>,
    pub detail_json: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct StoredPluginPackageCatalogProjectionRow {
    pub installation_id: Uuid,
    pub package_code: String,
    pub package_version: String,
    pub catalog_snapshot_json: serde_json::Value,
    pub projection_status: String,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

pub struct PgPluginMapper;

impl PgPluginMapper {
    pub fn to_installation_record(
        row: StoredPluginInstallationRow,
    ) -> Result<PluginInstallationRecord> {
        Ok(PluginInstallationRecord {
            id: row.id,
            scope_id: row.scope_id,
            category: domain::ExtensionCategory::parse(&row.category)
                .ok_or_else(|| anyhow!("unknown extension category: {}", row.category))?,
            organization: row.organization,
            provider_code: row.provider_code,
            plugin_id: row.plugin_id,
            plugin_version: row.plugin_version,
            contract_version: row.contract_version,
            protocol: row.protocol,
            display_name: row.display_name,
            source_kind: row.source_kind,
            trust_level: row.trust_level,
            verification_status: parse_verification_status(&row.verification_status)?,
            desired_state: parse_desired_state(&row.desired_state)?,
            expected_checksum: row.expected_checksum,
            signature_status: domain::ExtensionSignatureStatus::parse(&row.signature_status)
                .ok_or_else(|| {
                    anyhow!(
                        "unknown extension signature_status: {}",
                        row.signature_status
                    )
                })?,
            signature_algorithm: row.signature_algorithm,
            signing_key_id: row.signing_key_id,
            legacy_manifest_compatibility: row.legacy_manifest_compatibility,
            metadata_json: row.metadata_json,
            is_system_reserved: row.is_system_reserved,
            created_by: row.created_by,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub fn to_assignment_record(row: StoredPluginAssignmentRow) -> Result<PluginAssignmentRecord> {
        Ok(PluginAssignmentRecord {
            id: row.id,
            installation_id: row.installation_id,
            workspace_id: row.workspace_id,
            provider_code: row.provider_code,
            assigned_by: row.assigned_by,
            created_at: row.created_at,
        })
    }

    pub fn to_artifact_instance_record(
        row: StoredPluginArtifactInstanceRow,
    ) -> Result<PluginArtifactInstanceRecord> {
        Ok(PluginArtifactInstanceRecord {
            node_id: row.node_id,
            installation_id: row.installation_id,
            local_version: row.local_version,
            local_checksum: row.local_checksum,
            local_path: row.local_path,
            package_path: row.package_path,
            manifest_fingerprint: row.manifest_fingerprint,
            artifact_status: parse_artifact_instance_status(&row.artifact_status)?,
            runtime_status: parse_runtime_status(&row.runtime_status)?,
            availability_status: parse_availability_status(&row.availability_status)?,
            checked_at: row.checked_at,
            last_error: row.last_error,
            is_current: row.is_current,
        })
    }

    pub fn to_task_record(row: StoredPluginTaskRow) -> Result<PluginTaskRecord> {
        Ok(PluginTaskRecord {
            id: row.id,
            installation_id: row.installation_id,
            workspace_id: row.workspace_id,
            provider_code: row.provider_code,
            task_kind: parse_task_kind(&row.task_kind)?,
            status: parse_task_status(&row.status)?,
            status_message: row.status_message,
            detail_json: row.detail_json,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            finished_at: row.finished_at,
        })
    }

    pub fn to_package_catalog_projection_record(
        row: StoredPluginPackageCatalogProjectionRow,
    ) -> Result<PluginPackageCatalogProjectionRecord> {
        Ok(PluginPackageCatalogProjectionRecord {
            installation_id: row.installation_id,
            package_code: row.package_code,
            package_version: row.package_version,
            catalog_snapshot_json: row.catalog_snapshot_json,
            projection_status: parse_catalog_projection_status(&row.projection_status)?,
            last_error_message: row.last_error_message,
            refreshed_at: row.refreshed_at,
            updated_at: row.updated_at,
        })
    }
}

fn parse_catalog_projection_status(value: &str) -> Result<PluginPackageCatalogProjectionStatus> {
    match value {
        "ok" => Ok(PluginPackageCatalogProjectionStatus::Ok),
        "missing" => Ok(PluginPackageCatalogProjectionStatus::Missing),
        "failed" => Ok(PluginPackageCatalogProjectionStatus::Failed),
        _ => Err(anyhow!(
            "unknown plugin package catalog projection status: {value}"
        )),
    }
}

pub fn parse_verification_status(value: &str) -> Result<PluginVerificationStatus> {
    match value {
        "pending" => Ok(PluginVerificationStatus::Pending),
        "valid" => Ok(PluginVerificationStatus::Valid),
        "invalid" => Ok(PluginVerificationStatus::Invalid),
        _ => Err(anyhow!("unknown plugin verification_status: {value}")),
    }
}

pub fn parse_desired_state(value: &str) -> Result<PluginDesiredState> {
    match value {
        "disabled" => Ok(PluginDesiredState::Disabled),
        "pending_restart" => Ok(PluginDesiredState::PendingRestart),
        "active_requested" => Ok(PluginDesiredState::ActiveRequested),
        _ => Err(anyhow!("unknown plugin desired_state: {value}")),
    }
}

pub fn parse_artifact_instance_status(value: &str) -> Result<PluginArtifactInstanceStatus> {
    match value {
        "missing" => Ok(PluginArtifactInstanceStatus::Missing),
        "ready" => Ok(PluginArtifactInstanceStatus::Ready),
        "outdated" => Ok(PluginArtifactInstanceStatus::Outdated),
        "mismatched" => Ok(PluginArtifactInstanceStatus::Mismatched),
        "corrupted" => Ok(PluginArtifactInstanceStatus::Corrupted),
        "load_failed" => Ok(PluginArtifactInstanceStatus::LoadFailed),
        _ => Err(anyhow!("unknown plugin artifact instance status: {value}")),
    }
}

pub fn parse_runtime_status(value: &str) -> Result<PluginRuntimeStatus> {
    match value {
        "inactive" => Ok(PluginRuntimeStatus::Inactive),
        "active" => Ok(PluginRuntimeStatus::Active),
        "load_failed" => Ok(PluginRuntimeStatus::LoadFailed),
        _ => Err(anyhow!("unknown plugin runtime_status: {value}")),
    }
}

pub fn parse_availability_status(value: &str) -> Result<PluginAvailabilityStatus> {
    match value {
        "disabled" => Ok(PluginAvailabilityStatus::Disabled),
        "pending_restart" => Ok(PluginAvailabilityStatus::PendingRestart),
        "artifact_missing" => Ok(PluginAvailabilityStatus::ArtifactMissing),
        "install_incomplete" => Ok(PluginAvailabilityStatus::InstallIncomplete),
        "load_failed" => Ok(PluginAvailabilityStatus::LoadFailed),
        "available" => Ok(PluginAvailabilityStatus::Available),
        _ => Err(anyhow!("unknown plugin availability_status: {value}")),
    }
}

pub fn parse_task_kind(value: &str) -> Result<PluginTaskKind> {
    match value {
        "install" => Ok(PluginTaskKind::Install),
        "upgrade" => Ok(PluginTaskKind::Upgrade),
        "uninstall" => Ok(PluginTaskKind::Uninstall),
        "enable" => Ok(PluginTaskKind::Enable),
        "disable" => Ok(PluginTaskKind::Disable),
        "assign" => Ok(PluginTaskKind::Assign),
        "unassign" => Ok(PluginTaskKind::Unassign),
        "switch_version" => Ok(PluginTaskKind::SwitchVersion),
        _ => Err(anyhow!("unknown plugin task_kind: {value}")),
    }
}

pub fn parse_task_status(value: &str) -> Result<PluginTaskStatus> {
    match value {
        "queued" => Ok(PluginTaskStatus::Queued),
        "running" => Ok(PluginTaskStatus::Running),
        "succeeded" => Ok(PluginTaskStatus::Succeeded),
        "failed" => Ok(PluginTaskStatus::Failed),
        "canceled" => Ok(PluginTaskStatus::Canceled),
        "timed_out" => Ok(PluginTaskStatus::TimedOut),
        _ => Err(anyhow!("unknown plugin task status: {value}")),
    }
}
