use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use async_trait::async_trait;
use domain::{
    ApplicationBuild, ArtifactRebuildability, BackupComponent, BackupComponentDisposition,
    BackupComponentId, BackupComponentKind, BackupComponentRestoreTarget, BackupJournalEvent,
    BackupJournalEventKind, BackupJournalSubject, BackupManifest, BackupSetId,
    BackupSourceIdentity, ContentDigest, KeyFingerprint, MigrationHead, RecoveryJobId,
    RecoveryJobState, RecoveryStepKind, SealedBackupManifest,
};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::{
    ports::{
        BackupComponentReader, BackupComponentWriter, BackupKeyMaterial, BackupKeyProvider,
        BackupKeyProviderError, BackupRepository, BackupRepositoryError, BackupSetCatalogEntry,
    },
    system_backup::{authenticate_backup_manifest, encrypt_backup_stream},
    system_recovery::{
        ExecuteOfflineRecoveryCommand, OfflineRecoveryExecutor, OfflineRecoveryTargets,
        RecoveryStepContext, RecoveryStepTarget, RecoveryStepTargetError,
    },
};

#[tokio::test]
async fn interrupted_executor_skips_journaled_step_and_stages_identity_only_artifact() {
    let fixture = recovery_fixture().await;
    fixture.seed_handoff(true);

    let receipt = fixture.executor.execute(fixture.command).await.unwrap();

    assert_eq!(receipt.resumed_steps, vec![RecoveryStepKind::PostgreSql]);
    assert_eq!(fixture.postgres.promotes.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.objects.promotes.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.artifacts.promotes.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.artifacts.identities.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn failed_later_step_journals_terminal_and_compensates_prior_promotions() {
    let fixture = recovery_fixture().await;
    fixture.seed_handoff(false);
    fixture
        .objects
        .fail_promote_once
        .store(true, Ordering::SeqCst);

    assert!(fixture.executor.execute(fixture.command).await.is_err());

    assert_eq!(fixture.postgres.rollbacks.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.objects.rollbacks.load(Ordering::SeqCst), 1);
    let journal = fixture.repository.journal.lock().unwrap();
    assert!(journal.iter().any(|event| matches!(
        &event.event,
        BackupJournalEventKind::TerminalFailure { code }
            if code == "offline_restore_business_objects_failed"
    )));
}

#[tokio::test]
async fn post_health_finalize_is_explicit_and_idempotent() {
    let fixture = recovery_fixture().await;
    fixture.seed_handoff(false);
    fixture.executor.execute(fixture.command).await.unwrap();

    fixture
        .executor
        .finalize_promoted_targets(fixture.command)
        .await
        .unwrap();
    fixture
        .executor
        .finalize_promoted_targets(fixture.command)
        .await
        .unwrap();

    assert_eq!(fixture.postgres.finalizes.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.objects.finalizes.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.artifacts.finalizes.load(Ordering::SeqCst), 2);
}

struct RecoveryFixture {
    executor: OfflineRecoveryExecutor,
    repository: Arc<MemoryRecoveryRepository>,
    command: ExecuteOfflineRecoveryCommand,
    postgres: Arc<RecordingTarget>,
    objects: Arc<RecordingTarget>,
    artifacts: Arc<RecordingTarget>,
}

impl RecoveryFixture {
    fn seed_handoff(&self, postgres_completed: bool) {
        let actor = Uuid::now_v7();
        let mut events = vec![
            journal_event(
                self.command,
                actor,
                0,
                BackupJournalEventKind::RecoveryStateChanged {
                    state: RecoveryJobState::Draining,
                },
            ),
            journal_event(
                self.command,
                actor,
                1,
                BackupJournalEventKind::RecoveryOfflineHandoffReady {
                    target_backup_set_id: self.command.backup_set_id,
                    safety_backup_set_id: BackupSetId::new(),
                    plan_digest: digest(b"plan"),
                },
            ),
        ];
        if postgres_completed {
            events.push(journal_event(
                self.command,
                actor,
                2,
                BackupJournalEventKind::RecoveryStateChanged {
                    state: RecoveryJobState::Restoring,
                },
            ));
            events.push(journal_event(
                self.command,
                actor,
                3,
                BackupJournalEventKind::RecoveryStepCompleted {
                    step: RecoveryStepKind::PostgreSql,
                },
            ));
        }
        *self.repository.journal.lock().unwrap() = events;
    }
}

async fn recovery_fixture() -> RecoveryFixture {
    let backup_set_id = BackupSetId::new();
    let key_fingerprint = digest(b"key-fingerprint");
    let key_fingerprint = KeyFingerprint::try_from(key_fingerprint.as_str()).unwrap();
    let key_bytes = vec![7_u8; 32];
    let key = BackupKeyMaterial::new(key_fingerprint.clone(), key_bytes.clone()).unwrap();
    let mut encrypted = BTreeMap::new();
    let mut components = Vec::new();
    for (id, kind, target, bytes) in [
        (
            "postgres",
            BackupComponentKind::PostgreSql,
            BackupComponentRestoreTarget::PostgreSql,
            b"postgres-dump".as_slice(),
        ),
        (
            "object",
            BackupComponentKind::BusinessObject,
            BackupComponentRestoreTarget::BusinessObject {
                storage_id: Uuid::now_v7(),
                object_path: "objects/demo".to_string(),
            },
            b"object-bytes".as_slice(),
        ),
        (
            "uploaded-artifact",
            BackupComponentKind::ExtensionArtifact,
            BackupComponentRestoreTarget::Artifact {
                category: "capability-plugins".to_string(),
                organization: "acme".to_string(),
                artifact_id: "uploaded".to_string(),
                version: "1.0.0".to_string(),
            },
            b"artifact-bytes".as_slice(),
        ),
    ] {
        let component_id = BackupComponentId::try_from(id).unwrap();
        let (receipt, envelope) =
            encrypt_component(backup_set_id, &component_id, bytes, &key).await;
        encrypted.insert(component_id.clone(), envelope);
        components.push(BackupComponent {
            component_id,
            kind,
            source_identity: BackupSourceIdentity::try_from(format!("source/{id}")).unwrap(),
            content_type: "application/octet-stream".to_string(),
            size_bytes: receipt.plaintext_size_bytes,
            content_digest: receipt.plaintext_digest,
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: if matches!(
                kind,
                BackupComponentKind::ExtensionArtifact | BackupComponentKind::McpArtifact
            ) {
                ArtifactRebuildability::NonRebuildable
            } else {
                ArtifactRebuildability::NotApplicable
            },
            restore_target: target,
        });
    }
    let identity =
        BackupSourceIdentity::try_from("plugin:capability-plugins/acme/builtin@1.0.0").unwrap();
    components.push(BackupComponent {
        component_id: BackupComponentId::try_from("builtin-artifact").unwrap(),
        kind: BackupComponentKind::ExtensionArtifact,
        source_identity: identity.clone(),
        content_type: "application/vnd.1flowbase.extension-artifact".to_string(),
        size_bytes: 0,
        content_digest: digest(identity.as_str().as_bytes()),
        disposition: BackupComponentDisposition::IdentityOnly,
        rebuildability: ArtifactRebuildability::Rebuildable,
        restore_target: BackupComponentRestoreTarget::Artifact {
            category: "capability-plugins".to_string(),
            organization: "acme".to_string(),
            artifact_id: "builtin".to_string(),
            version: "1.0.0".to_string(),
        },
    });
    let total_size = components
        .iter()
        .map(|component| component.size_bytes)
        .sum();
    let manifest = BackupManifest::try_new(
        backup_set_id,
        OffsetDateTime::now_utc(),
        ApplicationBuild::try_from("build-1").unwrap(),
        MigrationHead::try_from(digest(b"migration").as_str()).unwrap(),
        KeyFingerprint::try_from(digest(b"master-key").as_str()).unwrap(),
        key_fingerprint.clone(),
        components,
        total_size,
        digest(b"envelope"),
    )
    .unwrap();
    let sealed = authenticate_backup_manifest(manifest, &key).unwrap();
    let repository = Arc::new(MemoryRecoveryRepository {
        sealed,
        encrypted,
        journal: Mutex::new(Vec::new()),
    });
    let postgres = Arc::new(RecordingTarget::default());
    let objects = Arc::new(RecordingTarget::default());
    let artifacts = Arc::new(RecordingTarget::default());
    let executor = OfflineRecoveryExecutor::new(
        repository.clone(),
        Arc::new(StaticKeyProvider {
            fingerprint: key_fingerprint,
            key_bytes,
        }),
        OfflineRecoveryTargets {
            postgres: postgres.clone(),
            business_objects: objects.clone(),
            extension_artifacts: artifacts.clone(),
        },
    );
    let command = ExecuteOfflineRecoveryCommand {
        recovery_job_id: RecoveryJobId::new(),
        backup_set_id,
    };
    RecoveryFixture {
        executor,
        repository,
        command,
        postgres,
        objects,
        artifacts,
    }
}

async fn encrypt_component(
    backup_set_id: BackupSetId,
    component_id: &BackupComponentId,
    bytes: &[u8],
    key: &BackupKeyMaterial,
) -> (crate::system_backup::EncryptedStreamReceipt, Vec<u8>) {
    let (mut encrypted_reader, encrypted_writer) = tokio::io::duplex(4096);
    let encrypt = encrypt_backup_stream(
        Cursor::new(bytes.to_vec()),
        encrypted_writer,
        key,
        backup_set_id,
        component_id,
    );
    let collect = async {
        let mut output = Vec::new();
        encrypted_reader.read_to_end(&mut output).await.unwrap();
        output
    };
    let (receipt, output) = tokio::join!(encrypt, collect);
    (receipt.unwrap(), output)
}

fn journal_event(
    command: ExecuteOfflineRecoveryCommand,
    actor_user_id: Uuid,
    sequence: u64,
    event: BackupJournalEventKind,
) -> BackupJournalEvent {
    BackupJournalEvent {
        event_id: Uuid::now_v7(),
        sequence,
        subject: BackupJournalSubject::Recovery(command.recovery_job_id),
        backup_set_id: command.backup_set_id,
        actor_user_id: Some(actor_user_id),
        occurred_at: OffsetDateTime::now_utc(),
        event,
    }
}

fn digest(value: &[u8]) -> ContentDigest {
    ContentDigest::try_from(format!("{:x}", Sha256::digest(value))).unwrap()
}

#[derive(Default)]
struct RecordingTarget {
    promotes: AtomicUsize,
    rollbacks: AtomicUsize,
    finalizes: AtomicUsize,
    identities: AtomicUsize,
    fail_promote_once: AtomicBool,
}

#[async_trait]
impl RecoveryStepTarget for RecordingTarget {
    async fn begin(
        &self,
        _context: &RecoveryStepContext,
        _components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        Ok(())
    }

    async fn stage_component(
        &self,
        _context: &RecoveryStepContext,
        _component: &BackupComponent,
        mut plaintext: BackupComponentReader,
    ) -> Result<(), RecoveryStepTargetError> {
        let mut bytes = Vec::new();
        plaintext
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| RecoveryStepTargetError::Staging)?;
        Ok(())
    }

    async fn stage_identity(
        &self,
        _context: &RecoveryStepContext,
        _component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        self.identities.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn promote(
        &self,
        _context: &RecoveryStepContext,
        _components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        self.promotes.fetch_add(1, Ordering::SeqCst);
        if self.fail_promote_once.swap(false, Ordering::SeqCst) {
            Err(RecoveryStepTargetError::Promotion)
        } else {
            Ok(())
        }
    }

    async fn rollback(
        &self,
        _context: &RecoveryStepContext,
        _components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        self.rollbacks.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn finalize(
        &self,
        _context: &RecoveryStepContext,
        _components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        self.finalizes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct StaticKeyProvider {
    fingerprint: KeyFingerprint,
    key_bytes: Vec<u8>,
}

#[async_trait]
impl BackupKeyProvider for StaticKeyProvider {
    async fn active_key(&self) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        BackupKeyMaterial::new(self.fingerprint.clone(), self.key_bytes.clone())
            .ok_or(BackupKeyProviderError::Unavailable)
    }

    async fn key_for(
        &self,
        fingerprint: &KeyFingerprint,
    ) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        if fingerprint != &self.fingerprint {
            return Err(BackupKeyProviderError::NotFound);
        }
        self.active_key().await
    }
}

struct MemoryRecoveryRepository {
    sealed: SealedBackupManifest,
    encrypted: BTreeMap<BackupComponentId, Vec<u8>>,
    journal: Mutex<Vec<BackupJournalEvent>>,
}

#[async_trait]
impl BackupRepository for MemoryRecoveryRepository {
    async fn begin_staging(
        &self,
        _backup_set_id: BackupSetId,
    ) -> Result<(), BackupRepositoryError> {
        Err(BackupRepositoryError::Unavailable)
    }

    async fn open_staging_component(
        &self,
        _backup_set_id: BackupSetId,
        _component_id: &BackupComponentId,
    ) -> Result<BackupComponentWriter, BackupRepositoryError> {
        Err(BackupRepositoryError::Unavailable)
    }

    async fn abort_staging(
        &self,
        _backup_set_id: BackupSetId,
    ) -> Result<(), BackupRepositoryError> {
        Err(BackupRepositoryError::Unavailable)
    }

    async fn seal(&self, _manifest: &SealedBackupManifest) -> Result<(), BackupRepositoryError> {
        Err(BackupRepositoryError::Unavailable)
    }

    async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, BackupRepositoryError> {
        Ok(Vec::new())
    }

    async fn load_manifest(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SealedBackupManifest, BackupRepositoryError> {
        if backup_set_id != self.sealed.manifest().backup_set_id() {
            return Err(BackupRepositoryError::NotFound);
        }
        Ok(self.sealed.clone())
    }

    async fn open_component(
        &self,
        _backup_set_id: BackupSetId,
        component_id: &BackupComponentId,
    ) -> Result<BackupComponentReader, BackupRepositoryError> {
        self.encrypted
            .get(component_id)
            .cloned()
            .map(|bytes| Box::pin(Cursor::new(bytes)) as BackupComponentReader)
            .ok_or(BackupRepositoryError::NotFound)
    }

    async fn delete(&self, _backup_set_id: BackupSetId) -> Result<(), BackupRepositoryError> {
        Err(BackupRepositoryError::Unavailable)
    }

    async fn append_journal_event(
        &self,
        event: &BackupJournalEvent,
    ) -> Result<(), BackupRepositoryError> {
        let mut journal = self.journal.lock().unwrap();
        if event.sequence != journal.len() as u64 {
            return Err(BackupRepositoryError::Conflict);
        }
        journal.push(event.clone());
        Ok(())
    }

    async fn read_journal(
        &self,
        _subject: BackupJournalSubject,
    ) -> Result<Vec<BackupJournalEvent>, BackupRepositoryError> {
        Ok(self.journal.lock().unwrap().clone())
    }
}
