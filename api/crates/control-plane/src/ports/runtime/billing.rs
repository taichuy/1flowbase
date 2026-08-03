use super::*;

#[derive(Debug, Clone)]
pub struct AppendUsageLedgerInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub failover_attempt_id: Option<Uuid>,
    pub provider_instance_id: Option<Uuid>,
    pub gateway_route_id: Option<Uuid>,
    pub model_id: Option<String>,
    pub upstream_model_id: Option<String>,
    pub upstream_request_id: Option<String>,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub input_cache_hit_tokens: Option<i64>,
    pub input_cache_miss_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub price_snapshot: Option<serde_json::Value>,
    pub cost_snapshot: Option<serde_json::Value>,
    pub usage_status: domain::UsageLedgerStatus,
    pub raw_usage: serde_json::Value,
    pub normalized_usage: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AppendCostLedgerInput {
    pub flow_run_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub usage_ledger_id: Option<Uuid>,
    pub workspace_id: Uuid,
    pub provider_instance_id: Option<Uuid>,
    pub provider_account_id: Option<Uuid>,
    pub gateway_route_id: Option<Uuid>,
    pub model_id: Option<String>,
    pub upstream_model_id: Option<String>,
    pub price_snapshot: serde_json::Value,
    pub raw_cost: Option<String>,
    pub normalized_cost: Option<String>,
    pub settlement_currency: Option<String>,
    pub cost_source: String,
    pub cost_status: String,
}

#[derive(Debug, Clone)]
pub struct AppendCreditLedgerInput {
    pub workspace_id: Uuid,
    pub user_id: Option<Uuid>,
    pub application_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub flow_run_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub cost_ledger_id: Option<Uuid>,
    pub transaction_type: String,
    pub amount: String,
    pub balance_after: Option<String>,
    pub credit_unit: String,
    pub reason: String,
    pub idempotency_key: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct AppendBillingSessionInput {
    pub workspace_id: Uuid,
    pub flow_run_id: Option<Uuid>,
    pub client_request_id: Option<String>,
    pub idempotency_key: String,
    pub route_id: Option<Uuid>,
    pub provider_account_id: Option<Uuid>,
    pub status: domain::BillingSessionStatus,
    pub reserved_credit_ledger_id: Option<Uuid>,
    pub settled_credit_ledger_id: Option<Uuid>,
    pub refund_credit_ledger_id: Option<Uuid>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpsertDataModelSideEffectReceiptInput {
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
}

#[derive(Debug, Clone)]
pub struct DataModelSideEffectReceiptClaim {
    pub record: domain::DataModelSideEffectReceiptRecord,
    pub claimed: bool,
}

#[derive(Debug, Clone)]
pub struct AppendModelFailoverAttemptLedgerInput {
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub llm_turn_span_id: Option<Uuid>,
    pub queue_snapshot_id: Option<Uuid>,
    pub attempt_index: i32,
    pub provider_instance_id: Option<Uuid>,
    pub provider_code: String,
    pub upstream_model_id: String,
    pub protocol: String,
    pub request_ref: Option<String>,
    pub request_hash: Option<String>,
    pub started_at: OffsetDateTime,
    pub first_token_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub status: String,
    pub failed_after_first_token: bool,
    pub upstream_request_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message_ref: Option<String>,
    pub usage_ledger_id: Option<Uuid>,
    pub cost_ledger_id: Option<Uuid>,
    pub response_ref: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LinkUsageLedgerToModelFailoverAttemptInput {
    pub failover_attempt_id: Uuid,
    pub usage_ledger_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct AppendCapabilityInvocationInput {
    pub flow_run_id: Uuid,
    pub span_id: Option<Uuid>,
    pub capability_id: String,
    pub requested_by_span_id: Option<Uuid>,
    pub requester_kind: String,
    pub arguments_ref: Option<String>,
    pub authorization_status: String,
    pub authorization_reason: Option<String>,
    pub result_ref: Option<String>,
    pub normalized_result: Option<serde_json::Value>,
    pub started_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub error_payload: Option<serde_json::Value>,
}
