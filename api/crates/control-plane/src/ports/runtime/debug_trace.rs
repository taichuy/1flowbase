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
