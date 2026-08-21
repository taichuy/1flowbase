use std::{collections::BTreeSet, sync::Arc};

use argon2::Argon2;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use domain::{
    ApplicationBuild, ArtifactRebuildability, BackupComponent, BackupComponentDisposition,
    BackupComponentId, BackupComponentKind, BackupComponentRestoreTarget, BackupJob, BackupJobId,
    BackupJobState, BackupJournalEvent, BackupJournalEventKind, BackupJournalSubject,
    BackupManifest, BackupProtection, BackupSetId, BackupSourceIdentity, ContentDigest,
    KeyFingerprint, MigrationHead, SealedBackupManifest, SYSTEM_BACKUP_CHUNK_SIZE_BYTES,
};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

use crate::ports::{
    BackupComponentWriter, BackupKeyMaterial, BackupKeyProvider, BackupRepository,
    BackupSetCatalogEntry,
};

use super::{
    authenticate_backup_manifest, decrypt_backup_stream, encrypt_backup_stream,
    verify_backup_manifest, BackupEnvelopeError,
};

pub const BACKUP_BUNDLE_MAGIC: &[u8; 8] = b"1FBKBND1";
const BACKUP_BUNDLE_IO_BUFFER_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupComponentDescriptor {
    pub component_id: BackupComponentId,
    pub kind: BackupComponentKind,
    pub source_identity: BackupSourceIdentity,
    pub content_type: String,
    pub disposition: BackupComponentDisposition,
    pub rebuildability: ArtifactRebuildability,
    pub restore_target: BackupComponentRestoreTarget,
}

#[derive(Debug, Error)]
pub enum BackupSourceError {
    #[error("backup source is unavailable")]
    Unavailable,
    #[error("backup source changed while being captured")]
    Changed,
    #[error("backup source is invalid")]
    Invalid,
}

#[async_trait]
pub trait BackupComponentSource: Send + Sync {
    fn descriptor(&self) -> BackupComponentDescriptor;

    async fn write_to(&self, destination: BackupComponentWriter) -> Result<(), BackupSourceError>;
}

#[derive(Debug, Clone)]
pub struct CreateSystemBackupCommand {
    pub actor_user_id: Uuid,
    pub application_build: ApplicationBuild,
    pub migration_head: MigrationHead,
    pub master_key_fingerprint: KeyFingerprint,
    /// Base64 encoded source deployment secret. New backups intentionally carry this material so
    /// a single downloaded file can bootstrap a different deployment without a separate key.
    pub portable_source_master_key_base64: Option<String>,
    pub backup_password: Option<String>,
}

#[derive(Debug, Error)]
pub enum SystemBackupServiceError {
    #[error("backup repository operation failed")]
    Repository,
    #[error("backup key is unavailable")]
    Key,
    #[error("backup component source failed")]
    Source,
    #[error("backup envelope failed")]
    Envelope,
    #[error("backup manifest is invalid")]
    Manifest,
    #[error("backup verification failed")]
    Verification,
    #[error("backup transfer bundle is invalid")]
    Bundle,
}

pub struct SystemBackupService {
    repository: Arc<dyn BackupRepository>,
    key_provider: Arc<dyn BackupKeyProvider>,
}

impl SystemBackupService {
    pub fn new(
        repository: Arc<dyn BackupRepository>,
        key_provider: Arc<dyn BackupKeyProvider>,
    ) -> Self {
        Self {
            repository,
            key_provider,
        }
    }

    pub async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, SystemBackupServiceError> {
        self.repository
            .list()
            .await
            .map_err(|_| SystemBackupServiceError::Repository)
    }

    pub async fn get(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError> {
        self.repository
            .load_manifest(backup_set_id)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)
    }

    pub async fn delete(&self, backup_set_id: BackupSetId) -> Result<(), SystemBackupServiceError> {
        self.repository
            .delete(backup_set_id)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)
    }

    pub async fn download<W>(
        &self,
        backup_set_id: BackupSetId,
        mut destination: W,
    ) -> Result<(), SystemBackupServiceError>
    where
        W: AsyncWrite + Unpin + Send,
    {
        let sealed = self.get(backup_set_id).await?;
        let manifest_bytes =
            serde_json::to_vec(&sealed).map_err(|_| SystemBackupServiceError::Bundle)?;
        destination
            .write_all(BACKUP_BUNDLE_MAGIC)
            .await
            .map_err(|_| SystemBackupServiceError::Bundle)?;
        write_u64(&mut destination, manifest_bytes.len() as u64).await?;
        destination
            .write_all(&manifest_bytes)
            .await
            .map_err(|_| SystemBackupServiceError::Bundle)?;
        let embedded = sealed
            .manifest()
            .components()
            .iter()
            .filter(|component| component.disposition == BackupComponentDisposition::Embedded)
            .collect::<Vec<_>>();
        write_u32(&mut destination, embedded.len() as u32).await?;
        for component in embedded {
            let id = component.component_id.as_str().as_bytes();
            let id_len = u16::try_from(id.len()).map_err(|_| SystemBackupServiceError::Bundle)?;
            destination
                .write_all(&id_len.to_le_bytes())
                .await
                .map_err(|_| SystemBackupServiceError::Bundle)?;
            destination
                .write_all(id)
                .await
                .map_err(|_| SystemBackupServiceError::Bundle)?;
            let mut reader = self
                .repository
                .open_component(backup_set_id, &component.component_id)
                .await
                .map_err(|_| SystemBackupServiceError::Repository)?;
            let mut buffer = vec![0_u8; BACKUP_BUNDLE_IO_BUFFER_BYTES];
            loop {
                let read = reader
                    .read(&mut buffer)
                    .await
                    .map_err(|_| SystemBackupServiceError::Bundle)?;
                if read == 0 {
                    break;
                }
                write_u32(&mut destination, read as u32).await?;
                destination
                    .write_all(&buffer[..read])
                    .await
                    .map_err(|_| SystemBackupServiceError::Bundle)?;
            }
            write_u32(&mut destination, 0).await?;
        }
        destination
            .flush()
            .await
            .map_err(|_| SystemBackupServiceError::Bundle)
    }

    pub async fn import<R>(
        &self,
        source: R,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError>
    where
        R: AsyncRead + Unpin + Send,
    {
        self.import_with_password(source, None).await
    }

    pub async fn import_with_password<R>(
        &self,
        mut source: R,
        password: Option<&str>,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError>
    where
        R: AsyncRead + Unpin + Send,
    {
        let mut magic = [0_u8; 8];
        source
            .read_exact(&mut magic)
            .await
            .map_err(|_| SystemBackupServiceError::Bundle)?;
        if &magic != BACKUP_BUNDLE_MAGIC {
            return Err(SystemBackupServiceError::Bundle);
        }
        let manifest_len = read_u64(&mut source).await?;
        if manifest_len == 0 || manifest_len > 16 * 1024 * 1024 {
            return Err(SystemBackupServiceError::Bundle);
        }
        let mut manifest_bytes = vec![0_u8; manifest_len as usize];
        source
            .read_exact(&mut manifest_bytes)
            .await
            .map_err(|_| SystemBackupServiceError::Bundle)?;
        let sealed: SealedBackupManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|_| SystemBackupServiceError::Bundle)?;
        let backup_set_id = sealed.manifest().backup_set_id();
        let key = resolve_backup_key(self.key_provider.as_ref(), sealed.manifest(), password)
            .await
            .map_err(|_| SystemBackupServiceError::Key)?;
        verify_backup_manifest(&sealed, &key).map_err(map_envelope_error)?;
        if let Ok(existing) = self.repository.load_manifest(backup_set_id).await {
            if existing.authentication_tag() == sealed.authentication_tag()
                && existing.manifest().envelope_digest() == sealed.manifest().envelope_digest()
            {
                self.verify_with_password(backup_set_id, password).await?;
                return Ok(existing);
            }
            return Err(SystemBackupServiceError::Repository);
        }
        self.repository
            .begin_staging(backup_set_id)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)?;
        let result = self.import_in_staging(&mut source, &sealed, password).await;
        if result.is_err() {
            let _ = self.repository.abort_staging(backup_set_id).await;
        }
        result
    }

    async fn import_in_staging<R>(
        &self,
        source: &mut R,
        sealed: &SealedBackupManifest,
        password: Option<&str>,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError>
    where
        R: AsyncRead + Unpin + Send,
    {
        let backup_set_id = sealed.manifest().backup_set_id();
        let expected = sealed
            .manifest()
            .components()
            .iter()
            .filter(|component| component.disposition == BackupComponentDisposition::Embedded)
            .map(|component| component.component_id.clone())
            .collect::<BTreeSet<_>>();
        let count = read_u32(source).await? as usize;
        if count != expected.len() {
            return Err(SystemBackupServiceError::Bundle);
        }
        let mut imported = BTreeSet::new();
        for _ in 0..count {
            let mut id_len = [0_u8; 2];
            source
                .read_exact(&mut id_len)
                .await
                .map_err(|_| SystemBackupServiceError::Bundle)?;
            let id_len = u16::from_le_bytes(id_len) as usize;
            if id_len == 0 || id_len > 255 {
                return Err(SystemBackupServiceError::Bundle);
            }
            let mut id = vec![0_u8; id_len];
            source
                .read_exact(&mut id)
                .await
                .map_err(|_| SystemBackupServiceError::Bundle)?;
            let id = std::str::from_utf8(&id).map_err(|_| SystemBackupServiceError::Bundle)?;
            let component_id =
                BackupComponentId::try_from(id).map_err(|_| SystemBackupServiceError::Bundle)?;
            if !expected.contains(&component_id) || !imported.insert(component_id.clone()) {
                return Err(SystemBackupServiceError::Bundle);
            }
            let mut writer = self
                .repository
                .open_staging_component(backup_set_id, &component_id)
                .await
                .map_err(|_| SystemBackupServiceError::Repository)?;
            let mut component_bytes = 0_u64;
            loop {
                let length = read_u32(source).await? as usize;
                if length == 0 {
                    break;
                }
                if length > BACKUP_BUNDLE_IO_BUFFER_BYTES {
                    return Err(SystemBackupServiceError::Bundle);
                }
                let mut chunk = vec![0_u8; length];
                source
                    .read_exact(&mut chunk)
                    .await
                    .map_err(|_| SystemBackupServiceError::Bundle)?;
                writer
                    .write_all(&chunk)
                    .await
                    .map_err(|_| SystemBackupServiceError::Bundle)?;
                component_bytes = component_bytes
                    .checked_add(length as u64)
                    .ok_or(SystemBackupServiceError::Bundle)?;
            }
            if component_bytes == 0 {
                return Err(SystemBackupServiceError::Bundle);
            }
            writer
                .shutdown()
                .await
                .map_err(|_| SystemBackupServiceError::Bundle)?;
        }
        if imported != expected {
            return Err(SystemBackupServiceError::Bundle);
        }
        self.repository
            .seal(sealed)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)?;
        self.verify_with_password(backup_set_id, password).await?;
        Ok(sealed.clone())
    }

    pub async fn create(
        &self,
        command: CreateSystemBackupCommand,
        sources: Vec<Arc<dyn BackupComponentSource>>,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError> {
        self.create_under_existing_maintenance(BackupJobId::new(), command, sources)
            .await
    }

    /// Creates a backup when the host has already acquired and drained the maintenance fence.
    /// Callers must not use this as the public manual-backup entry point.
    pub async fn create_under_existing_maintenance(
        &self,
        job_id: BackupJobId,
        command: CreateSystemBackupCommand,
        mut sources: Vec<Arc<dyn BackupComponentSource>>,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError> {
        sources.sort_by_key(|source| source.descriptor().component_id);
        let backup_set_id = BackupSetId::new();
        let now = OffsetDateTime::now_utc();
        let mut job = BackupJob::new(job_id, backup_set_id, now);
        self.repository
            .begin_staging(backup_set_id)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)?;

        let result = self.create_in_staging(&command, &mut job, sources).await;
        if result.is_err() {
            let failed_at = OffsetDateTime::now_utc();
            let _ = job.transition(
                BackupJobState::Failed,
                failed_at,
                Some("backup_creation_failed".to_string()),
            );
            let _ = self
                .journal_state(
                    &command,
                    &job,
                    BackupJournalEventKind::TerminalFailure {
                        code: "backup_creation_failed".to_string(),
                    },
                )
                .await;
            let _ = self.repository.abort_staging(backup_set_id).await;
        }
        result
    }

    async fn create_in_staging(
        &self,
        command: &CreateSystemBackupCommand,
        job: &mut BackupJob,
        sources: Vec<Arc<dyn BackupComponentSource>>,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError> {
        self.transition(command, job, BackupJobState::Fencing)
            .await?;
        self.transition(command, job, BackupJobState::Capturing)
            .await?;
        let password_protection = command
            .backup_password
            .as_deref()
            .map(|password| {
                password_protection(
                    password,
                    command.portable_source_master_key_base64.as_deref(),
                )
            })
            .transpose()
            .map_err(|_| SystemBackupServiceError::Key)?;
        let (key, password_protection) = match password_protection {
            Some(protection) => (
                protection.key,
                Some((
                    protection.salt_base64,
                    protection.encrypted_source_master_key_base64,
                )),
            ),
            None => (
                self.key_provider
                    .active_key()
                    .await
                    .map_err(|_| SystemBackupServiceError::Key)?,
                None,
            ),
        };
        let mut components = Vec::with_capacity(sources.len());
        let mut envelope_digests = Vec::new();
        for source in sources {
            let descriptor = source.descriptor();
            let component = if descriptor.disposition == BackupComponentDisposition::IdentityOnly {
                identity_only_component(descriptor)?
            } else {
                let encrypted = self
                    .repository
                    .open_staging_component(job.backup_set_id(), &descriptor.component_id)
                    .await
                    .map_err(|_| SystemBackupServiceError::Repository)?;
                let (producer, consumer) =
                    tokio::io::duplex(SYSTEM_BACKUP_CHUNK_SIZE_BYTES as usize);
                let source_task =
                    tokio::spawn(async move { source.write_to(Box::pin(producer)).await });
                let receipt = encrypt_backup_stream(
                    consumer,
                    encrypted,
                    &key,
                    job.backup_set_id(),
                    &descriptor.component_id,
                )
                .await
                .map_err(map_envelope_error)?;
                source_task
                    .await
                    .map_err(|_| SystemBackupServiceError::Source)?
                    .map_err(|_| SystemBackupServiceError::Source)?;
                envelope_digests.push(receipt.envelope_digest.clone());
                BackupComponent {
                    component_id: descriptor.component_id,
                    kind: descriptor.kind,
                    source_identity: descriptor.source_identity,
                    content_type: descriptor.content_type,
                    size_bytes: receipt.plaintext_size_bytes,
                    content_digest: receipt.plaintext_digest,
                    disposition: descriptor.disposition,
                    rebuildability: descriptor.rebuildability,
                    restore_target: descriptor.restore_target,
                }
            };
            self.append_event(
                command,
                job,
                BackupJournalEventKind::ComponentSealed {
                    component_id: component.component_id.clone(),
                },
            )
            .await?;
            components.push(component);
        }
        self.transition(command, job, BackupJobState::Sealing)
            .await?;
        let total_size_bytes = components
            .iter()
            .try_fold(0_u64, |total, component| {
                total.checked_add(component.size_bytes)
            })
            .ok_or(SystemBackupServiceError::Manifest)?;
        let envelope_digest = combined_digest(&envelope_digests)?;
        let manifest = match (
            &command.portable_source_master_key_base64,
            password_protection,
        ) {
            (_, Some((salt_base64, encrypted_source_master_key_base64))) => {
                BackupManifest::try_new_password_encrypted(
                    job.backup_set_id(),
                    OffsetDateTime::now_utc(),
                    command.application_build.clone(),
                    command.migration_head.clone(),
                    command.master_key_fingerprint.clone(),
                    key.fingerprint().clone(),
                    salt_base64,
                    encrypted_source_master_key_base64,
                    components,
                    total_size_bytes,
                    envelope_digest,
                )
            }
            (Some(source_master_key_base64), None) => BackupManifest::try_new_portable(
                job.backup_set_id(),
                OffsetDateTime::now_utc(),
                command.application_build.clone(),
                command.migration_head.clone(),
                command.master_key_fingerprint.clone(),
                key.fingerprint().clone(),
                source_master_key_base64.clone(),
                components,
                total_size_bytes,
                envelope_digest,
            ),
            (None, None) => BackupManifest::try_new(
                job.backup_set_id(),
                OffsetDateTime::now_utc(),
                command.application_build.clone(),
                command.migration_head.clone(),
                command.master_key_fingerprint.clone(),
                key.fingerprint().clone(),
                components,
                total_size_bytes,
                envelope_digest,
            ),
        }
        .map_err(|_| SystemBackupServiceError::Manifest)?;
        let sealed = authenticate_backup_manifest(manifest, &key).map_err(map_envelope_error)?;
        self.repository
            .seal(&sealed)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)?;
        self.transition(command, job, BackupJobState::Verifying)
            .await?;
        self.verify_with_password(job.backup_set_id(), command.backup_password.as_deref())
            .await?;
        self.transition(command, job, BackupJobState::Succeeded)
            .await?;
        Ok(sealed)
    }

    pub async fn verify(&self, backup_set_id: BackupSetId) -> Result<(), SystemBackupServiceError> {
        self.verify_with_password(backup_set_id, None).await
    }

    pub async fn verify_with_password(
        &self,
        backup_set_id: BackupSetId,
        password: Option<&str>,
    ) -> Result<(), SystemBackupServiceError> {
        let sealed = self
            .repository
            .load_manifest(backup_set_id)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)?;
        let key = resolve_backup_key(self.key_provider.as_ref(), sealed.manifest(), password)
            .await
            .map_err(|_| SystemBackupServiceError::Key)?;
        verify_backup_manifest(&sealed, &key).map_err(map_envelope_error)?;
        for component in sealed
            .manifest()
            .components()
            .iter()
            .filter(|component| component.disposition == BackupComponentDisposition::Embedded)
        {
            let encrypted = self
                .repository
                .open_component(backup_set_id, &component.component_id)
                .await
                .map_err(|_| SystemBackupServiceError::Repository)?;
            let mut sink = tokio::io::sink();
            let receipt = decrypt_backup_stream(
                encrypted,
                &mut sink,
                &key,
                backup_set_id,
                &component.component_id,
            )
            .await
            .map_err(map_envelope_error)?;
            if receipt.plaintext_size_bytes != component.size_bytes
                || receipt.plaintext_digest != component.content_digest
            {
                return Err(SystemBackupServiceError::Verification);
            }
        }
        Ok(())
    }

    async fn transition(
        &self,
        command: &CreateSystemBackupCommand,
        job: &mut BackupJob,
        state: BackupJobState,
    ) -> Result<(), SystemBackupServiceError> {
        job.transition(state, OffsetDateTime::now_utc(), None)
            .map_err(|_| SystemBackupServiceError::Manifest)?;
        self.journal_state(
            command,
            job,
            BackupJournalEventKind::BackupStateChanged { state },
        )
        .await
    }

    async fn journal_state(
        &self,
        command: &CreateSystemBackupCommand,
        job: &BackupJob,
        event: BackupJournalEventKind,
    ) -> Result<(), SystemBackupServiceError> {
        self.append_event(command, job, event).await
    }

    async fn append_event(
        &self,
        command: &CreateSystemBackupCommand,
        job: &BackupJob,
        event: BackupJournalEventKind,
    ) -> Result<(), SystemBackupServiceError> {
        let subject = BackupJournalSubject::Backup(job.job_id());
        let sequence = self
            .repository
            .read_journal(subject)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)?
            .last()
            .map_or(0, |event| event.sequence.saturating_add(1));
        self.repository
            .append_journal_event(&BackupJournalEvent {
                event_id: Uuid::now_v7(),
                sequence,
                subject,
                backup_set_id: job.backup_set_id(),
                actor_user_id: Some(command.actor_user_id),
                occurred_at: OffsetDateTime::now_utc(),
                event,
            })
            .await
            .map_err(|_| SystemBackupServiceError::Repository)
    }
}

pub async fn read_backup_bundle_manifest<R>(
    mut source: R,
) -> Result<SealedBackupManifest, SystemBackupServiceError>
where
    R: AsyncRead + Unpin + Send,
{
    let mut magic = [0_u8; 8];
    source
        .read_exact(&mut magic)
        .await
        .map_err(|_| SystemBackupServiceError::Bundle)?;
    if &magic != BACKUP_BUNDLE_MAGIC {
        return Err(SystemBackupServiceError::Bundle);
    }
    let manifest_len = read_u64(&mut source).await?;
    if manifest_len == 0 || manifest_len > 16 * 1024 * 1024 {
        return Err(SystemBackupServiceError::Bundle);
    }
    let mut manifest_bytes = vec![0_u8; manifest_len as usize];
    source
        .read_exact(&mut manifest_bytes)
        .await
        .map_err(|_| SystemBackupServiceError::Bundle)?;
    serde_json::from_slice(&manifest_bytes).map_err(|_| SystemBackupServiceError::Bundle)
}

fn identity_only_component(
    descriptor: BackupComponentDescriptor,
) -> Result<BackupComponent, SystemBackupServiceError> {
    let digest = ContentDigest::try_from(format!(
        "{:x}",
        Sha256::digest(descriptor.source_identity.as_str().as_bytes())
    ))
    .map_err(|_| SystemBackupServiceError::Manifest)?;
    Ok(BackupComponent {
        component_id: descriptor.component_id,
        kind: descriptor.kind,
        source_identity: descriptor.source_identity,
        content_type: descriptor.content_type,
        size_bytes: 0,
        content_digest: digest,
        disposition: descriptor.disposition,
        rebuildability: descriptor.rebuildability,
        restore_target: descriptor.restore_target,
    })
}

fn combined_digest(digests: &[ContentDigest]) -> Result<ContentDigest, SystemBackupServiceError> {
    let mut hasher = Sha256::new();
    for digest in digests {
        hasher.update(digest.as_str().as_bytes());
    }
    ContentDigest::try_from(format!("{:x}", hasher.finalize()))
        .map_err(|_| SystemBackupServiceError::Manifest)
}

/// Resolves the key required to inspect a backup without allowing a portable package to depend on
/// the target deployment's environment. Legacy backups retain their fingerprint-routed provider
/// lookup and therefore still fail closed when their original key is unavailable.
pub async fn resolve_backup_key(
    key_provider: &dyn BackupKeyProvider,
    manifest: &BackupManifest,
    password: Option<&str>,
) -> Result<BackupKeyMaterial, crate::ports::BackupKeyProviderError> {
    match manifest.protection() {
        BackupProtection::LegacyEnvironmentKey => {
            key_provider
                .key_for(manifest.backup_key_fingerprint())
                .await
        }
        BackupProtection::PortableUnprotected {
            source_master_key_base64,
        } => {
            let source_master_key = STANDARD
                .decode(source_master_key_base64)
                .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
            let mut derivation = Sha256::new();
            derivation.update(b"1flowbase/system-backup/key/v1\0");
            derivation.update(source_master_key);
            let key = derivation.finalize().to_vec();
            let fingerprint = KeyFingerprint::try_from(format!("{:x}", Sha256::digest(&key)))
                .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
            if &fingerprint != manifest.backup_key_fingerprint() {
                return Err(crate::ports::BackupKeyProviderError::NotFound);
            }
            BackupKeyMaterial::new(fingerprint, key)
                .ok_or(crate::ports::BackupKeyProviderError::Unavailable)
        }
        BackupProtection::PasswordEncrypted { salt_base64, .. } => {
            let password = password.ok_or(crate::ports::BackupKeyProviderError::NotFound)?;
            let salt = STANDARD
                .decode(salt_base64)
                .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
            password_key(password, &salt)
                .and_then(|key| checked_key_material(key, manifest.backup_key_fingerprint()))
        }
    }
}

/// Obtains the source deployment secret only after the selected protection mode has been
/// satisfied. Callers use it during fresh-deployment bootstrap; it is never projected through
/// the Settings API or logged.
pub async fn recover_source_master_key(
    key_provider: &dyn BackupKeyProvider,
    manifest: &BackupManifest,
    password: Option<&str>,
) -> Result<String, crate::ports::BackupKeyProviderError> {
    match manifest.protection() {
        BackupProtection::PortableUnprotected {
            source_master_key_base64,
        } => {
            let bytes = STANDARD
                .decode(source_master_key_base64)
                .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
            String::from_utf8(bytes).map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)
        }
        BackupProtection::PasswordEncrypted {
            encrypted_source_master_key_base64,
            ..
        } => {
            let key = resolve_backup_key(key_provider, manifest, password).await?;
            let sealed = STANDARD
                .decode(encrypted_source_master_key_base64)
                .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
            let (nonce, ciphertext) = sealed.split_at(24);
            if nonce.len() != 24 {
                return Err(crate::ports::BackupKeyProviderError::Unavailable);
            }
            let cipher = XChaCha20Poly1305::new_from_slice(key.expose_bytes())
                .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
            let plaintext = cipher
                .decrypt(
                    XNonce::from_slice(nonce),
                    Payload {
                        msg: ciphertext,
                        aad: b"1flowbase/system-backup/recovery-material/v1",
                    },
                )
                .map_err(|_| crate::ports::BackupKeyProviderError::NotFound)?;
            String::from_utf8(plaintext)
                .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)
        }
        BackupProtection::LegacyEnvironmentKey => {
            Err(crate::ports::BackupKeyProviderError::NotFound)
        }
    }
}

pub(crate) struct PasswordProtection {
    pub(crate) key: BackupKeyMaterial,
    pub(crate) salt_base64: String,
    pub(crate) encrypted_source_master_key_base64: String,
}

pub(crate) fn password_protection(
    password: &str,
    source_master_key_base64: Option<&str>,
) -> Result<PasswordProtection, crate::ports::BackupKeyProviderError> {
    let source_master_key_base64 =
        source_master_key_base64.ok_or(crate::ports::BackupKeyProviderError::Unavailable)?;
    let source_master_key = STANDARD
        .decode(source_master_key_base64)
        .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
    let mut salt = [0_u8; 16];
    OsRng.fill_bytes(&mut salt);
    let key = password_key(password, &salt)?;
    let material = BackupKeyMaterial::new(
        KeyFingerprint::try_from(format!("{:x}", Sha256::digest(&key)))
            .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?,
        key.clone(),
    )
    .ok_or(crate::ports::BackupKeyProviderError::Unavailable)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
    let mut nonce = [0_u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &source_master_key,
                aad: b"1flowbase/system-backup/recovery-material/v1",
            },
        )
        .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
    let mut sealed = nonce.to_vec();
    sealed.extend(encrypted);
    Ok(PasswordProtection {
        key: material,
        salt_base64: STANDARD.encode(salt),
        encrypted_source_master_key_base64: STANDARD.encode(sealed),
    })
}

fn password_key(
    password: &str,
    salt: &[u8],
) -> Result<Vec<u8>, crate::ports::BackupKeyProviderError> {
    if password.is_empty() {
        return Err(crate::ports::BackupKeyProviderError::NotFound);
    }
    let mut key = [0_u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
    Ok(key.to_vec())
}

fn checked_key_material(
    key: Vec<u8>,
    expected: &KeyFingerprint,
) -> Result<BackupKeyMaterial, crate::ports::BackupKeyProviderError> {
    let fingerprint = KeyFingerprint::try_from(format!("{:x}", Sha256::digest(&key)))
        .map_err(|_| crate::ports::BackupKeyProviderError::Unavailable)?;
    if &fingerprint != expected {
        return Err(crate::ports::BackupKeyProviderError::NotFound);
    }
    BackupKeyMaterial::new(fingerprint, key)
        .ok_or(crate::ports::BackupKeyProviderError::Unavailable)
}

fn map_envelope_error(_: BackupEnvelopeError) -> SystemBackupServiceError {
    SystemBackupServiceError::Envelope
}

async fn write_u32<W>(writer: &mut W, value: u32) -> Result<(), SystemBackupServiceError>
where
    W: AsyncWrite + Unpin,
{
    writer
        .write_all(&value.to_le_bytes())
        .await
        .map_err(|_| SystemBackupServiceError::Bundle)
}

async fn write_u64<W>(writer: &mut W, value: u64) -> Result<(), SystemBackupServiceError>
where
    W: AsyncWrite + Unpin,
{
    writer
        .write_all(&value.to_le_bytes())
        .await
        .map_err(|_| SystemBackupServiceError::Bundle)
}

async fn read_u32<R>(reader: &mut R) -> Result<u32, SystemBackupServiceError>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = [0_u8; 4];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|_| SystemBackupServiceError::Bundle)?;
    Ok(u32::from_le_bytes(bytes))
}

async fn read_u64<R>(reader: &mut R) -> Result<u64, SystemBackupServiceError>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = [0_u8; 8];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|_| SystemBackupServiceError::Bundle)?;
    Ok(u64::from_le_bytes(bytes))
}
