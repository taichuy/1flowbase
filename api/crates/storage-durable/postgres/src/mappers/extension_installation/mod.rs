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
    pub node_id: String,
    pub source: String,
    pub trust: String,
    pub local_path: String,
    pub checksum: String,
    pub signature_status: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub warnings: serde_json::Value,
    pub receipt: serde_json::Value,
    pub status: String,
    pub installed_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
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
            node_id: row.node_id,
        },
        source: row.source,
        trust: row.trust,
        local_path: row.local_path,
        checksum: row.checksum,
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
        status: domain::ExtensionInstallationStatus::parse(&row.status)
            .ok_or_else(|| anyhow!("unknown extension installation status: {}", row.status))?,
        installed_by: row.installed_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
