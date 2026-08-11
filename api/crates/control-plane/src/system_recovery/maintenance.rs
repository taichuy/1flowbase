use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use domain::{BackupJobId, RecoveryJobId};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::sync::Notify;
use uuid::Uuid;

/// Complete inventory of in-process owners that can start writes while the API
/// process is online. Recovery uses this finite set to explain what is still
/// draining instead of inferring write ownership from URLs or task names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SystemWriteOwner {
    ApiMutation,
    ProviderRequestLogPersistence,
    WorkflowScheduleDispatch,
    WorkflowScheduleExecution,
}

impl SystemWriteOwner {
    pub const ALL: [Self; 4] = [
        Self::ApiMutation,
        Self::ProviderRequestLogPersistence,
        Self::WorkflowScheduleDispatch,
        Self::WorkflowScheduleExecution,
    ];

    const fn index(self) -> usize {
        match self {
            Self::ApiMutation => 0,
            Self::ProviderRequestLogPersistence => 1,
            Self::WorkflowScheduleDispatch => 2,
            Self::WorkflowScheduleExecution => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMaintenancePhase {
    Online,
    Draining,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMaintenanceOperation {
    Backup(BackupJobId),
    Recovery(RecoveryJobId),
}

impl SystemMaintenanceOperation {
    pub const fn recovery_job_id(self) -> Option<RecoveryJobId> {
        match self {
            Self::Backup(_) => None,
            Self::Recovery(job_id) => Some(job_id),
        }
    }
}

impl From<BackupJobId> for SystemMaintenanceOperation {
    fn from(job_id: BackupJobId) -> Self {
        Self::Backup(job_id)
    }
}

impl From<RecoveryJobId> for SystemMaintenanceOperation {
    fn from(job_id: RecoveryJobId) -> Self {
        Self::Recovery(job_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemWriteOwnerActivity {
    pub owner: SystemWriteOwner,
    pub active_writes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemMaintenanceSnapshot {
    pub phase: SystemMaintenancePhase,
    pub operation: Option<SystemMaintenanceOperation>,
    /// Compatibility projection for existing recovery-status responses.
    pub recovery_job_id: Option<RecoveryJobId>,
    pub started_at: Option<OffsetDateTime>,
    pub write_owners: Vec<SystemWriteOwnerActivity>,
}

impl SystemMaintenanceSnapshot {
    pub fn active_write_count(&self) -> usize {
        self.write_owners
            .iter()
            .map(|activity| activity.active_writes)
            .sum()
    }
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
#[error("system maintenance is already owned by {operation:?}")]
pub struct SystemMaintenanceLeaseConflict {
    pub operation: SystemMaintenanceOperation,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
#[error("system writes are fenced by {operation:?}")]
pub struct SystemWriteFenced {
    pub operation: SystemMaintenanceOperation,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum SystemMaintenanceDrainError {
    #[error("timed out while draining system writes")]
    Timeout,
    #[error("system maintenance lease is no longer current")]
    LeaseLost,
}

#[derive(Debug)]
enum InternalPhase {
    Online,
    Draining {
        lease_token: Uuid,
        operation: SystemMaintenanceOperation,
        started_at: OffsetDateTime,
    },
    Active {
        lease_token: Uuid,
        operation: SystemMaintenanceOperation,
        started_at: OffsetDateTime,
    },
}

#[derive(Debug)]
struct MaintenanceState {
    phase: InternalPhase,
    active_writes: [usize; SystemWriteOwner::ALL.len()],
}

impl Default for MaintenanceState {
    fn default() -> Self {
        Self {
            phase: InternalPhase::Online,
            active_writes: [0; SystemWriteOwner::ALL.len()],
        }
    }
}

/// Process-local source of truth for the online write fence.
///
/// It deliberately has no durable-store dependency: the recovery coordinator
/// can inspect it after the primary database has been stopped or replaced.
#[derive(Debug, Default)]
pub struct SystemMaintenance {
    state: Mutex<MaintenanceState>,
    changed: Notify,
}

impl SystemMaintenance {
    pub fn snapshot(&self) -> SystemMaintenanceSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        snapshot_from(&state)
    }

    pub async fn wait_until_online(&self) {
        loop {
            let changed = self.changed.notified();
            if matches!(
                self.state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .phase,
                InternalPhase::Online
            ) {
                return;
            }
            changed.await;
        }
    }

    pub fn try_enter_write(
        self: &Arc<Self>,
        owner: SystemWriteOwner,
    ) -> Result<SystemWritePermit, SystemWriteFenced> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match &state.phase {
            InternalPhase::Online => {
                state.active_writes[owner.index()] += 1;
                Ok(SystemWritePermit {
                    maintenance: self.clone(),
                    owner,
                })
            }
            InternalPhase::Draining { operation, .. } | InternalPhase::Active { operation, .. } => {
                Err(SystemWriteFenced {
                    operation: *operation,
                })
            }
        }
    }

    pub fn begin(
        self: &Arc<Self>,
        operation: impl Into<SystemMaintenanceOperation>,
        started_at: OffsetDateTime,
    ) -> Result<SystemMaintenanceLease, SystemMaintenanceLeaseConflict> {
        let operation = operation.into();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match &state.phase {
            InternalPhase::Online => {
                let lease_token = Uuid::now_v7();
                state.phase = InternalPhase::Draining {
                    lease_token,
                    operation,
                    started_at,
                };
                drop(state);
                self.changed.notify_waiters();
                Ok(SystemMaintenanceLease {
                    maintenance: self.clone(),
                    lease_token,
                    released: false,
                })
            }
            InternalPhase::Draining { operation, .. } | InternalPhase::Active { operation, .. } => {
                Err(SystemMaintenanceLeaseConflict {
                    operation: *operation,
                })
            }
        }
    }

    fn leave_write(&self, owner: SystemWriteOwner) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active = &mut state.active_writes[owner.index()];
        debug_assert!(*active > 0, "write permits must be released exactly once");
        *active = active.saturating_sub(1);
        drop(state);
        self.changed.notify_waiters();
    }

    fn release(&self, lease_token: Uuid) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let owns_lease = match &state.phase {
            InternalPhase::Online => false,
            InternalPhase::Draining {
                lease_token: current,
                ..
            }
            | InternalPhase::Active {
                lease_token: current,
                ..
            } => *current == lease_token,
        };
        if owns_lease {
            state.phase = InternalPhase::Online;
            drop(state);
            self.changed.notify_waiters();
        }
    }
}

#[derive(Debug)]
pub struct SystemWritePermit {
    maintenance: Arc<SystemMaintenance>,
    owner: SystemWriteOwner,
}

impl Drop for SystemWritePermit {
    fn drop(&mut self) {
        self.maintenance.leave_write(self.owner);
    }
}

pub struct SystemMaintenanceLease {
    maintenance: Arc<SystemMaintenance>,
    lease_token: Uuid,
    released: bool,
}

impl SystemMaintenanceLease {
    pub async fn wait_for_drain(
        &self,
        timeout: Duration,
    ) -> Result<SystemMaintenanceSnapshot, SystemMaintenanceDrainError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let changed = self.maintenance.changed.notified();
            {
                let mut state = self
                    .maintenance
                    .state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                match &state.phase {
                    InternalPhase::Draining {
                        lease_token,
                        operation,
                        started_at,
                    } if *lease_token == self.lease_token => {
                        if state.active_writes.iter().all(|count| *count == 0) {
                            state.phase = InternalPhase::Active {
                                lease_token: *lease_token,
                                operation: *operation,
                                started_at: *started_at,
                            };
                            return Ok(snapshot_from(&state));
                        }
                    }
                    InternalPhase::Active { lease_token, .. }
                        if *lease_token == self.lease_token =>
                    {
                        return Ok(snapshot_from(&state));
                    }
                    _ => return Err(SystemMaintenanceDrainError::LeaseLost),
                }
            }

            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                return Err(SystemMaintenanceDrainError::Timeout);
            }
        }
    }

    pub fn finish(mut self) {
        self.maintenance.release(self.lease_token);
        self.released = true;
    }
}

impl Drop for SystemMaintenanceLease {
    fn drop(&mut self) {
        if !self.released {
            self.maintenance.release(self.lease_token);
        }
    }
}

fn snapshot_from(state: &MaintenanceState) -> SystemMaintenanceSnapshot {
    let (phase, operation, started_at) = match &state.phase {
        InternalPhase::Online => (SystemMaintenancePhase::Online, None, None),
        InternalPhase::Draining {
            operation,
            started_at,
            ..
        } => (
            SystemMaintenancePhase::Draining,
            Some(*operation),
            Some(*started_at),
        ),
        InternalPhase::Active {
            operation,
            started_at,
            ..
        } => (
            SystemMaintenancePhase::Active,
            Some(*operation),
            Some(*started_at),
        ),
    };
    let write_owners = SystemWriteOwner::ALL
        .into_iter()
        .map(|owner| SystemWriteOwnerActivity {
            owner,
            active_writes: state.active_writes[owner.index()],
        })
        .collect();

    SystemMaintenanceSnapshot {
        phase,
        operation,
        recovery_job_id: operation.and_then(SystemMaintenanceOperation::recovery_job_id),
        started_at,
        write_owners,
    }
}
