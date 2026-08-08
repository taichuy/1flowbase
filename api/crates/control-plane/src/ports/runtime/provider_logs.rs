use super::*;

pub const PROVIDER_REQUEST_LOG_QUEUE: &str = "provider-request-logs";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderRequestLogTask {
    pub scope_id: Uuid,
    pub attempt_id: Uuid,
    pub flow_run_id: Uuid,
    #[serde(default)]
    pub node_run_id: Option<Uuid>,
    pub user_id: Uuid,
    #[serde(default)]
    pub user_account: Option<String>,
    #[serde(default)]
    pub application_id: Option<Uuid>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    pub application_name: String,
    pub attempt_index: i32,
    pub is_retry: bool,
    pub retry_reason: Option<String>,
    pub provider_instance_id: Option<Uuid>,
    pub provider_instance_display_name: Option<String>,
    pub provider_code: String,
    pub protocol: String,
    pub upstream_model_id: String,
    pub reasoning_effort: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub failed_after_first_token: bool,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub started_at: OffsetDateTime,
    pub first_token_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub time_to_first_token_ms: Option<i64>,
    pub total_duration_ms: Option<i64>,
}

pub type InsertModelProviderRequestLogInput = ProviderRequestLogTask;

pub const MODEL_PROVIDER_REQUEST_LOG_DELETE_BATCH_LIMIT: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteModelProviderRequestLogsInput {
    pub scope_id: Uuid,
    pub attempt_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearModelProviderRequestLogsBatchInput {
    pub scope_id: Uuid,
    pub snapshot_created_before: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearModelProviderRequestLogsBatchResult {
    pub deleted_count: u64,
    pub has_more: bool,
    pub snapshot_created_before: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListModelProviderRequestLogsPageInput {
    pub scope_id: Uuid,
    pub flow_run_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub application_name: Option<String>,
    pub provider_instance_id: Option<Uuid>,
    pub model_id: Option<String>,
    pub status: Option<String>,
    pub zero_output_only: bool,
    pub started_after: Option<OffsetDateTime>,
    pub started_before: Option<OffsetDateTime>,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelProviderRequestLogRecord {
    pub attempt_id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub user_account: Option<String>,
    pub application_id: Option<Uuid>,
    pub conversation_id: Option<String>,
    pub application_name: String,
    pub attempt_index: i32,
    pub is_retry: bool,
    pub retry_reason: Option<String>,
    pub provider_instance_id: Option<Uuid>,
    pub provider_instance_display_name: Option<String>,
    pub provider_code: String,
    pub protocol: String,
    pub upstream_model_id: String,
    pub reasoning_effort: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub failed_after_first_token: bool,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub started_at: OffsetDateTime,
    pub first_token_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub time_to_first_token_ms: Option<i64>,
    pub total_duration_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelProviderRequestLogsPage {
    pub items: Vec<ModelProviderRequestLogRecord>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
}
