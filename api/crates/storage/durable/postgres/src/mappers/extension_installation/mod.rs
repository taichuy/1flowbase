use anyhow::{anyhow, Result};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub struct StoredExtensionInstallationRow {
    pub id: Uuid,
    pub category: String,
    pub organization: String,
    pub artifact_id: String,
    pub artifact_version: String,
    pub source_kind: String,
    pub trust_level: String,
    pub expected_checksum: Option<String>,
    pub signature_status: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub warnings: serde_json::Value,
    pub receipt: serde_json::Value,
    pub application_action: String,
    pub is_system_reserved: bool,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub node_id: String,
    pub local_path: Option<String>,
    pub local_checksum: Option<String>,
    pub status: String,
    pub is_current: bool,
}

pub fn to_extension_installation_record(
    row: StoredExtensionInstallationRow,
) -> Result<domain::ExtensionInstallationRecord> {
    Ok(domain::ExtensionInstallationRecord {
        id: row.id,
        identity: domain::ExtensionInstallationIdentity {
            category: domain::ExtensionCategory::parse(&row.category)
                .ok_or_else(|| anyhow!("unknown extension category: {}", row.category))?,
            organization: row.organization,
            artifact_id: row.artifact_id,
            version: row.artifact_version,
        },
        source_kind: row.source_kind,
        trust_level: row.trust_level,
        expected_checksum: row.expected_checksum,
        signature_status: domain::ExtensionSignatureStatus::parse(&row.signature_status)
            .ok_or_else(|| {
                anyhow!(
                    "unknown extension signature status: {}",
                    row.signature_status
                )
            })?,
        signature_algorithm: row.signature_algorithm,
        signing_key_id: row.signing_key_id,
        warnings: serde_json::from_value(row.warnings)?,
        receipt: row.receipt,
        application_action: domain::ExtensionApplicationAction::parse(&row.application_action)
            .ok_or_else(|| {
                anyhow!(
                    "unknown extension application action: {}",
                    row.application_action
                )
            })?,
        is_system_reserved: row.is_system_reserved,
        node_id: row.node_id,
        local_path: row.local_path,
        local_checksum: row.local_checksum,
        status: domain::ExtensionInstallationStatus::parse(&row.status)
            .ok_or_else(|| anyhow!("unknown extension installation status: {}", row.status))?,
        is_current: row.is_current,
        created_by: row.created_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
