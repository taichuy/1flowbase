use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GatewayLogQuery {
    pub conversation_id: Option<Uuid>,
    pub turn_id: Option<Uuid>,
    pub flow_run_id: Option<Uuid>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
#[derive(Debug, Clone)]
pub struct GatewayLogScope {
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub api_key_id: Option<Uuid>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GatewayLogMetrics {
    pub invocation_count: i64,
    pub attempt_count: i64,
    pub failed_attempt_count: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub elapsed_ms: Option<i64>,
    pub model_duration_ms: Option<i64>,
    pub tool_result_wait_ms: Option<i64>,
    pub costs: Vec<GatewayLogCost>,
    pub unknown_cost_attempts: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GatewayLogCost {
    pub currency_code: String,
    pub amount: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GatewayLogMessage {
    pub id: String,
    pub kind: String,
    pub phase: Option<String>,
    pub text: Option<String>,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<String>,
    pub tool_result: Option<String>,
    pub result_received: bool,
    pub execution_verified: bool,
    pub flow_run_id: Uuid,
    pub sequence: i64,
    pub identity_status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GatewayLogEntry {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub identity_status: String,
    pub thread_id: Option<String>,
    pub client_turn_id: Option<String>,
    pub request_kind: Option<String>,
    pub parent_thread_id: Option<String>,
    pub parent_turn_id: Option<String>,
    pub relation_status: String,
    pub parent_task_id: Option<Uuid>,
    pub parent_conversation_id: Option<Uuid>,
    pub forked_from_thread_id: Option<String>,
    pub identity_sources: Vec<String>,
    pub completion_status: String,
    pub observations: Vec<String>,
    pub flow_run_id: Option<Uuid>,
    pub caused_by_run_id: Option<Uuid>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: Option<String>,
    pub attempt_index: Option<i32>,
    pub is_retry: Option<bool>,
    pub error_code: Option<String>,
    pub metrics: GatewayLogMetrics,
    pub messages: Vec<GatewayLogMessage>,
    pub messages_has_more: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GatewayLogPage {
    pub items: Vec<GatewayLogEntry>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[async_trait::async_trait]
pub trait GatewayLogRepository: Send + Sync {
    /// Rebuild a scoped conversation projection from retained formal facts.
    /// Returns false when the conversation is outside the supplied scope.
    async fn rebuild_gateway_log_conversation(
        &self,
        scope: &GatewayLogScope,
        conversation_id: Uuid,
    ) -> anyhow::Result<bool>;
    async fn list_gateway_log_page(
        &self,
        scope: &GatewayLogScope,
        query: &GatewayLogQuery,
    ) -> anyhow::Result<GatewayLogPage>;
}
