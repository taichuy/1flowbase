use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    BackupComponentId, BackupJobId, BackupJobState, BackupSetId, RecoveryJobId, RecoveryJobState,
    RecoveryStepKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "job_kind", content = "job_id", rename_all = "snake_case")]
pub enum BackupJournalSubject {
    Backup(BackupJobId),
    Recovery(RecoveryJobId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "event_kind", rename_all = "snake_case")]
pub enum BackupJournalEventKind {
    BackupStateChanged { state: BackupJobState },
    RecoveryStateChanged { state: RecoveryJobState },
    RecoveryStepCompleted { step: RecoveryStepKind },
    ComponentSealed { component_id: BackupComponentId },
    TerminalFailure { code: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupJournalEvent {
    pub event_id: Uuid,
    pub sequence: u64,
    pub subject: BackupJournalSubject,
    pub backup_set_id: BackupSetId,
    pub actor_user_id: Option<Uuid>,
    pub occurred_at: OffsetDateTime,
    pub event: BackupJournalEventKind,
}
