use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    ApplicationBuild, BackupComponentDisposition, BackupComponentId, BackupComponentKind,
    BackupJournalEvent, BackupJournalEventKind, BackupJournalSubject, BackupManifest, BackupSetId,
    ContentDigest, MigrationHead, RecoveryJobId, RecoveryJobState, RecoveryStepKind,
};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::ports::BackupRepository;

use super::{
    ExecuteOfflineRecoveryCommand, OfflineRecoveryError, OfflineRecoveryExecutor,
    OfflineRecoveryHandoff,
};

const OFFLINE_RESTORE_STEPS: [RecoveryStepKind; 3] = [
    RecoveryStepKind::PostgreSql,
    RecoveryStepKind::BusinessObjects,
    RecoveryStepKind::ExtensionArtifacts,
];
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
#[error("post-restore dependency failed")]
pub struct PostRestoreDependencyError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostRestoreRecoveryContext {
    pub recovery_job_id: RecoveryJobId,
    pub backup_set_id: BackupSetId,
    pub safety_backup_set_id: BackupSetId,
    pub actor_user_id: Uuid,
}

/// A secret-free, idempotent projection derived from one external journal event.
///
/// `audit_id` is deliberately identical to `source_event_id`; durable adapters must treat a
/// repeated, byte-equivalent projection as success and reject a conflicting row with the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAuditOutcome {
    Succeeded,
    RolledBack,
    ManualRecoveryRequired,
}

impl RecoveryAuditOutcome {
    pub const fn event_code(self) -> &'static str {
        match self {
            Self::Succeeded => "system.recovery.succeeded",
            Self::RolledBack => "system.recovery.rolled_back",
            Self::ManualRecoveryRequired => "system.recovery.manual_recovery_required",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::RolledBack => "rolled_back",
            Self::ManualRecoveryRequired => "manual_recovery_required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RecoveryAuditComponentSnapshot {
    pub component_id: BackupComponentId,
    pub kind: BackupComponentKind,
    pub content_digest: ContentDigest,
    pub size_bytes: u64,
    pub disposition: BackupComponentDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RecoveryAuditSnapshot {
    pub backup_set_id: BackupSetId,
    pub application_build: ApplicationBuild,
    pub migration_head: MigrationHead,
    pub components: Vec<RecoveryAuditComponentSnapshot>,
}

impl RecoveryAuditSnapshot {
    fn from_manifest(manifest: &BackupManifest) -> Self {
        let mut components = manifest
            .components()
            .iter()
            .map(|component| RecoveryAuditComponentSnapshot {
                component_id: component.component_id.clone(),
                kind: component.kind,
                content_digest: component.content_digest.clone(),
                size_bytes: component.size_bytes,
                disposition: component.disposition,
            })
            .collect::<Vec<_>>();
        components.sort_by(|left, right| left.component_id.cmp(&right.component_id));
        Self {
            backup_set_id: manifest.backup_set_id(),
            application_build: manifest.application_build().clone(),
            migration_head: manifest.migration_head().clone(),
            components,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryAuditProjection {
    pub audit_id: Uuid,
    pub source_event_id: Uuid,
    pub recovery_job_id: RecoveryJobId,
    pub backup_set_id: BackupSetId,
    pub safety_backup_set_id: BackupSetId,
    pub actor_user_id: Uuid,
    pub outcome: RecoveryAuditOutcome,
    pub failure_code: Option<String>,
    pub before_snapshot: RecoveryAuditSnapshot,
    pub requested_target_snapshot: RecoveryAuditSnapshot,
    pub effective_after_snapshot: Option<RecoveryAuditSnapshot>,
    pub occurred_at: OffsetDateTime,
}

#[async_trait]
pub trait PostRestoreReconcileTarget: Send + Sync {
    /// Rebuilds runtime/registry projections from restored durable state without remote repair.
    async fn reconcile(
        &self,
        context: &PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError>;
}

#[async_trait]
pub trait PostRestoreHealthVerifier: Send + Sync {
    /// Verifies the finite database, registry, permission, login, object and plugin health matrix.
    async fn verify(
        &self,
        context: &PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError>;
}

#[async_trait]
pub trait RecoveryEphemeralState: Send + Sync {
    /// Invalidates all pre-restore sessions and resets cache/queue state; it never reconstructs
    /// ephemeral entries from the backup.
    async fn invalidate_after_restore(
        &self,
        context: &PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError>;
}

#[async_trait]
pub trait RecoveryAuditProjector: Send + Sync {
    async fn project(
        &self,
        projection: &RecoveryAuditProjection,
    ) -> Result<(), PostRestoreDependencyError>;
}

#[async_trait]
pub trait RecoveryRestoreControl: Send + Sync {
    async fn rollback_promoted_targets(
        &self,
        command: ExecuteOfflineRecoveryCommand,
    ) -> Result<(), PostRestoreDependencyError>;

    async fn finalize_promoted_targets(
        &self,
        command: ExecuteOfflineRecoveryCommand,
    ) -> Result<(), PostRestoreDependencyError>;
}

#[async_trait]
impl RecoveryRestoreControl for OfflineRecoveryExecutor {
    async fn rollback_promoted_targets(
        &self,
        command: ExecuteOfflineRecoveryCommand,
    ) -> Result<(), PostRestoreDependencyError> {
        OfflineRecoveryExecutor::rollback_promoted_targets(self, command)
            .await
            .map_err(|_| PostRestoreDependencyError)
    }

    async fn finalize_promoted_targets(
        &self,
        command: ExecuteOfflineRecoveryCommand,
    ) -> Result<(), PostRestoreDependencyError> {
        OfflineRecoveryExecutor::finalize_promoted_targets(self, command)
            .await
            .map_err(|_| PostRestoreDependencyError)
    }
}

/// Job-scoped ownership of the maintenance fence.
///
/// `retain` must leave the fence active without leaking this wrapper. The supported host adapter
/// persists that disposition until explicit operator intervention or process restart recovery.
pub trait RecoveryCompletionLease: Send {
    fn release(self: Box<Self>);
    fn retain(self: Box<Self>);
}

impl RecoveryCompletionLease for OfflineRecoveryHandoff {
    fn release(self: Box<Self>) {
        (*self).finish();
    }

    fn retain(self: Box<Self>) {
        (*self).retain();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostRestoreRecoveryOutcome {
    Succeeded,
    RolledBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OfflineFailureSettlement {
    RolledBack,
    ManualRecoveryRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostRestoreRecoveryReceipt {
    pub recovery_job_id: RecoveryJobId,
    pub backup_set_id: BackupSetId,
    pub outcome: PostRestoreRecoveryOutcome,
    pub failure_code: Option<String>,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum PostRestoreRecoveryError {
    #[error("external recovery journal is unavailable")]
    Journal,
    #[error("external recovery journal is invalid")]
    InvalidJournal,
    #[error("manual recovery is required: {code}")]
    ManualRecoveryRequired { code: &'static str },
}

pub struct PostRestoreRecoveryService {
    repository: Arc<dyn BackupRepository>,
    restore: Arc<dyn RecoveryRestoreControl>,
    reconcile: Arc<dyn PostRestoreReconcileTarget>,
    health: Arc<dyn PostRestoreHealthVerifier>,
    ephemeral: Arc<dyn RecoveryEphemeralState>,
    audit: Arc<dyn RecoveryAuditProjector>,
}

impl PostRestoreRecoveryService {
    pub fn new(
        repository: Arc<dyn BackupRepository>,
        restore: Arc<dyn RecoveryRestoreControl>,
        reconcile: Arc<dyn PostRestoreReconcileTarget>,
        health: Arc<dyn PostRestoreHealthVerifier>,
        ephemeral: Arc<dyn RecoveryEphemeralState>,
        audit: Arc<dyn RecoveryAuditProjector>,
    ) -> Self {
        Self {
            repository,
            restore,
            reconcile,
            health,
            ephemeral,
            audit,
        }
    }

    pub async fn run(
        &self,
        command: ExecuteOfflineRecoveryCommand,
        lease: Box<dyn RecoveryCompletionLease>,
    ) -> Result<PostRestoreRecoveryReceipt, PostRestoreRecoveryError> {
        let events = match self
            .repository
            .read_journal(BackupJournalSubject::Recovery(command.recovery_job_id))
            .await
        {
            Ok(events) => events,
            Err(_) => {
                lease.retain();
                return Err(PostRestoreRecoveryError::Journal);
            }
        };
        let mut progress = match PostRestoreJournalProgress::try_from_events(command, &events) {
            Ok(progress) => progress,
            Err(error) => {
                lease.retain();
                return Err(error);
            }
        };

        match progress.state {
            RecoveryJobState::Succeeded
            | RecoveryJobState::RolledBack
            | RecoveryJobState::ManualRecoveryRequired => {
                return self.complete_terminal(&mut progress, lease).await;
            }
            RecoveryJobState::Reconciling | RecoveryJobState::Verifying => {}
            _ => {
                lease.retain();
                return Err(PostRestoreRecoveryError::InvalidJournal);
            }
        }

        let context = progress.context();
        if !progress
            .completed_steps
            .contains(&RecoveryStepKind::Reconcile)
        {
            if self.reconcile.reconcile(&context).await.is_err() {
                return self
                    .fail_and_rollback(&mut progress, lease, "post_restore_reconcile_failed")
                    .await;
            }
            if progress
                .append_step(self.repository.as_ref(), RecoveryStepKind::Reconcile)
                .await
                .is_err()
            {
                lease.retain();
                return Err(PostRestoreRecoveryError::Journal);
            }
        }

        if progress.state == RecoveryJobState::Reconciling
            && progress
                .append_state(self.repository.as_ref(), RecoveryJobState::Verifying)
                .await
                .is_err()
        {
            lease.retain();
            return Err(PostRestoreRecoveryError::Journal);
        }

        if !progress
            .completed_steps
            .contains(&RecoveryStepKind::HealthVerification)
        {
            if self.health.verify(&context).await.is_err() {
                return self
                    .fail_and_rollback(&mut progress, lease, "post_restore_health_failed")
                    .await;
            }
            if progress
                .append_step(
                    self.repository.as_ref(),
                    RecoveryStepKind::HealthVerification,
                )
                .await
                .is_err()
            {
                lease.retain();
                return Err(PostRestoreRecoveryError::Journal);
            }
        }

        if self
            .ephemeral
            .invalidate_after_restore(&context)
            .await
            .is_err()
        {
            return self
                .fail_and_rollback(&mut progress, lease, "post_restore_ephemeral_reset_failed")
                .await;
        }

        if self
            .restore
            .finalize_promoted_targets(command)
            .await
            .is_err()
        {
            return self
                .enter_manual(&mut progress, lease, "post_restore_finalize_failed")
                .await;
        }
        if progress
            .append_state(self.repository.as_ref(), RecoveryJobState::Succeeded)
            .await
            .is_err()
        {
            return self
                .enter_manual(&mut progress, lease, "post_restore_success_journal_failed")
                .await;
        }
        self.complete_terminal(&mut progress, lease).await
    }

    /// Converts an offline executor failure into a durable, auditable terminal recovery outcome.
    ///
    /// Only errors whose executor contract proves that the previous target remains effective (or
    /// that compensation completed) may release maintenance as `RolledBack`. Journal ambiguity
    /// and failed compensation always fail closed as `ManualRecoveryRequired`.
    pub async fn settle_offline_failure(
        &self,
        command: ExecuteOfflineRecoveryCommand,
        error: OfflineRecoveryError,
        lease: Box<dyn RecoveryCompletionLease>,
    ) -> Result<PostRestoreRecoveryReceipt, PostRestoreRecoveryError> {
        let events = match self
            .repository
            .read_journal(BackupJournalSubject::Recovery(command.recovery_job_id))
            .await
        {
            Ok(events) => events,
            Err(_) => {
                lease.retain();
                return Err(PostRestoreRecoveryError::Journal);
            }
        };
        let mut progress =
            match PostRestoreJournalProgress::try_from_offline_failure_events(command, &events) {
                Ok(progress) => progress,
                Err(error) => {
                    lease.retain();
                    return Err(error);
                }
            };
        let failure_code = offline_failure_code(&error);

        match offline_failure_settlement(&error) {
            OfflineFailureSettlement::RolledBack => {
                if progress
                    .append_failure(self.repository.as_ref(), failure_code)
                    .await
                    .is_err()
                    || progress
                        .append_state(self.repository.as_ref(), RecoveryJobState::RolledBack)
                        .await
                        .is_err()
                {
                    lease.retain();
                    return Err(PostRestoreRecoveryError::Journal);
                }
                self.complete_terminal(&mut progress, lease).await
            }
            OfflineFailureSettlement::ManualRecoveryRequired => {
                self.enter_manual(&mut progress, lease, failure_code).await
            }
        }
    }

    async fn fail_and_rollback(
        &self,
        progress: &mut PostRestoreJournalProgress,
        lease: Box<dyn RecoveryCompletionLease>,
        failure_code: &'static str,
    ) -> Result<PostRestoreRecoveryReceipt, PostRestoreRecoveryError> {
        if progress
            .append_failure(self.repository.as_ref(), failure_code)
            .await
            .is_err()
        {
            lease.retain();
            return Err(PostRestoreRecoveryError::Journal);
        }
        if self
            .restore
            .rollback_promoted_targets(progress.command)
            .await
            .is_ok()
        {
            if progress
                .append_state(self.repository.as_ref(), RecoveryJobState::RolledBack)
                .await
                .is_err()
            {
                lease.retain();
                return Err(PostRestoreRecoveryError::Journal);
            }
            return self.complete_terminal(progress, lease).await;
        }
        self.enter_manual(progress, lease, "post_restore_compensation_failed")
            .await
    }

    async fn enter_manual(
        &self,
        progress: &mut PostRestoreJournalProgress,
        lease: Box<dyn RecoveryCompletionLease>,
        code: &'static str,
    ) -> Result<PostRestoreRecoveryReceipt, PostRestoreRecoveryError> {
        if progress
            .append_failure(self.repository.as_ref(), code)
            .await
            .is_err()
        {
            lease.retain();
            return Err(PostRestoreRecoveryError::Journal);
        }
        if progress
            .append_state(
                self.repository.as_ref(),
                RecoveryJobState::ManualRecoveryRequired,
            )
            .await
            .is_err()
        {
            lease.retain();
            return Err(PostRestoreRecoveryError::Journal);
        }
        // A manual terminal state is already the durable safety decision. Projection remains
        // retryable from the terminal event and must never disguise the need for intervention.
        let _ = self.project_terminal_audit(progress).await;
        lease.retain();
        Err(PostRestoreRecoveryError::ManualRecoveryRequired { code })
    }

    async fn complete_terminal(
        &self,
        progress: &mut PostRestoreJournalProgress,
        lease: Box<dyn RecoveryCompletionLease>,
    ) -> Result<PostRestoreRecoveryReceipt, PostRestoreRecoveryError> {
        if !progress
            .completed_steps
            .contains(&RecoveryStepKind::AuditProjection)
            && self.project_terminal_audit(progress).await.is_err()
        {
            lease.retain();
            return match progress.state {
                RecoveryJobState::ManualRecoveryRequired => {
                    Err(PostRestoreRecoveryError::ManualRecoveryRequired {
                        code: "manual_recovery_required",
                    })
                }
                _ => Err(PostRestoreRecoveryError::Journal),
            };
        }

        match progress.state {
            RecoveryJobState::Succeeded => {
                lease.release();
                Ok(progress.receipt(PostRestoreRecoveryOutcome::Succeeded, None))
            }
            RecoveryJobState::RolledBack => {
                lease.release();
                Ok(progress.receipt(
                    PostRestoreRecoveryOutcome::RolledBack,
                    progress.last_failure_code.clone(),
                ))
            }
            RecoveryJobState::ManualRecoveryRequired => {
                lease.retain();
                Err(PostRestoreRecoveryError::ManualRecoveryRequired {
                    code: "manual_recovery_required",
                })
            }
            _ => {
                lease.retain();
                Err(PostRestoreRecoveryError::InvalidJournal)
            }
        }
    }

    async fn project_terminal_audit(
        &self,
        progress: &mut PostRestoreJournalProgress,
    ) -> Result<(), PostRestoreRecoveryError> {
        let terminal = progress
            .terminal_event
            .ok_or(PostRestoreRecoveryError::InvalidJournal)?;
        let target = self
            .repository
            .load_manifest(progress.command.backup_set_id)
            .await
            .map_err(|_| PostRestoreRecoveryError::Journal)?;
        let safety = self
            .repository
            .load_manifest(progress.safety_backup_set_id)
            .await
            .map_err(|_| PostRestoreRecoveryError::Journal)?;
        if target.manifest().backup_set_id() != progress.command.backup_set_id
            || safety.manifest().backup_set_id() != progress.safety_backup_set_id
        {
            return Err(PostRestoreRecoveryError::InvalidJournal);
        }

        let outcome = match terminal.state {
            RecoveryJobState::Succeeded => RecoveryAuditOutcome::Succeeded,
            RecoveryJobState::RolledBack => RecoveryAuditOutcome::RolledBack,
            RecoveryJobState::ManualRecoveryRequired => {
                RecoveryAuditOutcome::ManualRecoveryRequired
            }
            _ => return Err(PostRestoreRecoveryError::InvalidJournal),
        };
        let before_snapshot = RecoveryAuditSnapshot::from_manifest(safety.manifest());
        let requested_target_snapshot = RecoveryAuditSnapshot::from_manifest(target.manifest());
        let effective_after_snapshot = match outcome {
            RecoveryAuditOutcome::Succeeded => Some(requested_target_snapshot.clone()),
            RecoveryAuditOutcome::RolledBack => Some(before_snapshot.clone()),
            RecoveryAuditOutcome::ManualRecoveryRequired => None,
        };
        let projection = RecoveryAuditProjection {
            audit_id: terminal.event_id,
            source_event_id: terminal.event_id,
            recovery_job_id: progress.command.recovery_job_id,
            backup_set_id: progress.command.backup_set_id,
            safety_backup_set_id: progress.safety_backup_set_id,
            actor_user_id: progress.actor_user_id,
            outcome,
            failure_code: progress.last_failure_code.clone(),
            before_snapshot,
            requested_target_snapshot,
            effective_after_snapshot,
            occurred_at: terminal.occurred_at,
        };
        self.audit
            .project(&projection)
            .await
            .map_err(|_| PostRestoreRecoveryError::Journal)?;
        progress
            .append_step(self.repository.as_ref(), RecoveryStepKind::AuditProjection)
            .await
    }
}

#[derive(Clone, Copy)]
struct RecoveryTerminalEvent {
    event_id: Uuid,
    occurred_at: OffsetDateTime,
    state: RecoveryJobState,
}

struct PostRestoreJournalProgress {
    command: ExecuteOfflineRecoveryCommand,
    actor_user_id: Uuid,
    safety_backup_set_id: BackupSetId,
    next_sequence: u64,
    state: RecoveryJobState,
    completed_steps: Vec<RecoveryStepKind>,
    terminal_event: Option<RecoveryTerminalEvent>,
    last_failure_code: Option<String>,
}

#[derive(Clone, Copy)]
enum JournalProgressExpectation {
    PostRestoreOrTerminal,
    OfflineFailure,
}

impl PostRestoreJournalProgress {
    fn try_from_events(
        command: ExecuteOfflineRecoveryCommand,
        events: &[BackupJournalEvent],
    ) -> Result<Self, PostRestoreRecoveryError> {
        Self::try_from_events_for(
            command,
            events,
            JournalProgressExpectation::PostRestoreOrTerminal,
        )
    }

    fn try_from_offline_failure_events(
        command: ExecuteOfflineRecoveryCommand,
        events: &[BackupJournalEvent],
    ) -> Result<Self, PostRestoreRecoveryError> {
        Self::try_from_events_for(command, events, JournalProgressExpectation::OfflineFailure)
    }

    fn try_from_events_for(
        command: ExecuteOfflineRecoveryCommand,
        events: &[BackupJournalEvent],
        expectation: JournalProgressExpectation,
    ) -> Result<Self, PostRestoreRecoveryError> {
        let subject = BackupJournalSubject::Recovery(command.recovery_job_id);
        let mut actor_user_id = None;
        let mut bootstrap_actor_assigned = false;
        let mut safety_backup_set_id = None;
        let mut handoff_ready = false;
        let mut state = None;
        let mut completed_steps = Vec::new();
        let mut terminal_event = None;
        let mut last_failure_code = None;

        for (index, event) in events.iter().enumerate() {
            if event.sequence != index as u64
                || event.subject != subject
                || event.backup_set_id != command.backup_set_id
            {
                return Err(PostRestoreRecoveryError::InvalidJournal);
            }
            let Some(actor) = event.actor_user_id else {
                return Err(PostRestoreRecoveryError::InvalidJournal);
            };
            let assigns_bootstrap_actor = matches!(
                &event.event,
                BackupJournalEventKind::RecoveryBootstrapActorAssigned { actor_user_id: assigned }
                    if *assigned == actor
            );
            if let Some(current) = actor_user_id.filter(|current| *current != actor) {
                if current != Uuid::nil() || !assigns_bootstrap_actor || bootstrap_actor_assigned {
                    return Err(PostRestoreRecoveryError::InvalidJournal);
                }
            }
            actor_user_id = Some(actor);

            if terminal_event.is_some() {
                if matches!(
                    event.event,
                    BackupJournalEventKind::RecoveryStepCompleted {
                        step: RecoveryStepKind::AuditProjection
                    }
                ) && !completed_steps.contains(&RecoveryStepKind::AuditProjection)
                {
                    completed_steps.push(RecoveryStepKind::AuditProjection);
                    continue;
                }
                return Err(PostRestoreRecoveryError::InvalidJournal);
            }

            match &event.event {
                BackupJournalEventKind::RecoveryStateChanged { state: next } => {
                    if state.is_some_and(|current| !valid_state_transition(current, *next)) {
                        return Err(PostRestoreRecoveryError::InvalidJournal);
                    }
                    state = Some(*next);
                    if next.is_terminal() {
                        terminal_event = Some(RecoveryTerminalEvent {
                            event_id: event.event_id,
                            occurred_at: event.occurred_at,
                            state: *next,
                        });
                    }
                }
                BackupJournalEventKind::RecoverySafetyBackupVerified {
                    safety_backup_set_id: current,
                    ..
                } => {
                    if safety_backup_set_id.replace(*current).is_some() {
                        return Err(PostRestoreRecoveryError::InvalidJournal);
                    }
                }
                BackupJournalEventKind::RecoveryOfflineHandoffReady {
                    target_backup_set_id,
                    safety_backup_set_id: current,
                    ..
                } => {
                    if handoff_ready
                        || *target_backup_set_id != command.backup_set_id
                        || safety_backup_set_id != Some(*current)
                    {
                        return Err(PostRestoreRecoveryError::InvalidJournal);
                    }
                    handoff_ready = true;
                }
                BackupJournalEventKind::RecoveryBootstrapActorAssigned {
                    actor_user_id: assigned,
                } => {
                    if !handoff_ready || bootstrap_actor_assigned || *assigned != actor {
                        return Err(PostRestoreRecoveryError::InvalidJournal);
                    }
                    bootstrap_actor_assigned = true;
                }
                BackupJournalEventKind::RecoveryStepCompleted { step } => {
                    let valid = match step {
                        RecoveryStepKind::PostgreSql => completed_steps.is_empty(),
                        RecoveryStepKind::BusinessObjects => {
                            completed_steps.as_slice() == [RecoveryStepKind::PostgreSql]
                        }
                        RecoveryStepKind::ExtensionArtifacts => {
                            completed_steps.as_slice()
                                == [
                                    RecoveryStepKind::PostgreSql,
                                    RecoveryStepKind::BusinessObjects,
                                ]
                        }
                        RecoveryStepKind::Reconcile => {
                            completed_steps.as_slice() == OFFLINE_RESTORE_STEPS
                        }
                        RecoveryStepKind::HealthVerification => {
                            completed_steps.as_slice()
                                == [
                                    RecoveryStepKind::PostgreSql,
                                    RecoveryStepKind::BusinessObjects,
                                    RecoveryStepKind::ExtensionArtifacts,
                                    RecoveryStepKind::Reconcile,
                                ]
                        }
                        RecoveryStepKind::AuditProjection => false,
                    };
                    if !valid {
                        return Err(PostRestoreRecoveryError::InvalidJournal);
                    }
                    completed_steps.push(*step);
                }
                BackupJournalEventKind::RecoveryIntentConfirmed {
                    target_backup_set_id,
                    ..
                } if *target_backup_set_id != command.backup_set_id => {
                    return Err(PostRestoreRecoveryError::InvalidJournal)
                }
                BackupJournalEventKind::TerminalFailure { code } => {
                    if code.is_empty() {
                        return Err(PostRestoreRecoveryError::InvalidJournal);
                    }
                    last_failure_code = Some(code.clone());
                }
                _ => {}
            }
        }

        let state = state.ok_or(PostRestoreRecoveryError::InvalidJournal)?;
        let reconciled = completed_steps.contains(&RecoveryStepKind::Reconcile);
        let health_verified = completed_steps.contains(&RecoveryStepKind::HealthVerification);
        let audit_projected = completed_steps.contains(&RecoveryStepKind::AuditProjection);
        let offline_restore_completed = completed_steps.starts_with(&OFFLINE_RESTORE_STEPS);
        let expectation_invalid = match expectation {
            JournalProgressExpectation::PostRestoreOrTerminal => match state {
                RecoveryJobState::Reconciling => {
                    !offline_restore_completed || health_verified || audit_projected
                }
                RecoveryJobState::Verifying => {
                    !offline_restore_completed || !reconciled || audit_projected
                }
                RecoveryJobState::Succeeded => {
                    !offline_restore_completed || !reconciled || !health_verified
                }
                RecoveryJobState::RolledBack | RecoveryJobState::ManualRecoveryRequired => false,
                _ => true,
            },
            JournalProgressExpectation::OfflineFailure => {
                !matches!(
                    state,
                    RecoveryJobState::Draining | RecoveryJobState::Restoring
                ) || reconciled
                    || health_verified
                    || audit_projected
                    || terminal_event.is_some()
                    || (state == RecoveryJobState::Draining && !completed_steps.is_empty())
            }
        };
        if !handoff_ready
            || expectation_invalid
            || (state.is_terminal() != terminal_event.is_some())
            || (audit_projected && !state.is_terminal())
            || (matches!(
                state,
                RecoveryJobState::RolledBack | RecoveryJobState::ManualRecoveryRequired
            ) && last_failure_code.is_none())
        {
            return Err(PostRestoreRecoveryError::InvalidJournal);
        }

        Ok(Self {
            command,
            actor_user_id: actor_user_id.ok_or(PostRestoreRecoveryError::InvalidJournal)?,
            safety_backup_set_id: safety_backup_set_id
                .ok_or(PostRestoreRecoveryError::InvalidJournal)?,
            next_sequence: events.len() as u64,
            state,
            completed_steps,
            terminal_event,
            last_failure_code,
        })
    }

    fn context(&self) -> PostRestoreRecoveryContext {
        PostRestoreRecoveryContext {
            recovery_job_id: self.command.recovery_job_id,
            backup_set_id: self.command.backup_set_id,
            safety_backup_set_id: self.safety_backup_set_id,
            actor_user_id: self.actor_user_id,
        }
    }

    fn receipt(
        &self,
        outcome: PostRestoreRecoveryOutcome,
        failure_code: Option<String>,
    ) -> PostRestoreRecoveryReceipt {
        PostRestoreRecoveryReceipt {
            recovery_job_id: self.command.recovery_job_id,
            backup_set_id: self.command.backup_set_id,
            outcome,
            failure_code,
        }
    }

    async fn append_step(
        &mut self,
        repository: &dyn BackupRepository,
        step: RecoveryStepKind,
    ) -> Result<(), PostRestoreRecoveryError> {
        self.append(
            repository,
            BackupJournalEventKind::RecoveryStepCompleted { step },
        )
        .await?;
        self.completed_steps.push(step);
        Ok(())
    }

    async fn append_state(
        &mut self,
        repository: &dyn BackupRepository,
        state: RecoveryJobState,
    ) -> Result<(), PostRestoreRecoveryError> {
        let event = self
            .append(
                repository,
                BackupJournalEventKind::RecoveryStateChanged { state },
            )
            .await?;
        self.state = state;
        if state.is_terminal() {
            self.terminal_event = Some(RecoveryTerminalEvent {
                event_id: event.event_id,
                occurred_at: event.occurred_at,
                state,
            });
        }
        Ok(())
    }

    async fn append_failure(
        &mut self,
        repository: &dyn BackupRepository,
        code: &'static str,
    ) -> Result<(), PostRestoreRecoveryError> {
        if self.last_failure_code.as_deref() == Some(code) {
            return Ok(());
        }
        self.append(
            repository,
            BackupJournalEventKind::TerminalFailure {
                code: code.to_string(),
            },
        )
        .await?;
        self.last_failure_code = Some(code.to_string());
        Ok(())
    }

    async fn append(
        &mut self,
        repository: &dyn BackupRepository,
        event: BackupJournalEventKind,
    ) -> Result<BackupJournalEvent, PostRestoreRecoveryError> {
        let event = BackupJournalEvent {
            event_id: Uuid::now_v7(),
            sequence: self.next_sequence,
            subject: BackupJournalSubject::Recovery(self.command.recovery_job_id),
            backup_set_id: self.command.backup_set_id,
            actor_user_id: Some(self.actor_user_id),
            occurred_at: OffsetDateTime::now_utc(),
            event,
        };
        repository
            .append_journal_event(&event)
            .await
            .map_err(|_| PostRestoreRecoveryError::Journal)?;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(PostRestoreRecoveryError::InvalidJournal)?;
        Ok(event)
    }
}

fn valid_state_transition(current: RecoveryJobState, next: RecoveryJobState) -> bool {
    matches!(
        (current, next),
        (
            RecoveryJobState::Preflight,
            RecoveryJobState::AwaitingConfirmation
        ) | (
            RecoveryJobState::AwaitingConfirmation,
            RecoveryJobState::SafetyBackup
        ) | (RecoveryJobState::SafetyBackup, RecoveryJobState::Fencing)
            | (RecoveryJobState::Fencing, RecoveryJobState::Draining)
            | (RecoveryJobState::Draining, RecoveryJobState::Restoring)
            | (RecoveryJobState::Restoring, RecoveryJobState::Reconciling)
            | (RecoveryJobState::Reconciling, RecoveryJobState::Verifying)
            | (RecoveryJobState::Verifying, RecoveryJobState::Succeeded)
            | (RecoveryJobState::Draining, RecoveryJobState::RolledBack)
            | (RecoveryJobState::Restoring, RecoveryJobState::RolledBack)
            | (RecoveryJobState::Reconciling, RecoveryJobState::RolledBack)
            | (RecoveryJobState::Verifying, RecoveryJobState::RolledBack)
    ) || (!current.is_terminal()
        && !matches!(
            current,
            RecoveryJobState::Preflight | RecoveryJobState::AwaitingConfirmation
        )
        && next == RecoveryJobState::ManualRecoveryRequired)
}

fn offline_failure_settlement(error: &OfflineRecoveryError) -> OfflineFailureSettlement {
    match error {
        OfflineRecoveryError::Repository
        | OfflineRecoveryError::Key
        | OfflineRecoveryError::Manifest
        | OfflineRecoveryError::Component
        | OfflineRecoveryError::Step { .. } => OfflineFailureSettlement::RolledBack,
        OfflineRecoveryError::Journal
        | OfflineRecoveryError::InvalidJournal
        | OfflineRecoveryError::Compensation { .. } => {
            OfflineFailureSettlement::ManualRecoveryRequired
        }
    }
}

fn offline_failure_code(error: &OfflineRecoveryError) -> &'static str {
    match error {
        OfflineRecoveryError::Journal => "offline_restore_journal_failed",
        OfflineRecoveryError::InvalidJournal => "offline_restore_invalid_journal",
        OfflineRecoveryError::Repository => "offline_restore_repository_failed",
        OfflineRecoveryError::Key => "offline_restore_key_failed",
        OfflineRecoveryError::Manifest => "offline_restore_manifest_failed",
        OfflineRecoveryError::Component => "offline_restore_component_failed",
        OfflineRecoveryError::Step { step } => offline_step_failure_code(*step),
        OfflineRecoveryError::Compensation { step } => offline_compensation_failure_code(*step),
    }
}

fn offline_step_failure_code(step: RecoveryStepKind) -> &'static str {
    match step {
        RecoveryStepKind::PostgreSql => "offline_restore_postgresql_failed",
        RecoveryStepKind::BusinessObjects => "offline_restore_business_objects_failed",
        RecoveryStepKind::ExtensionArtifacts => "offline_restore_extension_artifacts_failed",
        RecoveryStepKind::Reconcile => "offline_restore_reconcile_failed",
        RecoveryStepKind::HealthVerification => "offline_restore_health_verification_failed",
        RecoveryStepKind::AuditProjection => "offline_restore_audit_projection_failed",
    }
}

fn offline_compensation_failure_code(step: RecoveryStepKind) -> &'static str {
    match step {
        RecoveryStepKind::PostgreSql => "offline_restore_postgresql_compensation_failed",
        RecoveryStepKind::BusinessObjects => "offline_restore_business_objects_compensation_failed",
        RecoveryStepKind::ExtensionArtifacts => {
            "offline_restore_extension_artifacts_compensation_failed"
        }
        RecoveryStepKind::Reconcile => "offline_restore_reconcile_compensation_failed",
        RecoveryStepKind::HealthVerification => {
            "offline_restore_health_verification_compensation_failed"
        }
        RecoveryStepKind::AuditProjection => "offline_restore_audit_projection_compensation_failed",
    }
}
