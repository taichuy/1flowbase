use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use async_trait::async_trait;
use control_plane::ports::{
    BackupComponentReader, BackupComponentWriter, BackupRepository, BackupRepositoryError,
    BackupSetCatalogEntry,
};
use domain::{
    BackupComponentDisposition, BackupComponentId, BackupJournalEvent, BackupJournalSubject,
    BackupSetAvailability, BackupSetId, SealedBackupManifest,
};
use sha2::{Digest, Sha256};
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LocalBackupRepository {
    root: PathBuf,
}

impl LocalBackupRepository {
    pub async fn open(
        root: impl AsRef<Path>,
        protected_roots: &[PathBuf],
    ) -> Result<Self, BackupRepositoryError> {
        fs::create_dir_all(root.as_ref())
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?;
        let root = fs::canonicalize(root.as_ref())
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?;
        for protected_root in protected_roots {
            let protected_root = fs::canonicalize(protected_root)
                .await
                .map_err(|_| BackupRepositoryError::PathOverlap)?;
            if paths_overlap(&root, &protected_root) {
                return Err(BackupRepositoryError::PathOverlap);
            }
        }
        for child in ["staging", "sets", "journal"] {
            fs::create_dir_all(root.join(child))
                .await
                .map_err(|_| BackupRepositoryError::Unavailable)?;
        }
        Ok(Self { root })
    }

    fn staging_path(&self, backup_set_id: BackupSetId) -> PathBuf {
        self.root
            .join("staging")
            .join(backup_set_id.as_uuid().to_string())
    }

    fn set_path(&self, backup_set_id: BackupSetId) -> PathBuf {
        self.root
            .join("sets")
            .join(backup_set_id.as_uuid().to_string())
    }

    fn component_path(base: &Path, component_id: &BackupComponentId) -> PathBuf {
        base.join("components").join(format!(
            "{}.enc",
            sha256_hex(component_id.as_str().as_bytes())
        ))
    }

    fn journal_path(&self, subject: BackupJournalSubject) -> PathBuf {
        let name = match subject {
            BackupJournalSubject::Backup(job_id) => {
                format!("backup-{}", job_id.as_uuid())
            }
            BackupJournalSubject::Recovery(job_id) => {
                format!("recovery-{}", job_id.as_uuid())
            }
        };
        self.root.join("journal").join(name)
    }

    async fn read_manifest_at(path: &Path) -> Result<SealedBackupManifest, BackupRepositoryError> {
        let bytes = fs::read(path.join("manifest.json"))
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => BackupRepositoryError::NotFound,
                _ => BackupRepositoryError::Unavailable,
            })?;
        serde_json::from_slice(&bytes).map_err(|_| BackupRepositoryError::Integrity)
    }
}

#[async_trait]
impl BackupRepository for LocalBackupRepository {
    async fn begin_staging(&self, backup_set_id: BackupSetId) -> Result<(), BackupRepositoryError> {
        let staging = self.staging_path(backup_set_id);
        fs::create_dir(&staging)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::AlreadyExists => BackupRepositoryError::Conflict,
                _ => BackupRepositoryError::Unavailable,
            })?;
        fs::create_dir(staging.join("components"))
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)
    }

    async fn open_staging_component(
        &self,
        backup_set_id: BackupSetId,
        component_id: &BackupComponentId,
    ) -> Result<BackupComponentWriter, BackupRepositoryError> {
        let path = Self::component_path(&self.staging_path(backup_set_id), component_id);
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::AlreadyExists => BackupRepositoryError::Conflict,
                std::io::ErrorKind::NotFound => BackupRepositoryError::NotFound,
                _ => BackupRepositoryError::Unavailable,
            })?;
        Ok(Box::pin(file))
    }

    async fn abort_staging(&self, backup_set_id: BackupSetId) -> Result<(), BackupRepositoryError> {
        match fs::remove_dir_all(self.staging_path(backup_set_id)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(BackupRepositoryError::Unavailable),
        }
    }

    async fn seal(&self, sealed: &SealedBackupManifest) -> Result<(), BackupRepositoryError> {
        let manifest = sealed.manifest();
        let staging = self.staging_path(manifest.backup_set_id());
        for component in manifest
            .components()
            .iter()
            .filter(|component| component.disposition == BackupComponentDisposition::Embedded)
        {
            let metadata = fs::metadata(Self::component_path(&staging, &component.component_id))
                .await
                .map_err(|_| BackupRepositoryError::Integrity)?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(BackupRepositoryError::Integrity);
            }
        }

        let target = self.set_path(manifest.backup_set_id());
        if fs::try_exists(&target)
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?
        {
            let existing = Self::read_manifest_at(&target).await?;
            if existing.manifest().envelope_digest() == manifest.envelope_digest()
                && existing.authentication_tag() == sealed.authentication_tag()
            {
                if fs::try_exists(&staging)
                    .await
                    .map_err(|_| BackupRepositoryError::Unavailable)?
                {
                    fs::remove_dir_all(staging)
                        .await
                        .map_err(|_| BackupRepositoryError::Unavailable)?;
                }
                return Ok(());
            }
            return Err(BackupRepositoryError::Conflict);
        }

        let manifest_bytes =
            serde_json::to_vec(sealed).map_err(|_| BackupRepositoryError::Integrity)?;
        let manifest_path = staging.join("manifest.json");
        let mut manifest_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&manifest_path)
            .await
            .map_err(|_| BackupRepositoryError::Conflict)?;
        manifest_file
            .write_all(&manifest_bytes)
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?;
        manifest_file
            .sync_all()
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?;
        drop(manifest_file);
        fs::rename(staging, target)
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)
    }

    async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, BackupRepositoryError> {
        let mut entries = fs::read_dir(self.root.join("sets"))
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?;
        let mut catalog = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?
        {
            if !entry
                .file_type()
                .await
                .map_err(|_| BackupRepositoryError::Unavailable)?
                .is_dir()
            {
                continue;
            }
            let Ok(uuid) = Uuid::from_str(entry.file_name().to_string_lossy().as_ref()) else {
                continue;
            };
            let backup_set_id = BackupSetId::from_uuid(uuid);
            match Self::read_manifest_at(&entry.path()).await {
                Ok(sealed) => catalog.push(BackupSetCatalogEntry {
                    backup_set_id,
                    created_at: sealed.manifest().created_at(),
                    availability: BackupSetAvailability::Ready,
                    total_size_bytes: sealed.manifest().total_size_bytes(),
                    envelope_digest: Some(sealed.manifest().envelope_digest().clone()),
                }),
                Err(BackupRepositoryError::Integrity) => catalog.push(BackupSetCatalogEntry {
                    backup_set_id,
                    created_at: time::OffsetDateTime::UNIX_EPOCH,
                    availability: BackupSetAvailability::Corrupt,
                    total_size_bytes: 0,
                    envelope_digest: None,
                }),
                Err(error) => return Err(error),
            }
        }
        catalog.sort_by_key(|entry| std::cmp::Reverse(entry.created_at));
        Ok(catalog)
    }

    async fn load_manifest(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SealedBackupManifest, BackupRepositoryError> {
        Self::read_manifest_at(&self.set_path(backup_set_id)).await
    }

    async fn open_component(
        &self,
        backup_set_id: BackupSetId,
        component_id: &BackupComponentId,
    ) -> Result<BackupComponentReader, BackupRepositoryError> {
        let file = OpenOptions::new()
            .read(true)
            .open(Self::component_path(
                &self.set_path(backup_set_id),
                component_id,
            ))
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => BackupRepositoryError::NotFound,
                _ => BackupRepositoryError::Unavailable,
            })?;
        Ok(Box::pin(file))
    }

    async fn delete(&self, backup_set_id: BackupSetId) -> Result<(), BackupRepositoryError> {
        fs::remove_dir_all(self.set_path(backup_set_id))
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => BackupRepositoryError::NotFound,
                _ => BackupRepositoryError::Unavailable,
            })
    }

    async fn append_journal_event(
        &self,
        event: &BackupJournalEvent,
    ) -> Result<(), BackupRepositoryError> {
        let journal = self.journal_path(event.subject);
        fs::create_dir_all(&journal)
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?;
        let event_dir = journal.join(format!("{:020}", event.sequence));
        fs::create_dir(&event_dir)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::AlreadyExists => BackupRepositoryError::Conflict,
                _ => BackupRepositoryError::Unavailable,
            })?;
        let bytes = serde_json::to_vec(event).map_err(|_| BackupRepositoryError::Integrity)?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(event_dir.join("event.json"))
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?;
        if file.write_all(&bytes).await.is_err() {
            let _ = fs::remove_dir_all(event_dir).await;
            return Err(BackupRepositoryError::Unavailable);
        }
        if file.sync_all().await.is_err() {
            drop(file);
            let _ = fs::remove_dir_all(event_dir).await;
            return Err(BackupRepositoryError::Unavailable);
        }
        Ok(())
    }

    async fn read_journal(
        &self,
        subject: BackupJournalSubject,
    ) -> Result<Vec<BackupJournalEvent>, BackupRepositoryError> {
        let journal = self.journal_path(subject);
        if !fs::try_exists(&journal)
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?
        {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(journal)
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?;
        let mut paths = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|_| BackupRepositoryError::Unavailable)?
        {
            paths.push(entry.path().join("event.json"));
        }
        paths.sort();
        let mut events = Vec::with_capacity(paths.len());
        for path in paths {
            let bytes = fs::read(path)
                .await
                .map_err(|_| BackupRepositoryError::Integrity)?;
            let event = serde_json::from_slice::<BackupJournalEvent>(&bytes)
                .map_err(|_| BackupRepositoryError::Integrity)?;
            if event.subject != subject
                || events.last().is_some_and(|previous: &BackupJournalEvent| {
                    previous.sequence >= event.sequence
                })
            {
                return Err(BackupRepositoryError::Integrity);
            }
            events.push(event);
        }
        Ok(events)
    }
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
