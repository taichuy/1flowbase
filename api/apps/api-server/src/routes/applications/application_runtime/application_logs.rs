use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use utoipa::ToSchema;

use super::{
    AnswerSnapshotResponse, ApplicationRunStitchedTraceResponse, CallbackTaskResponse,
    CheckpointResponse, FlowRunResponse, NodeRunResponse, RunEventResponse,
};

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunSubjectResponse {
    pub kind: String,
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunPrincipalResponse {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunCorrelationResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication_version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunLogResponse {
    pub id: String,
    pub application_id: String,
    pub application_type: String,
    pub run_object_kind: String,
    pub run_kind: String,
    pub status: String,
    pub title: String,
    pub execution_stage: String,
    pub invocation_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility_mode: Option<String>,
    pub subject: ApplicationRunSubjectResponse,
    pub principal: ApplicationRunPrincipalResponse,
    pub correlation: ApplicationRunCorrelationResponse,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunStatisticsResponse {
    pub count_tokens_input_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub input_cache_hit_tokens: Option<i64>,
    pub input_cache_hit_rate: Option<f64>,
    pub unique_node_count: i64,
    pub tool_callback_count: i64,
}

pub fn input_cache_hit_rate_for_response(
    total_tokens: Option<i64>,
    input_cache_hit_tokens: Option<i64>,
) -> Option<f64> {
    let total_tokens = total_tokens?;
    let input_cache_hit_tokens = input_cache_hit_tokens?;
    if total_tokens <= 0 {
        return None;
    }

    Some(((input_cache_hit_tokens as f64 / total_tokens as f64) * 10_000.0).round() / 10_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_cache_hit_rate_for_response_uses_total_tokens() {
        assert_eq!(
            input_cache_hit_rate_for_response(Some(49_901), Some(49_063)),
            Some(0.9832)
        );
    }

    #[test]
    fn input_cache_hit_rate_for_response_ignores_empty_total() {
        assert_eq!(
            input_cache_hit_rate_for_response(Some(0), Some(49_063)),
            None
        );
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationRunTypedDetailResponse {
    pub kind: String,
    pub flow_run: FlowRunResponse,
    pub answer_snapshot: Option<AnswerSnapshotResponse>,
    pub node_runs: Vec<NodeRunResponse>,
    pub checkpoints: Vec<CheckpointResponse>,
    pub callback_tasks: Vec<CallbackTaskResponse>,
    pub events: Vec<RunEventResponse>,
    pub stitched_trace: Vec<ApplicationRunStitchedTraceResponse>,
}

pub fn principal_response(principal: domain::FlowRunPrincipal) -> ApplicationRunPrincipalResponse {
    ApplicationRunPrincipalResponse {
        kind: principal.kind.as_str().to_string(),
        id: principal.id.map(|value| value.to_string()),
        display_name: principal.display_name,
    }
}

pub fn format_time(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("OffsetDateTime RFC3339 formatting should be valid for stored run timestamps")
}

pub fn format_optional_time(value: Option<OffsetDateTime>) -> Option<String> {
    value.map(format_time)
}
