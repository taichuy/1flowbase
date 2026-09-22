use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use control_plane_contracts::system_backup::selective::{
    SelectiveBackupPreview, SelectiveBackupRepository, SelectiveObjectStorage,
    SELECTIVE_BACKUP_CONTENT_TYPE,
};
use domain::{BackupComponent, BackupComponentKind, BackupSetId, RecoveryJobId};
use tokio::io::AsyncSeekExt;
use uuid::Uuid;

use super::SystemBackupService;
use crate::{
    file_management::{
        BusinessObjectRecoveryTarget, RecoveryObjectStorage, RecoveryObjectStorageResolver,
    },
    ports::BackupComponentReader,
    system_recovery::{RecoveryStepContext, RecoveryStepTarget, RecoveryStepTargetError},
};

/// Coordinates a database transaction with staged business objects. Backup records never invoke
/// model provisioning or plugin installation, and the legacy database replacement path is separate.
#[derive(Debug, thiserror::Error)]
#[error("selective import requires recovery inspection: {0}")]
pub struct SelectiveRestoreNeedsRepair(pub &'static str);

pub struct SelectiveSystemBackupService {
    repository: Arc<dyn SelectiveBackupRepository>,
    backups: Arc<SystemBackupService>,
    registry: Arc<storage_object::FileStorageDriverRegistry>,
}

impl SelectiveSystemBackupService {
    pub fn new(
        repository: Arc<dyn SelectiveBackupRepository>,
        backups: Arc<SystemBackupService>,
        registry: Arc<storage_object::FileStorageDriverRegistry>,
    ) -> Self {
        Self {
            repository,
            backups,
            registry,
        }
    }

    pub async fn preflight(
        &self,
        id: BackupSetId,
        password: Option<&str>,
        target_master_key: &str,
    ) -> Result<SelectiveBackupPreview> {
        let sealed = self.backups.get(id).await?;
        let rows = rows_component(sealed.manifest().components())?;
        // Integrity applies to every included object, even when only table compatibility is shown.
        self.backups.verify_with_password(id, password).await?;
        let source_key = self.backups.source_master_key(id, password).await?;
        let mut plaintext = self.materialize(id, rows, password).await?;
        self.repository
            .preflight(plaintext.reader().await?, &source_key, target_master_key)
            .await
    }

    pub async fn restore(
        &self,
        id: BackupSetId,
        password: Option<&str>,
        target_master_key: &str,
        confirm_missing_plugins: bool,
        recovery_job_id: RecoveryJobId,
    ) -> Result<SelectiveBackupPreview> {
        let sealed = self.backups.get(id).await?;
        let manifest = sealed.manifest();
        let rows = rows_component(manifest.components())?;
        self.backups.verify_with_password(id, password).await?;
        let source_key = self.backups.source_master_key(id, password).await?;
        let mut plaintext = self.materialize(id, rows, password).await?;
        let prepared = self
            .repository
            .prepare_restore(
                plaintext.reader().await?,
                &source_key,
                target_master_key,
                confirm_missing_plugins,
            )
            .await?;
        let objects: Vec<_> = manifest
            .components()
            .iter()
            .filter(|component| component.kind == BackupComponentKind::BusinessObject)
            .cloned()
            .collect();
        let target = BusinessObjectRecoveryTarget::new(
            Arc::new(PreparedObjectResolver::new(prepared.object_storages())),
            self.registry.clone(),
        );
        let context = RecoveryStepContext {
            recovery_job_id,
            backup_set_id: id,
            migration_head: manifest.migration_head().clone(),
        };
        target.begin(&context, &objects).await?;
        let stage_result: Result<()> = async {
            for object in &objects {
                let mut plaintext = self.materialize(id, object, password).await?;
                target
                    .stage_component(&context, object, plaintext.reader().await?)
                    .await?;
            }
            target.promote(&context, &objects).await?;
            Ok(())
        }
        .await;
        if let Err(error) = stage_result {
            let compensation = target.rollback(&context, &objects).await;
            if prepared.rollback().await.is_err() {
                return Err(SelectiveRestoreNeedsRepair("database_rollback_failed").into());
            }
            if compensation.is_err() {
                return Err(SelectiveRestoreNeedsRepair("object_rollback_failed").into());
            }
            return Err(error);
        }
        let preview = match prepared.commit().await {
            Ok(preview) => preview,
            Err(error) => {
                // A transport error during COMMIT does not prove the database rolled back.
                // Keep both the promoted objects and their rollback copies, and retain the
                // maintenance fence until an operator can resolve the commit outcome.
                tracing::error!(%error, recovery_job_id = %recovery_job_id.as_uuid(),
                    "selective database commit outcome is uncertain");
                return Err(SelectiveRestoreNeedsRepair("database_commit_uncertain").into());
            }
        };
        // Commit already succeeded. Cleanup failure is observable but must not invite a retry
        // under the false claim that no data was imported; retained recovery objects are harmless.
        if let Err(error) = target.finalize(&context, &objects).await {
            tracing::warn!(%error, recovery_job_id = %recovery_job_id.as_uuid(),
                "selective import committed; object staging cleanup requires attention");
        }
        Ok(preview)
    }

    async fn materialize(
        &self,
        id: BackupSetId,
        component: &BackupComponent,
        password: Option<&str>,
    ) -> Result<PlaintextComponent> {
        let mut plaintext = PlaintextComponent::new().await?;
        self.backups
            .write_plaintext_component(id, &component.component_id, password, &mut plaintext.file)
            .await?;
        Ok(plaintext)
    }
}

fn rows_component(components: &[BackupComponent]) -> Result<&BackupComponent> {
    let mut rows = components.iter().filter(|component| {
        component.kind == BackupComponentKind::PostgreSql
            && component.content_type == SELECTIVE_BACKUP_CONTENT_TYPE
    });
    let row = rows
        .next()
        .context("selective backup row component missing")?;
    if rows.next().is_some()
        || components.iter().any(|component| {
            component.component_id != row.component_id
                && component.kind != BackupComponentKind::BusinessObject
        })
    {
        bail!("selective backup contains an unsupported component");
    }
    Ok(row)
}

struct PreparedObjectResolver(BTreeMap<Uuid, RecoveryObjectStorage>);
impl PreparedObjectResolver {
    fn new(storages: &[SelectiveObjectStorage]) -> Self {
        Self(
            storages
                .iter()
                .map(|storage| {
                    (
                        storage.storage_id,
                        RecoveryObjectStorage {
                            driver_type: storage.driver_type.clone(),
                            config_json: storage.config_json.clone(),
                        },
                    )
                })
                .collect(),
        )
    }
}
#[async_trait]
impl RecoveryObjectStorageResolver for PreparedObjectResolver {
    async fn resolve(
        &self,
        storage_id: Uuid,
    ) -> Result<RecoveryObjectStorage, RecoveryStepTargetError> {
        self.0
            .get(&storage_id)
            .cloned()
            .ok_or(RecoveryStepTargetError::InvalidTarget)
    }
}

/// Authenticated decompression input is disk-backed, never a buffer the size of the backup.
/// The random create_new file is private and is removed on every normal/error return.
struct PlaintextComponent {
    file: tokio::fs::File,
    path: PathBuf,
}
impl PlaintextComponent {
    async fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("1flowbase-restore-{}", Uuid::now_v7()));
        let mut options = tokio::fs::OpenOptions::new();
        options.create_new(true).read(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(&path).await?;
        #[cfg(unix)]
        std::fs::remove_file(&path)?;
        Ok(Self { file, path })
    }
    async fn reader(&mut self) -> Result<BackupComponentReader> {
        self.file.rewind().await?;
        Ok(Box::pin(self.file.try_clone().await?))
    }
}
impl Drop for PlaintextComponent {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
