use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{
    ApplicationBuild, BackupManifest, KeyFingerprint, MigrationHead,
    LEGACY_SYSTEM_BACKUP_FORMAT_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupCompatibilityTarget {
    pub format_version: u32,
    /// The target binary identity is retained for diagnostics; it is not a restore gate.
    pub application_build: ApplicationBuild,
    pub migration_head: MigrationHead,
    /// Every source migration state that the target binary can migrate forward from.
    pub supported_source_migration_heads: BTreeSet<MigrationHead>,
    pub master_key_fingerprint: KeyFingerprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupIncompatibility {
    FormatVersion,
    MigrationHead,
    MasterKeyFingerprint,
}

pub fn strict_backup_compatibility(
    manifest: &BackupManifest,
    target: &BackupCompatibilityTarget,
) -> Result<(), Vec<BackupIncompatibility>> {
    let mut reasons = Vec::new();
    if manifest.format_version() != target.format_version
        && manifest.format_version() != LEGACY_SYSTEM_BACKUP_FORMAT_VERSION
    {
        reasons.push(BackupIncompatibility::FormatVersion);
    }
    if !target
        .supported_source_migration_heads
        .contains(manifest.migration_head())
    {
        reasons.push(BackupIncompatibility::MigrationHead);
    }
    // A portable backup carries the source deployment key material inside the backup contract.
    // The target master key is therefore a deployment bootstrap concern, not a compatibility gate.
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(reasons)
    }
}
