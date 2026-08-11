use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use domain::{
    BackupJournalEvent, BackupJournalEventKind, BackupJournalSubject, BackupSetId, ContentDigest,
    RecoveryJob, RecoveryJobId, RecoveryJobState,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    ports::BackupRepository,
    system_backup::{BackupComponentSource, CreateSystemBackupCommand, SystemBackupService},
};

use super::{
    RecoveryPlan, RecoveryPreflightFailure, RecoveryPreflightService, SystemMaintenance,
    SystemMaintenanceLease,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedRecoveryIntent {
    intent_id: Uuid,
    recovery_job_id: RecoveryJobId,
    actor_user_id: Uuid,
    backup_set_id: BackupSetId,
    plan_digest: ContentDigest,
    confirmed_at: OffsetDateTime,
    expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum ConfirmedRecoveryIntentError {
    #[error("confirmed recovery intent expiry must be later than confirmation")]
    InvalidExpiry,
}

impl ConfirmedRecoveryIntent {
    pub fn try_new(
        intent_id: Uuid,
        recovery_job_id: RecoveryJobId,
        actor_user_id: Uuid,
        backup_set_id: BackupSetId,
        plan_digest: ContentDigest,
        confirmed_at: OffsetDateTime,
        expires_at: OffsetDateTime,
    ) -> Result<Self, ConfirmedRecoveryIntentError> {
        if expires_at <= confirmed_at {
            return Err(ConfirmedRecoveryIntentError::InvalidExpiry);
        }
        Ok(Self {
            intent_id,
            recovery_job_id,
            actor_user_id,
            backup_set_id,
            plan_digest,
            confirmed_at,
            expires_at,
        })
    }

    pub const fn recovery_job_id(&self) -> RecoveryJobId {
        self.recovery_job_id
    }

    pub const fn actor_user_id(&self) -> Uuid {
        self.actor_user_id
    }

    pub const fn backup_set_id(&self) -> BackupSetId {
        self.backup_set_id
    }

    pub fn plan_digest(&self) -> &ContentDigest {
        &self.plan_digest
    }
}

pub struct PrepareRecoveryCommand {
    pub intent: ConfirmedRecoveryIntent,
    pub safety_backup_command: CreateSystemBackupCommand,
    pub safety_backup_sources: Vec<Arc<dyn BackupComponentSource>>,
    pub drain_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineRecoveryHandoffReady {
    pub recovery_job_id: RecoveryJobId,
    pub target_backup_set_id: BackupSetId,
    pub safety_backup_set_id: BackupSetId,
    pub plan_digest: ContentDigest,
}

pub struct OfflineRecoveryHandoff {
    pub job: RecoveryJob,
    pub plan: RecoveryPlan,
    pub plan_digest: ContentDigest,
    lease: SystemMaintenanceLease,
}

impl OfflineRecoveryHandoff {
    pub fn abort(self) {
        self.lease.finish();
    }
}

struct ActiveRecoveryHandoff {
    ready: OfflineRecoveryHandoffReady,
    handoff: OfflineRecoveryHandoff,
}

#[derive(Debug, Error)]
pub enum RecoveryCoordinatorError {
    #[error("confirmed recovery intent has expired")]
    IntentExpired,
    #[error("confirmed recovery intent is not bound to the current recovery plan")]
    IntentPlanMismatch,
    #[error("safety backup actor does not match the confirmed recovery actor")]
    ActorMismatch,
    #[error("recovery target is no longer compatible")]
    IncompatiblePlan(Vec<RecoveryPreflightFailure>),
    #[error("recovery plan digest could not be produced")]
    PlanDigest,
    #[error("recovery journal is unavailable")]
    Journal,
    #[error("recovery job state could not advance")]
    JobState,
    #[error("system maintenance lease is unavailable")]
    MaintenanceBusy,
    #[error("system writes did not drain")]
    Drain,
    #[error("safety backup creation or verification failed")]
    SafetyBackup,
    #[error("an offline recovery handoff is already active")]
    HandoffActive,
    #[error("offline recovery handoff was not found")]
    HandoffNotFound,
}

pub struct RecoveryCoordinator {
    preflight: Arc<RecoveryPreflightService>,
    backups: Arc<SystemBackupService>,
    repository: Arc<dyn BackupRepository>,
    maintenance: Arc<SystemMaintenance>,
    active_handoff: Mutex<Option<ActiveRecoveryHandoff>>,
}

impl RecoveryCoordinator {
    pub fn new(
        preflight: Arc<RecoveryPreflightService>,
        backups: Arc<SystemBackupService>,
        repository: Arc<dyn BackupRepository>,
        maintenance: Arc<SystemMaintenance>,
    ) -> Self {
        Self {
            preflight,
            backups,
            repository,
            maintenance,
            active_handoff: Mutex::new(None),
        }
    }

    pub async fn prepare_offline_handoff(
        &self,
        command: PrepareRecoveryCommand,
    ) -> Result<OfflineRecoveryHandoffReady, RecoveryCoordinatorError> {
        if command.intent.expires_at < OffsetDateTime::now_utc() {
            return Err(RecoveryCoordinatorError::IntentExpired);
        }
        if command.intent.actor_user_id != command.safety_backup_command.actor_user_id {
            return Err(RecoveryCoordinatorError::ActorMismatch);
        }
        if self
            .active_handoff
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
        {
            return Err(RecoveryCoordinatorError::HandoffActive);
        }

        let plan = self.preflight.plan(command.intent.backup_set_id).await;
        if !plan.is_compatible() {
            return Err(RecoveryCoordinatorError::IncompatiblePlan(plan.failures));
        }
        let plan_digest = recovery_plan_digest(&plan)?;
        if plan_digest != command.intent.plan_digest {
            return Err(RecoveryCoordinatorError::IntentPlanMismatch);
        }

        let mut job = RecoveryJob::new(
            command.intent.recovery_job_id,
            command.intent.backup_set_id,
            OffsetDateTime::now_utc(),
        );
        self.append_state(&job, command.intent.actor_user_id)
            .await?;
        transition_job(&mut job, RecoveryJobState::AwaitingConfirmation)?;
        self.append_state(&job, command.intent.actor_user_id)
            .await?;
        self.append_event(
            &job,
            command.intent.actor_user_id,
            BackupJournalEventKind::RecoveryIntentConfirmed {
                intent_id: command.intent.intent_id,
                target_backup_set_id: command.intent.backup_set_id,
                plan_digest: plan_digest.clone(),
                confirmed_at: command.intent.confirmed_at,
                expires_at: command.intent.expires_at,
            },
        )
        .await?;
        transition_job(&mut job, RecoveryJobState::SafetyBackup)?;
        self.append_state(&job, command.intent.actor_user_id)
            .await?;

        let lease = self
            .maintenance
            .begin(job.job_id(), OffsetDateTime::now_utc())
            .map_err(|_| RecoveryCoordinatorError::MaintenanceBusy)?;
        lease
            .wait_for_drain(command.drain_timeout)
            .await
            .map_err(|_| RecoveryCoordinatorError::Drain)?;

        let safety_manifest = self
            .backups
            .create(command.safety_backup_command, command.safety_backup_sources)
            .await
            .map_err(|_| RecoveryCoordinatorError::SafetyBackup)?;
        let safety_backup_set_id = safety_manifest.manifest().backup_set_id();
        self.backups
            .verify(safety_backup_set_id)
            .await
            .map_err(|_| RecoveryCoordinatorError::SafetyBackup)?;
        job.record_safety_backup(safety_backup_set_id)
            .map_err(|_| RecoveryCoordinatorError::JobState)?;
        self.append_event(
            &job,
            command.intent.actor_user_id,
            BackupJournalEventKind::RecoverySafetyBackupVerified {
                safety_backup_set_id,
                plan_digest: plan_digest.clone(),
            },
        )
        .await?;
        transition_job(&mut job, RecoveryJobState::Fencing)?;
        self.append_state(&job, command.intent.actor_user_id)
            .await?;
        transition_job(&mut job, RecoveryJobState::Draining)?;
        self.append_state(&job, command.intent.actor_user_id)
            .await?;
        self.append_event(
            &job,
            command.intent.actor_user_id,
            BackupJournalEventKind::RecoveryOfflineHandoffReady {
                target_backup_set_id: command.intent.backup_set_id,
                safety_backup_set_id,
                plan_digest: plan_digest.clone(),
            },
        )
        .await?;

        let ready = OfflineRecoveryHandoffReady {
            recovery_job_id: job.job_id(),
            target_backup_set_id: job.backup_set_id(),
            safety_backup_set_id,
            plan_digest: plan_digest.clone(),
        };
        let active = ActiveRecoveryHandoff {
            ready: ready.clone(),
            handoff: OfflineRecoveryHandoff {
                job,
                plan,
                plan_digest,
                lease,
            },
        };
        let mut slot = self
            .active_handoff
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot.is_some() {
            return Err(RecoveryCoordinatorError::HandoffActive);
        }
        *slot = Some(active);
        Ok(ready)
    }

    pub fn active_handoff(&self) -> Option<OfflineRecoveryHandoffReady> {
        self.active_handoff
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|active| active.ready.clone())
    }

    pub fn claim_offline_handoff(
        &self,
        recovery_job_id: RecoveryJobId,
    ) -> Result<OfflineRecoveryHandoff, RecoveryCoordinatorError> {
        let mut slot = self
            .active_handoff
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot
            .as_ref()
            .is_none_or(|active| active.ready.recovery_job_id != recovery_job_id)
        {
            return Err(RecoveryCoordinatorError::HandoffNotFound);
        }
        let Some(active) = slot.take() else {
            return Err(RecoveryCoordinatorError::HandoffNotFound);
        };
        Ok(active.handoff)
    }

    pub fn abort(&self, recovery_job_id: RecoveryJobId) -> Result<(), RecoveryCoordinatorError> {
        self.claim_offline_handoff(recovery_job_id)?.abort();
        Ok(())
    }

    async fn append_state(
        &self,
        job: &RecoveryJob,
        actor_user_id: Uuid,
    ) -> Result<(), RecoveryCoordinatorError> {
        self.append_event(
            job,
            actor_user_id,
            BackupJournalEventKind::RecoveryStateChanged { state: job.state() },
        )
        .await
    }

    async fn append_event(
        &self,
        job: &RecoveryJob,
        actor_user_id: Uuid,
        event: BackupJournalEventKind,
    ) -> Result<(), RecoveryCoordinatorError> {
        let subject = BackupJournalSubject::Recovery(job.job_id());
        let sequence = self
            .repository
            .read_journal(subject)
            .await
            .map_err(|_| RecoveryCoordinatorError::Journal)?
            .last()
            .map_or(0, |event| event.sequence.saturating_add(1));
        self.repository
            .append_journal_event(&BackupJournalEvent {
                event_id: Uuid::now_v7(),
                sequence,
                subject,
                backup_set_id: job.backup_set_id(),
                actor_user_id: Some(actor_user_id),
                occurred_at: OffsetDateTime::now_utc(),
                event,
            })
            .await
            .map_err(|_| RecoveryCoordinatorError::Journal)
    }
}

pub fn recovery_plan_digest(
    plan: &RecoveryPlan,
) -> Result<ContentDigest, RecoveryCoordinatorError> {
    #[derive(Serialize)]
    struct DigestInput<'a> {
        backup_set_id: BackupSetId,
        required_space_bytes: u64,
        available_space_bytes: u64,
        database_replaced: bool,
        business_object_count: u64,
        extension_artifact_count: u64,
        mcp_artifact_count: u64,
        active_work: &'a [super::RecoveryActiveWork],
        failures: &'a [RecoveryPreflightFailure],
    }

    let bytes = serde_json::to_vec(&DigestInput {
        backup_set_id: plan.backup_set_id,
        required_space_bytes: plan.required_space_bytes,
        available_space_bytes: plan.available_space_bytes,
        database_replaced: plan.impact.database_replaced,
        business_object_count: plan.impact.business_object_count,
        extension_artifact_count: plan.impact.extension_artifact_count,
        mcp_artifact_count: plan.impact.mcp_artifact_count,
        active_work: &plan.impact.active_work,
        failures: &plan.failures,
    })
    .map_err(|_| RecoveryCoordinatorError::PlanDigest)?;
    ContentDigest::try_from(format!("{:x}", Sha256::digest(bytes)))
        .map_err(|_| RecoveryCoordinatorError::PlanDigest)
}

fn transition_job(
    job: &mut RecoveryJob,
    state: RecoveryJobState,
) -> Result<(), RecoveryCoordinatorError> {
    job.transition(state, OffsetDateTime::now_utc(), None)
        .map_err(|_| RecoveryCoordinatorError::JobState)
}
