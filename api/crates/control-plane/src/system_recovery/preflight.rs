use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    strict_backup_compatibility, BackupCompatibilityTarget, BackupComponentDisposition,
    BackupComponentKind, BackupIncompatibility, BackupSetId, SealedBackupManifest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::system_backup::SystemBackupService;

const RECOVERY_SPACE_MULTIPLIER: u64 = 3;
const RECOVERY_SPACE_RESERVE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryActiveWork {
    pub owner_id: String,
    pub active_count: u64,
    pub drainable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryTargetSnapshot {
    pub compatibility: BackupCompatibilityTarget,
    pub available_space_bytes: u64,
    pub postgres_toolchain_compatible: bool,
    pub postgres_restore_privileges: bool,
    pub target_roots_separated: bool,
    pub active_work: Vec<RecoveryActiveWork>,
}

#[async_trait]
pub trait RecoveryTargetProbe: Send + Sync {
    async fn snapshot(&self) -> Result<RecoveryTargetSnapshot, RecoveryPreflightError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPreflightFailure {
    TargetProbe,
    BackupUnavailable,
    BackupIntegrity,
    FormatVersion,
    MigrationHead,
    MasterKeyFingerprint,
    ComponentInventory,
    InsufficientSpace,
    PostgreSqlToolchain,
    PostgreSqlPrivileges,
    TargetRootOverlap,
    NonDrainableWork,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryImpactPreview {
    pub database_replaced: bool,
    pub business_object_count: u64,
    pub extension_artifact_count: u64,
    pub mcp_artifact_count: u64,
    pub active_work: Vec<RecoveryActiveWork>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryPlan {
    pub backup_set_id: BackupSetId,
    pub required_space_bytes: u64,
    pub available_space_bytes: u64,
    pub impact: RecoveryImpactPreview,
    pub failures: Vec<RecoveryPreflightFailure>,
}

impl RecoveryPlan {
    pub fn is_compatible(&self) -> bool {
        self.failures.is_empty()
    }
}

#[derive(Debug, Error)]
pub enum RecoveryPreflightError {
    #[error("recovery target probe failed")]
    TargetProbe,
}

pub struct RecoveryPreflightService {
    backups: Arc<SystemBackupService>,
    target: Arc<dyn RecoveryTargetProbe>,
}

impl RecoveryPreflightService {
    pub fn new(backups: Arc<SystemBackupService>, target: Arc<dyn RecoveryTargetProbe>) -> Self {
        Self { backups, target }
    }

    /// Produces a deterministic, read-only plan. No maintenance lease, safety backup or recovery
    /// journal entry is created until a compatible plan is explicitly confirmed.
    pub async fn plan(&self, backup_set_id: BackupSetId) -> RecoveryPlan {
        self.plan_with_password(backup_set_id, None).await
    }

    /// Password-protected backups require their password even for a read-only integrity check.
    /// The password is caller-owned and is never retained by the plan or journal.
    pub async fn plan_with_password(
        &self,
        backup_set_id: BackupSetId,
        password: Option<&str>,
    ) -> RecoveryPlan {
        let target = match self.target.snapshot().await {
            Ok(target) => target,
            Err(_) => return unavailable_plan(backup_set_id),
        };
        let sealed = match self.backups.get(backup_set_id).await {
            Ok(sealed) => sealed,
            Err(_) => {
                return failed_plan(
                    backup_set_id,
                    &target,
                    RecoveryPreflightFailure::BackupUnavailable,
                )
            }
        };
        let mut failures = Vec::new();
        if self
            .backups
            .verify_with_password(backup_set_id, password)
            .await
            .is_err()
        {
            failures.push(RecoveryPreflightFailure::BackupIntegrity);
        }
        if let Err(reasons) = strict_backup_compatibility(sealed.manifest(), &target.compatibility)
        {
            failures.extend(reasons.into_iter().map(map_incompatibility));
        }
        if !valid_component_inventory(&sealed) {
            failures.push(RecoveryPreflightFailure::ComponentInventory);
        }
        let required_space_bytes = required_space(sealed.manifest().total_size_bytes());
        if target.available_space_bytes < required_space_bytes {
            failures.push(RecoveryPreflightFailure::InsufficientSpace);
        }
        if !target.postgres_toolchain_compatible {
            failures.push(RecoveryPreflightFailure::PostgreSqlToolchain);
        }
        if !target.postgres_restore_privileges {
            failures.push(RecoveryPreflightFailure::PostgreSqlPrivileges);
        }
        if !target.target_roots_separated {
            failures.push(RecoveryPreflightFailure::TargetRootOverlap);
        }
        if target
            .active_work
            .iter()
            .any(|work| work.active_count > 0 && !work.drainable)
        {
            failures.push(RecoveryPreflightFailure::NonDrainableWork);
        }
        failures.sort();
        failures.dedup();
        RecoveryPlan {
            backup_set_id,
            required_space_bytes,
            available_space_bytes: target.available_space_bytes,
            impact: impact_preview(&sealed, target.active_work),
            failures,
        }
    }
}

fn valid_component_inventory(sealed: &SealedBackupManifest) -> bool {
    let postgres = sealed
        .manifest()
        .components()
        .iter()
        .filter(|component| component.kind == BackupComponentKind::PostgreSql)
        .count();
    postgres == 1
        && sealed.manifest().components().iter().all(|component| {
            component.disposition != BackupComponentDisposition::Embedded
                || component.kind == BackupComponentKind::BusinessObject
                || component.size_bytes > 0
        })
}

fn required_space(backup_size: u64) -> u64 {
    backup_size
        .saturating_mul(RECOVERY_SPACE_MULTIPLIER)
        .saturating_add(RECOVERY_SPACE_RESERVE_BYTES)
}

fn impact_preview(
    sealed: &SealedBackupManifest,
    active_work: Vec<RecoveryActiveWork>,
) -> RecoveryImpactPreview {
    let count = |kind| {
        sealed
            .manifest()
            .components()
            .iter()
            .filter(|component| component.kind == kind)
            .count() as u64
    };
    RecoveryImpactPreview {
        database_replaced: true,
        business_object_count: count(BackupComponentKind::BusinessObject),
        extension_artifact_count: count(BackupComponentKind::ExtensionArtifact),
        mcp_artifact_count: count(BackupComponentKind::McpArtifact),
        active_work,
    }
}

fn map_incompatibility(value: BackupIncompatibility) -> RecoveryPreflightFailure {
    match value {
        BackupIncompatibility::FormatVersion => RecoveryPreflightFailure::FormatVersion,
        BackupIncompatibility::MigrationHead => RecoveryPreflightFailure::MigrationHead,
        BackupIncompatibility::MasterKeyFingerprint => {
            RecoveryPreflightFailure::MasterKeyFingerprint
        }
    }
}

fn failed_plan(
    backup_set_id: BackupSetId,
    target: &RecoveryTargetSnapshot,
    failure: RecoveryPreflightFailure,
) -> RecoveryPlan {
    RecoveryPlan {
        backup_set_id,
        required_space_bytes: 0,
        available_space_bytes: target.available_space_bytes,
        impact: RecoveryImpactPreview {
            database_replaced: true,
            business_object_count: 0,
            extension_artifact_count: 0,
            mcp_artifact_count: 0,
            active_work: target.active_work.clone(),
        },
        failures: vec![failure],
    }
}

fn unavailable_plan(backup_set_id: BackupSetId) -> RecoveryPlan {
    RecoveryPlan {
        backup_set_id,
        required_space_bytes: 0,
        available_space_bytes: 0,
        impact: RecoveryImpactPreview {
            database_replaced: true,
            business_object_count: 0,
            extension_artifact_count: 0,
            mcp_artifact_count: 0,
            active_work: Vec::new(),
        },
        failures: vec![RecoveryPreflightFailure::TargetProbe],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_budget_covers_staging_safety_backup_and_reserve() {
        assert_eq!(
            required_space(1024),
            3 * 1024 + RECOVERY_SPACE_RESERVE_BYTES
        );
        assert_eq!(required_space(u64::MAX), u64::MAX);
    }

    #[test]
    fn incompatible_plan_is_not_executable() {
        let plan = unavailable_plan(BackupSetId::new());
        assert!(!plan.is_compatible());
        assert_eq!(plan.failures, vec![RecoveryPreflightFailure::TargetProbe]);
    }
}
