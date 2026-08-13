use super::*;

#[derive(Debug, Clone)]
pub struct DebugVariableCacheKey {
    pub node_id: String,
    pub variable_key: String,
}

#[derive(Debug, Clone)]
pub struct DebugVariableCacheEntry {
    pub node_id: String,
    pub variable_key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpsertDebugVariableCacheEntryInput {
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub draft_id: Uuid,
    pub actor_user_id: Uuid,
    pub node_id: String,
    pub variable_key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct DeleteDebugVariableCacheEntriesInput {
    pub application_id: Uuid,
    pub draft_id: Uuid,
    pub actor_user_id: Uuid,
    pub keys: Option<Vec<DebugVariableCacheKey>>,
}

#[derive(Debug, Clone)]
pub struct CreateRuntimeDebugArtifactInput {
    pub artifact_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Option<Uuid>,
    pub node_run_id: Option<Uuid>,
    pub run_event_id: Option<Uuid>,
    pub artifact_kind: String,
    pub content_type: String,
    pub original_size_bytes: i64,
    pub preview_size_bytes: i64,
    pub storage_id: Uuid,
    pub storage_ref: String,
    pub retention_state: String,
}

#[derive(Debug, Clone)]
pub struct GetRuntimeDebugArtifactInput {
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub artifact_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct AppendRuntimeSpanInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub parent_span_id: Option<Uuid>,
    pub kind: domain::RuntimeSpanKind,
    pub name: String,
    pub status: domain::RuntimeSpanStatus,
    pub capability_id: Option<String>,
    pub input_ref: Option<String>,
    pub output_ref: Option<String>,
    pub error_payload: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct AppendRuntimeEventInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub parent_span_id: Option<Uuid>,
    pub event_type: String,
    pub layer: domain::RuntimeEventLayer,
    pub source: domain::RuntimeEventSource,
    pub trust_level: domain::RuntimeTrustLevel,
    pub item_id: Option<Uuid>,
    pub ledger_ref: Option<String>,
    pub payload: serde_json::Value,
    pub visibility: domain::RuntimeEventVisibility,
    pub durability: domain::RuntimeEventDurability,
}

#[derive(Debug, Clone)]
pub struct AppendRuntimeItemInput {
    pub flow_run_id: Uuid,
    pub span_id: Option<Uuid>,
    pub kind: domain::RuntimeItemKind,
    pub status: domain::RuntimeItemStatus,
    pub source_event_id: Option<Uuid>,
    pub input_ref: Option<String>,
    pub output_ref: Option<String>,
    pub usage_ledger_id: Option<Uuid>,
    pub trust_level: domain::RuntimeTrustLevel,
}

#[derive(Debug, Clone)]
pub struct AppendContextProjectionInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub llm_turn_span_id: Option<Uuid>,
    pub projection_kind: String,
    pub merge_stage_ref: Option<String>,
    pub source_transcript_ref: Option<String>,
    pub source_item_refs: serde_json::Value,
    pub compaction_event_id: Option<Uuid>,
    pub summary_version: Option<String>,
    pub model_input_ref: String,
    pub model_input_hash: String,
    pub compacted_summary_ref: Option<String>,
    pub previous_projection_id: Option<Uuid>,
    pub token_estimate: Option<i64>,
    pub provider_continuation_metadata: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct PutCanonicalRuntimeContentInput {
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AppendContextVersionInput {
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub parent_context_version_id: Option<Uuid>,
    pub sequence: i64,
    pub transition_kind: domain::ContextTransitionKind,
    pub transition_actor: domain::ContextTransitionActor,
    pub declared_compaction_provenance: Option<serde_json::Value>,
    pub actual_content_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct BindInvocationContextInput {
    pub invocation_span_id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub context_version_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct AppendProviderInvocationContextInput {
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub invocation_span_id: Uuid,
    pub actual_context: serde_json::Value,
    pub context_epoch: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AppendRecoveryHistoryInput {
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub sequence: i64,
    pub state_code: domain::RecoveryStateCode,
    pub coordinate: domain::RecoveryCoordinate,
    pub context_version_id: Uuid,
    pub recovery_content_id: Option<Uuid>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeContextContentVersion {
    pub context_version_id: Uuid,
    pub sequence: i64,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct PersistWaitingCallbackTaskInput {
    pub id: Uuid,
    pub callback_kind: String,
    pub request_payload: serde_json::Value,
    pub external_ref_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub enum PersistWaitingKind {
    Human,
    Callback(PersistWaitingCallbackTaskInput),
}

#[derive(Debug, Clone)]
pub struct PersistWaitingStateInput {
    pub checkpoint_id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Uuid,
    pub expected_status: domain::FlowRunStatus,
    pub output_payload: serde_json::Value,
    pub checkpoint_status: String,
    pub checkpoint_reason: String,
    pub locator_payload: serde_json::Value,
    pub variable_snapshot: serde_json::Value,
    pub checkpoint_external_ref_payload: Option<serde_json::Value>,
    pub context_content: serde_json::Value,
    pub parent_context_version_id: Option<Uuid>,
    pub context_transition_kind: domain::ContextTransitionKind,
    pub recovery_idempotency_key: String,
    pub resume_claim_id: Option<Uuid>,
    pub resume_claim_token: Option<Uuid>,
    pub waiting_event: AppendRuntimeEventInput,
    pub kind: PersistWaitingKind,
}

#[derive(Debug, Clone)]
pub struct PersistedWaitingState {
    pub flow_run: domain::FlowRunRecord,
    pub checkpoint: domain::CheckpointRecord,
    pub callback_task: Option<domain::CallbackTaskRecord>,
    pub waiting_event: domain::RuntimeEventRecord,
    pub recovery_history: domain::RecoveryHistoryRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeClaimKind {
    Human,
    Callback,
}

impl ResumeClaimKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Callback => "callback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeClaimStatus {
    Processing,
    Succeeded,
    Failed,
}

impl ResumeClaimStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Processing => "processing",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AcquireResumeClaimInput {
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub checkpoint_id: Uuid,
    pub callback_task_id: Option<Uuid>,
    pub kind: ResumeClaimKind,
    pub request_payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ResumeClaimRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub checkpoint_id: Uuid,
    pub callback_task_id: Option<Uuid>,
    pub kind: ResumeClaimKind,
    pub status: ResumeClaimStatus,
    pub request_payload: serde_json::Value,
    pub claim_token: Uuid,
    pub generation: i64,
    pub lease_expires_at: OffsetDateTime,
    pub error_payload: Option<serde_json::Value>,
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeClaimDisposition {
    Acquired,
    InProgress,
    Completed,
}

#[derive(Debug, Clone)]
pub struct AcquireResumeClaimOutput {
    pub claim: ResumeClaimRecord,
    pub disposition: ResumeClaimDisposition,
}

#[derive(Debug, Clone)]
pub struct FinishResumeClaimInput {
    pub claim_id: Uuid,
    pub claim_token: Uuid,
    pub status: ResumeClaimStatus,
    pub error_payload: Option<serde_json::Value>,
    pub completed_at: OffsetDateTime,
}
