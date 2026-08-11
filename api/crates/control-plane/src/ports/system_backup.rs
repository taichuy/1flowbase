use std::pin::Pin;

use async_trait::async_trait;
use domain::{
    BackupComponentId, BackupJournalEvent, BackupJournalSubject, BackupSetAvailability,
    BackupSetId, ContentDigest, KeyFingerprint, SealedBackupManifest,
};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::io::{AsyncRead, AsyncWrite};

pub type BackupComponentReader = Pin<Box<dyn AsyncRead + Send + Unpin + 'static>>;
pub type BackupComponentWriter = Pin<Box<dyn AsyncWrite + Send + Unpin + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSetCatalogEntry {
    pub backup_set_id: BackupSetId,
    pub created_at: OffsetDateTime,
    pub availability: BackupSetAvailability,
    pub total_size_bytes: u64,
    pub envelope_digest: ContentDigest,
}

#[derive(Debug, Error)]
pub enum BackupRepositoryError {
    #[error("backup set was not found")]
    NotFound,
    #[error("backup set already exists with different content")]
    Conflict,
    #[error("backup repository path overlaps a protected data root")]
    PathOverlap,
    #[error("backup set is not sealed")]
    NotSealed,
    #[error("backup repository integrity check failed")]
    Integrity,
    #[error("backup repository operation failed")]
    Unavailable,
}

#[async_trait]
pub trait BackupRepository: Send + Sync {
    async fn begin_staging(&self, backup_set_id: BackupSetId) -> Result<(), BackupRepositoryError>;

    async fn open_staging_component(
        &self,
        backup_set_id: BackupSetId,
        component_id: &BackupComponentId,
    ) -> Result<BackupComponentWriter, BackupRepositoryError>;

    async fn seal(&self, manifest: &SealedBackupManifest) -> Result<(), BackupRepositoryError>;

    async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, BackupRepositoryError>;

    async fn load_manifest(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SealedBackupManifest, BackupRepositoryError>;

    async fn open_component(
        &self,
        backup_set_id: BackupSetId,
        component_id: &BackupComponentId,
    ) -> Result<BackupComponentReader, BackupRepositoryError>;

    async fn delete(&self, backup_set_id: BackupSetId) -> Result<(), BackupRepositoryError>;

    async fn append_journal_event(
        &self,
        event: &BackupJournalEvent,
    ) -> Result<(), BackupRepositoryError>;

    async fn read_journal(
        &self,
        subject: BackupJournalSubject,
    ) -> Result<Vec<BackupJournalEvent>, BackupRepositoryError>;
}

pub struct BackupKeyMaterial {
    fingerprint: KeyFingerprint,
    bytes: Vec<u8>,
}

impl BackupKeyMaterial {
    pub fn new(fingerprint: KeyFingerprint, bytes: Vec<u8>) -> Option<Self> {
        (!bytes.is_empty()).then_some(Self { fingerprint, bytes })
    }

    pub fn fingerprint(&self) -> &KeyFingerprint {
        &self.fingerprint
    }

    pub fn expose_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for BackupKeyMaterial {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

#[derive(Debug, Error)]
pub enum BackupKeyProviderError {
    #[error("backup key was not found")]
    NotFound,
    #[error("backup key provider is unavailable")]
    Unavailable,
}

#[async_trait]
pub trait BackupKeyProvider: Send + Sync {
    async fn active_key(&self) -> Result<BackupKeyMaterial, BackupKeyProviderError>;

    async fn key_for(
        &self,
        fingerprint: &KeyFingerprint,
    ) -> Result<BackupKeyMaterial, BackupKeyProviderError>;
}
