use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use super::{BackupSetId, RecoveryJobId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryJobState {
    Preflight,
    AwaitingConfirmation,
    SafetyBackup,
    Fencing,
    Draining,
    Restoring,
    Reconciling,
    Verifying,
    Succeeded,
    RolledBack,
    ManualRecoveryRequired,
}

impl RecoveryJobState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::RolledBack | Self::ManualRecoveryRequired
        )
    }

    fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Preflight, Self::AwaitingConfirmation)
                | (Self::AwaitingConfirmation, Self::SafetyBackup)
                | (Self::SafetyBackup, Self::Fencing)
                | (Self::Fencing, Self::Draining)
                | (Self::Draining, Self::Restoring)
                | (Self::Restoring, Self::Reconciling)
                | (Self::Reconciling, Self::Verifying)
                | (Self::Verifying, Self::Succeeded)
                | (Self::Restoring, Self::RolledBack)
                | (Self::Reconciling, Self::RolledBack)
                | (Self::Verifying, Self::RolledBack)
        ) || (!self.is_terminal()
            && !matches!(self, Self::Preflight | Self::AwaitingConfirmation)
            && next == Self::ManualRecoveryRequired)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStepKind {
    PostgreSql,
    BusinessObjects,
    ExtensionArtifacts,
    Reconcile,
    HealthVerification,
    AuditProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryJob {
    job_id: RecoveryJobId,
    backup_set_id: BackupSetId,
    safety_backup_set_id: Option<BackupSetId>,
    state: RecoveryJobState,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    terminal_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryJobTransitionError {
    InvalidTransition,
    TerminalCodeRequired,
    TerminalCodeForbidden,
}

impl RecoveryJob {
    pub const fn job_id(&self) -> RecoveryJobId {
        self.job_id
    }

    pub const fn backup_set_id(&self) -> BackupSetId {
        self.backup_set_id
    }

    pub const fn safety_backup_set_id(&self) -> Option<BackupSetId> {
        self.safety_backup_set_id
    }

    pub const fn state(&self) -> RecoveryJobState {
        self.state
    }

    pub const fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub const fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    pub fn terminal_code(&self) -> Option<&str> {
        self.terminal_code.as_deref()
    }

    pub fn new(
        job_id: RecoveryJobId,
        backup_set_id: BackupSetId,
        created_at: OffsetDateTime,
    ) -> Self {
        Self {
            job_id,
            backup_set_id,
            safety_backup_set_id: None,
            state: RecoveryJobState::Preflight,
            created_at,
            updated_at: created_at,
            terminal_code: None,
        }
    }

    pub fn record_safety_backup(
        &mut self,
        safety_backup_set_id: BackupSetId,
    ) -> Result<(), RecoveryJobTransitionError> {
        if self.state != RecoveryJobState::SafetyBackup || self.safety_backup_set_id.is_some() {
            return Err(RecoveryJobTransitionError::InvalidTransition);
        }
        self.safety_backup_set_id = Some(safety_backup_set_id);
        Ok(())
    }

    pub fn transition(
        &mut self,
        next: RecoveryJobState,
        updated_at: OffsetDateTime,
        terminal_code: Option<String>,
    ) -> Result<(), RecoveryJobTransitionError> {
        if self.state == RecoveryJobState::SafetyBackup
            && next == RecoveryJobState::Fencing
            && self.safety_backup_set_id.is_none()
        {
            return Err(RecoveryJobTransitionError::InvalidTransition);
        }
        if !self.state.can_transition_to(next) {
            return Err(RecoveryJobTransitionError::InvalidTransition);
        }
        match (next.is_terminal(), terminal_code.as_deref()) {
            (true, None | Some("")) if next != RecoveryJobState::Succeeded => {
                return Err(RecoveryJobTransitionError::TerminalCodeRequired)
            }
            (false, Some(_)) | (true, Some(_)) if next == RecoveryJobState::Succeeded => {
                return Err(RecoveryJobTransitionError::TerminalCodeForbidden)
            }
            _ => {}
        }
        self.state = next;
        self.updated_at = updated_at;
        self.terminal_code = terminal_code;
        Ok(())
    }
}
