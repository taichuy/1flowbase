use super::*;

#[async_trait]
pub trait RuntimeRegistrySync: Send + Sync {
    async fn rebuild(&self) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct UpsertCompiledPlanInput {
    pub actor_user_id: Uuid,
    pub flow_id: Uuid,
    pub flow_draft_id: Uuid,
    pub schema_version: String,
    pub document_hash: String,
    pub document_updated_at: OffsetDateTime,
    pub plan: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct CreateFlowRunInput {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub flow_id: Uuid,
    pub flow_draft_id: Uuid,
    pub compiled_plan_id: Uuid,
    pub debug_session_id: String,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub run_mode: domain::FlowRunMode,
    pub target_node_id: Option<String>,
    pub title: String,
    pub status: domain::FlowRunStatus,
    pub input_payload: serde_json::Value,
    pub started_at: OffsetDateTime,
    pub api_key_id: Option<Uuid>,
    pub publication_version_id: Option<Uuid>,
    pub external_user: Option<String>,
    pub external_conversation_id: Option<String>,
    pub external_trace_id: Option<String>,
    pub compatibility_mode: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateFlowRunShellInput {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub flow_id: Uuid,
    pub flow_draft_id: Uuid,
    pub debug_session_id: String,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub run_mode: domain::FlowRunMode,
    pub target_node_id: Option<String>,
    pub title: String,
    pub status: domain::FlowRunStatus,
    pub input_payload: serde_json::Value,
    pub started_at: OffsetDateTime,
    pub api_key_id: Option<Uuid>,
    pub publication_version_id: Option<Uuid>,
    pub external_user: Option<String>,
    pub external_conversation_id: Option<String>,
    pub external_trace_id: Option<String>,
    pub compatibility_mode: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AttachCompiledPlanToFlowRunInput {
    pub flow_run_id: Uuid,
    pub compiled_plan_id: Uuid,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub status: domain::FlowRunStatus,
}

#[derive(Debug, Clone)]
pub struct FailQueuedFlowRunShellInput {
    pub flow_run_id: Uuid,
    pub output_payload: serde_json::Value,
    pub error_payload: serde_json::Value,
    pub finished_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct CreateNodeRunInput {
    pub flow_run_id: Uuid,
    pub node_id: String,
    pub node_type: String,
    pub node_alias: String,
    pub status: domain::NodeRunStatus,
    pub input_payload: serde_json::Value,
    pub debug_payload: serde_json::Value,
    pub started_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct UpdateNodeRunInput {
    pub node_run_id: Uuid,
    pub status: domain::NodeRunStatus,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub metrics_payload: serde_json::Value,
    pub debug_payload: serde_json::Value,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct CompleteNodeRunInput {
    pub node_run_id: Uuid,
    pub status: domain::NodeRunStatus,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub metrics_payload: serde_json::Value,
    pub debug_payload: serde_json::Value,
    pub finished_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct UpdateFlowRunInput {
    pub flow_run_id: Uuid,
    pub status: domain::FlowRunStatus,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub finished_at: Option<OffsetDateTime>,
}

/// A terminal flow result owns the canonical status and both terminal event names. Keeping those
/// facts in one type prevents a failed or partial result from being persisted as a successful
/// terminal.
#[derive(Debug, Clone)]
pub enum CommitFlowRunTerminalResult {
    Succeeded {
        output_payload: serde_json::Value,
    },
    Incomplete {
        output_payload: serde_json::Value,
    },
    Failed {
        output_payload: serde_json::Value,
        error_payload: serde_json::Value,
    },
    Cancelled {
        output_payload: serde_json::Value,
        error_payload: Option<serde_json::Value>,
    },
}

impl CommitFlowRunTerminalResult {
    pub fn status(&self) -> domain::FlowRunStatus {
        match self {
            Self::Succeeded { .. } => domain::FlowRunStatus::Succeeded,
            Self::Incomplete { .. } => domain::FlowRunStatus::Incomplete,
            Self::Failed { .. } => domain::FlowRunStatus::Failed,
            Self::Cancelled { .. } => domain::FlowRunStatus::Cancelled,
        }
    }

    pub fn output_payload(&self) -> &serde_json::Value {
        match self {
            Self::Succeeded { output_payload }
            | Self::Incomplete { output_payload }
            | Self::Failed { output_payload, .. }
            | Self::Cancelled { output_payload, .. } => output_payload,
        }
    }

    pub fn error_payload(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Failed { error_payload, .. } => Some(error_payload),
            Self::Cancelled { error_payload, .. } => error_payload.as_ref(),
            Self::Succeeded { .. } | Self::Incomplete { .. } => None,
        }
    }

    pub fn flow_run_event_type(&self) -> &'static str {
        match self {
            Self::Succeeded { .. } => "flow_run_completed",
            Self::Incomplete { .. } => "flow_run_incomplete",
            Self::Failed { .. } => "flow_run_failed",
            Self::Cancelled { .. } => "flow_run_cancelled",
        }
    }

    pub fn runtime_event_type(&self) -> &'static str {
        match self {
            Self::Succeeded { .. } => "flow_finished",
            Self::Incomplete { .. } => "flow_incomplete",
            Self::Failed { .. } => "flow_failed",
            Self::Cancelled { .. } => "flow_cancelled",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommitFlowRunTerminalInput {
    pub flow_run_id: Uuid,
    pub expected_status: domain::FlowRunStatus,
    pub result: CommitFlowRunTerminalResult,
    pub flow_run_event_payload: serde_json::Value,
    pub terminal_event_payload: serde_json::Value,
    pub finished_at: OffsetDateTime,
}

/// A loser receipt deliberately carries no proposed result. The caller must re-read the durable
/// winner instead of treating its stale candidate as a second terminal owner.
#[derive(Debug, Clone)]
pub enum CommitFlowRunTerminalReceipt {
    Winner(domain::FlowRunRecord),
    WinnerWithPostCommitProjectionWarning(domain::FlowRunRecord),
    Loser,
}

/// The durable half of the published-stream EOF recovery. The status update and both terminal
/// facts must commit together so a retry cannot observe a failed run with a missing terminal.
#[derive(Debug, Clone)]
pub struct FinalizePublishedRunMissingStreamTerminalPersistenceInput {
    pub flow_run_id: Uuid,
    pub expected_status: domain::FlowRunStatus,
    pub output_payload: serde_json::Value,
    pub error_payload: serde_json::Value,
    pub terminal_event_payload: serde_json::Value,
    pub finished_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub enum FinalizePublishedRunMissingStreamTerminalPersistenceOutcome {
    Finalized(domain::FlowRunRecord),
    FinalizedWithPostCommitProjectionWarning(domain::FlowRunRecord),
    CasMiss,
}

#[derive(Debug, Clone)]
pub struct CompleteFlowRunInput {
    pub flow_run_id: Uuid,
    pub status: domain::FlowRunStatus,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub finished_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct AppendRunEventInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub event_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpdateFlowRunPayloadsInput {
    pub flow_run_id: Uuid,
    pub input_payload: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct UpdateNodeRunPayloadsInput {
    pub node_run_id: Uuid,
    pub input_payload: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub metrics_payload: serde_json::Value,
    pub debug_payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpdateRunEventPayloadInput {
    pub run_event_id: Uuid,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckpointPayloadsInput {
    pub checkpoint_id: Uuid,
    pub locator_payload: serde_json::Value,
    pub variable_snapshot: serde_json::Value,
    pub external_ref_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct UpdateCallbackTaskPayloadsInput {
    pub callback_task_id: Uuid,
    pub request_payload: serde_json::Value,
    pub response_payload: Option<serde_json::Value>,
    pub external_ref_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct RecordFlowRunCallbackResumeAttemptInput {
    pub flow_run_id: Uuid,
    pub callback_task_id: Uuid,
    pub source: String,
    pub response_payload: serde_json::Value,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct RecordFlowRunCallbackResumeAttemptOutput {
    pub attempt: domain::FlowRunCallbackResumeAttemptRecord,
    pub inserted: bool,
}

#[derive(Debug, Clone)]
pub struct FinishFlowRunCallbackResumeAttemptInput {
    pub attempt_id: Uuid,
    pub status: domain::FlowRunCallbackResumeAttemptStatus,
    pub error_payload: Option<serde_json::Value>,
    pub completed_at: OffsetDateTime,
}
