use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    BackupComponent, BackupComponentDisposition, BackupComponentKind, BackupJournalEvent,
    BackupJournalEventKind, BackupJournalSubject, BackupManifest, BackupSetId, MigrationHead,
    RecoveryJobId, RecoveryJobState, RecoveryStepKind,
};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::io::duplex;
use uuid::Uuid;

use crate::{
    ports::{BackupComponentReader, BackupKeyMaterial, BackupKeyProvider, BackupRepository},
    system_backup::{decrypt_backup_stream, verify_backup_manifest},
};

const RECOVERY_PIPE_BUFFER_BYTES: usize = 256 * 1024;
const OFFLINE_RESTORE_STEPS: [RecoveryStepKind; 3] = [
    RecoveryStepKind::PostgreSql,
    RecoveryStepKind::BusinessObjects,
    RecoveryStepKind::ExtensionArtifacts,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryStepContext {
    pub recovery_job_id: RecoveryJobId,
    pub backup_set_id: BackupSetId,
    pub migration_head: MigrationHead,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum RecoveryStepTargetError {
    #[error("recovery target does not match the backup component inventory")]
    InvalidTarget,
    #[error("recovery target staging failed")]
    Staging,
    #[error("recovery target verification failed")]
    Integrity,
    #[error("recovery target promotion failed")]
    Promotion,
    #[error("recovery target compensation failed")]
    Compensation,
    #[error("recovery target is unavailable")]
    Unavailable,
}

/// A single offline restore step with durable staging and compensating rollback.
///
/// Implementations must not depend on Axum state or the primary application pool. `begin` and
/// `stage_component` may be repeated after interruption. `promote` must either leave the previous
/// target recoverable or return an error that `rollback` can compensate.
#[async_trait]
pub trait RecoveryStepTarget: Send + Sync {
    async fn begin(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError>;

    async fn stage_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
        plaintext: BackupComponentReader,
    ) -> Result<(), RecoveryStepTargetError>;

    /// Verifies that a rebuildable identity-only artifact can be reconstructed by the local
    /// installation/catalog inventory. This method must not download a missing artifact.
    async fn stage_identity(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError>;

    async fn promote(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError>;

    async fn rollback(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError>;

    /// Removes staging and rollback material only after post-restore health has passed.
    async fn finalize(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError>;
}

pub struct OfflineRecoveryTargets {
    pub postgres: Arc<dyn RecoveryStepTarget>,
    pub business_objects: Arc<dyn RecoveryStepTarget>,
    pub extension_artifacts: Arc<dyn RecoveryStepTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecuteOfflineRecoveryCommand {
    pub recovery_job_id: RecoveryJobId,
    pub backup_set_id: BackupSetId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineRecoveryReceipt {
    pub recovery_job_id: RecoveryJobId,
    pub backup_set_id: BackupSetId,
    pub executed_steps: Vec<RecoveryStepKind>,
    pub resumed_steps: Vec<RecoveryStepKind>,
}

#[derive(Debug, Error)]
pub enum OfflineRecoveryError {
    #[error("external recovery journal is unavailable")]
    Journal,
    #[error("external recovery journal is invalid")]
    InvalidJournal,
    #[error("backup repository is unavailable")]
    Repository,
    #[error("backup key is unavailable")]
    Key,
    #[error("backup manifest authentication failed")]
    Manifest,
    #[error("backup component is missing or corrupt")]
    Component,
    #[error("offline recovery step {step:?} failed")]
    Step { step: RecoveryStepKind },
    #[error("offline recovery step {step:?} could not be compensated")]
    Compensation { step: RecoveryStepKind },
}

pub struct OfflineRecoveryExecutor {
    repository: Arc<dyn BackupRepository>,
    key_provider: Arc<dyn BackupKeyProvider>,
    targets: OfflineRecoveryTargets,
}

impl OfflineRecoveryExecutor {
    pub fn new(
        repository: Arc<dyn BackupRepository>,
        key_provider: Arc<dyn BackupKeyProvider>,
        targets: OfflineRecoveryTargets,
    ) -> Self {
        Self {
            repository,
            key_provider,
            targets,
        }
    }

    pub async fn execute(
        &self,
        command: ExecuteOfflineRecoveryCommand,
    ) -> Result<OfflineRecoveryReceipt, OfflineRecoveryError> {
        let subject = BackupJournalSubject::Recovery(command.recovery_job_id);
        let events = self
            .repository
            .read_journal(subject)
            .await
            .map_err(|_| OfflineRecoveryError::Journal)?;
        let mut progress = JournalProgress::try_from_events(command, &events)?;
        let sealed = self
            .repository
            .load_manifest(command.backup_set_id)
            .await
            .map_err(|_| OfflineRecoveryError::Repository)?;
        let key = self
            .key_provider
            .key_for(sealed.manifest().backup_key_fingerprint())
            .await
            .map_err(|_| OfflineRecoveryError::Key)?;
        verify_backup_manifest(&sealed, &key).map_err(|_| OfflineRecoveryError::Manifest)?;
        validate_restore_inventory(sealed.manifest())?;
        if progress.state == RecoveryJobState::Draining {
            progress
                .append(
                    self.repository.as_ref(),
                    BackupJournalEventKind::RecoveryStateChanged {
                        state: RecoveryJobState::Restoring,
                    },
                )
                .await?;
            progress.state = RecoveryJobState::Restoring;
        }

        let context = RecoveryStepContext {
            recovery_job_id: command.recovery_job_id,
            backup_set_id: command.backup_set_id,
            migration_head: sealed.manifest().migration_head().clone(),
        };
        let mut executed_steps = Vec::new();
        let mut resumed_steps = Vec::new();
        for step in OFFLINE_RESTORE_STEPS {
            if progress.completed_steps.contains(&step) {
                resumed_steps.push(step);
                continue;
            }
            let components = components_for_step(sealed.manifest(), step);
            let target = self.target(step)?;
            if let Err(error) = self
                .execute_step(target, &context, &components, &key, step)
                .await
            {
                return self
                    .record_failure_and_compensate(
                        &mut progress,
                        sealed.manifest(),
                        &context,
                        step,
                        error,
                    )
                    .await;
            }
            if let Err(error) = progress
                .append(
                    self.repository.as_ref(),
                    BackupJournalEventKind::RecoveryStepCompleted { step },
                )
                .await
            {
                let current = match target.rollback(&context, &components).await {
                    Ok(()) => error,
                    Err(_) => OfflineRecoveryError::Compensation { step },
                };
                return match self
                    .compensate_completed(sealed.manifest(), &context, &progress.completed_steps)
                    .await
                {
                    Ok(()) => Err(current),
                    Err(step) => Err(OfflineRecoveryError::Compensation { step }),
                };
            }
            progress.completed_steps.push(step);
            executed_steps.push(step);
        }

        if progress.state == RecoveryJobState::Restoring {
            progress
                .append(
                    self.repository.as_ref(),
                    BackupJournalEventKind::RecoveryStateChanged {
                        state: RecoveryJobState::Reconciling,
                    },
                )
                .await?;
        }
        Ok(OfflineRecoveryReceipt {
            recovery_job_id: command.recovery_job_id,
            backup_set_id: command.backup_set_id,
            executed_steps,
            resumed_steps,
        })
    }

    /// Restores every promoted target from retained rollback material.
    ///
    /// B5 invokes this after reconcile or health failure, before journaling `rolled_back` or
    /// `manual_recovery_required`.
    pub async fn rollback_promoted_targets(
        &self,
        command: ExecuteOfflineRecoveryCommand,
    ) -> Result<(), OfflineRecoveryError> {
        let (manifest, context) = self.load_completed_restore_inventory(command).await?;
        match self
            .compensate_completed(&manifest, &context, &OFFLINE_RESTORE_STEPS)
            .await
        {
            Ok(()) => Ok(()),
            Err(step) => Err(OfflineRecoveryError::Compensation { step }),
        }
    }

    /// Deletes rollback and staging material only after B5 has passed post-restore health checks.
    pub async fn finalize_promoted_targets(
        &self,
        command: ExecuteOfflineRecoveryCommand,
    ) -> Result<(), OfflineRecoveryError> {
        let (manifest, context) = self.load_completed_restore_inventory(command).await?;
        for step in OFFLINE_RESTORE_STEPS {
            let target = self.target(step)?;
            let components = components_for_step(&manifest, step);
            target
                .finalize(&context, &components)
                .await
                .map_err(|_| OfflineRecoveryError::Step { step })?;
        }
        Ok(())
    }

    async fn load_completed_restore_inventory(
        &self,
        command: ExecuteOfflineRecoveryCommand,
    ) -> Result<(BackupManifest, RecoveryStepContext), OfflineRecoveryError> {
        let events = self
            .repository
            .read_journal(BackupJournalSubject::Recovery(command.recovery_job_id))
            .await
            .map_err(|_| OfflineRecoveryError::Journal)?;
        validate_completed_restore_journal(command, &events)?;
        let sealed = self
            .repository
            .load_manifest(command.backup_set_id)
            .await
            .map_err(|_| OfflineRecoveryError::Repository)?;
        let key = self
            .key_provider
            .key_for(sealed.manifest().backup_key_fingerprint())
            .await
            .map_err(|_| OfflineRecoveryError::Key)?;
        verify_backup_manifest(&sealed, &key).map_err(|_| OfflineRecoveryError::Manifest)?;
        validate_restore_inventory(sealed.manifest())?;
        let manifest = sealed.into_manifest();
        let context = RecoveryStepContext {
            recovery_job_id: command.recovery_job_id,
            backup_set_id: command.backup_set_id,
            migration_head: manifest.migration_head().clone(),
        };
        Ok((manifest, context))
    }

    async fn execute_step(
        &self,
        target: &dyn RecoveryStepTarget,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
        key: &BackupKeyMaterial,
        step: RecoveryStepKind,
    ) -> Result<(), OfflineRecoveryError> {
        if target.begin(context, components).await.is_err() {
            return self
                .rollback_after_error(target, context, components, step)
                .await;
        }
        for component in components {
            if component.disposition == BackupComponentDisposition::IdentityOnly {
                if target.stage_identity(context, component).await.is_err() {
                    return self
                        .rollback_after_error(target, context, components, step)
                        .await;
                }
                continue;
            }
            let encrypted = match self
                .repository
                .open_component(context.backup_set_id, &component.component_id)
                .await
            {
                Ok(encrypted) => encrypted,
                Err(_) => {
                    return self
                        .rollback_after_error(target, context, components, step)
                        .await
                }
            };
            let (plaintext_reader, plaintext_writer) = duplex(RECOVERY_PIPE_BUFFER_BYTES);
            let decrypt = decrypt_backup_stream(
                encrypted,
                plaintext_writer,
                key,
                context.backup_set_id,
                &component.component_id,
            );
            let stage = target.stage_component(context, component, Box::pin(plaintext_reader));
            let (decrypted, staged) = tokio::join!(decrypt, stage);
            let valid = matches!(
                (decrypted, staged),
                (Ok(receipt), Ok(()))
                    if receipt.plaintext_size_bytes == component.size_bytes
                        && receipt.plaintext_digest == component.content_digest
            );
            if !valid {
                return self
                    .rollback_after_error(target, context, components, step)
                    .await;
            }
        }
        if target.promote(context, components).await.is_err() {
            return self
                .rollback_after_error(target, context, components, step)
                .await;
        }
        Ok(())
    }

    async fn rollback_after_error(
        &self,
        target: &dyn RecoveryStepTarget,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
        step: RecoveryStepKind,
    ) -> Result<(), OfflineRecoveryError> {
        match target.rollback(context, components).await {
            Ok(()) => Err(OfflineRecoveryError::Step { step }),
            Err(_) => Err(OfflineRecoveryError::Compensation { step }),
        }
    }

    async fn record_failure_and_compensate(
        &self,
        progress: &mut JournalProgress,
        manifest: &BackupManifest,
        context: &RecoveryStepContext,
        failed_step: RecoveryStepKind,
        error: OfflineRecoveryError,
    ) -> Result<OfflineRecoveryReceipt, OfflineRecoveryError> {
        let journaled = progress
            .append(
                self.repository.as_ref(),
                BackupJournalEventKind::TerminalFailure {
                    code: format!("offline_restore_{}_failed", step_code(failed_step)),
                },
            )
            .await;
        let failure = if journaled.is_err() {
            OfflineRecoveryError::Journal
        } else {
            error
        };
        match self
            .compensate_completed(manifest, context, &progress.completed_steps)
            .await
        {
            Ok(()) => Err(failure),
            Err(step) => {
                let _ = progress
                    .append(
                        self.repository.as_ref(),
                        BackupJournalEventKind::TerminalFailure {
                            code: format!(
                                "offline_restore_{}_compensation_failed",
                                step_code(step)
                            ),
                        },
                    )
                    .await;
                Err(OfflineRecoveryError::Compensation { step })
            }
        }
    }

    async fn compensate_completed(
        &self,
        manifest: &BackupManifest,
        context: &RecoveryStepContext,
        completed_steps: &[RecoveryStepKind],
    ) -> Result<(), RecoveryStepKind> {
        let mut compensation_failed = None;
        for step in completed_steps.iter().rev().copied() {
            let target = match self.target(step) {
                Ok(target) => target,
                Err(_) => {
                    compensation_failed = Some(step);
                    continue;
                }
            };
            let components = components_for_step(manifest, step);
            if target.rollback(context, &components).await.is_err() {
                compensation_failed = Some(step);
            }
        }
        match compensation_failed {
            Some(step) => Err(step),
            None => Ok(()),
        }
    }

    fn target(
        &self,
        step: RecoveryStepKind,
    ) -> Result<&dyn RecoveryStepTarget, OfflineRecoveryError> {
        match step {
            RecoveryStepKind::PostgreSql => Ok(self.targets.postgres.as_ref()),
            RecoveryStepKind::BusinessObjects => Ok(self.targets.business_objects.as_ref()),
            RecoveryStepKind::ExtensionArtifacts => Ok(self.targets.extension_artifacts.as_ref()),
            RecoveryStepKind::Reconcile
            | RecoveryStepKind::HealthVerification
            | RecoveryStepKind::AuditProjection => Err(OfflineRecoveryError::InvalidJournal),
        }
    }
}

struct JournalProgress {
    command: ExecuteOfflineRecoveryCommand,
    actor_user_id: Uuid,
    next_sequence: u64,
    state: RecoveryJobState,
    completed_steps: Vec<RecoveryStepKind>,
}

impl JournalProgress {
    fn try_from_events(
        command: ExecuteOfflineRecoveryCommand,
        events: &[BackupJournalEvent],
    ) -> Result<Self, OfflineRecoveryError> {
        let subject = BackupJournalSubject::Recovery(command.recovery_job_id);
        let mut actor_user_id = None;
        let mut state = None;
        let mut handoff_ready = false;
        let mut completed_steps = Vec::new();
        for (index, event) in events.iter().enumerate() {
            if event.sequence != index as u64
                || event.subject != subject
                || event.backup_set_id != command.backup_set_id
            {
                return Err(OfflineRecoveryError::InvalidJournal);
            }
            if let Some(actor) = event.actor_user_id {
                if actor_user_id
                    .replace(actor)
                    .is_some_and(|current| current != actor)
                {
                    return Err(OfflineRecoveryError::InvalidJournal);
                }
            }
            match &event.event {
                BackupJournalEventKind::RecoveryStateChanged { state: next } => {
                    state = Some(*next);
                }
                BackupJournalEventKind::RecoveryOfflineHandoffReady {
                    target_backup_set_id,
                    ..
                } => {
                    if *target_backup_set_id != command.backup_set_id || handoff_ready {
                        return Err(OfflineRecoveryError::InvalidJournal);
                    }
                    handoff_ready = true;
                }
                BackupJournalEventKind::RecoveryStepCompleted { step } => {
                    if !OFFLINE_RESTORE_STEPS.contains(step)
                        || completed_steps.contains(step)
                        || *step != OFFLINE_RESTORE_STEPS[completed_steps.len()]
                    {
                        return Err(OfflineRecoveryError::InvalidJournal);
                    }
                    completed_steps.push(*step);
                }
                BackupJournalEventKind::TerminalFailure { .. } => {
                    return Err(OfflineRecoveryError::InvalidJournal)
                }
                _ => {}
            }
        }
        let actor_user_id = actor_user_id.ok_or(OfflineRecoveryError::InvalidJournal)?;
        let state = state.ok_or(OfflineRecoveryError::InvalidJournal)?;
        if !handoff_ready
            || !matches!(
                state,
                RecoveryJobState::Draining
                    | RecoveryJobState::Restoring
                    | RecoveryJobState::Reconciling
            )
            || (state == RecoveryJobState::Draining && !completed_steps.is_empty())
            || (state == RecoveryJobState::Reconciling
                && completed_steps.len() != OFFLINE_RESTORE_STEPS.len())
        {
            return Err(OfflineRecoveryError::InvalidJournal);
        }
        Ok(Self {
            command,
            actor_user_id,
            next_sequence: events.len() as u64,
            state,
            completed_steps,
        })
    }

    async fn append(
        &mut self,
        repository: &dyn BackupRepository,
        event: BackupJournalEventKind,
    ) -> Result<(), OfflineRecoveryError> {
        repository
            .append_journal_event(&BackupJournalEvent {
                event_id: Uuid::now_v7(),
                sequence: self.next_sequence,
                subject: BackupJournalSubject::Recovery(self.command.recovery_job_id),
                backup_set_id: self.command.backup_set_id,
                actor_user_id: Some(self.actor_user_id),
                occurred_at: OffsetDateTime::now_utc(),
                event,
            })
            .await
            .map_err(|_| OfflineRecoveryError::Journal)?;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(OfflineRecoveryError::InvalidJournal)?;
        Ok(())
    }
}

fn validate_restore_inventory(manifest: &BackupManifest) -> Result<(), OfflineRecoveryError> {
    let postgres = manifest
        .components()
        .iter()
        .filter(|component| {
            component.kind == BackupComponentKind::PostgreSql
                && component.disposition == BackupComponentDisposition::Embedded
        })
        .count();
    if postgres != 1 {
        return Err(OfflineRecoveryError::Manifest);
    }
    Ok(())
}

fn validate_completed_restore_journal(
    command: ExecuteOfflineRecoveryCommand,
    events: &[BackupJournalEvent],
) -> Result<(), OfflineRecoveryError> {
    let subject = BackupJournalSubject::Recovery(command.recovery_job_id);
    let mut handoff_ready = false;
    let mut completed_steps = Vec::new();
    for (index, event) in events.iter().enumerate() {
        if event.sequence != index as u64
            || event.subject != subject
            || event.backup_set_id != command.backup_set_id
        {
            return Err(OfflineRecoveryError::InvalidJournal);
        }
        match &event.event {
            BackupJournalEventKind::RecoveryOfflineHandoffReady {
                target_backup_set_id,
                ..
            } => {
                if *target_backup_set_id != command.backup_set_id || handoff_ready {
                    return Err(OfflineRecoveryError::InvalidJournal);
                }
                handoff_ready = true;
            }
            BackupJournalEventKind::RecoveryStepCompleted { step }
                if OFFLINE_RESTORE_STEPS.contains(step) =>
            {
                if completed_steps.contains(step)
                    || *step != OFFLINE_RESTORE_STEPS[completed_steps.len()]
                {
                    return Err(OfflineRecoveryError::InvalidJournal);
                }
                completed_steps.push(*step);
            }
            _ => {}
        }
    }
    if !handoff_ready || completed_steps.len() != OFFLINE_RESTORE_STEPS.len() {
        return Err(OfflineRecoveryError::InvalidJournal);
    }
    Ok(())
}

fn components_for_step(manifest: &BackupManifest, step: RecoveryStepKind) -> Vec<BackupComponent> {
    manifest
        .components()
        .iter()
        .filter(|component| match step {
            RecoveryStepKind::PostgreSql => {
                component.kind == BackupComponentKind::PostgreSql
                    && component.disposition == BackupComponentDisposition::Embedded
            }
            RecoveryStepKind::BusinessObjects => {
                component.kind == BackupComponentKind::BusinessObject
                    && component.disposition == BackupComponentDisposition::Embedded
            }
            RecoveryStepKind::ExtensionArtifacts => matches!(
                component.kind,
                BackupComponentKind::ExtensionArtifact | BackupComponentKind::McpArtifact
            ),
            RecoveryStepKind::Reconcile
            | RecoveryStepKind::HealthVerification
            | RecoveryStepKind::AuditProjection => false,
        })
        .cloned()
        .collect()
}

fn step_code(step: RecoveryStepKind) -> &'static str {
    match step {
        RecoveryStepKind::PostgreSql => "postgresql",
        RecoveryStepKind::BusinessObjects => "business_objects",
        RecoveryStepKind::ExtensionArtifacts => "extension_artifacts",
        RecoveryStepKind::Reconcile => "reconcile",
        RecoveryStepKind::HealthVerification => "health_verification",
        RecoveryStepKind::AuditProjection => "audit_projection",
    }
}
