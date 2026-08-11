use std::{collections::BTreeSet, fmt, sync::Arc};

use async_trait::async_trait;
use domain::{
    BackupComponent, BackupComponentDisposition, BackupComponentKind, BackupComponentRestoreTarget,
    ContentDigest,
};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::{
    ports::BackupComponentReader,
    system_recovery::{RecoveryStepContext, RecoveryStepTarget, RecoveryStepTargetError},
};

const RECOVERY_OBJECT_NAMESPACE: &str = "__1flowbase_recovery";

#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryObjectStorage {
    pub driver_type: String,
    pub config_json: serde_json::Value,
}

impl fmt::Debug for RecoveryObjectStorage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveryObjectStorage")
            .field("driver_type", &self.driver_type)
            .field("config_json", &"<redacted>")
            .finish()
    }
}

#[async_trait]
pub trait RecoveryObjectStorageResolver: Send + Sync {
    async fn resolve(
        &self,
        storage_id: Uuid,
    ) -> Result<RecoveryObjectStorage, RecoveryStepTargetError>;
}

pub struct BusinessObjectRecoveryTarget {
    resolver: Arc<dyn RecoveryObjectStorageResolver>,
    registry: Arc<storage_object::FileStorageDriverRegistry>,
}

impl BusinessObjectRecoveryTarget {
    pub fn new(
        resolver: Arc<dyn RecoveryObjectStorageResolver>,
        registry: Arc<storage_object::FileStorageDriverRegistry>,
    ) -> Self {
        Self { resolver, registry }
    }

    async fn resolved(
        &self,
        component: &BackupComponent,
    ) -> Result<ResolvedObject, RecoveryStepTargetError> {
        let BackupComponentRestoreTarget::BusinessObject {
            storage_id,
            object_path,
        } = &component.restore_target
        else {
            return Err(RecoveryStepTargetError::InvalidTarget);
        };
        if object_path == RECOVERY_OBJECT_NAMESPACE
            || object_path.starts_with(&format!("{RECOVERY_OBJECT_NAMESPACE}/"))
        {
            return Err(RecoveryStepTargetError::InvalidTarget);
        }
        let storage = self.resolver.resolve(*storage_id).await?;
        let driver = self
            .registry
            .get(&storage.driver_type)
            .ok_or(RecoveryStepTargetError::InvalidTarget)?;
        driver
            .validate_config(&storage.config_json)
            .map_err(|_| RecoveryStepTargetError::InvalidTarget)?;
        Ok(ResolvedObject {
            storage_id: *storage_id,
            object_path: object_path.clone(),
            storage,
            driver,
        })
    }

    async fn stage(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
        plaintext: BackupComponentReader,
    ) -> Result<(), RecoveryStepTargetError> {
        let resolved = self.resolved(component).await?;
        let paths = recovery_paths(context, component, &resolved);
        resolved
            .driver
            .put_object_stream(storage_object::FileStoragePutStreamInput {
                config_json: &resolved.storage.config_json,
                object_path: &paths.staging,
                content_type: Some(&component.content_type),
                content_length: component.size_bytes,
                reader: plaintext,
            })
            .await
            .map_err(|_| RecoveryStepTargetError::Staging)?;
        verify_component_object(&resolved, &paths.staging, component).await
    }

    async fn prepare_rollback(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        let resolved = self.resolved(component).await?;
        let paths = recovery_paths(context, component, &resolved);
        let rollback = object_receipt_optional(&resolved, &paths.rollback).await?;
        let absent = object_receipt_optional(&resolved, &paths.absent_marker).await?;
        if rollback.is_some() && absent.is_some() {
            return Err(RecoveryStepTargetError::Integrity);
        }
        if rollback.is_some() || absent.is_some() {
            return Ok(());
        }
        match object_receipt_optional(&resolved, &resolved.object_path).await? {
            Some(original) => {
                copy_object(&resolved, &resolved.object_path, &paths.rollback).await?;
                let copied = object_receipt(&resolved, &paths.rollback).await?;
                let current = object_receipt(&resolved, &resolved.object_path).await?;
                if copied != original || current != original {
                    let _ = delete_object(&resolved, &paths.rollback).await;
                    return Err(RecoveryStepTargetError::Integrity);
                }
            }
            None => {
                resolved
                    .driver
                    .put_object_stream(storage_object::FileStoragePutStreamInput {
                        config_json: &resolved.storage.config_json,
                        object_path: &paths.absent_marker,
                        content_type: Some("application/vnd.1flowbase.recovery-absent"),
                        content_length: 0,
                        reader: Box::pin(tokio::io::empty()),
                    })
                    .await
                    .map_err(|_| RecoveryStepTargetError::Staging)?;
            }
        }
        Ok(())
    }

    async fn promote_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        let resolved = self.resolved(component).await?;
        let paths = recovery_paths(context, component, &resolved);
        copy_object(&resolved, &paths.staging, &resolved.object_path).await?;
        verify_component_object(&resolved, &resolved.object_path, component).await
    }

    async fn rollback_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        let resolved = self.resolved(component).await?;
        let paths = recovery_paths(context, component, &resolved);
        let rollback = object_receipt_optional(&resolved, &paths.rollback).await?;
        let absent = object_receipt_optional(&resolved, &paths.absent_marker).await?;
        if rollback.is_some() && absent.is_some() {
            return Err(RecoveryStepTargetError::Compensation);
        }
        if let Some(expected) = rollback {
            copy_object(&resolved, &paths.rollback, &resolved.object_path).await?;
            if object_receipt(&resolved, &resolved.object_path).await? != expected {
                return Err(RecoveryStepTargetError::Compensation);
            }
            delete_object(&resolved, &paths.rollback).await?;
        } else if absent.is_some() {
            delete_object(&resolved, &resolved.object_path).await?;
            delete_object(&resolved, &paths.absent_marker).await?;
        }
        delete_object(&resolved, &paths.staging).await?;
        Ok(())
    }

    async fn finalize_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        let resolved = self.resolved(component).await?;
        let paths = recovery_paths(context, component, &resolved);
        for path in [paths.staging, paths.rollback, paths.absent_marker] {
            delete_object(&resolved, &path).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl RecoveryStepTarget for BusinessObjectRecoveryTarget {
    async fn begin(
        &self,
        _context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        validate_business_object_components(components)
    }

    async fn stage_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
        plaintext: BackupComponentReader,
    ) -> Result<(), RecoveryStepTargetError> {
        validate_business_object_components(std::slice::from_ref(component))?;
        self.stage(context, component, plaintext).await
    }

    async fn stage_identity(
        &self,
        _context: &RecoveryStepContext,
        _component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        Err(RecoveryStepTargetError::InvalidTarget)
    }

    async fn promote(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        validate_business_object_components(components)?;
        for component in components {
            self.prepare_rollback(context, component).await?;
        }
        for component in components {
            self.promote_component(context, component).await?;
        }
        Ok(())
    }

    async fn rollback(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        let mut failed = false;
        for component in components.iter().rev() {
            if self.rollback_component(context, component).await.is_err() {
                failed = true;
            }
        }
        if failed {
            Err(RecoveryStepTargetError::Compensation)
        } else {
            Ok(())
        }
    }

    async fn finalize(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        let mut failed = false;
        for component in components {
            if self.finalize_component(context, component).await.is_err() {
                failed = true;
            }
        }
        if failed {
            Err(RecoveryStepTargetError::Unavailable)
        } else {
            Ok(())
        }
    }
}

struct ResolvedObject {
    storage_id: Uuid,
    object_path: String,
    storage: RecoveryObjectStorage,
    driver: Arc<dyn storage_object::FileStorageDriver>,
}

struct RecoveryObjectPaths {
    staging: String,
    rollback: String,
    absent_marker: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectReceipt {
    size_bytes: u64,
    content_digest: ContentDigest,
    content_type: Option<String>,
}

fn validate_business_object_components(
    components: &[BackupComponent],
) -> Result<(), RecoveryStepTargetError> {
    let mut targets = BTreeSet::new();
    for component in components {
        let BackupComponentRestoreTarget::BusinessObject {
            storage_id,
            object_path,
        } = &component.restore_target
        else {
            return Err(RecoveryStepTargetError::InvalidTarget);
        };
        if component.kind != BackupComponentKind::BusinessObject
            || component.disposition != BackupComponentDisposition::Embedded
            || object_path == RECOVERY_OBJECT_NAMESPACE
            || object_path.starts_with(&format!("{RECOVERY_OBJECT_NAMESPACE}/"))
            || !targets.insert((*storage_id, object_path.clone()))
        {
            return Err(RecoveryStepTargetError::InvalidTarget);
        }
    }
    Ok(())
}

fn recovery_paths(
    context: &RecoveryStepContext,
    component: &BackupComponent,
    resolved: &ResolvedObject,
) -> RecoveryObjectPaths {
    let mut hasher = Sha256::new();
    hasher.update(resolved.storage_id.as_bytes());
    hasher.update(resolved.object_path.as_bytes());
    hasher.update(component.component_id.as_str().as_bytes());
    let identity = format!("{:x}", hasher.finalize());
    let base = format!(
        "{RECOVERY_OBJECT_NAMESPACE}/{}/{identity}",
        context.recovery_job_id.as_uuid()
    );
    RecoveryObjectPaths {
        staging: format!("{base}/staging"),
        rollback: format!("{base}/rollback"),
        absent_marker: format!("{base}/absent"),
    }
}

async fn copy_object(
    resolved: &ResolvedObject,
    source: &str,
    destination: &str,
) -> Result<(), RecoveryStepTargetError> {
    let opened = resolved
        .driver
        .open_read_stream(storage_object::OpenReadInput {
            config_json: &resolved.storage.config_json,
            object_path: source,
        })
        .await
        .map_err(map_storage_error)?;
    let snapshot = opened.snapshot.clone();
    let content_type = opened.content_type.clone();
    resolved
        .driver
        .put_object_stream(storage_object::FileStoragePutStreamInput {
            config_json: &resolved.storage.config_json,
            object_path: destination,
            content_type: content_type.as_deref(),
            content_length: snapshot.content_length,
            reader: opened.reader,
        })
        .await
        .map_err(map_storage_error)?;
    resolved
        .driver
        .verify_read_unchanged(storage_object::VerifyReadUnchangedInput {
            config_json: &resolved.storage.config_json,
            object_path: source,
            snapshot: &snapshot,
        })
        .await
        .map_err(map_storage_error)
}

async fn verify_component_object(
    resolved: &ResolvedObject,
    path: &str,
    component: &BackupComponent,
) -> Result<(), RecoveryStepTargetError> {
    let receipt = object_receipt(resolved, path).await?;
    if receipt.size_bytes != component.size_bytes
        || receipt.content_digest != component.content_digest
        || receipt.content_type.as_deref() != Some(component.content_type.as_str())
    {
        return Err(RecoveryStepTargetError::Integrity);
    }
    Ok(())
}

async fn object_receipt_optional(
    resolved: &ResolvedObject,
    path: &str,
) -> Result<Option<ObjectReceipt>, RecoveryStepTargetError> {
    match resolved
        .driver
        .open_read_stream(storage_object::OpenReadInput {
            config_json: &resolved.storage.config_json,
            object_path: path,
        })
        .await
    {
        Ok(opened) => object_receipt_from_opened(resolved, path, opened)
            .await
            .map(Some),
        Err(storage_object::FileStorageError::ObjectNotFound) => Ok(None),
        Err(error) => Err(map_storage_error(error)),
    }
}

async fn object_receipt(
    resolved: &ResolvedObject,
    path: &str,
) -> Result<ObjectReceipt, RecoveryStepTargetError> {
    let opened = resolved
        .driver
        .open_read_stream(storage_object::OpenReadInput {
            config_json: &resolved.storage.config_json,
            object_path: path,
        })
        .await
        .map_err(map_storage_error)?;
    object_receipt_from_opened(resolved, path, opened).await
}

async fn object_receipt_from_opened(
    resolved: &ResolvedObject,
    path: &str,
    mut opened: storage_object::OpenReadStreamResult,
) -> Result<ObjectReceipt, RecoveryStepTargetError> {
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;
    let mut buffer = vec![0_u8; storage_object::FILE_STORAGE_STREAM_BUFFER_BYTES];
    loop {
        let read = opened
            .reader
            .read(&mut buffer)
            .await
            .map_err(|_| RecoveryStepTargetError::Unavailable)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size_bytes = size_bytes
            .checked_add(read as u64)
            .ok_or(RecoveryStepTargetError::Integrity)?;
    }
    if size_bytes != opened.snapshot.content_length {
        return Err(RecoveryStepTargetError::Integrity);
    }
    resolved
        .driver
        .verify_read_unchanged(storage_object::VerifyReadUnchangedInput {
            config_json: &resolved.storage.config_json,
            object_path: path,
            snapshot: &opened.snapshot,
        })
        .await
        .map_err(map_storage_error)?;
    let digest = ContentDigest::try_from(format!("{:x}", hasher.finalize()))
        .map_err(|_| RecoveryStepTargetError::Integrity)?;
    Ok(ObjectReceipt {
        size_bytes,
        content_digest: digest,
        content_type: opened.content_type,
    })
}

async fn delete_object(
    resolved: &ResolvedObject,
    path: &str,
) -> Result<(), RecoveryStepTargetError> {
    resolved
        .driver
        .delete_object(storage_object::DeleteObjectInput {
            config_json: &resolved.storage.config_json,
            object_path: path,
        })
        .await
        .map_err(map_storage_error)
}

fn map_storage_error(_error: storage_object::FileStorageError) -> RecoveryStepTargetError {
    RecoveryStepTargetError::Unavailable
}
