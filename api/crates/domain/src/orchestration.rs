use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::runtime_observability::RuntimeEventRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowRunMode {
    DebugNodePreview,
    DebugFlowRun,
    PublishedApiRun,
    WorkflowHttpRun,
    WorkflowScheduleRun,
}

impl FlowRunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DebugNodePreview => "debug_node_preview",
            Self::DebugFlowRun => "debug_flow_run",
            Self::PublishedApiRun => "published_api_run",
            Self::WorkflowHttpRun => "workflow_http_run",
            Self::WorkflowScheduleRun => "workflow_schedule_run",
        }
    }

    pub fn invocation_context(
        self,
        created_by: Option<Uuid>,
        authorized_account: Option<String>,
        api_key_id: Option<Uuid>,
    ) -> FlowRunInvocationContext {
        let (execution_stage, invocation_source, principal) = match self {
            Self::PublishedApiRun => (
                FlowRunExecutionStage::Published,
                FlowRunInvocationSource::AgentFlowApi,
                FlowRunPrincipal::application_api_key(api_key_id),
            ),
            Self::WorkflowHttpRun => {
                let principal = match api_key_id {
                    Some(api_key_id) => FlowRunPrincipal::user_api_key(Some(api_key_id)),
                    None => FlowRunPrincipal::user(created_by, authorized_account),
                };
                (
                    FlowRunExecutionStage::Published,
                    FlowRunInvocationSource::WorkflowHttp,
                    principal,
                )
            }
            Self::WorkflowScheduleRun => (
                FlowRunExecutionStage::Published,
                FlowRunInvocationSource::WorkflowSchedule,
                FlowRunPrincipal::scheduler(),
            ),
            Self::DebugNodePreview | Self::DebugFlowRun => (
                FlowRunExecutionStage::Debug,
                FlowRunInvocationSource::Debug,
                FlowRunPrincipal::user(created_by, authorized_account),
            ),
        };

        FlowRunInvocationContext {
            execution_stage,
            invocation_source,
            principal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowRunExecutionStage {
    Debug,
    Published,
}

impl FlowRunExecutionStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Published => "published",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowRunInvocationSource {
    AgentFlowApi,
    WorkflowHttp,
    WorkflowSchedule,
    Debug,
}

impl FlowRunInvocationSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentFlowApi => "agent_flow_api",
            Self::WorkflowHttp => "workflow_http",
            Self::WorkflowSchedule => "workflow_schedule",
            Self::Debug => "debug",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowRunPrincipalKind {
    User,
    ApplicationApiKey,
    UserApiKey,
    Scheduler,
}

impl FlowRunPrincipalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::ApplicationApiKey => "application_api_key",
            Self::UserApiKey => "user_api_key",
            Self::Scheduler => "scheduler",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowRunPrincipal {
    pub kind: FlowRunPrincipalKind,
    pub id: Option<Uuid>,
    pub display_name: Option<String>,
}

impl FlowRunPrincipal {
    fn user(id: Option<Uuid>, display_name: Option<String>) -> Self {
        Self {
            kind: FlowRunPrincipalKind::User,
            id,
            display_name,
        }
    }

    fn application_api_key(id: Option<Uuid>) -> Self {
        Self {
            kind: FlowRunPrincipalKind::ApplicationApiKey,
            id,
            display_name: None,
        }
    }

    fn user_api_key(id: Option<Uuid>) -> Self {
        Self {
            kind: FlowRunPrincipalKind::UserApiKey,
            id,
            display_name: None,
        }
    }

    fn scheduler() -> Self {
        Self {
            kind: FlowRunPrincipalKind::Scheduler,
            id: None,
            display_name: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowRunInvocationContext {
    pub execution_stage: FlowRunExecutionStage,
    pub invocation_source: FlowRunInvocationSource,
    pub principal: FlowRunPrincipal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallbackTaskStatus {
    Pending,
    Completed,
    Cancelled,
}

impl CallbackTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowRunCallbackResumeAttemptStatus {
    Received,
    Processing,
    Succeeded,
    Failed,
    Cancelled,
}

impl FlowRunCallbackResumeAttemptStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Processing => "processing",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowRunStatus {
    Queued,
    Running,
    WaitingCallback,
    WaitingHuman,
    Paused,
    Succeeded,
    Incomplete,
    Failed,
    Cancelled,
}

impl FlowRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::WaitingCallback => "waiting_callback",
            Self::WaitingHuman => "waiting_human",
            Self::Paused => "paused",
            Self::Succeeded => "succeeded",
            Self::Incomplete => "incomplete",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeRunStatus {
    Pending,
    Ready,
    Running,
    Streaming,
    WaitingTool,
    WaitingCallback,
    WaitingHuman,
    Retrying,
    Succeeded,
    Failed,
    Skipped,
}

impl NodeRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Streaming => "streaming",
            Self::WaitingTool => "waiting_tool",
            Self::WaitingCallback => "waiting_callback",
            Self::WaitingHuman => "waiting_human",
            Self::Retrying => "retrying",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPlanRecord {
    pub id: Uuid,
    pub flow_id: Uuid,
    pub draft_id: Uuid,
    pub schema_version: String,
    pub document_hash: String,
    pub document_updated_at: OffsetDateTime,
    pub plan: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowRunRecord {
    pub id: Uuid,
    pub application_id: Uuid,
    pub flow_id: Uuid,
    pub draft_id: Uuid,
    pub compiled_plan_id: Option<Uuid>,
    pub debug_session_id: String,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub run_mode: FlowRunMode,
    pub target_node_id: Option<String>,
    pub title: String,
    pub status: FlowRunStatus,
    pub input_payload: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub created_by: Uuid,
    pub authorized_account: Option<String>,
    pub api_key_id: Option<Uuid>,
    pub publication_version_id: Option<Uuid>,
    pub external_user: Option<String>,
    pub external_conversation_id: Option<String>,
    pub external_trace_id: Option<String>,
    pub compatibility_mode: Option<String>,
    pub idempotency_key: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRunRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_id: String,
    pub node_type: String,
    pub node_alias: String,
    pub status: NodeRunStatus,
    pub input_payload: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub metrics_payload: serde_json::Value,
    pub debug_payload: serde_json::Value,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub status: String,
    pub reason: String,
    pub locator_payload: serde_json::Value,
    pub variable_snapshot: serde_json::Value,
    pub external_ref_payload: Option<serde_json::Value>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallbackTaskRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Uuid,
    pub callback_kind: String,
    pub status: CallbackTaskStatus,
    pub request_payload: serde_json::Value,
    pub response_payload: Option<serde_json::Value>,
    pub external_ref_payload: Option<serde_json::Value>,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowRunCallbackResumeAttemptRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub callback_task_id: Uuid,
    pub source: String,
    pub status: FlowRunCallbackResumeAttemptStatus,
    pub response_payload: serde_json::Value,
    pub idempotency_key: String,
    pub error_payload: Option<serde_json::Value>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEventRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub sequence: i64,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDebugArtifactRecord {
    pub id: Uuid,
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
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataModelSideEffectReceiptRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub draft_id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Uuid,
    pub node_id: String,
    pub action: String,
    pub model_code: String,
    pub record_id: Option<String>,
    pub deleted_id: Option<String>,
    pub affected_count: i64,
    pub idempotency_key: String,
    pub payload_hash: String,
    pub actor_user_id: Uuid,
    pub scope_id: Uuid,
    pub status: String,
    pub output_payload: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunSummary {
    pub id: Uuid,
    pub run_mode: FlowRunMode,
    pub status: FlowRunStatus,
    pub target_node_id: Option<String>,
    pub title: String,
    pub user_id: Option<String>,
    pub created_by: Option<Uuid>,
    pub authorized_account: Option<String>,
    pub api_key_id: Option<Uuid>,
    pub publication_version_id: Option<Uuid>,
    pub external_conversation_id: Option<String>,
    pub external_trace_id: Option<String>,
    pub compatibility_mode: Option<String>,
    pub idempotency_key: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplicationRunLogSummary {
    pub run: ApplicationRunSummary,
    pub total_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub input_cache_hit_tokens: Option<i64>,
    pub input_cache_hit_rate: Option<f64>,
    pub unique_node_count: i64,
    pub tool_callback_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationConversationRunSummary {
    pub id: Uuid,
    pub status: FlowRunStatus,
    pub query: Option<String>,
    pub model: Option<String>,
    pub answer: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunConversationMessageItem {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub display_sequence: i64,
    pub source_kind: String,
    pub role: Option<String>,
    pub content: Option<String>,
    pub query: Option<String>,
    pub model: Option<String>,
    pub answer: Option<String>,
    pub detail_run_id: Option<Uuid>,
    pub can_open_detail: bool,
    pub is_current: bool,
    pub status: String,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub projection_version: i32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationRunTraceProjectionStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Stale,
    Partial,
}

impl ApplicationRunTraceProjectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Stale => "stale",
            Self::Partial => "partial",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunTraceProjectionDiagnostic {
    pub last_error_code: Option<String>,
    pub last_error_stage: Option<String>,
    pub last_error_source_kind: Option<String>,
    pub last_error_source_locator: Option<String>,
    pub last_error_message: Option<String>,
    pub last_error_ref: Option<String>,
    pub retriable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunTraceProjectionStatusRecord {
    pub flow_run_id: Uuid,
    pub projection_version: i32,
    pub status: ApplicationRunTraceProjectionStatus,
    pub source_watermark: String,
    pub attempt_count: i32,
    pub last_attempt_at: Option<OffsetDateTime>,
    pub last_success_at: Option<OffsetDateTime>,
    pub last_error_code: Option<String>,
    pub last_error_stage: Option<String>,
    pub last_error_source_kind: Option<String>,
    pub last_error_source_locator: Option<String>,
    pub last_error_message: Option<String>,
    pub last_error_ref: Option<String>,
    pub retriable: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunTraceNodeRecord {
    pub trace_node_id: Uuid,
    pub flow_run_id: Uuid,
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
    pub projection_version: i32,
    pub source_watermark: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunTraceNodeContentRecord {
    pub trace_node_id: Uuid,
    pub content_kind: String,
    pub payload: serde_json::Value,
    pub source_refs: serde_json::Value,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunDetail {
    pub flow_run: FlowRunRecord,
    pub node_runs: Vec<NodeRunRecord>,
    pub checkpoints: Vec<CheckpointRecord>,
    pub callback_tasks: Vec<CallbackTaskRecord>,
    pub events: Vec<RunEventRecord>,
    pub stitched_trace: Vec<ApplicationRunStitchedTrace>,
    pub subagent_traces: Vec<ApplicationRunSubagentTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunStitchedTrace {
    pub source_flow_run: FlowRunRecord,
    pub node_runs: Vec<NodeRunRecord>,
    pub callback_tasks: Vec<CallbackTaskRecord>,
    pub events: Vec<RunEventRecord>,
    pub runtime_events: Vec<RuntimeEventRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunSubagentTrace {
    pub parent_tool_call_id: String,
    pub parent_callback_task_id: Uuid,
    pub source_flow_run: FlowRunRecord,
    pub node_runs: Vec<NodeRunRecord>,
    pub callback_tasks: Vec<CallbackTaskRecord>,
    pub events: Vec<RunEventRecord>,
    pub runtime_events: Vec<RuntimeEventRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeLastRun {
    pub flow_run: FlowRunRecord,
    pub node_run: NodeRunRecord,
    pub checkpoints: Vec<CheckpointRecord>,
    pub events: Vec<RunEventRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDebugPreviewResult {
    pub flow_run: FlowRunRecord,
    pub node_run: NodeRunRecord,
    pub events: Vec<RunEventRecord>,
    pub preview_payload: serde_json::Value,
}

#[cfg(test)]
mod invocation_context_tests {
    use super::*;

    #[test]
    fn maps_run_modes_and_real_credentials_to_orthogonal_observability_dimensions() {
        let user_id = Uuid::now_v7();
        let api_key_id = Uuid::now_v7();
        let cases = [
            (
                FlowRunMode::PublishedApiRun,
                Some(api_key_id),
                FlowRunExecutionStage::Published,
                FlowRunInvocationSource::AgentFlowApi,
                FlowRunPrincipalKind::ApplicationApiKey,
            ),
            (
                FlowRunMode::WorkflowHttpRun,
                Some(api_key_id),
                FlowRunExecutionStage::Published,
                FlowRunInvocationSource::WorkflowHttp,
                FlowRunPrincipalKind::UserApiKey,
            ),
            (
                FlowRunMode::WorkflowScheduleRun,
                None,
                FlowRunExecutionStage::Published,
                FlowRunInvocationSource::WorkflowSchedule,
                FlowRunPrincipalKind::Scheduler,
            ),
            (
                FlowRunMode::DebugFlowRun,
                None,
                FlowRunExecutionStage::Debug,
                FlowRunInvocationSource::Debug,
                FlowRunPrincipalKind::User,
            ),
        ];

        for (mode, credential, stage, source, principal_kind) in cases {
            let context = mode.invocation_context(
                Some(user_id),
                Some("publication creator".to_string()),
                credential,
            );
            assert_eq!(context.execution_stage, stage);
            assert_eq!(context.invocation_source, source);
            assert_eq!(context.principal.kind, principal_kind);
            if matches!(principal_kind, FlowRunPrincipalKind::Scheduler) {
                assert_eq!(context.principal.id, None);
                assert_eq!(context.principal.display_name, None);
            }
        }

        let current_user_context = FlowRunMode::WorkflowHttpRun.invocation_context(
            Some(user_id),
            Some("current user".to_string()),
            None,
        );
        assert_eq!(
            current_user_context.principal.kind,
            FlowRunPrincipalKind::User
        );
        assert_eq!(current_user_context.principal.id, Some(user_id));
        assert_eq!(
            current_user_context.principal.display_name.as_deref(),
            Some("current user")
        );
    }
}
