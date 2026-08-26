use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use async_trait::async_trait;
use domain::{
    ArtifactRebuildability, BackupComponentDisposition, BackupComponentId, BackupComponentKind,
    BackupComponentRestoreTarget, BackupSourceIdentity,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::system_backup::{BackupComponentDescriptor, BackupComponentSource, BackupSourceError};

pub use crate::ports::{
    BackupObjectDatabaseReference, BackupObjectInventoryRecord, BackupObjectInventoryRepository,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BackupObjectIdentity {
    pub storage_id: Uuid,
    pub object_path: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct BackupObject {
    identity: BackupObjectIdentity,
    driver_type: String,
    storage_config: serde_json::Value,
    content_type: String,
    size_bytes: u64,
    references: BTreeSet<BackupObjectDatabaseReference>,
}

impl BackupObject {
    pub fn identity(&self) -> &BackupObjectIdentity {
        &self.identity
    }

    pub fn driver_type(&self) -> &str {
        &self.driver_type
    }

    pub fn storage_config(&self) -> &serde_json::Value {
        &self.storage_config
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn references(&self) -> &BTreeSet<BackupObjectDatabaseReference> {
        &self.references
    }
}

impl fmt::Debug for BackupObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupObject")
            .field("identity", &self.identity)
            .field("driver_type", &self.driver_type)
            .field("storage_config", &"<redacted>")
            .field("content_type", &self.content_type)
            .field("size_bytes", &self.size_bytes)
            .field("references", &self.references)
            .finish()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BackupObjectExportError {
    #[error("business object inventory is unavailable")]
    InventoryUnavailable,
    #[error("business object inventory contains an invalid record")]
    InvalidInventoryRecord,
    #[error("business object inventory contains conflicting references")]
    ConflictingReferences,
    #[error("business object storage driver is not registered")]
    UnsupportedDriver,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BusinessObjectBackupInventory {
    objects: Vec<BackupObject>,
}

impl BusinessObjectBackupInventory {
    pub fn try_from_records(
        records: Vec<BackupObjectInventoryRecord>,
    ) -> Result<Self, BackupObjectExportError> {
        let mut owners = BTreeMap::<BackupObjectDatabaseReference, BackupObjectIdentity>::new();
        let mut objects = BTreeMap::<BackupObjectIdentity, BackupObject>::new();
        for record in records {
            if record.driver_type.is_empty()
                || record.driver_type.trim() != record.driver_type
                || record.object_path.is_empty()
                || record.object_path.trim() != record.object_path
                || record.content_type.is_empty()
                || record.content_type.trim() != record.content_type
            {
                return Err(BackupObjectExportError::InvalidInventoryRecord);
            }
            let identity = BackupObjectIdentity {
                storage_id: record.storage_id,
                object_path: record.object_path,
            };
            if owners
                .insert(record.reference.clone(), identity.clone())
                .is_some_and(|previous| previous != identity)
            {
                return Err(BackupObjectExportError::ConflictingReferences);
            }
            match objects.entry(identity.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(BackupObject {
                        identity,
                        driver_type: record.driver_type,
                        storage_config: record.storage_config,
                        content_type: record.content_type,
                        size_bytes: record.size_bytes,
                        references: BTreeSet::from([record.reference]),
                    });
                }
                Entry::Occupied(mut entry) => {
                    let existing = entry.get_mut();
                    if existing.driver_type != record.driver_type
                        || existing.storage_config != record.storage_config
                        || existing.content_type != record.content_type
                        || existing.size_bytes != record.size_bytes
                    {
                        return Err(BackupObjectExportError::ConflictingReferences);
                    }
                    existing.references.insert(record.reference);
                }
            }
        }
        Ok(Self {
            objects: objects.into_values().collect(),
        })
    }

    pub fn objects(&self) -> &[BackupObject] {
        &self.objects
    }
}

pub struct BusinessObjectBackupExporter<R> {
    repository: R,
    registry: Arc<storage_object::FileStorageDriverRegistry>,
}

impl<R> BusinessObjectBackupExporter<R>
where
    R: BackupObjectInventoryRepository + Clone + Send + Sync + 'static,
{
    pub fn new(repository: R, registry: Arc<storage_object::FileStorageDriverRegistry>) -> Self {
        Self {
            repository,
            registry,
        }
    }

    pub async fn inventory(
        &self,
    ) -> Result<BusinessObjectBackupInventory, BackupObjectExportError> {
        let records = self
            .repository
            .list_backup_object_inventory()
            .await
            .map_err(|_| BackupObjectExportError::InventoryUnavailable)?;
        BusinessObjectBackupInventory::try_from_records(records)
    }

    pub async fn sources(
        &self,
    ) -> Result<Vec<Arc<dyn BackupComponentSource>>, BackupObjectExportError> {
        let inventory = self.inventory().await?;
        inventory
            .objects
            .into_iter()
            .map(|object| {
                let driver = self
                    .registry
                    .get(object.driver_type())
                    .ok_or(BackupObjectExportError::UnsupportedDriver)?;
                let descriptor = object_descriptor(&object)?;
                Ok::<Arc<dyn BackupComponentSource>, BackupObjectExportError>(Arc::new(
                    BusinessObjectBackupSource {
                        object,
                        driver,
                        descriptor,
                    },
                ))
            })
            .collect()
    }
}

struct BusinessObjectBackupSource {
    object: BackupObject,
    driver: Arc<dyn storage_object::FileStorageDriver>,
    descriptor: BackupComponentDescriptor,
}

#[async_trait]
impl BackupComponentSource for BusinessObjectBackupSource {
    fn descriptor(&self) -> BackupComponentDescriptor {
        self.descriptor.clone()
    }

    async fn write_to(
        &self,
        mut destination: crate::ports::BackupComponentWriter,
    ) -> Result<(), BackupSourceError> {
        let mut opened = self
            .driver
            .open_read_stream(storage_object::OpenReadInput {
                config_json: self.object.storage_config(),
                object_path: &self.object.identity().object_path,
            })
            .await
            .map_err(map_storage_open_error)?;
        if opened.snapshot.content_length != self.object.size_bytes()
            || opened
                .content_type
                .as_deref()
                .is_some_and(|value| value != self.object.content_type())
        {
            return Err(BackupSourceError::Changed);
        }

        let mut remaining = self.object.size_bytes();
        let mut buffer = vec![0_u8; storage_object::FILE_STORAGE_STREAM_BUFFER_BYTES];
        while remaining > 0 {
            let limit = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| BackupSourceError::Invalid)?;
            let read = opened
                .reader
                .read(&mut buffer[..limit])
                .await
                .map_err(|_| BackupSourceError::Unavailable)?;
            if read == 0 {
                return Err(BackupSourceError::Changed);
            }
            destination
                .write_all(&buffer[..read])
                .await
                .map_err(|_| BackupSourceError::Unavailable)?;
            remaining -= read as u64;
        }
        let mut extra = [0_u8; 1];
        if opened
            .reader
            .read(&mut extra)
            .await
            .map_err(|_| BackupSourceError::Unavailable)?
            != 0
        {
            return Err(BackupSourceError::Changed);
        }
        self.driver
            .verify_read_unchanged(storage_object::VerifyReadUnchangedInput {
                config_json: self.object.storage_config(),
                object_path: &self.object.identity().object_path,
                snapshot: &opened.snapshot,
            })
            .await
            .map_err(map_storage_verify_error)?;
        destination
            .flush()
            .await
            .map_err(|_| BackupSourceError::Unavailable)
    }
}

fn object_descriptor(
    object: &BackupObject,
) -> Result<BackupComponentDescriptor, BackupObjectExportError> {
    let mut hasher = Sha256::new();
    hasher.update(object.identity().storage_id.as_bytes());
    hasher.update([0]);
    hasher.update(object.identity().object_path.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    let component_id = BackupComponentId::try_from(format!("object/{digest}"))
        .map_err(|_| BackupObjectExportError::InvalidInventoryRecord)?;
    let source_identity = BackupSourceIdentity::try_from(format!(
        "storage-object/{}/{digest}",
        object.identity().storage_id
    ))
    .map_err(|_| BackupObjectExportError::InvalidInventoryRecord)?;
    Ok(BackupComponentDescriptor {
        component_id,
        kind: BackupComponentKind::BusinessObject,
        source_identity,
        content_type: object.content_type().to_string(),
        disposition: BackupComponentDisposition::Embedded,
        rebuildability: ArtifactRebuildability::NotApplicable,
        restore_target: BackupComponentRestoreTarget::BusinessObject {
            storage_id: object.identity().storage_id,
            object_path: object.identity().object_path.clone(),
        },
    })
}

fn map_storage_open_error(error: storage_object::FileStorageError) -> BackupSourceError {
    match error {
        storage_object::FileStorageError::ObjectChanged
        | storage_object::FileStorageError::ObjectLengthMismatch => BackupSourceError::Changed,
        storage_object::FileStorageError::InvalidConfig(_)
        | storage_object::FileStorageError::ObjectSnapshotUnavailable
        | storage_object::FileStorageError::ObjectTooLarge
        | storage_object::FileStorageError::UnsupportedDriver(_) => BackupSourceError::Invalid,
        storage_object::FileStorageError::ObjectNotFound
        | storage_object::FileStorageError::Other(_) => BackupSourceError::Unavailable,
    }
}

fn map_storage_verify_error(error: storage_object::FileStorageError) -> BackupSourceError {
    match error {
        storage_object::FileStorageError::ObjectChanged
        | storage_object::FileStorageError::ObjectNotFound
        | storage_object::FileStorageError::ObjectLengthMismatch => BackupSourceError::Changed,
        other => map_storage_open_error(other),
    }
}
