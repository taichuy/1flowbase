use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use super::{BackupJobId, BackupSetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupJobState {
    Queued,
    Fencing,
    Capturing,
    Sealing,
    Verifying,
    Succeeded,
    Failed,
}

impl BackupJobState {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }

    fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Fencing)
                | (Self::Fencing, Self::Capturing)
                | (Self::Capturing, Self::Sealing)
                | (Self::Sealing, Self::Verifying)
                | (Self::Verifying, Self::Succeeded)
        ) || (!self.is_terminal() && next == Self::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupJob {
    job_id: BackupJobId,
    backup_set_id: BackupSetId,
    state: BackupJobState,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    failure_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupJobTransitionError {
    InvalidTransition,
    FailureCodeRequired,
    FailureCodeForbidden,
}

impl BackupJob {
    pub const fn job_id(&self) -> BackupJobId {
        self.job_id
    }

    pub const fn backup_set_id(&self) -> BackupSetId {
        self.backup_set_id
    }

    pub const fn state(&self) -> BackupJobState {
        self.state
    }

    pub const fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub const fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    pub fn failure_code(&self) -> Option<&str> {
        self.failure_code.as_deref()
    }

    pub fn new(
        job_id: BackupJobId,
        backup_set_id: BackupSetId,
        created_at: OffsetDateTime,
    ) -> Self {
        Self {
            job_id,
            backup_set_id,
            state: BackupJobState::Queued,
            created_at,
            updated_at: created_at,
            failure_code: None,
        }
    }

    pub fn transition(
        &mut self,
        next: BackupJobState,
        updated_at: OffsetDateTime,
        failure_code: Option<String>,
    ) -> Result<(), BackupJobTransitionError> {
        if !self.state.can_transition_to(next) {
            return Err(BackupJobTransitionError::InvalidTransition);
        }
        match (next, failure_code.as_deref()) {
            (BackupJobState::Failed, None | Some("")) => {
                return Err(BackupJobTransitionError::FailureCodeRequired)
            }
            (BackupJobState::Failed, Some(_)) => {}
            (_, Some(_)) => return Err(BackupJobTransitionError::FailureCodeForbidden),
            (_, None) => {}
        }
        self.state = next;
        self.updated_at = updated_at;
        self.failure_code = failure_code;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupSetAvailability {
    Ready,
    Corrupt,
    Incompatible,
}
