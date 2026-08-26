use async_trait::async_trait;
use domain::{BackupComponent, BackupSetId, MigrationHead, RecoveryJobId};
use thiserror::Error;

use crate::ports::BackupComponentReader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryStepContext {
    pub recovery_job_id: RecoveryJobId,
    pub backup_set_id: BackupSetId,
    pub migration_head: MigrationHead,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum RecoveryStepTargetError {
    #[error("recovery target does not match the backup component inventory")]
    InvalidTarget,
    #[error("recovery target staging failed")]
    Staging,
    #[error("recovery target verification failed")]
    Integrity,
    #[error("recovery target promotion failed")]
    Promotion,
    #[error("recovery target compensation failed")]
    Compensation,
    #[error("recovery target is unavailable")]
    Unavailable,
}

/// A single offline restore step with durable staging and compensating rollback.
///
/// Implementations must not depend on Axum state or the primary application pool. `begin` and
/// `stage_component` may be repeated after interruption. `promote` must either leave the previous
/// target recoverable or return an error that `rollback` can compensate.
#[async_trait]
pub trait RecoveryStepTarget: Send + Sync {
    async fn begin(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError>;

    async fn stage_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
        plaintext: BackupComponentReader,
    ) -> Result<(), RecoveryStepTargetError>;

    /// Verifies that a rebuildable identity-only artifact can be reconstructed by the local
    /// installation/catalog inventory. This method must not download a missing artifact.
    async fn stage_identity(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError>;

    async fn promote(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError>;

    async fn rollback(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError>;

    /// Removes staging and rollback material only after post-restore health has passed.
    async fn finalize(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError>;
}
