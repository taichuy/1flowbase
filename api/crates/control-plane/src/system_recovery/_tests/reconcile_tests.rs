use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
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
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    ports::{
        BackupComponentReader, BackupComponentWriter, BackupKeyMaterial, BackupRepository,
        BackupRepositoryError, BackupSetCatalogEntry,
    },
    system_backup::authenticate_backup_manifest,
    system_recovery::{
        ExecuteOfflineRecoveryCommand, OfflineRecoveryError, PostRestoreDependencyError,
        PostRestoreHealthVerifier, PostRestoreReconcileTarget, PostRestoreRecoveryError,
        PostRestoreRecoveryOutcome, PostRestoreRecoveryService, RecoveryAuditOutcome,
        RecoveryAuditProjection, RecoveryAuditProjector, RecoveryCompletionLease,
        RecoveryEphemeralState, RecoveryRestoreControl,
    },
};

#[tokio::test]
async fn healthy_restore_reconciles_resets_ephemeral_projects_audit_finalizes_and_releases() {
    let fixture = Fixture::new();

    let receipt = fixture.run().await.unwrap();

    assert_eq!(receipt.outcome, PostRestoreRecoveryOutcome::Succeeded);
    assert_eq!(
        fixture.scenario.calls(),
        vec![
            "reconcile",
            "health",
            "ephemeral",
            "finalize",
            "audit",
            "release",
        ]
    );
    assert_eq!(
        fixture.scenario.ephemeral_state(),
        EphemeralFixtureState {
            old_session_present: false,
            cache_present: false,
            queued_task_present: false,
        }
    );
    assert_eq!(fixture.scenario.projections().len(), 1);
    let projection = fixture.scenario.projections().into_values().next().unwrap();
    assert_eq!(
        projection.source_event_id,
        fixture
            .terminal_state_event(RecoveryJobState::Succeeded)
            .event_id,
        "the terminal external journal event is the audit idempotency source"
    );
    assert_eq!(projection.outcome, RecoveryAuditOutcome::Succeeded);
    assert_eq!(projection.failure_code, None);
    assert_eq!(
        projection.effective_after_snapshot,
        Some(projection.requested_target_snapshot.clone())
    );
    assert_ne!(
        projection.before_snapshot.application_build,
        projection.requested_target_snapshot.application_build
    );
    assert_eq!(fixture.journal_state(), RecoveryJobState::Succeeded);
    assert_eq!(
        fixture.completed_post_restore_steps(),
        vec![
            RecoveryStepKind::Reconcile,
            RecoveryStepKind::HealthVerification,
            RecoveryStepKind::AuditProjection,
        ]
    );
}

#[tokio::test]
async fn unhealthy_restore_rolls_every_promoted_target_back_before_releasing_maintenance() {
    let fixture = Fixture::new();
    fixture.scenario.fail_health.store(true, Ordering::SeqCst);

    let receipt = fixture.run().await.unwrap();

    assert_eq!(receipt.outcome, PostRestoreRecoveryOutcome::RolledBack);
    assert_eq!(
        receipt.failure_code.as_deref(),
        Some("post_restore_health_failed")
    );
    assert_eq!(
        fixture.scenario.calls(),
        vec!["reconcile", "health", "rollback", "audit", "release"]
    );
    assert_eq!(fixture.journal_state(), RecoveryJobState::RolledBack);
    let projection = fixture.scenario.projections().into_values().next().unwrap();
    assert_eq!(
        projection.source_event_id,
        fixture
            .terminal_state_event(RecoveryJobState::RolledBack)
            .event_id
    );
    assert_eq!(projection.outcome, RecoveryAuditOutcome::RolledBack);
    assert_eq!(
        projection.failure_code.as_deref(),
        Some("post_restore_health_failed")
    );
    assert_eq!(
        projection.effective_after_snapshot,
        Some(projection.before_snapshot.clone())
    );
}

#[tokio::test]
async fn rollback_failure_enters_manual_recovery_and_retains_maintenance() {
    let fixture = Fixture::new();
    fixture.scenario.fail_health.store(true, Ordering::SeqCst);
    fixture.scenario.fail_rollback.store(true, Ordering::SeqCst);

    let error = fixture.run().await.unwrap_err();

    assert_eq!(
        error,
        PostRestoreRecoveryError::ManualRecoveryRequired {
            code: "post_restore_compensation_failed"
        }
    );
    assert_eq!(
        fixture.scenario.calls(),
        vec!["reconcile", "health", "rollback", "audit", "retain"]
    );
    assert_eq!(
        fixture.journal_state(),
        RecoveryJobState::ManualRecoveryRequired
    );
    let projection = fixture.scenario.projections().into_values().next().unwrap();
    assert_eq!(
        projection.source_event_id,
        fixture
            .terminal_state_event(RecoveryJobState::ManualRecoveryRequired)
            .event_id
    );
    assert_eq!(
        projection.outcome,
        RecoveryAuditOutcome::ManualRecoveryRequired
    );
    assert_eq!(projection.effective_after_snapshot, None);
}

#[tokio::test]
async fn finalize_failure_enters_manual_recovery_without_claiming_rollback_is_safe() {
    let fixture = Fixture::new();
    fixture.scenario.fail_finalize.store(true, Ordering::SeqCst);

    let error = fixture.run().await.unwrap_err();

    assert_eq!(
        error,
        PostRestoreRecoveryError::ManualRecoveryRequired {
            code: "post_restore_finalize_failed"
        }
    );
    assert_eq!(
        fixture.scenario.calls(),
        vec![
            "reconcile",
            "health",
            "ephemeral",
            "finalize",
            "audit",
            "retain",
        ]
    );
    assert_eq!(
        fixture.journal_state(),
        RecoveryJobState::ManualRecoveryRequired
    );
    let projection = fixture.scenario.projections().into_values().next().unwrap();
    assert_eq!(
        projection.source_event_id,
        fixture
            .terminal_state_event(RecoveryJobState::ManualRecoveryRequired)
            .event_id
    );
}

#[tokio::test]
async fn retrying_after_audit_step_journal_failure_reuses_event_id_without_duplicate_audit() {
    let fixture = Fixture::new();
    fixture
        .repository
        .fail_next_audit_step
        .store(true, Ordering::SeqCst);

    assert_eq!(
        fixture.run().await.unwrap_err(),
        PostRestoreRecoveryError::Journal
    );
    assert_eq!(fixture.journal_state(), RecoveryJobState::Succeeded);
    assert_eq!(fixture.scenario.projections().len(), 1);

    let receipt = fixture.run().await.unwrap();

    assert_eq!(receipt.outcome, PostRestoreRecoveryOutcome::Succeeded);
    assert_eq!(fixture.scenario.audit_attempts(), 2);
    assert_eq!(fixture.scenario.projections().len(), 1);
    let projection = fixture.scenario.projections().into_values().next().unwrap();
    assert_eq!(projection.source_event_id, projection.audit_id);
    assert_eq!(projection.recovery_job_id, fixture.command.recovery_job_id);
    assert_eq!(projection.backup_set_id, fixture.command.backup_set_id);
}

#[tokio::test]
async fn offline_executor_errors_settle_from_explicit_compensation_evidence() {
    struct Case {
        name: &'static str,
        phase: FixturePhase,
        error: OfflineRecoveryError,
        expected_state: RecoveryJobState,
        expected_outcome: RecoveryAuditOutcome,
        expected_failure_code: &'static str,
    }

    let cases = vec![
        Case {
            name: "journal ambiguity",
            phase: FixturePhase::Restoring,
            error: OfflineRecoveryError::Journal,
            expected_state: RecoveryJobState::ManualRecoveryRequired,
            expected_outcome: RecoveryAuditOutcome::ManualRecoveryRequired,
            expected_failure_code: "offline_restore_journal_failed",
        },
        Case {
            name: "invalid journal",
            phase: FixturePhase::Restoring,
            error: OfflineRecoveryError::InvalidJournal,
            expected_state: RecoveryJobState::ManualRecoveryRequired,
            expected_outcome: RecoveryAuditOutcome::ManualRecoveryRequired,
            expected_failure_code: "offline_restore_invalid_journal",
        },
        Case {
            name: "repository before restore",
            phase: FixturePhase::Draining,
            error: OfflineRecoveryError::Repository,
            expected_state: RecoveryJobState::RolledBack,
            expected_outcome: RecoveryAuditOutcome::RolledBack,
            expected_failure_code: "offline_restore_repository_failed",
        },
        Case {
            name: "key before restore",
            phase: FixturePhase::Draining,
            error: OfflineRecoveryError::Key,
            expected_state: RecoveryJobState::RolledBack,
            expected_outcome: RecoveryAuditOutcome::RolledBack,
            expected_failure_code: "offline_restore_key_failed",
        },
        Case {
            name: "manifest before restore",
            phase: FixturePhase::Draining,
            error: OfflineRecoveryError::Manifest,
            expected_state: RecoveryJobState::RolledBack,
            expected_outcome: RecoveryAuditOutcome::RolledBack,
            expected_failure_code: "offline_restore_manifest_failed",
        },
        Case {
            name: "component inventory before restore",
            phase: FixturePhase::Draining,
            error: OfflineRecoveryError::Component,
            expected_state: RecoveryJobState::RolledBack,
            expected_outcome: RecoveryAuditOutcome::RolledBack,
            expected_failure_code: "offline_restore_component_failed",
        },
        Case {
            name: "step compensation completed",
            phase: FixturePhase::Restoring,
            error: OfflineRecoveryError::Step {
                step: RecoveryStepKind::PostgreSql,
            },
            expected_state: RecoveryJobState::RolledBack,
            expected_outcome: RecoveryAuditOutcome::RolledBack,
            expected_failure_code: "offline_restore_postgresql_failed",
        },
        Case {
            name: "step compensation failed",
            phase: FixturePhase::Restoring,
            error: OfflineRecoveryError::Compensation {
                step: RecoveryStepKind::BusinessObjects,
            },
            expected_state: RecoveryJobState::ManualRecoveryRequired,
            expected_outcome: RecoveryAuditOutcome::ManualRecoveryRequired,
            expected_failure_code: "offline_restore_business_objects_compensation_failed",
        },
    ];

    for case in cases {
        let fixture = Fixture::at_phase(case.phase);
        let result = fixture.settle_offline_failure(case.error).await;

        assert_eq!(
            fixture.journal_state(),
            case.expected_state,
            "{}",
            case.name
        );
        match case.expected_state {
            RecoveryJobState::RolledBack => {
                let receipt = result.unwrap_or_else(|error| {
                    panic!("{} must settle as rolled back: {error}", case.name)
                });
                assert_eq!(receipt.outcome, PostRestoreRecoveryOutcome::RolledBack);
                assert_eq!(fixture.scenario.calls(), vec!["audit", "release"]);
            }
            RecoveryJobState::ManualRecoveryRequired => {
                assert_eq!(
                    result.unwrap_err(),
                    PostRestoreRecoveryError::ManualRecoveryRequired {
                        code: case.expected_failure_code,
                    },
                    "{}",
                    case.name
                );
                assert_eq!(fixture.scenario.calls(), vec!["audit", "retain"]);
            }
            state => panic!("unexpected test terminal state {state:?}"),
        }
        let projection = fixture.scenario.projections().into_values().next().unwrap();
        assert_eq!(projection.outcome, case.expected_outcome, "{}", case.name);
        assert_eq!(
            projection.failure_code.as_deref(),
            Some(case.expected_failure_code),
            "{}",
            case.name
        );
        assert_eq!(
            projection.source_event_id,
            fixture.terminal_state_event(case.expected_state).event_id,
            "{}",
            case.name
        );
    }
}

#[derive(Clone, Copy)]
enum FixturePhase {
    PostRestore,
    Draining,
    Restoring,
}

struct Fixture {
    command: ExecuteOfflineRecoveryCommand,
    repository: Arc<JournalRepository>,
    scenario: Arc<Scenario>,
    service: PostRestoreRecoveryService,
}

impl Fixture {
    fn new() -> Self {
        Self::at_phase(FixturePhase::PostRestore)
    }

    fn at_phase(phase: FixturePhase) -> Self {
        let recovery_job_id = RecoveryJobId::new();
        let backup_set_id = BackupSetId::new();
        let safety_backup_set_id = BackupSetId::new();
        let actor_user_id = Uuid::now_v7();
        let mut journal = recovery_journal(
            recovery_job_id,
            backup_set_id,
            safety_backup_set_id,
            actor_user_id,
        );
        match phase {
            FixturePhase::PostRestore => {}
            FixturePhase::Draining => journal.truncate(8),
            FixturePhase::Restoring => journal.truncate(9),
        }
        let repository = Arc::new(JournalRepository::new(
            journal,
            [
                (backup_set_id, sealed_manifest(backup_set_id, 'a')),
                (
                    safety_backup_set_id,
                    sealed_manifest(safety_backup_set_id, 'b'),
                ),
            ]
            .into_iter()
            .collect(),
        ));
        let scenario = Arc::new(Scenario::default());
        let service = PostRestoreRecoveryService::new(
            repository.clone(),
            scenario.clone(),
            scenario.clone(),
            scenario.clone(),
            scenario.clone(),
            scenario.clone(),
        );
        Self {
            command: ExecuteOfflineRecoveryCommand {
                recovery_job_id,
                backup_set_id,
            },
            repository,
            scenario,
            service,
        }
    }

    async fn run(
        &self,
    ) -> Result<crate::system_recovery::PostRestoreRecoveryReceipt, PostRestoreRecoveryError> {
        self.service
            .run(
                self.command,
                Box::new(TestLease::new(self.scenario.clone())),
            )
            .await
    }

    async fn settle_offline_failure(
        &self,
        error: OfflineRecoveryError,
    ) -> Result<crate::system_recovery::PostRestoreRecoveryReceipt, PostRestoreRecoveryError> {
        self.service
            .settle_offline_failure(
                self.command,
                error,
                Box::new(TestLease::new(self.scenario.clone())),
            )
            .await
    }

    fn journal_state(&self) -> RecoveryJobState {
        self.repository
            .events()
            .iter()
            .rev()
            .find_map(|event| match event.event {
                BackupJournalEventKind::RecoveryStateChanged { state } => Some(state),
                _ => None,
            })
            .unwrap()
    }

    fn completed_post_restore_steps(&self) -> Vec<RecoveryStepKind> {
        self.repository
            .events()
            .into_iter()
            .filter_map(|event| match event.event {
                BackupJournalEventKind::RecoveryStepCompleted { step }
                    if matches!(
                        step,
                        RecoveryStepKind::Reconcile
                            | RecoveryStepKind::HealthVerification
                            | RecoveryStepKind::AuditProjection
                    ) =>
                {
                    Some(step)
                }
                _ => None,
            })
            .collect()
    }

    fn terminal_state_event(&self, expected: RecoveryJobState) -> BackupJournalEvent {
        self.repository
            .events()
            .into_iter()
            .find(|event| {
                matches!(
                    event.event,
                    BackupJournalEventKind::RecoveryStateChanged { state } if state == expected
                )
            })
            .expect("expected terminal recovery state event")
    }
}

#[derive(Default)]
struct Scenario {
    calls: Mutex<Vec<&'static str>>,
    projections: Mutex<BTreeMap<Uuid, RecoveryAuditProjection>>,
    audit_attempts: Mutex<usize>,
    ephemeral: Mutex<EphemeralFixtureState>,
    fail_health: AtomicBool,
    fail_rollback: AtomicBool,
    fail_finalize: AtomicBool,
}

impl Scenario {
    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: &'static str) {
        self.calls.lock().unwrap().push(call);
    }

    fn projections(&self) -> BTreeMap<Uuid, RecoveryAuditProjection> {
        self.projections.lock().unwrap().clone()
    }

    fn audit_attempts(&self) -> usize {
        *self.audit_attempts.lock().unwrap()
    }

    fn ephemeral_state(&self) -> EphemeralFixtureState {
        *self.ephemeral.lock().unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EphemeralFixtureState {
    old_session_present: bool,
    cache_present: bool,
    queued_task_present: bool,
}

impl Default for EphemeralFixtureState {
    fn default() -> Self {
        Self {
            old_session_present: true,
            cache_present: true,
            queued_task_present: true,
        }
    }
}

#[async_trait]
impl PostRestoreReconcileTarget for Scenario {
    async fn reconcile(
        &self,
        _context: &crate::system_recovery::PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError> {
        self.record("reconcile");
        Ok(())
    }
}

#[async_trait]
impl PostRestoreHealthVerifier for Scenario {
    async fn verify(
        &self,
        _context: &crate::system_recovery::PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError> {
        self.record("health");
        if self.fail_health.load(Ordering::SeqCst) {
            return Err(PostRestoreDependencyError);
        }
        Ok(())
    }
}

#[async_trait]
impl RecoveryEphemeralState for Scenario {
    async fn invalidate_after_restore(
        &self,
        _context: &crate::system_recovery::PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError> {
        self.record("ephemeral");
        *self.ephemeral.lock().unwrap() = EphemeralFixtureState {
            old_session_present: false,
            cache_present: false,
            queued_task_present: false,
        };
        Ok(())
    }
}

#[async_trait]
impl RecoveryAuditProjector for Scenario {
    async fn project(
        &self,
        projection: &RecoveryAuditProjection,
    ) -> Result<(), PostRestoreDependencyError> {
        self.record("audit");
        *self.audit_attempts.lock().unwrap() += 1;
        let mut projections = self.projections.lock().unwrap();
        match projections.get(&projection.audit_id) {
            Some(existing) if existing != projection => Err(PostRestoreDependencyError),
            Some(_) => Ok(()),
            None => {
                projections.insert(projection.audit_id, projection.clone());
                Ok(())
            }
        }
    }
}

#[async_trait]
impl RecoveryRestoreControl for Scenario {
    async fn rollback_promoted_targets(
        &self,
        _command: ExecuteOfflineRecoveryCommand,
    ) -> Result<(), PostRestoreDependencyError> {
        self.record("rollback");
        if self.fail_rollback.load(Ordering::SeqCst) {
            return Err(PostRestoreDependencyError);
        }
        Ok(())
    }

    async fn finalize_promoted_targets(
        &self,
        _command: ExecuteOfflineRecoveryCommand,
    ) -> Result<(), PostRestoreDependencyError> {
        self.record("finalize");
        if self.fail_finalize.load(Ordering::SeqCst) {
            return Err(PostRestoreDependencyError);
        }
        Ok(())
    }
}

struct TestLease {
    scenario: Arc<Scenario>,
}

impl TestLease {
    fn new(scenario: Arc<Scenario>) -> Self {
        Self { scenario }
    }
}

impl RecoveryCompletionLease for TestLease {
    fn release(self: Box<Self>) {
        self.scenario.record("release");
    }

    fn retain(self: Box<Self>) {
        self.scenario.record("retain");
    }
}

struct JournalRepository {
    events: Mutex<Vec<BackupJournalEvent>>,
    manifests: BTreeMap<BackupSetId, SealedBackupManifest>,
    fail_next_audit_step: AtomicBool,
}

impl JournalRepository {
    fn new(
        events: Vec<BackupJournalEvent>,
        manifests: BTreeMap<BackupSetId, SealedBackupManifest>,
    ) -> Self {
        Self {
            events: Mutex::new(events),
            manifests,
            fail_next_audit_step: AtomicBool::new(false),
        }
    }

    fn events(&self) -> Vec<BackupJournalEvent> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl BackupRepository for JournalRepository {
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
        Err(BackupRepositoryError::Unavailable)
    }

    async fn load_manifest(
        &self,
        backup_set_id: BackupSetId,
    ) -> Result<SealedBackupManifest, BackupRepositoryError> {
        self.manifests
            .get(&backup_set_id)
            .cloned()
            .ok_or(BackupRepositoryError::NotFound)
    }

    async fn open_component(
        &self,
        _backup_set_id: BackupSetId,
        _component_id: &BackupComponentId,
    ) -> Result<BackupComponentReader, BackupRepositoryError> {
        Err(BackupRepositoryError::Unavailable)
    }

    async fn delete(&self, _backup_set_id: BackupSetId) -> Result<(), BackupRepositoryError> {
        Err(BackupRepositoryError::Unavailable)
    }

    async fn append_journal_event(
        &self,
        event: &BackupJournalEvent,
    ) -> Result<(), BackupRepositoryError> {
        if matches!(
            event.event,
            BackupJournalEventKind::RecoveryStepCompleted {
                step: RecoveryStepKind::AuditProjection
            }
        ) && self.fail_next_audit_step.swap(false, Ordering::SeqCst)
        {
            return Err(BackupRepositoryError::Unavailable);
        }
        let mut events = self.events.lock().unwrap();
        if event.sequence != events.len() as u64 {
            return Err(BackupRepositoryError::Conflict);
        }
        events.push(event.clone());
        Ok(())
    }

    async fn read_journal(
        &self,
        subject: BackupJournalSubject,
    ) -> Result<Vec<BackupJournalEvent>, BackupRepositoryError> {
        let events = self.events();
        if events.iter().any(|event| event.subject != subject) {
            return Err(BackupRepositoryError::Integrity);
        }
        Ok(events)
    }
}

fn recovery_journal(
    recovery_job_id: RecoveryJobId,
    backup_set_id: BackupSetId,
    safety_backup_set_id: BackupSetId,
    actor_user_id: Uuid,
) -> Vec<BackupJournalEvent> {
    let plan_digest = ContentDigest::try_from("a".repeat(64)).unwrap();
    let intent_id = Uuid::now_v7();
    let events = vec![
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Preflight,
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::AwaitingConfirmation,
        },
        BackupJournalEventKind::RecoveryIntentConfirmed {
            intent_id,
            target_backup_set_id: backup_set_id,
            plan_digest: plan_digest.clone(),
            confirmed_at: OffsetDateTime::UNIX_EPOCH,
            expires_at: OffsetDateTime::UNIX_EPOCH + Duration::minutes(5),
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::SafetyBackup,
        },
        BackupJournalEventKind::RecoverySafetyBackupVerified {
            safety_backup_set_id,
            plan_digest: plan_digest.clone(),
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Fencing,
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Draining,
        },
        BackupJournalEventKind::RecoveryOfflineHandoffReady {
            target_backup_set_id: backup_set_id,
            safety_backup_set_id,
            plan_digest,
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Restoring,
        },
        BackupJournalEventKind::RecoveryStepCompleted {
            step: RecoveryStepKind::PostgreSql,
        },
        BackupJournalEventKind::RecoveryStepCompleted {
            step: RecoveryStepKind::BusinessObjects,
        },
        BackupJournalEventKind::RecoveryStepCompleted {
            step: RecoveryStepKind::ExtensionArtifacts,
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Reconciling,
        },
    ];
    events
        .into_iter()
        .enumerate()
        .map(|(sequence, event)| BackupJournalEvent {
            event_id: Uuid::now_v7(),
            sequence: sequence as u64,
            subject: BackupJournalSubject::Recovery(recovery_job_id),
            backup_set_id,
            actor_user_id: Some(actor_user_id),
            occurred_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(sequence as i64),
            event,
        })
        .collect()
}

fn sealed_manifest(backup_set_id: BackupSetId, digest_seed: char) -> SealedBackupManifest {
    let fingerprint = KeyFingerprint::try_from("c".repeat(64)).unwrap();
    let key = BackupKeyMaterial::new(fingerprint.clone(), vec![7; 32]).unwrap();
    let content_digest = ContentDigest::try_from(digest_seed.to_string().repeat(64)).unwrap();
    let manifest = BackupManifest::try_new(
        backup_set_id,
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from(format!("build-{digest_seed}")).unwrap(),
        MigrationHead::try_from(format!("schema-{digest_seed}")).unwrap(),
        KeyFingerprint::try_from("d".repeat(64)).unwrap(),
        fingerprint,
        vec![BackupComponent {
            component_id: BackupComponentId::try_from("postgresql").unwrap(),
            kind: BackupComponentKind::PostgreSql,
            source_identity: BackupSourceIdentity::try_from("postgresql/durable").unwrap(),
            content_type: "application/vnd.postgresql.custom-dump".to_owned(),
            size_bytes: 1,
            content_digest,
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
            restore_target: BackupComponentRestoreTarget::PostgreSql,
        }],
        1,
        ContentDigest::try_from("e".repeat(64)).unwrap(),
    )
    .unwrap();
    authenticate_backup_manifest(manifest, &key).unwrap()
}
