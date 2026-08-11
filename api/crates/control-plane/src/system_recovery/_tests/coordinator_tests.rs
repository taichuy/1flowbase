use std::{
    collections::BTreeMap,
    io::Cursor,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use async_trait::async_trait;
use domain::{
    ApplicationBuild, ArtifactRebuildability, BackupCompatibilityTarget,
    BackupComponentDisposition, BackupComponentId, BackupComponentKind,
    BackupComponentRestoreTarget, BackupJournalEvent, BackupJournalEventKind, BackupJournalSubject,
    BackupSetAvailability, BackupSetId, BackupSourceIdentity, KeyFingerprint, MigrationHead,
    RecoveryJobId,
};
use time::OffsetDateTime;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

use crate::{
    ports::{
        BackupComponentReader, BackupComponentWriter, BackupKeyMaterial, BackupKeyProvider,
        BackupKeyProviderError, BackupRepository, BackupRepositoryError, BackupSetCatalogEntry,
    },
    system_backup::{
        BackupComponentDescriptor, BackupComponentSource, BackupSourceError,
        CreateSystemBackupCommand, SystemBackupService,
    },
    system_recovery::{
        recovery_plan_digest, ConfirmedRecoveryIntent, PrepareRecoveryCommand, RecoveryActiveWork,
        RecoveryCoordinator, RecoveryImpactPreview, RecoveryPlan, RecoveryPreflightError,
        RecoveryPreflightFailure, RecoveryPreflightService, RecoveryTargetProbe,
        RecoveryTargetSnapshot, SystemMaintenance, SystemMaintenancePhase,
    },
};

#[derive(Default)]
struct MemoryBackupRepository {
    components: Arc<Mutex<BTreeMap<(BackupSetId, BackupComponentId), Vec<u8>>>>,
    manifests: Mutex<BTreeMap<BackupSetId, domain::SealedBackupManifest>>,
    journals: Mutex<BTreeMap<BackupJournalSubject, Vec<BackupJournalEvent>>>,
}

struct CapturingWriter {
    key: (BackupSetId, BackupComponentId),
    components: Arc<Mutex<BTreeMap<(BackupSetId, BackupComponentId), Vec<u8>>>>,
    bytes: Vec<u8>,
}

impl AsyncWrite for CapturingWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.bytes.extend_from_slice(buffer);
        Poll::Ready(Ok(buffer.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl Drop for CapturingWriter {
    fn drop(&mut self) {
        self.components
            .lock()
            .unwrap()
            .insert(self.key.clone(), std::mem::take(&mut self.bytes));
    }
}

#[async_trait]
impl BackupRepository for MemoryBackupRepository {
    async fn begin_staging(
        &self,
        _backup_set_id: BackupSetId,
    ) -> Result<(), BackupRepositoryError> {
        Ok(())
    }

    async fn open_staging_component(
        &self,
        backup_set_id: BackupSetId,
        component_id: &BackupComponentId,
    ) -> Result<BackupComponentWriter, BackupRepositoryError> {
        Ok(Box::pin(CapturingWriter {
            key: (backup_set_id, component_id.clone()),
            components: self.components.clone(),
            bytes: Vec::new(),
        }))
    }

    async fn abort_staging(
        &self,
        _backup_set_id: BackupSetId,
    ) -> Result<(), BackupRepositoryError> {
        Ok(())
    }

    async fn seal(
        &self,
        manifest: &domain::SealedBackupManifest,
    ) -> Result<(), BackupRepositoryError> {
        self.manifests
            .lock()
            .unwrap()
            .insert(manifest.manifest().backup_set_id(), manifest.clone());
        Ok(())
    }

    async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, BackupRepositoryError> {
        Ok(self
            .manifests
            .lock()
            .unwrap()
            .values()
            .map(|sealed| BackupSetCatalogEntry {
                backup_set_id: sealed.manifest().backup_set_id(),
                created_at: sealed.manifest().created_at(),
                availability: BackupSetAvailability::Ready,
                total_size_bytes: sealed.manifest().total_size_bytes(),
                envelope_digest: Some(sealed.manifest().envelope_digest().clone()),
            })
            .collect())
    }

    async fn load_manifest(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<domain::SealedBackupManifest, BackupRepositoryError> {
        self.manifests
            .lock()
            .unwrap()
            .get(&backup_set_id)
            .cloned()
            .ok_or(BackupRepositoryError::NotFound)
    }

    async fn open_component(
        &self,
        backup_set_id: BackupSetId,
        component_id: &BackupComponentId,
    ) -> Result<BackupComponentReader, BackupRepositoryError> {
        let bytes = self
            .components
            .lock()
            .unwrap()
            .get(&(backup_set_id, component_id.clone()))
            .cloned()
            .ok_or(BackupRepositoryError::NotFound)?;
        Ok(Box::pin(Cursor::new(bytes)))
    }

    async fn delete(&self, backup_set_id: BackupSetId) -> Result<(), BackupRepositoryError> {
        self.manifests.lock().unwrap().remove(&backup_set_id);
        Ok(())
    }

    async fn append_journal_event(
        &self,
        event: &BackupJournalEvent,
    ) -> Result<(), BackupRepositoryError> {
        let mut journals = self.journals.lock().unwrap();
        let journal = journals.entry(event.subject).or_default();
        if journal.len() as u64 != event.sequence {
            return Err(BackupRepositoryError::Conflict);
        }
        journal.push(event.clone());
        Ok(())
    }

    async fn read_journal(
        &self,
        subject: BackupJournalSubject,
    ) -> Result<Vec<BackupJournalEvent>, BackupRepositoryError> {
        Ok(self
            .journals
            .lock()
            .unwrap()
            .get(&subject)
            .cloned()
            .unwrap_or_default())
    }
}

struct FixedKeyProvider;

fn fingerprint(character: char) -> KeyFingerprint {
    KeyFingerprint::try_from(std::iter::repeat(character).take(64).collect::<String>()).unwrap()
}

#[async_trait]
impl BackupKeyProvider for FixedKeyProvider {
    async fn active_key(&self) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        Ok(BackupKeyMaterial::new(fingerprint('a'), vec![7_u8; 32]).unwrap())
    }

    async fn key_for(
        &self,
        _fingerprint: &KeyFingerprint,
    ) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        self.active_key().await
    }
}

struct FixedSource {
    fail: bool,
}

#[async_trait]
impl BackupComponentSource for FixedSource {
    fn descriptor(&self) -> BackupComponentDescriptor {
        BackupComponentDescriptor {
            component_id: BackupComponentId::try_from("postgres/main").unwrap(),
            kind: BackupComponentKind::PostgreSql,
            source_identity: BackupSourceIdentity::try_from("postgres/main").unwrap(),
            content_type: "application/vnd.postgresql.custom-dump".to_owned(),
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
            restore_target: BackupComponentRestoreTarget::PostgreSql,
        }
    }

    async fn write_to(
        &self,
        mut destination: BackupComponentWriter,
    ) -> Result<(), BackupSourceError> {
        if self.fail {
            return Err(BackupSourceError::Unavailable);
        }
        destination
            .write_all(b"postgres fixture")
            .await
            .map_err(|_| BackupSourceError::Unavailable)
    }
}

#[derive(Clone)]
struct FixedTargetProbe {
    compatibility: BackupCompatibilityTarget,
}

#[async_trait]
impl RecoveryTargetProbe for FixedTargetProbe {
    async fn snapshot(&self) -> Result<RecoveryTargetSnapshot, RecoveryPreflightError> {
        Ok(RecoveryTargetSnapshot {
            compatibility: self.compatibility.clone(),
            available_space_bytes: u64::MAX,
            postgres_toolchain_compatible: true,
            postgres_restore_privileges: true,
            target_roots_separated: true,
            active_work: Vec::<RecoveryActiveWork>::new(),
        })
    }
}

fn backup_command(actor_user_id: Uuid) -> CreateSystemBackupCommand {
    CreateSystemBackupCommand {
        actor_user_id,
        application_build: ApplicationBuild::try_from("git.test").unwrap(),
        migration_head: MigrationHead::try_from("migration.test").unwrap(),
        master_key_fingerprint: fingerprint('b'),
    }
}

async fn coordinator_fixture() -> (
    Arc<MemoryBackupRepository>,
    Arc<SystemBackupService>,
    Arc<SystemMaintenance>,
    RecoveryCoordinator,
    BackupSetId,
    Uuid,
) {
    let repository = Arc::new(MemoryBackupRepository::default());
    let backups = Arc::new(SystemBackupService::new(
        repository.clone(),
        Arc::new(FixedKeyProvider),
    ));
    let actor_user_id = Uuid::now_v7();
    let target = backups
        .create(
            backup_command(actor_user_id),
            vec![Arc::new(FixedSource { fail: false })],
        )
        .await
        .unwrap();
    let compatibility = BackupCompatibilityTarget {
        format_version: target.manifest().format_version(),
        application_build: target.manifest().application_build().clone(),
        migration_head: target.manifest().migration_head().clone(),
        master_key_fingerprint: target.manifest().master_key_fingerprint().clone(),
    };
    let preflight = Arc::new(RecoveryPreflightService::new(
        backups.clone(),
        Arc::new(FixedTargetProbe { compatibility }),
    ));
    let maintenance = Arc::new(SystemMaintenance::default());
    let coordinator = RecoveryCoordinator::new(
        preflight,
        backups.clone(),
        repository.clone(),
        maintenance.clone(),
    );
    (
        repository,
        backups,
        maintenance,
        coordinator,
        target.manifest().backup_set_id(),
        actor_user_id,
    )
}

async fn confirmed_intent(
    coordinator_backups: Arc<SystemBackupService>,
    target_probe: Arc<dyn RecoveryTargetProbe>,
    target_backup_set_id: BackupSetId,
    actor_user_id: Uuid,
) -> ConfirmedRecoveryIntent {
    let plan = RecoveryPreflightService::new(coordinator_backups, target_probe)
        .plan(target_backup_set_id)
        .await;
    let now = OffsetDateTime::now_utc();
    ConfirmedRecoveryIntent::try_new(
        Uuid::now_v7(),
        RecoveryJobId::new(),
        actor_user_id,
        target_backup_set_id,
        recovery_plan_digest(&plan).unwrap(),
        now,
        now + time::Duration::minutes(5),
    )
    .unwrap()
}

#[test]
fn plan_digest_ignores_capacity_drift_while_the_space_decision_is_unchanged() {
    let backup_set_id = BackupSetId::new();
    let plan = |available_space_bytes, failures| RecoveryPlan {
        backup_set_id,
        required_space_bytes: 100,
        available_space_bytes,
        impact: RecoveryImpactPreview {
            database_replaced: true,
            business_object_count: 1,
            extension_artifact_count: 2,
            mcp_artifact_count: 3,
            active_work: Vec::new(),
        },
        failures,
    };

    let sufficient = plan(200, Vec::new());
    let sufficient_after_drift = plan(300, Vec::new());
    assert_eq!(
        recovery_plan_digest(&sufficient).unwrap(),
        recovery_plan_digest(&sufficient_after_drift).unwrap()
    );

    let insufficient = plan(99, vec![RecoveryPreflightFailure::InsufficientSpace]);
    assert_ne!(
        recovery_plan_digest(&sufficient).unwrap(),
        recovery_plan_digest(&insufficient).unwrap()
    );
}

#[tokio::test]
async fn verified_safety_backup_and_journal_keep_fence_until_explicit_handoff_abort() {
    let (repository, backups, maintenance, coordinator, target_id, actor_user_id) =
        coordinator_fixture().await;
    let target_manifest = backups.get(target_id).await.unwrap();
    let target_probe: Arc<dyn RecoveryTargetProbe> = Arc::new(FixedTargetProbe {
        compatibility: BackupCompatibilityTarget {
            format_version: target_manifest.manifest().format_version(),
            application_build: target_manifest.manifest().application_build().clone(),
            migration_head: target_manifest.manifest().migration_head().clone(),
            master_key_fingerprint: target_manifest.manifest().master_key_fingerprint().clone(),
        },
    });
    let intent = confirmed_intent(backups, target_probe, target_id, actor_user_id).await;
    let job_id = intent.recovery_job_id();

    let ready = coordinator
        .prepare_offline_handoff(PrepareRecoveryCommand {
            intent,
            safety_backup_command: backup_command(actor_user_id),
            safety_backup_sources: vec![Arc::new(FixedSource { fail: false })],
            drain_timeout: Duration::from_secs(1),
        })
        .await
        .unwrap();

    assert_eq!(maintenance.snapshot().phase, SystemMaintenancePhase::Active);
    assert_eq!(coordinator.active_handoff(), Some(ready.clone()));
    let journal = repository
        .read_journal(BackupJournalSubject::Recovery(job_id))
        .await
        .unwrap();
    assert!(journal.iter().all(|event| {
        event.actor_user_id == Some(actor_user_id) && event.backup_set_id == target_id
    }));
    assert!(journal.iter().any(|event| matches!(
        event.event,
        BackupJournalEventKind::RecoverySafetyBackupVerified { .. }
    )));
    assert!(journal.iter().any(|event| matches!(
        event.event,
        BackupJournalEventKind::RecoveryOfflineHandoffReady { .. }
    )));

    let handoff = coordinator.claim_offline_handoff(job_id).unwrap();
    assert!(coordinator.active_handoff().is_none());
    assert_eq!(maintenance.snapshot().phase, SystemMaintenancePhase::Active);
    handoff.abort();
    assert_eq!(maintenance.snapshot().phase, SystemMaintenancePhase::Online);
}

#[tokio::test]
async fn safety_backup_failure_releases_fence_without_a_ready_handoff() {
    let (_repository, backups, maintenance, coordinator, target_id, actor_user_id) =
        coordinator_fixture().await;
    let target_manifest = backups.get(target_id).await.unwrap();
    let target_probe: Arc<dyn RecoveryTargetProbe> = Arc::new(FixedTargetProbe {
        compatibility: BackupCompatibilityTarget {
            format_version: target_manifest.manifest().format_version(),
            application_build: target_manifest.manifest().application_build().clone(),
            migration_head: target_manifest.manifest().migration_head().clone(),
            master_key_fingerprint: target_manifest.manifest().master_key_fingerprint().clone(),
        },
    });
    let intent = confirmed_intent(backups, target_probe, target_id, actor_user_id).await;

    let result = coordinator
        .prepare_offline_handoff(PrepareRecoveryCommand {
            intent,
            safety_backup_command: backup_command(actor_user_id),
            safety_backup_sources: vec![Arc::new(FixedSource { fail: true })],
            drain_timeout: Duration::from_secs(1),
        })
        .await;

    assert!(result.is_err());
    assert!(coordinator.active_handoff().is_none());
    assert_eq!(maintenance.snapshot().phase, SystemMaintenancePhase::Online);
}
