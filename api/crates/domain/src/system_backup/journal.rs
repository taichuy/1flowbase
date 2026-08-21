use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    BackupComponentId, BackupJobId, BackupJobState, BackupSetId, ContentDigest, RecoveryJobId,
    RecoveryJobState, RecoveryStepKind,
};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(tag = "job_kind", content = "job_id", rename_all = "snake_case")]
pub enum BackupJournalSubject {
    Backup(BackupJobId),
    Recovery(RecoveryJobId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "event_kind", rename_all = "snake_case")]
pub enum BackupJournalEventKind {
    BackupStateChanged {
        state: BackupJobState,
    },
    RecoveryStateChanged {
        state: RecoveryJobState,
    },
    RecoveryStepCompleted {
        step: RecoveryStepKind,
    },
    RecoveryIntentConfirmed {
        intent_id: Uuid,
        target_backup_set_id: BackupSetId,
        plan_digest: ContentDigest,
        #[schema(value_type = String)]
        confirmed_at: OffsetDateTime,
        #[schema(value_type = String)]
        expires_at: OffsetDateTime,
    },
    RecoverySafetyBackupVerified {
        safety_backup_set_id: BackupSetId,
        plan_digest: ContentDigest,
    },
    RecoveryOfflineHandoffReady {
        target_backup_set_id: BackupSetId,
        safety_backup_set_id: BackupSetId,
        plan_digest: ContentDigest,
    },
    /// Fresh-deployment bootstrap learns its root actor only after the PostgreSQL restore. This
    /// durable assignment lets post-restore health and audit use the restored identity without
    /// putting a user identifier into the portable bundle's pre-start contract.
    RecoveryBootstrapActorAssigned {
        actor_user_id: Uuid,
    },
    ComponentSealed {
        component_id: BackupComponentId,
    },
    TerminalFailure {
        code: String,
    },
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
