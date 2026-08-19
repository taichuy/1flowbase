use std::{path::PathBuf, sync::Arc};

use control_plane::{
    file_management::BusinessObjectBackupExporter,
    plugin_management::load_backup_artifact_sources,
    ports::{BackupRepository, BackupSetCatalogEntry},
    system_backup::{
        BackupComponentSource, CreateSystemBackupCommand, SystemBackupService,
        SystemBackupServiceError,
    },
    system_recovery::{
        ConfirmedRecoveryIntent, OfflineRecoveryHandoffReady, PrepareRecoveryCommand,
        RecoveryCoordinator, RecoveryCoordinatorError, RecoveryPlan, RecoveryPreflightService,
        SystemMaintenance, SystemMaintenanceOperation, SystemMaintenanceSnapshot,
    },
};
use domain::{
    strict_backup_compatibility, ApplicationBuild, BackupIncompatibility, BackupJobId,
    BackupJournalEvent, BackupJournalSubject, BackupSetId, KeyFingerprint, RecoveryJobId,
    SealedBackupManifest,
};
use sha2::{Digest, Sha256};
use storage_durable::{MainDurableStore, PostgreSqlToolchain};
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
            EnvironmentBackupKeyProvider::from_master_key(&config.provider_secret_master_key)
                .map_err(|_| SystemBackupRuntimeError::Key)?,
        );
        postgres_toolchain
            .verify_server_compatibility(store.pool())
            .await
            .map_err(|_| SystemBackupRuntimeError::PostgreSqlPreflight)?;
        let application_build = ApplicationBuild::try_from(config.system_build_identity.clone())
            .map_err(|_| SystemBackupRuntimeError::Configuration)?;
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
            .create_under_existing_maintenance(backup_job_id, actor_user_id)
            .await;
        lease.finish();
        result
    }

    pub async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, SystemBackupRuntimeError> {
        self.service.list().await.map_err(Into::into)
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
        let migration_head = storage_durable::migration_head(self.store.pool())
            .await
            .map_err(|_| SystemBackupRuntimeError::PostgreSqlPreflight)?;
        let target = domain::BackupCompatibilityTarget {
            format_version: domain::SYSTEM_BACKUP_FORMAT_VERSION,
            application_build: self.application_build.clone(),
            migration_head,
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
        let verification = self.service.verify(backup_set_id).await;
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
            .download(backup_set_id, destination)
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
        let sealed = self.service.import(source).await?;
        self.repository
            .record_verification(sealed.manifest().backup_set_id(), true)
            .await
            .map_err(|_| SystemBackupRuntimeError::Repository)?;
        Ok(sealed)
    }

    pub async fn preflight(&self, backup_set_id: BackupSetId) -> RecoveryPlan {
        self.preflight.plan(backup_set_id).await
    }

    pub async fn prepare_recovery(
        &self,
        intent: ConfirmedRecoveryIntent,
    ) -> Result<OfflineRecoveryHandoffReady, SystemBackupRuntimeError> {
        let actor_user_id = intent.actor_user_id();
        let (safety_backup_command, safety_backup_sources) =
            self.backup_inputs(actor_user_id).await?;
        self.recovery
            .prepare_offline_handoff(PrepareRecoveryCommand {
                intent,
                safety_backup_command,
                safety_backup_sources,
                drain_timeout: std::time::Duration::from_secs(30),
            })
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
        let migration_head = storage_durable::migration_head(self.store.pool())
            .await
            .map_err(|_| SystemBackupRuntimeError::PostgreSqlPreflight)?;
        let mut sources: Vec<Arc<dyn BackupComponentSource>> =
            vec![Arc::new(storage_durable::PostgreSqlLogicalBackup::new(
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
                .map_err(|_| SystemBackupRuntimeError::SourceInventory)?,
        );
        Ok((
            CreateSystemBackupCommand {
                actor_user_id,
                application_build: self.application_build.clone(),
                migration_head,
                master_key_fingerprint: self.master_key_fingerprint.clone(),
            },
            sources,
        ))
    }

    async fn create_under_existing_maintenance(
        &self,
        backup_job_id: BackupJobId,
        actor_user_id: Uuid,
    ) -> Result<SealedBackupManifest, SystemBackupRuntimeError> {
        let (command, sources) = self.backup_inputs(actor_user_id).await?;
        let manifest = self
            .service
            .create_under_existing_maintenance(backup_job_id, command, sources)
            .await?;
        self.service
            .verify(manifest.manifest().backup_set_id())
            .await?;
        self.repository
            .record_verification(manifest.manifest().backup_set_id(), true)
            .await
            .map_err(|_| SystemBackupRuntimeError::Repository)?;
        Ok(manifest)
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

fn paths_overlap(left: &std::path::Path, right: &std::path::Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}
