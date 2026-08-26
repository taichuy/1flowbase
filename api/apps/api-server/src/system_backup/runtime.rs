use std::{path::PathBuf, sync::Arc};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use control_plane::{
    file_management::BusinessObjectBackupExporter,
    plugin_management::{
        load_backup_artifact_sources, BackupArtifactInventoryError, BackupArtifactSourceLoadError,
    },
    ports::{BackupRepository, BackupSetCatalogEntry},
    system_backup::{
        BackupComponentSource, CreateSystemBackupCommand, SystemBackupService,
        SystemBackupServiceError,
    },
    system_recovery::{
        ConfirmedRecoveryIntent, OfflineRecoveryHandoffReady, PrepareRecoveryCommand,
        RecoveryCoordinator, RecoveryCoordinatorError, RecoveryPlan, RecoveryPreflightService,
        SystemMaintenance, SystemMaintenanceOperation, SystemMaintenancePhase,
        SystemMaintenanceSnapshot,
    },
};
use domain::{
    strict_backup_compatibility, ApplicationBuild, BackupIncompatibility, BackupJobId,
    BackupJobState, BackupJournalEvent, BackupJournalEventKind, BackupJournalSubject, BackupSetId,
    KeyFingerprint, RecoveryJobId, SealedBackupManifest,
};
use sha2::{Digest, Sha256};
use storage_durable_postgres::{MainDurableStore, PostgreSqlToolchain};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use uuid::Uuid;

use crate::{
    config::ApiConfig,
    system_backup::{
        discover_postgres_toolchain, local_repository::BackupVerificationReceipt,
        EnvironmentBackupKeyProvider, LocalBackupRepository,
    },
    system_recovery::ApiRecoveryTargetProbe,
};

/// Host-owned assembly for the system-backup contract.
///
/// Repository, key material and component-source dependencies remain private so HTTP routes can
/// only invoke complete backup operations rather than assembling partial manifests themselves.
pub struct SystemBackupRuntime {
    service: Arc<SystemBackupService>,
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    database_url: String,
    api_node_id: String,
    application_build: ApplicationBuild,
    master_key_fingerprint: KeyFingerprint,
    portable_source_master_key_base64: String,
    postgres_toolchain: PostgreSqlToolchain,
    preflight: Arc<RecoveryPreflightService>,
    recovery: Arc<RecoveryCoordinator>,
    maintenance: Arc<SystemMaintenance>,
    repository: Arc<LocalBackupRepository>,
}

pub(crate) struct SystemBackupDetail {
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

impl SystemBackupRuntime {
    pub async fn open(
        store: MainDurableStore,
        file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
        maintenance: Arc<SystemMaintenance>,
        config: &ApiConfig,
    ) -> Result<Self, SystemBackupRuntimeError> {
        let postgres_toolchain = discover_postgres_toolchain()
            .await
            .map_err(|_| SystemBackupRuntimeError::PostgreSqlToolchainUnavailable)?;
        Self::open_with_postgres_toolchain(
            store,
            file_storage_registry,
            maintenance,
            config,
            postgres_toolchain,
        )
        .await
    }

    pub(crate) async fn open_with_postgres_toolchain(
        store: MainDurableStore,
        file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
        maintenance: Arc<SystemMaintenance>,
        config: &ApiConfig,
        postgres_toolchain: PostgreSqlToolchain,
    ) -> Result<Self, SystemBackupRuntimeError> {
        let protected_roots = protected_data_roots(config);
        let target_roots_separated = protected_roots.iter().enumerate().all(|(index, root)| {
            protected_roots
                .iter()
                .skip(index + 1)
                .all(|other| !paths_overlap(root, other))
        });
        for root in &protected_roots {
            tokio::fs::create_dir_all(root)
                .await
                .map_err(|_| SystemBackupRuntimeError::Configuration)?;
        }
        let repository = Arc::new(
            LocalBackupRepository::open(&config.system_backup_repository_root, &protected_roots)
                .await
                .map_err(|_| SystemBackupRuntimeError::Repository)?,
        );
        let key_provider = Arc::new(
            EnvironmentBackupKeyProvider::from_master_key_with_legacy(
                &config.provider_secret_master_key,
                config.legacy_system_backup_key_base64.as_deref(),
            )
            .map_err(|_| SystemBackupRuntimeError::Key)?,
        );
        postgres_toolchain
            .verify_server_compatibility(store.pool())
            .await
            .map_err(|_| SystemBackupRuntimeError::PostgreSqlPreflight)?;
        let application_build = config.application_build.clone();
        let master_key_fingerprint = KeyFingerprint::try_from(format!(
            "{:x}",
            Sha256::digest(config.provider_secret_master_key.as_bytes())
        ))
        .map_err(|_| SystemBackupRuntimeError::Configuration)?;

        let service = Arc::new(SystemBackupService::new(repository.clone(), key_provider));
        let target_probe = Arc::new(
            ApiRecoveryTargetProbe::new(
                store.pool().clone(),
                application_build.clone(),
                &config.provider_secret_master_key,
                &config.system_backup_repository_root,
                target_roots_separated,
                maintenance.clone(),
                postgres_toolchain.clone(),
            )
            .map_err(|_| SystemBackupRuntimeError::Configuration)?,
        );
        let preflight = Arc::new(RecoveryPreflightService::new(service.clone(), target_probe));
        let recovery = Arc::new(RecoveryCoordinator::new(
            preflight.clone(),
            service.clone(),
            repository.clone(),
            maintenance.clone(),
        ));

        Ok(Self {
            service,
            store,
            file_storage_registry,
            database_url: config.database_url.clone(),
            api_node_id: config.api_node_id.clone(),
            application_build,
            master_key_fingerprint,
            portable_source_master_key_base64: STANDARD.encode(&config.provider_secret_master_key),
            postgres_toolchain,
            preflight,
            recovery,
            maintenance,
            repository,
        })
    }

    pub async fn create(
        &self,
        actor_user_id: Uuid,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError> {
        self.create_with_password(actor_user_id, None).await
    }

    pub async fn create_with_password(
        &self,
        actor_user_id: Uuid,
        backup_password: Option<String>,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError> {
        let backup_job_id = BackupJobId::new();
        let lease = self
            .maintenance
            .begin(
                SystemMaintenanceOperation::Backup(backup_job_id),
                time::OffsetDateTime::now_utc(),
            )
            .map_err(|_| SystemBackupRuntimeError::MaintenanceBusy)?;
        lease
            .wait_for_drain(std::time::Duration::from_secs(30))
            .await
            .map_err(|_| SystemBackupRuntimeError::Drain)?;

        let result = self
            .create_under_existing_maintenance(backup_job_id, actor_user_id, backup_password)
            .await;
        lease.finish();
        result
    }

    /// Reserves the shared maintenance owner and persists its queued journal checkpoint before
    /// dispatching the slow fenced backup work onto the process runtime.
    pub async fn queue_manual_backup(
        self: &Arc<Self>,
        actor_user_id: Uuid,
        backup_password: Option<String>,
    ) -> Result<QueuedSystemBackup, SystemBackupRuntimeError> {
        let queued = QueuedSystemBackup {
            backup_job_id: BackupJobId::new(),
            backup_set_id: BackupSetId::new(),
        };
        let lease = self
            .maintenance
            .begin(
                SystemMaintenanceOperation::Backup(queued.backup_job_id),
                time::OffsetDateTime::now_utc(),
            )
            .map_err(|_| SystemBackupRuntimeError::MaintenanceBusy)?;
        if let Err(error) = self
            .service
            .queue_manual_backup(queued.backup_job_id, queued.backup_set_id, actor_user_id)
            .await
        {
            lease.finish();
            return Err(error.into());
        }

        let runtime = self.clone();
        tokio::spawn(async move {
            runtime
                .run_queued_manual_backup(lease, queued, actor_user_id, backup_password)
                .await;
        });
        Ok(queued)
    }

    pub async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, SystemBackupRuntimeError> {
        self.service.list().await.map_err(Into::into)
    }

    pub async fn backup_job_status(
        &self,
        backup_job_id: BackupJobId,
    ) -> Result<Option<SystemBackupJobStatus>, SystemBackupRuntimeError> {
        let events = self
            .repository
            .read_journal(BackupJournalSubject::Backup(backup_job_id))
            .await
            .map_err(|_| SystemBackupRuntimeError::Repository)?;
        let Some(first) = events.first() else {
            return Ok(None);
        };
        let backup_set_id = first.backup_set_id;
        let mut state = BackupJobState::Queued;
        let mut failure_code = None;
        let mut sealed_components = 0_u64;
        for event in events {
            match event.event {
                BackupJournalEventKind::BackupStateChanged { state: next } => state = next,
                BackupJournalEventKind::TerminalFailure { code } => failure_code = Some(code),
                BackupJournalEventKind::ComponentSealed { .. } => {
                    sealed_components = sealed_components.saturating_add(1)
                }
                _ => {}
            }
        }
        state = observed_backup_job_state(state, backup_job_id, &self.maintenance.snapshot());
        Ok(Some(SystemBackupJobStatus {
            backup_job_id,
            backup_set_id,
            state,
            failure_code,
            sealed_components,
        }))
    }

    pub async fn get(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError> {
        self.service.get(backup_set_id).await.map_err(Into::into)
    }

    pub(crate) async fn detail(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SystemBackupDetail, SystemBackupRuntimeError> {
        let sealed = self.service.get(backup_set_id).await?;
        let migration_head = storage_durable_postgres::current_migration_head()
            .map_err(|_| SystemBackupRuntimeError::PostgreSqlPreflight)?;
        let supported_source_migration_heads = storage_durable_postgres::supported_migration_heads()
            .map_err(|_| SystemBackupRuntimeError::PostgreSqlPreflight)?;
        let target = domain::BackupCompatibilityTarget {
            format_version: domain::SYSTEM_BACKUP_FORMAT_VERSION,
            application_build: self.application_build.clone(),
            migration_head,
            supported_source_migration_heads,
            master_key_fingerprint: self.master_key_fingerprint.clone(),
        };
        let compatibility_failures = strict_backup_compatibility(sealed.manifest(), &target)
            .err()
            .unwrap_or_default();
        let verification = self
            .repository
            .read_verification(backup_set_id)
            .await
            .map_err(|_| SystemBackupRuntimeError::Repository)?;
        let (creation_journal, recovery_history) = self
            .repository
            .events_for_backup_set(backup_set_id)
            .await
            .map_err(|_| SystemBackupRuntimeError::Repository)?;
        Ok(SystemBackupDetail {
            sealed,
            compatibility_failures,
            verification,
            creation_journal,
            recovery_history,
        })
    }

    pub async fn verify(&self, backup_set_id: BackupSetId) -> Result<(), SystemBackupRuntimeError> {
        self.verify_with_password(backup_set_id, None).await
    }

    pub async fn verify_with_password(
        &self,
        backup_set_id: BackupSetId,
        password: Option<&str>,
    ) -> Result<(), SystemBackupRuntimeError> {
        let verification = self
            .service
            .verify_with_password(backup_set_id, password)
            .await;
        let verified = verification.is_ok();
        let receipt = self
            .repository
            .record_verification(backup_set_id, verified)
            .await;
        match (verification, receipt) {
            (Err(error), _) => Err(error.into()),
            (Ok(()), Err(_)) => Err(SystemBackupRuntimeError::Repository),
            (Ok(()), Ok(_)) => Ok(()),
        }
    }

    pub async fn delete(&self, backup_set_id: BackupSetId) -> Result<(), SystemBackupRuntimeError> {
        self.service.delete(backup_set_id).await.map_err(Into::into)
    }

    pub async fn download<W>(
        &self,
        backup_set_id: BackupSetId,
        destination: W,
    ) -> Result<(), SystemBackupRuntimeError>
    where
        W: AsyncWrite + Unpin + Send,
    {
        self.service
            .download_portable(
                backup_set_id,
                &self.portable_source_master_key_base64,
                destination,
            )
            .await
            .map_err(Into::into)
    }

    pub async fn import<R>(
        &self,
        source: R,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError>
    where
        R: AsyncRead + Unpin + Send,
    {
        self.import_with_password(source, None).await
    }

    pub async fn import_with_password<R>(
        &self,
        source: R,
        password: Option<&str>,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError>
    where
        R: AsyncRead + Unpin + Send,
    {
        let sealed = self.service.import_with_password(source, password).await?;
        self.repository
            .record_verification(sealed.manifest().backup_set_id(), true)
            .await
            .map_err(|_| SystemBackupRuntimeError::Repository)?;
        Ok(sealed)
    }

    pub async fn preflight(&self, backup_set_id: BackupSetId) -> RecoveryPlan {
        self.preflight.plan(backup_set_id).await
    }

    pub async fn preflight_with_password(
        &self,
        backup_set_id: BackupSetId,
        password: Option<&str>,
    ) -> RecoveryPlan {
        self.preflight
            .plan_with_password(backup_set_id, password)
            .await
    }

    pub async fn prepare_recovery(
        &self,
        intent: ConfirmedRecoveryIntent,
        target_backup_password: Option<String>,
    ) -> Result<OfflineRecoveryHandoffReady, SystemBackupRuntimeError> {
        let lease = self.reserve_recovery_maintenance(intent.recovery_job_id())?;
        self.prepare_recovery_with_maintenance_lease(intent, target_backup_password, lease)
            .await
    }

    pub fn reserve_recovery_maintenance(
        &self,
        recovery_job_id: RecoveryJobId,
    ) -> Result<control_plane::system_recovery::SystemMaintenanceLease, SystemBackupRuntimeError>
    {
        self.maintenance
            .begin(
                SystemMaintenanceOperation::Recovery(recovery_job_id),
                time::OffsetDateTime::now_utc(),
            )
            .map_err(|_| SystemBackupRuntimeError::MaintenanceBusy)
    }

    pub async fn prepare_recovery_with_maintenance_lease(
        &self,
        intent: ConfirmedRecoveryIntent,
        target_backup_password: Option<String>,
        lease: control_plane::system_recovery::SystemMaintenanceLease,
    ) -> Result<OfflineRecoveryHandoffReady, SystemBackupRuntimeError> {
        let actor_user_id = intent.actor_user_id();
        let (safety_backup_command, safety_backup_sources) =
            self.backup_inputs(actor_user_id, None).await?;
        self.recovery
            .prepare_offline_handoff_with_lease(
                PrepareRecoveryCommand {
                    intent,
                    target_backup_password,
                    safety_backup_command,
                    safety_backup_sources,
                    drain_timeout: std::time::Duration::from_secs(30),
                },
                lease,
            )
            .await
            .map_err(Into::into)
    }

    pub fn active_recovery(&self) -> Option<OfflineRecoveryHandoffReady> {
        self.recovery.active_handoff()
    }

    pub fn maintenance_status(&self) -> SystemMaintenanceSnapshot {
        self.maintenance.snapshot()
    }

    pub async fn recovery_journal(
        &self,
        recovery_job_id: RecoveryJobId,
    ) -> Result<Vec<BackupJournalEvent>, SystemBackupRuntimeError> {
        self.repository
            .read_journal(BackupJournalSubject::Recovery(recovery_job_id))
            .await
            .map_err(|_| SystemBackupRuntimeError::Repository)
    }

    async fn backup_inputs(
        &self,
        actor_user_id: Uuid,
        backup_password: Option<String>,
    ) -> Result<
        (
            CreateSystemBackupCommand,
            Vec<Arc<dyn BackupComponentSource>>,
        ),
        SystemBackupRuntimeError,
    > {
        self.postgres_toolchain
            .verify_server_compatibility(self.store.pool())
            .await
            .map_err(|_| SystemBackupRuntimeError::PostgreSqlPreflight)?;
        let migration_head = storage_durable_postgres::migration_head(self.store.pool())
            .await
            .map_err(|_| SystemBackupRuntimeError::PostgreSqlPreflight)?;
        let mut sources: Vec<Arc<dyn BackupComponentSource>> =
            vec![Arc::new(storage_durable_postgres::PostgreSqlLogicalBackup::new(
                self.database_url.clone(),
                self.postgres_toolchain.clone(),
            ))];
        sources.extend(
            BusinessObjectBackupExporter::new(
                self.store.clone(),
                self.file_storage_registry.clone(),
            )
            .sources()
            .await
            .map_err(|_| SystemBackupRuntimeError::SourceInventory)?,
        );
        sources.extend(
            load_backup_artifact_sources(&self.store, &self.api_node_id)
                .await
                .map_err(|error| map_backup_artifact_source_error(&self.api_node_id, error))?,
        );
        Ok((
            CreateSystemBackupCommand {
                actor_user_id,
                application_build: self.application_build.clone(),
                migration_head,
                master_key_fingerprint: self.master_key_fingerprint.clone(),
                portable_source_master_key_base64: Some(
                    self.portable_source_master_key_base64.clone(),
                ),
                backup_password,
            },
            sources,
        ))
    }

    async fn create_under_existing_maintenance(
        &self,
        backup_job_id: BackupJobId,
        actor_user_id: Uuid,
        backup_password: Option<String>,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError> {
        let (command, sources) = self
            .backup_inputs(actor_user_id, backup_password.clone())
            .await?;
        let manifest = self
            .service
            .create_under_existing_maintenance(backup_job_id, command, sources)
            .await?;
        self.service
            .verify_with_password(
                manifest.manifest().backup_set_id(),
                backup_password.as_deref(),
            )
            .await?;
        self.repository
            .record_verification(manifest.manifest().backup_set_id(), true)
            .await
            .map_err(|_| SystemBackupRuntimeError::Repository)?;
        Ok(manifest)
    }

    async fn run_queued_manual_backup(
        self: Arc<Self>,
        lease: control_plane::system_recovery::SystemMaintenanceLease,
        queued: QueuedSystemBackup,
        actor_user_id: Uuid,
        backup_password: Option<String>,
    ) {
        let result = async {
            lease
                .wait_for_drain(std::time::Duration::from_secs(30))
                .await
                .map_err(|_| "maintenance_drain_failed")?;
            let (command, sources) = self
                .backup_inputs(actor_user_id, backup_password.clone())
                .await
                .map_err(|_| "backup_input_failed")?;
            let manifest = self
                .service
                .create_queued_backup_under_existing_maintenance(
                    queued.backup_job_id,
                    queued.backup_set_id,
                    command,
                    sources,
                )
                .await
                .map_err(|_| "backup_creation_failed")?;
            self.repository
                .record_verification(manifest.manifest().backup_set_id(), true)
                .await
                .map_err(|_| "backup_verification_receipt_failed")?;
            Ok::<(), &str>(())
        }
        .await;
        if let Err(failure_code) = result {
            if failure_code != "backup_creation_failed" {
                if let Err(error) = self
                    .service
                    .fail_queued_manual_backup(
                        queued.backup_job_id,
                        queued.backup_set_id,
                        actor_user_id,
                        failure_code,
                    )
                    .await
                {
                    tracing::error!(
                        backup_job_id = %queued.backup_job_id.as_uuid(),
                        error = %error,
                        "system backup could not record queued-job failure"
                    );
                }
            }
            tracing::warn!(
                backup_job_id = %queued.backup_job_id.as_uuid(),
                failure_code,
                "queued system backup failed"
            );
        }
        lease.finish();
    }
}

fn map_backup_artifact_source_error(
    node_id: &str,
    error: BackupArtifactSourceLoadError,
) -> SystemBackupRuntimeError {
    match error.into_inventory_error() {
        Ok(error) => {
            tracing::warn!(
                node_id,
                installation_id = %error.installation_id,
                artifact_identity = %error.artifact_identity,
                reason = error.reason.as_str(),
                error = %error,
                "system backup source inventory is not restorable"
            );
            SystemBackupRuntimeError::SourceInventoryInvalid(error)
        }
        Err(error) => {
            tracing::error!(
                node_id,
                error_chain = %format!("{error:#}"),
                "system backup artifact source inventory could not be loaded"
            );
            SystemBackupRuntimeError::SourceInventory
        }
    }
}

fn protected_data_roots(config: &ApiConfig) -> Vec<PathBuf> {
    let mut roots = [
        &config.business_file_local_root,
        &config.provider_install_root,
        &config.mcp_template_library_root,
        &config.host_extension_dropin_root,
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect::<Vec<_>>();
    roots.sort_by_key(|root| root.components().count());
    roots.into_iter().fold(Vec::new(), |mut protected, root| {
        if !protected.iter().any(|parent| root.starts_with(parent)) {
            protected.push(root);
        }
        protected
    })
}

fn observed_backup_job_state(
    state: BackupJobState,
    backup_job_id: BackupJobId,
    maintenance: &SystemMaintenanceSnapshot,
) -> BackupJobState {
    if state == BackupJobState::Succeeded
        && maintenance.phase != SystemMaintenancePhase::Online
        && maintenance.operation == Some(SystemMaintenanceOperation::Backup(backup_job_id))
    {
        BackupJobState::Verifying
    } else {
        state
    }
}

fn paths_overlap(left: &std::path::Path, right: &std::path::Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

#[cfg(test)]
mod tests {
    use super::observed_backup_job_state;
    use control_plane::system_recovery::{
        SystemMaintenance, SystemMaintenanceOperation, SystemMaintenancePhase,
    };
    use domain::{BackupJobId, BackupJobState};
    use std::sync::Arc;
    use time::OffsetDateTime;

    #[test]
    fn queued_backup_is_not_observed_as_succeeded_until_its_maintenance_fence_is_released() {
        let maintenance = Arc::new(SystemMaintenance::default());
        let backup_job_id = BackupJobId::new();
        let lease = maintenance
            .begin(
                SystemMaintenanceOperation::Backup(backup_job_id),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        let active = maintenance.snapshot();

        assert_eq!(
            observed_backup_job_state(BackupJobState::Succeeded, backup_job_id, &active),
            BackupJobState::Verifying
        );

        lease.finish();
        let online = maintenance.snapshot();
        assert_eq!(
            observed_backup_job_state(BackupJobState::Succeeded, backup_job_id, &online),
            BackupJobState::Succeeded
        );
        assert_eq!(online.phase, SystemMaintenancePhase::Online);
    }
}
