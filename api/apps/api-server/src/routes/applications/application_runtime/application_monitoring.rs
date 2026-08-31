use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use control_plane::errors::ControlPlaneError;
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState, error_response::ApiError, response::ApiSuccess,
    runtime_activity::ApplicationRuntimeActivitySnapshot,
};

use super::application_logs::{format_optional_time, format_time};

const DEFAULT_TIME_RANGE_DAYS: i64 = 7;
pub(super) const SLOW_RUN_THRESHOLD_MS: i64 = 30_000;

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct ApplicationRunMonitoringQuery {
    #[serde(rename = "from")]
    pub from: Option<String>,
    pub to: Option<String>,
    pub time_range_days: Option<i64>,
    pub bucket: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringMetaResponse {
    pub started_from: Option<String>,
    pub started_to: Option<String>,
    pub bucket: String,
    pub slow_run_threshold_ms: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringReportResponse {
    pub meta: ApplicationRunMonitoringMetaResponse,
    pub overview: ApplicationRunMonitoringOverviewResponse,
    pub duration: ApplicationRunMonitoringDurationResponse,
    pub tokens: ApplicationRunMonitoringTokensResponse,
    pub tokens_comparison: ApplicationRunMonitoringTokensComparisonResponse,
    pub tool_callbacks: ApplicationRunMonitoringToolCallbacksResponse,
    pub nodes: ApplicationRunMonitoringNodesResponse,
    pub concurrency: ApplicationRunMonitoringConcurrencyResponse,
    pub tokens_trend: Vec<ApplicationRunMonitoringTokenTrendPointResponse>,
    pub protocols: Vec<ApplicationRunMonitoringProtocolBreakdownResponse>,
    pub sources: Vec<ApplicationRunMonitoringSourceBreakdownResponse>,
    pub authorized_accounts: Vec<ApplicationRunMonitoringAuthorizedAccountUsageResponse>,
    pub api_keys: Vec<ApplicationRunMonitoringApiKeyUsageResponse>,
    pub external_conversations: Vec<ApplicationRunMonitoringExternalConversationUsageResponse>,
    pub slowest_runs: Vec<ApplicationRunMonitoringRunRankResponse>,
    pub high_token_runs: Vec<ApplicationRunMonitoringRunRankResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringOverviewResponse {
    pub total_count: i64,
    pub success_count: i64,
    pub failed_count: i64,
    pub cancelled_count: i64,
    pub success_rate: f64,
    pub failed_rate: f64,
    pub running_count_included: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringDurationResponse {
    pub duration_recorded_count: i64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub slow_run_rate: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringTokensResponse {
    pub total_tokens_sum: i64,
    pub input_tokens_sum: i64,
    pub output_tokens_sum: i64,
    pub input_cache_hit_tokens_sum: i64,
    pub avg_tokens_per_run: f64,
    pub token_recorded_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringTokensComparisonResponse {
    pub previous_total_tokens_sum: i64,
    pub previous_run_count: i64,
    pub previous_avg_tokens_per_run: f64,
    pub token_change_rate: f64,
    pub run_count_change_rate: f64,
    pub avg_tokens_per_run_change_rate: f64,
    pub traffic_effect: f64,
    pub cost_per_run_effect: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringToolCallbacksResponse {
    pub total_tool_callback_count: i64,
    pub avg_tool_callback_count: f64,
    pub runs_with_tool_callback: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringNodesResponse {
    pub avg_unique_node_count: f64,
    pub max_unique_node_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringConcurrencyResponse {
    pub peak_concurrency: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringTokenTrendPointResponse {
    pub bucket_start: String,
    pub run_count: i64,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub input_cache_hit_tokens: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringProtocolBreakdownResponse {
    pub protocol: String,
    pub request_count: i64,
    pub success_rate: f64,
    pub avg_duration_ms: f64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringSourceBreakdownResponse {
    pub invocation_source: String,
    pub request_count: i64,
    pub success_rate: f64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringAuthorizedAccountUsageResponse {
    pub authorized_account: Option<String>,
    pub request_count: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub failed_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringApiKeyUsageResponse {
    pub api_key_id: String,
    pub api_key_name_snapshot: Option<String>,
    pub request_count: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub failed_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringExternalConversationUsageResponse {
    pub external_conversation_id: Option<String>,
    pub request_count: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub failed_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRunMonitoringRunRankResponse {
    pub flow_run_id: String,
    pub title: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<f64>,
    pub total_tokens: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/monitoring/runtime-activity",
    params(("id" = String, Path, description = "Application id")),
    responses(
        (status = 200, body = ApplicationRuntimeActivitySnapshot),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_runtime_activity(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<ApplicationRuntimeActivitySnapshot>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.monitoring.activity.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        super::interface_runtime_reads::ApplicationRuntimeReadsInput::GetRuntimeActivity {
            application_id: id,
        },
    )
    .await?;
    let super::interface_runtime_reads::ApplicationRuntimeReadsOutput::RuntimeActivity(response) =
        output
    else {
        unreachable!("application runtime activity binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/monitoring/run-metrics",
    params(
        ("id" = String, Path, description = "Application id"),
        ("from" = Option<String>, Query, description = "Inclusive started_at lower bound in RFC3339"),
        ("to" = Option<String>, Query, description = "Exclusive started_at upper bound in RFC3339"),
        ("time_range_days" = Option<i64>, Query, description = "Fallback started_at day window"),
        ("bucket" = Option<String>, Query, description = "Trend bucket: hour, day, week or month")
    ),
    responses(
        (status = 200, body = ApplicationRunMonitoringReportResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_monitoring_report(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<ApplicationRunMonitoringQuery>,
) -> Result<Json<ApiSuccess<ApplicationRunMonitoringReportResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.monitoring.report.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        super::interface_runtime_reads::ApplicationRuntimeReadsInput::GetMonitoringReport {
            application_id: id,
            query,
        },
    )
    .await?;
    let super::interface_runtime_reads::ApplicationRuntimeReadsOutput::MonitoringReport(response) =
        output
    else {
        unreachable!("application monitoring report binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

pub(super) fn parse_optional_time(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<OffsetDateTime>, ApiError> {
    value
        .map(|value| {
            OffsetDateTime::parse(value, &Rfc3339)
                .map_err(|_| ControlPlaneError::InvalidInput(field).into())
        })
        .transpose()
}

pub(super) fn default_started_from(
    query: &ApplicationRunMonitoringQuery,
) -> Option<OffsetDateTime> {
    let days = monitoring_time_range_days(query.time_range_days);

    Some(OffsetDateTime::now_utc() - Duration::days(days))
}

pub(super) fn normalize_monitoring_bucket(
    input: Option<&str>,
    time_range_days: Option<i64>,
) -> &'static str {
    match input {
        Some("hour") => "hour",
        Some("week") => "week",
        Some("month") => "month",
        Some("day") => "day",
        _ => match monitoring_time_range_days(time_range_days) {
            days if days <= 1 => "hour",
            days if days >= 180 => "month",
            days if days >= 60 => "week",
            _ => "day",
        },
    }
}

fn monitoring_time_range_days(time_range_days: Option<i64>) -> i64 {
    time_range_days
        .filter(|days| *days > 0)
        .unwrap_or(DEFAULT_TIME_RANGE_DAYS)
}

pub(super) fn to_report_response(
    report: control_plane::ports::ApplicationRunMonitoringReport,
    meta: ApplicationRunMonitoringMetaResponse,
) -> ApplicationRunMonitoringReportResponse {
    ApplicationRunMonitoringReportResponse {
        meta,
        overview: ApplicationRunMonitoringOverviewResponse {
            total_count: report.overview.total_count,
            success_count: report.overview.success_count,
            failed_count: report.overview.failed_count,
            cancelled_count: report.overview.cancelled_count,
            success_rate: report.overview.success_rate,
            failed_rate: report.overview.failed_rate,
            running_count_included: report.overview.running_count_included,
        },
        duration: ApplicationRunMonitoringDurationResponse {
            duration_recorded_count: report.duration.duration_recorded_count,
            avg_duration_ms: report.duration.avg_duration_ms,
            p50_duration_ms: report.duration.p50_duration_ms,
            p95_duration_ms: report.duration.p95_duration_ms,
            slow_run_rate: report.duration.slow_run_rate,
        },
        tokens: ApplicationRunMonitoringTokensResponse {
            total_tokens_sum: report.tokens.total_tokens_sum,
            input_tokens_sum: report.tokens.input_tokens_sum,
            output_tokens_sum: report.tokens.output_tokens_sum,
            input_cache_hit_tokens_sum: report.tokens.input_cache_hit_tokens_sum,
            avg_tokens_per_run: report.tokens.avg_tokens_per_run,
            token_recorded_count: report.tokens.token_recorded_count,
        },
        tokens_comparison: ApplicationRunMonitoringTokensComparisonResponse {
            previous_total_tokens_sum: report.tokens_comparison.previous_total_tokens_sum,
            previous_run_count: report.tokens_comparison.previous_run_count,
            previous_avg_tokens_per_run: report.tokens_comparison.previous_avg_tokens_per_run,
            token_change_rate: report.tokens_comparison.token_change_rate,
            run_count_change_rate: report.tokens_comparison.run_count_change_rate,
            avg_tokens_per_run_change_rate: report.tokens_comparison.avg_tokens_per_run_change_rate,
            traffic_effect: report.tokens_comparison.traffic_effect,
            cost_per_run_effect: report.tokens_comparison.cost_per_run_effect,
        },
        tool_callbacks: ApplicationRunMonitoringToolCallbacksResponse {
            total_tool_callback_count: report.tool_callbacks.total_tool_callback_count,
            avg_tool_callback_count: report.tool_callbacks.avg_tool_callback_count,
            runs_with_tool_callback: report.tool_callbacks.runs_with_tool_callback,
        },
        nodes: ApplicationRunMonitoringNodesResponse {
            avg_unique_node_count: report.nodes.avg_unique_node_count,
            max_unique_node_count: report.nodes.max_unique_node_count,
        },
        concurrency: ApplicationRunMonitoringConcurrencyResponse {
            peak_concurrency: report.concurrency.peak_concurrency,
        },
        tokens_trend: report
            .tokens_trend
            .into_iter()
            .map(|point| ApplicationRunMonitoringTokenTrendPointResponse {
                bucket_start: format_time(point.bucket_start),
                run_count: point.run_count,
                total_tokens: point.total_tokens,
                input_tokens: point.input_tokens,
                output_tokens: point.output_tokens,
                input_cache_hit_tokens: point.input_cache_hit_tokens,
            })
            .collect(),
        protocols: report
            .protocols
            .into_iter()
            .map(
                |protocol| ApplicationRunMonitoringProtocolBreakdownResponse {
                    protocol: protocol.protocol,
                    request_count: protocol.request_count,
                    success_rate: protocol.success_rate,
                    avg_duration_ms: protocol.avg_duration_ms,
                    total_tokens: protocol.total_tokens,
                },
            )
            .collect(),
        sources: report
            .sources
            .into_iter()
            .map(|source| ApplicationRunMonitoringSourceBreakdownResponse {
                invocation_source: source.invocation_source,
                request_count: source.request_count,
                success_rate: source.success_rate,
                total_tokens: source.total_tokens,
            })
            .collect(),
        authorized_accounts: report
            .authorized_accounts
            .into_iter()
            .map(
                |usage| ApplicationRunMonitoringAuthorizedAccountUsageResponse {
                    authorized_account: usage.authorized_account,
                    request_count: usage.request_count,
                    total_tokens: usage.total_tokens,
                    avg_duration_ms: usage.avg_duration_ms,
                    failed_count: usage.failed_count,
                },
            )
            .collect(),
        api_keys: report
            .api_keys
            .into_iter()
            .map(|usage| ApplicationRunMonitoringApiKeyUsageResponse {
                api_key_id: usage.api_key_id.to_string(),
                api_key_name_snapshot: usage.api_key_name_snapshot,
                request_count: usage.request_count,
                total_tokens: usage.total_tokens,
                avg_duration_ms: usage.avg_duration_ms,
                failed_count: usage.failed_count,
            })
            .collect(),
        external_conversations: report
            .external_conversations
            .into_iter()
            .map(
                |usage| ApplicationRunMonitoringExternalConversationUsageResponse {
                    external_conversation_id: usage.external_conversation_id,
                    request_count: usage.request_count,
                    total_tokens: usage.total_tokens,
                    avg_duration_ms: usage.avg_duration_ms,
                    failed_count: usage.failed_count,
                },
            )
            .collect(),
        slowest_runs: report
            .slowest_runs
            .into_iter()
            .map(to_run_rank_response)
            .collect(),
        high_token_runs: report
            .high_token_runs
            .into_iter()
            .map(to_run_rank_response)
            .collect(),
    }
}

fn to_run_rank_response(
    run: control_plane::ports::ApplicationRunMonitoringRunRank,
) -> ApplicationRunMonitoringRunRankResponse {
    ApplicationRunMonitoringRunRankResponse {
        flow_run_id: run.flow_run_id.to_string(),
        title: run.title,
        status: run.status.as_str().to_string(),
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
        duration_ms: run.duration_ms,
        total_tokens: run.total_tokens,
    }
}
