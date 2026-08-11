use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    BackupJournalEvent, BackupJournalEventKind, BackupJournalSubject, BackupSetId, RecoveryJobId,
    RecoveryJobState, RecoveryStepKind,
};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::ports::BackupRepository;

use super::{ExecuteOfflineRecoveryCommand, OfflineRecoveryExecutor, OfflineRecoveryHandoff};

const OFFLINE_RESTORE_STEPS: [RecoveryStepKind; 3] = [
    RecoveryStepKind::PostgreSql,
    RecoveryStepKind::BusinessObjects,
    RecoveryStepKind::ExtensionArtifacts,
];
const ALL_RECOVERY_STEPS: [RecoveryStepKind; 6] = [
    RecoveryStepKind::PostgreSql,
    RecoveryStepKind::BusinessObjects,
    RecoveryStepKind::ExtensionArtifacts,
    RecoveryStepKind::Reconcile,
    RecoveryStepKind::HealthVerification,
    RecoveryStepKind::AuditProjection,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryAuditProjection {
    pub audit_id: Uuid,
    pub source_event_id: Uuid,
    pub recovery_job_id: RecoveryJobId,
    pub backup_set_id: BackupSetId,
    pub safety_backup_set_id: BackupSetId,
    pub actor_user_id: Uuid,
    pub verified_at: OffsetDateTime,
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
            RecoveryJobState::Succeeded => {
                lease.release();
                return Ok(progress.receipt(PostRestoreRecoveryOutcome::Succeeded, None));
            }
            RecoveryJobState::RolledBack => {
                lease.release();
                return Ok(progress.receipt(
                    PostRestoreRecoveryOutcome::RolledBack,
                    progress.last_failure_code.clone(),
                ));
            }
            RecoveryJobState::ManualRecoveryRequired => {
                lease.retain();
                return Err(PostRestoreRecoveryError::ManualRecoveryRequired {
                    code: "manual_recovery_required",
                });
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

        if !progress
            .completed_steps
            .contains(&RecoveryStepKind::AuditProjection)
        {
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
            let Some((source_event_id, verified_at)) = progress.health_verification_event else {
                lease.retain();
                return Err(PostRestoreRecoveryError::InvalidJournal);
            };
            let projection = RecoveryAuditProjection {
                audit_id: source_event_id,
                source_event_id,
                recovery_job_id: context.recovery_job_id,
                backup_set_id: context.backup_set_id,
                safety_backup_set_id: context.safety_backup_set_id,
                actor_user_id: context.actor_user_id,
                verified_at,
            };
            if self.audit.project(&projection).await.is_err() {
                return self
                    .fail_and_rollback(&mut progress, lease, "post_restore_audit_projection_failed")
                    .await;
            }
            if progress
                .append_step(self.repository.as_ref(), RecoveryStepKind::AuditProjection)
                .await
                .is_err()
            {
                // Projection uses the health journal event id, so a retry is safe and cannot
                // duplicate the durable audit row.
                lease.retain();
                return Err(PostRestoreRecoveryError::Journal);
            }
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

        lease.release();
        Ok(progress.receipt(PostRestoreRecoveryOutcome::Succeeded, None))
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
            lease.release();
            return Ok(progress.receipt(
                PostRestoreRecoveryOutcome::RolledBack,
                Some(failure_code.to_string()),
            ));
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
        let _ = progress
            .append_failure(self.repository.as_ref(), code)
            .await;
        let _ = progress
            .append_state(
                self.repository.as_ref(),
                RecoveryJobState::ManualRecoveryRequired,
            )
            .await;
        lease.retain();
        Err(PostRestoreRecoveryError::ManualRecoveryRequired { code })
    }
}

struct PostRestoreJournalProgress {
    command: ExecuteOfflineRecoveryCommand,
    actor_user_id: Uuid,
    safety_backup_set_id: BackupSetId,
    next_sequence: u64,
    state: RecoveryJobState,
    completed_steps: Vec<RecoveryStepKind>,
    health_verification_event: Option<(Uuid, OffsetDateTime)>,
    last_failure_code: Option<String>,
}

impl PostRestoreJournalProgress {
    fn try_from_events(
        command: ExecuteOfflineRecoveryCommand,
        events: &[BackupJournalEvent],
    ) -> Result<Self, PostRestoreRecoveryError> {
        let subject = BackupJournalSubject::Recovery(command.recovery_job_id);
        let mut actor_user_id = None;
        let mut safety_backup_set_id = None;
        let mut handoff_ready = false;
        let mut state = None;
        let mut completed_steps = Vec::new();
        let mut health_verification_event = None;
        let mut last_failure_code = None;
        let mut terminal_seen = false;

        for (index, event) in events.iter().enumerate() {
            if terminal_seen
                || event.sequence != index as u64
                || event.subject != subject
                || event.backup_set_id != command.backup_set_id
            {
                return Err(PostRestoreRecoveryError::InvalidJournal);
            }
            let Some(actor) = event.actor_user_id else {
                return Err(PostRestoreRecoveryError::InvalidJournal);
            };
            if actor_user_id
                .replace(actor)
                .is_some_and(|current| current != actor)
            {
                return Err(PostRestoreRecoveryError::InvalidJournal);
            }

            match &event.event {
                BackupJournalEventKind::RecoveryStateChanged { state: next } => {
                    if state.is_some_and(|current| !valid_state_transition(current, *next)) {
                        return Err(PostRestoreRecoveryError::InvalidJournal);
                    }
                    state = Some(*next);
                    terminal_seen = next.is_terminal();
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
                BackupJournalEventKind::RecoveryStepCompleted { step } => {
                    if completed_steps.len() >= ALL_RECOVERY_STEPS.len()
                        || *step != ALL_RECOVERY_STEPS[completed_steps.len()]
                    {
                        return Err(PostRestoreRecoveryError::InvalidJournal);
                    }
                    completed_steps.push(*step);
                    if *step == RecoveryStepKind::HealthVerification {
                        health_verification_event = Some((event.event_id, event.occurred_at));
                    }
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
        if !handoff_ready
            || completed_steps.len() < OFFLINE_RESTORE_STEPS.len()
            || completed_steps[..OFFLINE_RESTORE_STEPS.len()] != OFFLINE_RESTORE_STEPS
            || matches!(state, RecoveryJobState::Reconciling)
                && completed_steps.len() > OFFLINE_RESTORE_STEPS.len() + 1
            || matches!(state, RecoveryJobState::Verifying)
                && !completed_steps.contains(&RecoveryStepKind::Reconcile)
            || matches!(state, RecoveryJobState::Succeeded)
                && completed_steps.len() != ALL_RECOVERY_STEPS.len()
            || completed_steps.contains(&RecoveryStepKind::HealthVerification)
                && health_verification_event.is_none()
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
            health_verification_event,
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
        let event = self
            .append(
                repository,
                BackupJournalEventKind::RecoveryStepCompleted { step },
            )
            .await?;
        self.completed_steps.push(step);
        if step == RecoveryStepKind::HealthVerification {
            self.health_verification_event = Some((event.event_id, event.occurred_at));
        }
        Ok(())
    }

    async fn append_state(
        &mut self,
        repository: &dyn BackupRepository,
        state: RecoveryJobState,
    ) -> Result<(), PostRestoreRecoveryError> {
        self.append(
            repository,
            BackupJournalEventKind::RecoveryStateChanged { state },
        )
        .await?;
        self.state = state;
        Ok(())
    }

    async fn append_failure(
        &mut self,
        repository: &dyn BackupRepository,
        code: &'static str,
    ) -> Result<(), PostRestoreRecoveryError> {
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
