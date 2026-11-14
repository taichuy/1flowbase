use std::sync::Arc;

use control_plane::{
    plugin_management::BackupArtifactInventoryError,
    ports::{BackupRepository, BackupSetCatalogEntry},
    system_backup::SystemBackupServiceError,
    system_recovery::{
        ConfirmedRecoveryIntent, OfflineRecoveryHandoffReady, RecoveryCoordinatorError,
        RecoveryPlan, SystemMaintenanceSnapshot,
    },
};
use domain::{
    BackupIncompatibility, BackupJobId, BackupJobState, BackupJournalEvent, BackupSetId,
    RecoveryJobId, SealedBackupManifest,
};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use uuid::Uuid;

use crate::system_backup::local_repository::BackupVerificationReceipt;

use control_plane_contracts::system_backup::selective::{
    SelectiveBackupCategory, SelectiveBackupSelection, SELECTIVE_BACKUP_CONTENT_TYPE,
};

pub struct SystemBackupDetail {
    pub sealed: SealedBackupManifest,
    pub compatibility_failures: Vec<BackupIncompatibility>,
    pub verification: Option<BackupVerificationReceipt>,
    pub creation_journal: Vec<BackupJournalEvent>,
    pub recovery_history: Vec<BackupJournalEvent>,
}
#[derive(Debug, Clone, Copy)]
pub struct QueuedSystemBackup {
    pub backup_job_id: BackupJobId,
    pub backup_set_id: BackupSetId,
}
#[derive(Debug, Clone)]
pub struct SystemBackupJobStatus {
    pub backup_job_id: BackupJobId,
    pub backup_set_id: BackupSetId,
    pub state: BackupJobState,
    pub failure_code: Option<String>,
    pub sealed_components: u64,
}
#[derive(Debug, Error)]
pub enum SystemBackupRuntimeError {
    #[error("selective backup operation failed")]
    Selective(#[from] anyhow::Error),
    #[error("system backup host configuration is invalid")]
    Configuration,
    #[error("system backup repository is unavailable")]
    Repository,
    #[error("system backup key provider is unavailable")]
    Key,
    #[error("PostgreSQL backup toolchain is unavailable")]
    PostgreSqlToolchainUnavailable,
    #[error("PostgreSQL backup preflight failed")]
    PostgreSqlPreflight,
    #[error("system backup source inventory is unavailable")]
    SourceInventory,
    #[error("system backup source inventory is invalid")]
    SourceInventoryInvalid(#[source] BackupArtifactInventoryError),
    #[error("system maintenance is already active")]
    MaintenanceBusy,
    #[error("system writes did not drain before backup")]
    Drain,
    #[error("system backup operation failed")]
    Service(#[from] SystemBackupServiceError),
    #[error("system recovery preparation failed")]
    Recovery(#[from] RecoveryCoordinatorError),
}
pub struct SystemBackupRuntime(Arc<dyn SystemBackupPort>);
impl SystemBackupRuntime {
    pub fn from_ready(port: Arc<dyn SystemBackupPort>) -> Self {
        Self(port)
    }
    pub fn is_selective(sealed: &SealedBackupManifest) -> bool {
        sealed.manifest().components().iter().any(|component| {
            component.kind == domain::BackupComponentKind::PostgreSql
                && component.content_type == SELECTIVE_BACKUP_CONTENT_TYPE
        })
    }
    pub async fn create(
        &self,
        actor_user_id: Uuid,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError> {
        self.0.create(actor_user_id).await
    }
    pub async fn create_with_password(
        &self,
        actor_user_id: Uuid,
        backup_password: Option<String>,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError> {
        self.0
            .create_with_password(actor_user_id, backup_password)
            .await
    }
    pub async fn queue_manual_backup(
        self: &Arc<Self>,
        actor_user_id: Uuid,
        backup_password: Option<String>,
    ) -> Result<QueuedSystemBackup, SystemBackupRuntimeError> {
        self.0
            .clone()
            .queue_manual_backup(actor_user_id, backup_password)
            .await
    }
    pub async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, SystemBackupRuntimeError> {
        self.0.list().await
    }
    pub async fn backup_job_status(
        &self,
        backup_job_id: BackupJobId,
    ) -> Result<Option<SystemBackupJobStatus>, SystemBackupRuntimeError> {
        self.0.backup_job_status(backup_job_id).await
    }
    pub async fn get(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError> {
        self.0.get(backup_set_id).await
    }
    pub async fn detail(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SystemBackupDetail, SystemBackupRuntimeError> {
        self.0.detail(backup_set_id).await
    }
    pub async fn verify(&self, backup_set_id: BackupSetId) -> Result<(), SystemBackupRuntimeError> {
        self.0.verify(backup_set_id).await
    }
    pub async fn verify_with_password(
        &self,
        backup_set_id: BackupSetId,
        password: Option<&str>,
    ) -> Result<(), SystemBackupRuntimeError> {
        self.0.verify_with_password(backup_set_id, password).await
    }
    pub async fn delete(&self, backup_set_id: BackupSetId) -> Result<(), SystemBackupRuntimeError> {
        self.0.delete(backup_set_id).await
    }
    pub async fn download<W>(
        &self,
        backup_set_id: BackupSetId,
        mut destination: W,
    ) -> Result<(), SystemBackupRuntimeError>
    where
        W: AsyncWrite + Unpin + Send,
    {
        self.0.download(backup_set_id, &mut destination).await
    }
    pub async fn import<R>(
        &self,
        mut source: R,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError>
    where
        R: AsyncRead + Unpin + Send,
    {
        self.0.import(&mut source).await
    }
    pub async fn import_with_password<R>(
        &self,
        mut source: R,
        password: Option<&str>,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError>
    where
        R: AsyncRead + Unpin + Send,
    {
        self.0.import_with_password(&mut source, password).await
    }
    pub async fn preflight(&self, backup_set_id: BackupSetId) -> RecoveryPlan {
        self.0.preflight(backup_set_id).await
    }
    pub async fn preflight_with_password(
        &self,
        backup_set_id: BackupSetId,
        password: Option<&str>,
    ) -> RecoveryPlan {
        self.0
            .preflight_with_password(backup_set_id, password)
            .await
    }
    pub async fn prepare_recovery(
        &self,
        intent: ConfirmedRecoveryIntent,
        target_backup_password: Option<String>,
    ) -> Result<OfflineRecoveryHandoffReady, SystemBackupRuntimeError> {
        self.0
            .prepare_recovery(intent, target_backup_password)
            .await
    }
    pub fn reserve_recovery_maintenance(
        &self,
        recovery_job_id: RecoveryJobId,
    ) -> Result<control_plane::system_recovery::SystemMaintenanceLease, SystemBackupRuntimeError>
    {
        self.0.reserve_recovery_maintenance(recovery_job_id)
    }
    pub async fn prepare_recovery_with_maintenance_lease(
        &self,
        intent: ConfirmedRecoveryIntent,
        target_backup_password: Option<String>,
        lease: control_plane::system_recovery::SystemMaintenanceLease,
    ) -> Result<OfflineRecoveryHandoffReady, SystemBackupRuntimeError> {
        self.0
            .prepare_recovery_with_maintenance_lease(intent, target_backup_password, lease)
            .await
    }
    pub fn active_recovery(&self) -> Option<OfflineRecoveryHandoffReady> {
        self.0.active_recovery()
    }
    pub fn maintenance_status(&self) -> SystemMaintenanceSnapshot {
        self.0.maintenance_status()
    }
    pub async fn recovery_journal(
        &self,
        recovery_job_id: RecoveryJobId,
    ) -> Result<Vec<BackupJournalEvent>, SystemBackupRuntimeError> {
        self.0.recovery_journal(recovery_job_id).await
    }
    pub async fn selective_catalog(
        &self,
    ) -> Result<Vec<SelectiveBackupCategory>, SystemBackupRuntimeError> {
        self.0.selective_catalog().await
    }
    pub async fn queue_selected_backup(
        self: &Arc<Self>,
        actor_user_id: Uuid,
        backup_password: Option<String>,
        selection: Vec<SelectiveBackupSelection>,
        include_file_bytes: bool,
    ) -> Result<QueuedSystemBackup, SystemBackupRuntimeError> {
        self.0
            .clone()
            .queue_selected_backup(
                actor_user_id,
                backup_password,
                selection,
                include_file_bytes,
            )
            .await
    }
    pub async fn restore_selected_backup(
        self: &Arc<Self>,
        intent: ConfirmedRecoveryIntent,
        password: Option<String>,
        confirm_missing_plugins: bool,
    ) -> Result<(), SystemBackupRuntimeError> {
        self.0
            .clone()
            .restore_selected_backup(intent, password, confirm_missing_plugins)
            .await
    }
}
#[async_trait::async_trait]
pub trait SystemBackupPort: Send + Sync {
    async fn create(
        &self,
        actor_user_id: Uuid,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError>;
    async fn create_with_password(
        &self,
        actor_user_id: Uuid,
        backup_password: Option<String>,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError>;
    async fn queue_manual_backup(
        self: Arc<Self>,
        actor_user_id: Uuid,
        backup_password: Option<String>,
    ) -> Result<QueuedSystemBackup, SystemBackupRuntimeError>;
    async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, SystemBackupRuntimeError>;
    async fn backup_job_status(
        &self,
        backup_job_id: BackupJobId,
    ) -> Result<Option<SystemBackupJobStatus>, SystemBackupRuntimeError>;
    async fn get(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError>;
    async fn detail(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SystemBackupDetail, SystemBackupRuntimeError>;
    async fn verify(&self, backup_set_id: BackupSetId) -> Result<(), SystemBackupRuntimeError>;
    async fn verify_with_password(
        &self,
        backup_set_id: BackupSetId,
        password: Option<&str>,
    ) -> Result<(), SystemBackupRuntimeError>;
    async fn delete(&self, backup_set_id: BackupSetId) -> Result<(), SystemBackupRuntimeError>;
    async fn download(
        &self,
        backup_set_id: BackupSetId,
        destination: &mut (dyn AsyncWrite + Unpin + Send),
    ) -> Result<(), SystemBackupRuntimeError>;
    async fn import(
        &self,
        source: &mut (dyn AsyncRead + Unpin + Send),
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError>;
    async fn import_with_password(
        &self,
        source: &mut (dyn AsyncRead + Unpin + Send),
        password: Option<&str>,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError>;
    async fn preflight(&self, backup_set_id: BackupSetId) -> RecoveryPlan;
    async fn preflight_with_password(
        &self,
        backup_set_id: BackupSetId,
        password: Option<&str>,
    ) -> RecoveryPlan;
    async fn prepare_recovery(
        &self,
        intent: ConfirmedRecoveryIntent,
        target_backup_password: Option<String>,
    ) -> Result<OfflineRecoveryHandoffReady, SystemBackupRuntimeError>;
    fn reserve_recovery_maintenance(
        &self,
        recovery_job_id: RecoveryJobId,
    ) -> Result<control_plane::system_recovery::SystemMaintenanceLease, SystemBackupRuntimeError>;
    async fn prepare_recovery_with_maintenance_lease(
        &self,
        intent: ConfirmedRecoveryIntent,
        target_backup_password: Option<String>,
        lease: control_plane::system_recovery::SystemMaintenanceLease,
    ) -> Result<OfflineRecoveryHandoffReady, SystemBackupRuntimeError>;
    fn active_recovery(&self) -> Option<OfflineRecoveryHandoffReady>;
    fn maintenance_status(&self) -> SystemMaintenanceSnapshot;
    async fn recovery_journal(
        &self,
        recovery_job_id: RecoveryJobId,
    ) -> Result<Vec<BackupJournalEvent>, SystemBackupRuntimeError>;
    async fn selective_catalog(
        &self,
    ) -> Result<Vec<SelectiveBackupCategory>, SystemBackupRuntimeError>;
    async fn queue_selected_backup(
        self: Arc<Self>,
        actor_user_id: Uuid,
        backup_password: Option<String>,
        selection: Vec<SelectiveBackupSelection>,
        include_file_bytes: bool,
    ) -> Result<QueuedSystemBackup, SystemBackupRuntimeError>;
    async fn restore_selected_backup(
        self: Arc<Self>,
        intent: ConfirmedRecoveryIntent,
        password: Option<String>,
        confirm_missing_plugins: bool,
    ) -> Result<(), SystemBackupRuntimeError>;
}
