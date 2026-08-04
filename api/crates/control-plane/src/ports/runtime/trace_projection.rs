use super::*;

#[derive(Debug, Clone)]
pub struct CreateCheckpointInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub status: String,
    pub reason: String,
    pub locator_payload: serde_json::Value,
    pub variable_snapshot: serde_json::Value,
    pub external_ref_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct CreateCallbackTaskInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Uuid,
    pub callback_kind: String,
    pub request_payload: serde_json::Value,
    pub external_ref_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct CallbackResumeWaitingNode {
    pub id: Uuid,
    pub status: domain::NodeRunStatus,
    pub output_payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct CallbackResumeContext {
    pub flow_run: domain::FlowRunRecord,
    pub callback_task: domain::CallbackTaskRecord,
    pub checkpoint: domain::CheckpointRecord,
    pub waiting_node: CallbackResumeWaitingNode,
    pub next_node_started_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct CompleteCallbackTaskInput {
    pub callback_task_id: Uuid,
    pub response_payload: serde_json::Value,
    pub completed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct ApplicationRunTraceNodeProjectionInput {
    pub trace_node_id: Uuid,
    pub parent_trace_node_id: Option<Uuid>,
    pub stable_locator: String,
    pub node_kind: String,
    pub owner_kind: Option<String>,
    pub owner_id: Option<String>,
    pub order_key: String,
    pub node_id: Option<String>,
    pub node_type: Option<String>,
    pub node_mode: Option<String>,
    pub node_alias: String,
    pub status: String,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub duration_ms: Option<i64>,
    pub metrics_payload: serde_json::Value,
    pub has_children: bool,
    pub child_count: i64,
    pub has_content: bool,
    pub content_ref: Option<String>,
    pub source_flow_run_id: Option<Uuid>,
    pub source_trace_node_id: Option<Uuid>,
    pub parent_callback_task_id: Option<Uuid>,
    pub parent_tool_call_id: Option<String>,
    pub trace_relation_kind: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApplicationRunTraceNodeContentProjectionInput {
    pub trace_node_id: Uuid,
    pub content_kind: String,
    pub payload: serde_json::Value,
    pub source_refs: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunTraceChildrenCursor {
    pub order_key: String,
    pub trace_node_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ListApplicationRunTraceChildrenPageInput {
    pub flow_run_id: Uuid,
    pub parent_trace_node_id: Uuid,
    pub page_size: i64,
    pub cursor: Option<ApplicationRunTraceChildrenCursor>,
}

#[derive(Debug, Clone)]
pub struct ListApplicationRunTraceChildrenPage {
    pub items: Vec<domain::ApplicationRunTraceNodeRecord>,
    pub has_more: bool,
    pub next_cursor: Option<ApplicationRunTraceChildrenCursor>,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct ApplicationRunTraceProjectionStatistics {
    pub total_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub input_cache_hit_tokens: Option<i64>,
    pub unique_node_count: i64,
    pub tool_callback_count: i64,
}

#[derive(Debug, Clone)]
pub struct ReplaceApplicationRunTraceProjectionInput {
    pub flow_run_id: Uuid,
    pub projection_version: i32,
    pub source_watermark: String,
    pub nodes: Vec<ApplicationRunTraceNodeProjectionInput>,
    pub contents: Vec<ApplicationRunTraceNodeContentProjectionInput>,
}

#[derive(Debug, Clone)]
pub struct UpsertApplicationRunTraceProjectionStatusInput {
    pub flow_run_id: Uuid,
    pub projection_version: i32,
    pub status: domain::ApplicationRunTraceProjectionStatus,
    pub source_watermark: String,
    pub attempt_count: i32,
    pub last_attempt_at: Option<OffsetDateTime>,
    pub last_success_at: Option<OffsetDateTime>,
    pub diagnostic: Option<domain::ApplicationRunTraceProjectionDiagnostic>,
}
