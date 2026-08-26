use super::*;

pub struct GetApplicationRunMonitoringReportInput {
    pub started_from: Option<OffsetDateTime>,
    pub started_to: Option<OffsetDateTime>,
    pub bucket: String,
    pub slow_run_threshold_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringReport {
    pub overview: ApplicationRunMonitoringOverview,
    pub duration: ApplicationRunMonitoringDuration,
    pub tokens: ApplicationRunMonitoringTokens,
    pub tokens_comparison: ApplicationRunMonitoringTokensComparison,
    pub tool_callbacks: ApplicationRunMonitoringToolCallbacks,
    pub nodes: ApplicationRunMonitoringNodes,
    pub concurrency: ApplicationRunMonitoringConcurrency,
    pub tokens_trend: Vec<ApplicationRunMonitoringTokenTrendPoint>,
    pub protocols: Vec<ApplicationRunMonitoringProtocolBreakdown>,
    pub sources: Vec<ApplicationRunMonitoringSourceBreakdown>,
    pub authorized_accounts: Vec<ApplicationRunMonitoringAuthorizedAccountUsage>,
    pub api_keys: Vec<ApplicationRunMonitoringApiKeyUsage>,
    pub external_conversations: Vec<ApplicationRunMonitoringExternalConversationUsage>,
    pub slowest_runs: Vec<ApplicationRunMonitoringRunRank>,
    pub high_token_runs: Vec<ApplicationRunMonitoringRunRank>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringOverview {
    pub total_count: i64,
    pub success_count: i64,
    pub failed_count: i64,
    pub cancelled_count: i64,
    pub success_rate: f64,
    pub failed_rate: f64,
    pub running_count_included: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringDuration {
    pub duration_recorded_count: i64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub slow_run_rate: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringTokens {
    pub total_tokens_sum: i64,
    pub input_tokens_sum: i64,
    pub output_tokens_sum: i64,
    pub input_cache_hit_tokens_sum: i64,
    pub avg_tokens_per_run: f64,
    pub token_recorded_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringTokensComparison {
    pub previous_total_tokens_sum: i64,
    pub previous_run_count: i64,
    pub previous_avg_tokens_per_run: f64,
    pub token_change_rate: f64,
    pub run_count_change_rate: f64,
    pub avg_tokens_per_run_change_rate: f64,
    pub traffic_effect: f64,
    pub cost_per_run_effect: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringToolCallbacks {
    pub total_tool_callback_count: i64,
    pub avg_tool_callback_count: f64,
    pub runs_with_tool_callback: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringNodes {
    pub avg_unique_node_count: f64,
    pub max_unique_node_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunMonitoringConcurrency {
    pub peak_concurrency: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRunMonitoringTokenTrendPoint {
    pub bucket_start: OffsetDateTime,
    pub run_count: i64,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub input_cache_hit_tokens: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringProtocolBreakdown {
    pub protocol: String,
    pub request_count: i64,
    pub success_rate: f64,
    pub avg_duration_ms: f64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringSourceBreakdown {
    pub invocation_source: String,
    pub request_count: i64,
    pub success_rate: f64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringAuthorizedAccountUsage {
    pub authorized_account: Option<String>,
    pub request_count: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub failed_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringApiKeyUsage {
    pub api_key_id: Uuid,
    pub api_key_name_snapshot: Option<String>,
    pub request_count: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub failed_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringExternalConversationUsage {
    pub external_conversation_id: Option<String>,
    pub request_count: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub failed_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationRunMonitoringRunRank {
    pub flow_run_id: Uuid,
    pub title: String,
    pub status: domain::FlowRunStatus,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub duration_ms: Option<f64>,
    pub total_tokens: Option<i64>,
}
