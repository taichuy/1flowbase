use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::sse::{KeepAlive, Sse},
    routing::{get, post, put},
    Json, Router,
};
use control_plane::{
    application::ApplicationService,
    errors::ControlPlaneError,
    orchestration_runtime::{
        debug_stream_events, CancelFlowRunCommand, CompleteCallbackTaskCommand,
        ContinueFlowDebugRunCommand, OrchestrationRuntimeService, PrepareFlowDebugRunCommand,
        ResumeFlowRunCommand, StartFlowDebugRunCommand, StartNodeDebugPreviewCommand,
    },
    ports::{
        ListApplicationConversationRunsPageInput, OrchestrationRuntimeRepository,
        RuntimeEventCloseReason, RuntimeEventStream, RuntimeEventStreamPolicy,
    },
};
use serde::{Deserialize, Serialize};
use storage_durable::MainDurableStore;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{error, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
};

use super::debug_run_stream;
mod application_logs;
pub(crate) mod debug_variable_cache;
pub(crate) mod debug_variable_snapshot;
mod runtime_debug_artifacts;

pub use debug_variable_cache::{
    delete_debug_variable_cache_entries, upsert_debug_variable_cache_entry,
};
pub use debug_variable_snapshot::{get_debug_variable_snapshot, DebugVariableSnapshotResponse};
use runtime_debug_artifacts::{
    application_run_answer, application_run_model, application_run_query,
    load_runtime_debug_artifact_response, offload_application_run_detail_artifacts,
};

fn is_terminal_runtime_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "flow_finished" | "flow_failed" | "flow_cancelled" | "waiting_human" | "waiting_callback"
    )
}

async fn fail_runtime_event_stream_if_missing_terminal(
    stream: Arc<dyn RuntimeEventStream>,
    run_id: Uuid,
    error: &anyhow::Error,
) {
    match stream.replay(run_id, None, usize::MAX).await {
        Ok(events)
            if events
                .iter()
                .any(|event| is_terminal_runtime_event(&event.event_type)) =>
        {
            return;
        }
        Ok(_) => {}
        Err(replay_error) => {
            warn!(
                flow_run_id = %run_id,
                error = %replay_error,
                "failed to check runtime event stream terminal state"
            );
        }
    }

    let error_payload = serde_json::json!({ "message": error.to_string() });
    if let Err(append_error) = stream
        .append(
            run_id,
            debug_stream_events::flow_failed(run_id, error_payload),
        )
        .await
    {
        warn!(
            flow_run_id = %run_id,
            event_type = "flow_failed",
            error = %append_error,
            "failed to append fallback runtime terminal event"
        );
    }
    if let Err(close_error) = stream
        .close_run(run_id, RuntimeEventCloseReason::Failed)
        .await
    {
        warn!(
            flow_run_id = %run_id,
            reason = ?RuntimeEventCloseReason::Failed,
            error = %close_error,
            "failed to close fallback runtime event stream"
        );
    }
}

fn spawn_debug_event_persister<R>(
    repository: R,
    stream: Arc<dyn RuntimeEventStream>,
    run_id: Uuid,
) -> JoinHandle<()>
where
    R: OrchestrationRuntimeRepository + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let Ok(mut subscription) = stream.subscribe(run_id, Some(0)).await else {
            warn!(
                flow_run_id = %run_id,
                "failed to subscribe runtime debug stream for durable event persistence"
            );
            return;
        };

        let mut batch = Vec::new();
        for event in subscription.replay {
            if push_debug_event_for_persistence(&repository, &mut batch, run_id, event).await {
                return;
            }
        }

        loop {
            let Some(event) = subscription.live_events.recv().await else {
                let _ = flush_debug_event_batch(&repository, &mut batch, run_id).await;
                return;
            };

            if push_debug_event_for_persistence(&repository, &mut batch, run_id, event).await {
                return;
            }
        }
    })
}

async fn wait_for_debug_event_persister(
    handle: JoinHandle<()>,
    application_id: Uuid,
    run_id: Uuid,
) {
    match tokio::time::timeout(std::time::Duration::from_secs(2), handle).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            warn!(
                application_id = %application_id,
                flow_run_id = %run_id,
                error = %error,
                "runtime debug stream persister task panicked"
            );
        }
        Err(_) => {
            warn!(
                application_id = %application_id,
                flow_run_id = %run_id,
                "runtime debug stream persister did not finish after terminal event"
            );
        }
    }
}

async fn push_debug_event_for_persistence<R>(
    repository: &R,
    batch: &mut Vec<control_plane::ports::RuntimeEventEnvelope>,
    run_id: Uuid,
    event: control_plane::ports::RuntimeEventEnvelope,
) -> bool
where
    R: OrchestrationRuntimeRepository,
{
    let is_terminal = is_terminal_runtime_event(&event.event_type);
    let is_stream_delta = event.event_type == "text_delta" || event.event_type == "reasoning_delta";
    if is_stream_delta
        && batch
            .last()
            .is_some_and(|previous| previous.event_type != event.event_type)
    {
        flush_debug_event_batch(repository, batch, run_id).await;
    }
    batch.push(event);
    if is_terminal || !is_stream_delta {
        return flush_debug_event_batch(repository, batch, run_id).await || is_terminal;
    }
    false
}

async fn flush_debug_event_batch<R>(
    repository: &R,
    batch: &mut Vec<control_plane::ports::RuntimeEventEnvelope>,
    run_id: Uuid,
) -> bool
where
    R: OrchestrationRuntimeRepository,
{
    if batch.is_empty() {
        return false;
    }

    let has_terminal = batch
        .iter()
        .any(|event| is_terminal_runtime_event(&event.event_type));
    let events = std::mem::take(batch);
    if let Err(error) = control_plane::orchestration_runtime::persist_runtime_debug_stream_events(
        repository, events,
    )
    .await
    {
        warn!(
            flow_run_id = %run_id,
            error = %error,
            "failed to persist runtime debug stream events"
        );
    }

    has_terminal
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartNodeDebugPreviewBody {
    pub input_payload: serde_json::Value,
    pub document: Option<serde_json::Value>,
    pub debug_session_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartFlowDebugRunBody {
    pub input_payload: serde_json::Value,
    pub document: Option<serde_json::Value>,
    pub debug_session_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DebugRunStreamQuery {
    pub from_sequence: Option<i64>,
    pub last_event_id: Option<String>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct ApplicationRunsQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub time_range_days: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResumeFlowRunBody {
    pub checkpoint_id: String,
    pub input_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompleteCallbackTaskBody {
    pub response_payload: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FlowRunSummaryResponse {
    pub id: String,
    pub application_id: String,
    pub application_type: String,
    pub run_object_kind: String,
    pub run_kind: String,
    pub run_mode: String,
    pub status: String,
    pub target_node_id: Option<String>,
    pub title: String,
    pub expand_id: Option<String>,
    pub authorized_account: Option<String>,
    pub source: String,
    pub protocol: Option<String>,
    pub subject: application_logs::ApplicationRunSubjectResponse,
    pub actor: application_logs::ApplicationRunActorResponse,
    pub correlation: application_logs::ApplicationRunCorrelationResponse,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FlowRunSummaryPageResponse {
    pub items: Vec<FlowRunSummaryResponse>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FlowRunResponse {
    pub id: String,
    pub application_id: String,
    pub flow_id: String,
    pub draft_id: String,
    pub compiled_plan_id: Option<String>,
    pub run_mode: String,
    pub status: String,
    pub target_node_id: Option<String>,
    pub title: String,
    pub expand_id: Option<String>,
    pub authorized_account: Option<String>,
    pub external_conversation_id: Option<String>,
    pub query: Option<String>,
    pub model: Option<String>,
    pub input_payload: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub created_by: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplicationConversationMessagesQuery {
    pub around_run_id: Option<Uuid>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationConversationMessageResponse {
    pub run_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub query: Option<String>,
    pub model: Option<String>,
    pub answer: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationConversationMessagesPageInfoResponse {
    pub has_before: bool,
    pub has_after: bool,
    pub before_cursor: Option<String>,
    pub after_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationConversationMessagesPageResponse {
    pub items: Vec<ApplicationConversationMessageResponse>,
    pub page: ApplicationConversationMessagesPageInfoResponse,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NodeRunResponse {
    pub id: String,
    pub flow_run_id: String,
    pub node_id: String,
    pub node_type: String,
    pub node_alias: String,
    pub status: String,
    pub input_payload: serde_json::Value,
    pub input_payload_view: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub metrics_payload: serde_json::Value,
    pub debug_payload: serde_json::Value,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CheckpointResponse {
    pub id: String,
    pub flow_run_id: String,
    pub node_run_id: Option<String>,
    pub status: String,
    pub reason: String,
    pub locator_payload: serde_json::Value,
    pub variable_snapshot: serde_json::Value,
    pub external_ref_payload: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CallbackTaskResponse {
    pub id: String,
    pub flow_run_id: String,
    pub node_run_id: String,
    pub callback_kind: String,
    pub status: String,
    pub request_payload: serde_json::Value,
    pub response_payload: Option<serde_json::Value>,
    pub external_ref_payload: Option<serde_json::Value>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunEventResponse {
    pub id: String,
    pub flow_run_id: String,
    pub node_run_id: Option<String>,
    pub sequence: i64,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationRunDetailResponse {
    pub run: application_logs::ApplicationRunLogResponse,
    pub detail: application_logs::ApplicationRunTypedDetailResponse,
    pub flow_run: FlowRunResponse,
    pub node_runs: Vec<NodeRunResponse>,
    pub checkpoints: Vec<CheckpointResponse>,
    pub callback_tasks: Vec<CallbackTaskResponse>,
    pub events: Vec<RunEventResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeDebugStreamResponse {
    pub parts: Vec<RuntimeDebugStreamPartResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeDebugStreamPartResponse {
    pub id: String,
    pub flow_run_id: String,
    pub item_id: Option<String>,
    pub span_id: Option<String>,
    pub part_type: String,
    pub status: String,
    pub trust_level: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeLastRunResponse {
    pub flow_run: FlowRunResponse,
    pub node_run: NodeRunResponse,
    pub checkpoints: Vec<CheckpointResponse>,
    pub events: Vec<RunEventResponse>,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/applications/:id/orchestration/debug-runs",
            post(start_flow_debug_run),
        )
        .route(
            "/applications/:id/orchestration/debug-runs/stream",
            post(start_flow_debug_run_stream),
        )
        .route(
            "/applications/:id/orchestration/runs/:run_id/debug-stream",
            get(subscribe_flow_debug_run_stream),
        )
        .route(
            "/applications/:id/orchestration/runs/:run_id/resume",
            post(resume_flow_run),
        )
        .route(
            "/applications/:id/orchestration/runs/:run_id/cancel",
            post(cancel_flow_run),
        )
        .route(
            "/applications/:id/orchestration/callback-tasks/:callback_task_id/complete",
            post(complete_callback_task),
        )
        .route(
            "/applications/:id/orchestration/nodes/:node_id/debug-runs",
            post(start_node_debug_preview),
        )
        .route(
            "/applications/:id/orchestration/debug-variable-snapshot",
            get(get_debug_variable_snapshot),
        )
        .route(
            "/applications/:id/orchestration/debug-variable-cache",
            put(upsert_debug_variable_cache_entry).delete(delete_debug_variable_cache_entries),
        )
        .route(
            "/applications/:id/orchestration/debug-artifacts/:artifact_id",
            get(get_runtime_debug_artifact),
        )
        .route("/applications/:id/logs/runs", get(list_application_runs))
        .route(
            "/applications/:id/logs/conversations/:conversation_id/messages",
            get(list_application_conversation_messages),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/conversation/messages",
            get(list_application_run_conversation_messages),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/nodes/:node_id",
            get(get_application_run_node_last_run),
        )
        .route(
            "/applications/:id/logs/runs/:run_id",
            get(get_application_run_detail),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/debug-stream",
            get(get_runtime_debug_stream),
        )
        .route(
            "/applications/:id/orchestration/nodes/:node_id/last-run",
            get(get_node_last_run),
        )
}

fn format_time(value: time::OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap()
}

fn format_optional_time(value: Option<time::OffsetDateTime>) -> Option<String> {
    value.map(format_time)
}

fn to_flow_run_summary_response(
    application: &domain::ApplicationRecord,
    summary: domain::ApplicationRunSummary,
) -> FlowRunSummaryResponse {
    let application_type = application.application_type.as_str().to_string();
    let run_object_kind = application.sections.logs.run_object_kind.clone();
    let subject = application_logs::ApplicationRunSubjectResponse {
        kind: application_type.clone(),
        id: Some(application.id.to_string()),
        draft_id: None,
        target_node_id: summary.target_node_id.clone(),
    };
    let actor = application_logs::actor_from_console_user(
        summary.user_id.clone(),
        summary.authorized_account.clone(),
    );
    let correlation = application_logs::ApplicationRunCorrelationResponse {
        api_key_id: summary.api_key_id.map(|value| value.to_string()),
        publication_version_id: summary
            .publication_version_id
            .map(|value| value.to_string()),
        external_user: summary.user_id.clone(),
        external_conversation_id: summary.external_conversation_id.clone(),
        external_trace_id: summary.external_trace_id.clone(),
        compatibility_mode: summary.compatibility_mode.clone(),
        idempotency_key: summary.idempotency_key.clone(),
    };

    FlowRunSummaryResponse {
        id: summary.id.to_string(),
        application_id: application.id.to_string(),
        application_type,
        run_object_kind,
        run_kind: summary.run_mode.as_str().to_string(),
        run_mode: summary.run_mode.as_str().to_string(),
        status: summary.status.as_str().to_string(),
        target_node_id: summary.target_node_id,
        title: summary.title,
        expand_id: summary.user_id,
        authorized_account: summary.authorized_account,
        source: application_logs::source_for_run(summary.api_key_id),
        protocol: summary.compatibility_mode,
        subject,
        actor,
        correlation,
        started_at: format_time(summary.started_at),
        finished_at: format_optional_time(summary.finished_at),
        created_at: format_time(summary.created_at),
        updated_at: format_time(summary.updated_at),
    }
}

fn application_runs_created_after(query: &ApplicationRunsQuery) -> Option<OffsetDateTime> {
    let days = query.time_range_days?;

    if days <= 0 {
        return None;
    }

    Some(OffsetDateTime::now_utc() - Duration::days(days))
}

fn normalize_application_run_sort_by(input: Option<&str>) -> &'static str {
    match input.unwrap_or("created_at") {
        "created_at" => "created_at",
        "started_at" => "started_at",
        "finished_at" => "finished_at",
        "updated_at" => "updated_at",
        _ => "created_at",
    }
}

fn normalize_application_run_sort_order(input: Option<&str>) -> &'static str {
    match input.unwrap_or("desc").to_ascii_lowercase().as_str() {
        "asc" => "asc",
        _ => "desc",
    }
}

fn to_flow_run_response(run: domain::FlowRunRecord) -> FlowRunResponse {
    FlowRunResponse {
        id: run.id.to_string(),
        application_id: run.application_id.to_string(),
        flow_id: run.flow_id.to_string(),
        draft_id: run.draft_id.to_string(),
        compiled_plan_id: run.compiled_plan_id.map(|value| value.to_string()),
        run_mode: run.run_mode.as_str().to_string(),
        status: run.status.as_str().to_string(),
        target_node_id: run.target_node_id,
        title: run.title,
        expand_id: run.external_user,
        authorized_account: run.authorized_account,
        external_conversation_id: run.external_conversation_id,
        query: application_run_query(&run.input_payload),
        model: application_run_model(&run.input_payload),
        input_payload: run.input_payload,
        output_payload: run.output_payload,
        error_payload: run.error_payload,
        created_by: run.created_by.to_string(),
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
        created_at: format_time(run.created_at),
        updated_at: format_time(run.updated_at),
    }
}

fn to_application_conversation_message_response(
    run: domain::FlowRunRecord,
    current_run_id: Option<Uuid>,
) -> ApplicationConversationMessageResponse {
    ApplicationConversationMessageResponse {
        run_id: run.id.to_string(),
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
        status: run.status.as_str().to_string(),
        query: application_run_query(&run.input_payload),
        model: application_run_model(&run.input_payload),
        answer: application_run_answer(&run.output_payload).or_else(|| {
            run.error_payload
                .as_ref()
                .and_then(|payload| application_run_answer(payload))
        }),
        is_current: current_run_id == Some(run.id),
    }
}

fn parse_optional_uuid_cursor(value: Option<&str>) -> Option<Uuid> {
    value.and_then(|value| Uuid::parse_str(value).ok())
}

fn fallback_conversation_messages_from_run(
    run: &domain::FlowRunRecord,
    query: &ApplicationConversationMessagesQuery,
) -> ApplicationConversationMessagesPageResponse {
    let limit = query.limit.unwrap_or(5).clamp(1, 50) as usize;
    let mut items = history_conversation_items_from_payload(run);

    items.push(ApplicationConversationMessageResponse {
        run_id: run.id.to_string(),
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
        status: run.status.as_str().to_string(),
        query: application_run_query(&run.input_payload),
        model: application_run_model(&run.input_payload),
        answer: application_run_answer(&run.output_payload).or_else(|| {
            run.error_payload
                .as_ref()
                .and_then(|payload| application_run_answer(payload))
        }),
        is_current: true,
    });

    let total = items.len();
    let (start, end) = fallback_conversation_window(run.id, total, limit, query);
    let page_items = items
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<Vec<_>>();

    ApplicationConversationMessagesPageResponse {
        items: page_items,
        page: ApplicationConversationMessagesPageInfoResponse {
            has_before: start > 0,
            has_after: end < total,
            before_cursor: (start > 0).then(|| fallback_conversation_cursor(run.id, start)),
            after_cursor: (end < total).then(|| fallback_conversation_cursor(run.id, end - 1)),
        },
    }
}

fn fallback_conversation_window(
    run_id: Uuid,
    total: usize,
    limit: usize,
    query: &ApplicationConversationMessagesQuery,
) -> (usize, usize) {
    if total == 0 {
        return (0, 0);
    }

    if let Some(before) = query
        .before
        .as_deref()
        .and_then(|cursor| parse_fallback_conversation_cursor(run_id, cursor))
    {
        let end = before.min(total);
        return (end.saturating_sub(limit), end);
    }

    if let Some(after) = query
        .after
        .as_deref()
        .and_then(|cursor| parse_fallback_conversation_cursor(run_id, cursor))
    {
        let start = (after + 1).min(total);
        return (start, (start + limit).min(total));
    }

    (total.saturating_sub(limit), total)
}

fn fallback_conversation_cursor(run_id: Uuid, index: usize) -> String {
    format!("{run_id}:history:{index}")
}

fn parse_fallback_conversation_cursor(run_id: Uuid, cursor: &str) -> Option<usize> {
    let (prefix, index) = cursor.rsplit_once(":history:")?;
    if prefix != run_id.to_string() {
        return None;
    }

    index.parse().ok()
}

fn history_conversation_items_from_payload(
    run: &domain::FlowRunRecord,
) -> Vec<ApplicationConversationMessageResponse> {
    let decoded_payload = decode_runtime_debug_artifact_preview(&run.input_payload);
    let source = decoded_payload.as_ref().unwrap_or(&run.input_payload);
    let start_payload = start_input_payload(source);
    let Some(history) = start_payload
        .get("history")
        .or_else(|| start_payload.get("messages"))
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    let mut items = Vec::new();
    let mut pending_user: Option<String> = None;

    for message in history {
        let role = message
            .get("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let Some(content) = conversation_message_content(message) else {
            continue;
        };

        match role {
            "user" => {
                if let Some(query) = pending_user.replace(content) {
                    items.push(fallback_history_item(run, items.len(), query, None));
                }
            }
            "assistant" => {
                if let Some(query) = pending_user.take() {
                    items.push(fallback_history_item(
                        run,
                        items.len(),
                        query,
                        Some(content),
                    ));
                }
            }
            _ => {}
        }
    }

    if let Some(query) = pending_user {
        items.push(fallback_history_item(run, items.len(), query, None));
    }

    items
}

fn fallback_history_item(
    run: &domain::FlowRunRecord,
    index: usize,
    query: String,
    answer: Option<String>,
) -> ApplicationConversationMessageResponse {
    ApplicationConversationMessageResponse {
        run_id: fallback_conversation_cursor(run.id, index),
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
        status: "succeeded".to_string(),
        query: Some(query),
        model: application_run_model(&run.input_payload),
        answer,
        is_current: false,
    }
}

fn conversation_message_content(message: &serde_json::Value) -> Option<String> {
    let content = message.get("content")?;
    if let Some(text) = content
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(text.to_string());
    }

    if let Some(array) = content.as_array() {
        let text = array
            .iter()
            .filter_map(conversation_content_part_text)
            .collect::<Vec<_>>()
            .join("");
        return (!text.is_empty()).then_some(text);
    }

    conversation_content_part_text(content)
}

fn conversation_content_part_text(part: &serde_json::Value) -> Option<String> {
    if let Some(text) = part
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(text.to_string());
    }

    part.get("text")
        .or_else(|| part.get("content"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn to_node_run_response(run: domain::NodeRunRecord) -> NodeRunResponse {
    let (input_payload, output_payload) = normalize_node_run_payloads_for_logs(&run);
    let input_payload_view = node_input_payload_view(&run.node_type, &input_payload);

    NodeRunResponse {
        id: run.id.to_string(),
        flow_run_id: run.flow_run_id.to_string(),
        node_id: run.node_id,
        node_type: run.node_type,
        node_alias: run.node_alias,
        status: run.status.as_str().to_string(),
        input_payload,
        input_payload_view,
        output_payload,
        error_payload: run.error_payload,
        metrics_payload: run.metrics_payload,
        debug_payload: run.debug_payload,
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
    }
}

fn normalize_node_run_payloads_for_logs(
    run: &domain::NodeRunRecord,
) -> (serde_json::Value, serde_json::Value) {
    if run.node_type == "start" {
        let input_payload = if run
            .input_payload
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
            && run
                .output_payload
                .as_object()
                .is_some_and(|object| !object.is_empty())
        {
            run.output_payload.clone()
        } else {
            run.input_payload.clone()
        };

        return (input_payload, serde_json::json!({}));
    }

    (run.input_payload.clone(), run.output_payload.clone())
}

fn node_input_payload_view(node_type: &str, payload: &serde_json::Value) -> serde_json::Value {
    if node_type != "start" {
        return payload.clone();
    }

    start_input_payload_summary_view(payload)
}

fn start_input_payload_summary_view(payload: &serde_json::Value) -> serde_json::Value {
    let decoded_payload = decode_runtime_debug_artifact_preview(payload);
    let source = decoded_payload.as_ref().unwrap_or(payload);
    let start_payload = start_input_payload(source);
    let artifact_object = payload.as_object().filter(|object| {
        object
            .get("__runtime_debug_artifact")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    });
    let mut view = serde_json::Map::new();

    view.insert(
        "kind".to_string(),
        serde_json::Value::String("start_input_summary".to_string()),
    );

    if let Some(object) = artifact_object {
        if let Some(value) = object
            .get("artifact_ref")
            .and_then(serde_json::Value::as_str)
        {
            view.insert(
                "artifact_ref".to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        if let Some(value) = object
            .get("is_truncated")
            .and_then(serde_json::Value::as_bool)
        {
            view.insert("is_truncated".to_string(), serde_json::Value::Bool(value));
        }
        if let Some(value) = object
            .get("original_size_bytes")
            .and_then(serde_json::Value::as_i64)
        {
            view.insert("original_size_bytes".to_string(), value.into());
        }
        if let Some(value) = object
            .get("preview_size_bytes")
            .and_then(serde_json::Value::as_i64)
        {
            view.insert("preview_size_bytes".to_string(), value.into());
        }
    }

    view.insert(
        "preview".to_string(),
        start_input_payload_preview(source, start_payload),
    );

    serde_json::Value::Object(view)
}

fn decode_runtime_debug_artifact_preview(payload: &serde_json::Value) -> Option<serde_json::Value> {
    let object = payload.as_object()?;
    if !object
        .get("__runtime_debug_artifact")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    object
        .get("preview")
        .and_then(serde_json::Value::as_str)
        .and_then(|preview| serde_json::from_str(preview).ok())
}

fn start_input_payload(payload: &serde_json::Value) -> &serde_json::Value {
    payload
        .get("node-start")
        .or_else(|| payload.get("start"))
        .unwrap_or(payload)
}

fn start_input_payload_preview(
    source: &serde_json::Value,
    start_payload: &serde_json::Value,
) -> serde_json::Value {
    let mut preview = serde_json::Map::new();
    let query = immediate_start_input_text(start_payload)
        .or_else(|| application_run_query(source))
        .unwrap_or_default();
    let model = immediate_start_model_value(start_payload)
        .or_else(|| application_run_model(source))
        .unwrap_or_default();

    preview.insert("query".to_string(), serde_json::Value::String(query));
    preview.insert("model".to_string(), serde_json::Value::String(model));
    preview.insert(
        "files".to_string(),
        named_array_value(start_payload, &["files", "attachments"])
            .map(|value| serde_json::Value::Array(value.clone()))
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
    );
    preview.insert(
        "history".to_string(),
        placeholder_array_for_named_array(start_payload, &["history", "messages"]),
    );
    preview.insert(
        "tools".to_string(),
        placeholder_array_for_named_array(
            start_payload,
            &["tools", "tool_registry", "tool_definitions"],
        ),
    );

    serde_json::Value::Object(preview)
}

fn placeholder_array_for_named_array(
    payload: &serde_json::Value,
    keys: &[&str],
) -> serde_json::Value {
    if find_named_array_len(payload, keys).unwrap_or_default() == 0 {
        return serde_json::Value::Array(Vec::new());
    }

    serde_json::Value::Array(vec![serde_json::Value::String("...".to_string())])
}

fn immediate_start_input_text(value: &serde_json::Value) -> Option<String> {
    let object = value.as_object()?;
    for key in ["query", "question", "prompt", "message", "input"] {
        if let Some(value) = object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

fn immediate_start_model_value(value: &serde_json::Value) -> Option<String> {
    value
        .as_object()?
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn find_named_array_len(payload: &serde_json::Value, keys: &[&str]) -> Option<usize> {
    named_array_value(payload, keys).map(Vec::len)
}

fn named_array_value<'a>(
    payload: &'a serde_json::Value,
    keys: &[&str],
) -> Option<&'a Vec<serde_json::Value>> {
    let object = payload.as_object()?;
    for key in keys {
        if let Some(array) = object.get(*key).and_then(serde_json::Value::as_array) {
            return Some(array);
        }
    }

    for value in object.values() {
        if let Some(array) = named_array_value(value, keys) {
            return Some(array);
        }
    }

    None
}

fn to_checkpoint_response(checkpoint: domain::CheckpointRecord) -> CheckpointResponse {
    CheckpointResponse {
        id: checkpoint.id.to_string(),
        flow_run_id: checkpoint.flow_run_id.to_string(),
        node_run_id: checkpoint.node_run_id.map(|value| value.to_string()),
        status: checkpoint.status,
        reason: checkpoint.reason,
        locator_payload: checkpoint.locator_payload,
        variable_snapshot: checkpoint.variable_snapshot,
        external_ref_payload: checkpoint.external_ref_payload,
        created_at: format_time(checkpoint.created_at),
    }
}

fn to_callback_task_response(task: domain::CallbackTaskRecord) -> CallbackTaskResponse {
    CallbackTaskResponse {
        id: task.id.to_string(),
        flow_run_id: task.flow_run_id.to_string(),
        node_run_id: task.node_run_id.to_string(),
        callback_kind: task.callback_kind,
        status: task.status.as_str().to_string(),
        request_payload: task.request_payload,
        response_payload: task.response_payload,
        external_ref_payload: task.external_ref_payload,
        created_at: format_time(task.created_at),
        completed_at: format_optional_time(task.completed_at),
    }
}

fn to_run_event_response(event: domain::RunEventRecord) -> RunEventResponse {
    RunEventResponse {
        id: event.id.to_string(),
        flow_run_id: event.flow_run_id.to_string(),
        node_run_id: event.node_run_id.map(|value| value.to_string()),
        sequence: event.sequence,
        event_type: event.event_type,
        payload: event.payload,
        created_at: format_time(event.created_at),
    }
}

fn to_application_run_detail_response(
    application: &domain::ApplicationRecord,
    detail: domain::ApplicationRunDetail,
) -> ApplicationRunDetailResponse {
    let flow_run = to_flow_run_response(detail.flow_run.clone());
    let node_runs = detail
        .node_runs
        .clone()
        .into_iter()
        .map(to_node_run_response)
        .collect::<Vec<_>>();
    let checkpoints = detail
        .checkpoints
        .clone()
        .into_iter()
        .map(to_checkpoint_response)
        .collect::<Vec<_>>();
    let callback_tasks = detail
        .callback_tasks
        .clone()
        .into_iter()
        .map(to_callback_task_response)
        .collect::<Vec<_>>();
    let events = detail
        .events
        .clone()
        .into_iter()
        .map(to_run_event_response)
        .collect::<Vec<_>>();
    let application_type = application.application_type.as_str().to_string();
    let run = application_logs::ApplicationRunLogResponse {
        id: detail.flow_run.id.to_string(),
        application_id: application.id.to_string(),
        application_type: application_type.clone(),
        run_object_kind: application.sections.logs.run_object_kind.clone(),
        run_kind: detail.flow_run.run_mode.as_str().to_string(),
        status: detail.flow_run.status.as_str().to_string(),
        title: detail.flow_run.title.clone(),
        source: application_logs::source_for_run(detail.flow_run.api_key_id),
        protocol: detail.flow_run.compatibility_mode.clone(),
        subject: application_logs::ApplicationRunSubjectResponse {
            kind: application_type,
            id: Some(detail.flow_run.flow_id.to_string()),
            draft_id: Some(detail.flow_run.draft_id.to_string()),
            target_node_id: detail.flow_run.target_node_id.clone(),
        },
        actor: application_logs::actor_from_console_user(
            Some(detail.flow_run.created_by.to_string()),
            detail.flow_run.authorized_account.clone(),
        ),
        correlation: application_logs::ApplicationRunCorrelationResponse {
            api_key_id: detail.flow_run.api_key_id.map(|value| value.to_string()),
            publication_version_id: detail
                .flow_run
                .publication_version_id
                .map(|value| value.to_string()),
            external_user: detail.flow_run.external_user.clone(),
            external_conversation_id: detail.flow_run.external_conversation_id.clone(),
            external_trace_id: detail.flow_run.external_trace_id.clone(),
            compatibility_mode: detail.flow_run.compatibility_mode.clone(),
            idempotency_key: detail.flow_run.idempotency_key.clone(),
        },
        started_at: application_logs::format_time(detail.flow_run.started_at),
        finished_at: application_logs::format_optional_time(detail.flow_run.finished_at),
        created_at: application_logs::format_time(detail.flow_run.created_at),
        updated_at: application_logs::format_time(detail.flow_run.updated_at),
    };
    let typed_detail = application_logs::ApplicationRunTypedDetailResponse {
        kind: application.application_type.as_str().to_string(),
        flow_run: flow_run.clone(),
        node_runs: node_runs.clone(),
        checkpoints: checkpoints.clone(),
        callback_tasks: callback_tasks.clone(),
        events: events.clone(),
    };

    ApplicationRunDetailResponse {
        run,
        detail: typed_detail,
        flow_run,
        node_runs,
        checkpoints,
        callback_tasks,
        events,
    }
}

fn to_node_last_run_response(last_run: domain::NodeLastRun) -> NodeLastRunResponse {
    NodeLastRunResponse {
        flow_run: to_flow_run_response(last_run.flow_run),
        node_run: to_node_run_response(last_run.node_run),
        checkpoints: last_run
            .checkpoints
            .into_iter()
            .map(to_checkpoint_response)
            .collect(),
        events: last_run
            .events
            .into_iter()
            .map(to_run_event_response)
            .collect(),
    }
}

fn to_runtime_debug_stream_part_response(
    part: observability::DebugStreamPart,
) -> RuntimeDebugStreamPartResponse {
    RuntimeDebugStreamPartResponse {
        id: part.id.to_string(),
        flow_run_id: part.flow_run_id.to_string(),
        item_id: part.item_id.map(|value| value.to_string()),
        span_id: part.span_id.map(|value| value.to_string()),
        part_type: part.part_type,
        status: part.status,
        trust_level: part.trust_level.as_str().to_string(),
        payload: part.payload,
    }
}

async fn ensure_application_visible(
    state: &Arc<ApiState>,
    actor_user_id: Uuid,
    application_id: Uuid,
) -> Result<domain::ApplicationRecord, ApiError> {
    Ok(ApplicationService::new(state.store.clone())
        .get_application(actor_user_id, application_id)
        .await?)
}

fn parse_runtime_event_cursor(run_id: Uuid, event_id: &str) -> Option<i64> {
    if let Ok(sequence) = event_id.parse::<i64>() {
        return Some(sequence);
    }

    let (cursor_run_id, sequence) = event_id.rsplit_once(':')?;
    if cursor_run_id != run_id.to_string() {
        return None;
    }

    sequence.parse::<i64>().ok()
}

fn debug_run_stream_from_sequence(
    run_id: Uuid,
    query: &DebugRunStreamQuery,
    headers: &HeaderMap,
) -> Option<i64> {
    query.from_sequence.or_else(|| {
        query
            .last_event_id
            .as_deref()
            .and_then(|event_id| parse_runtime_event_cursor(run_id, event_id))
            .or_else(|| {
                headers
                    .get("last-event-id")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|event_id| parse_runtime_event_cursor(run_id, event_id))
            })
    })
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/debug-runs",
    request_body = StartFlowDebugRunBody,
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 201, body = ApplicationRunDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn start_flow_debug_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<StartFlowDebugRunBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ApplicationRunDetailResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;

    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    );
    let detail = runtime_service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: context.user.id,
            application_id: id,
            input_payload: body.input_payload,
            document_snapshot: body.document,
            debug_session_id: body.debug_session_id,
        })
        .await?;
    let flow_run_id = detail.flow_run.id;
    let workspace_id = context.actor.current_workspace_id;
    let background_state = state.clone();

    tokio::spawn(async move {
        let background_service = OrchestrationRuntimeService::new(
            background_state.store.clone(),
            ApiProviderRuntime::new(background_state.provider_runtime.clone()),
            background_state.runtime_engine.clone(),
            background_state.provider_secret_master_key.clone(),
        );
        let continue_result = background_service
            .continue_flow_debug_run(ContinueFlowDebugRunCommand {
                application_id: id,
                flow_run_id,
                workspace_id,
            })
            .await;
        match continue_result {
            Ok(detail) => {
                if let Err(error) = offload_application_run_detail_artifacts(
                    background_state.clone(),
                    workspace_id,
                    id,
                    detail,
                )
                .await
                {
                    error!(
                        application_id = %id,
                        flow_run_id = %flow_run_id,
                        error = %error.0,
                        "failed to offload flow debug artifacts"
                    );
                }
            }
            Err(error) => {
                error!(
                    application_id = %id,
                    flow_run_id = %flow_run_id,
                    error = %error,
                    "failed to continue flow debug run"
                );
            }
        }
    });

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_application_run_detail_response(
            &application,
            detail,
        ))),
    ))
}

pub async fn start_flow_debug_run_stream(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(stream_query): Query<DebugRunStreamQuery>,
    Json(body): Json<StartFlowDebugRunBody>,
) -> Result<Sse<debug_run_stream::DebugRunSseStream>, ApiError> {
    let request_received_at = std::time::Instant::now();
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;

    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    );
    let shell = runtime_service
        .open_flow_debug_run_shell(StartFlowDebugRunCommand {
            actor_user_id: context.user.id,
            application_id: id,
            input_payload: body.input_payload.clone(),
            document_snapshot: body.document.clone(),
            debug_session_id: body.debug_session_id.clone(),
        })
        .await?;
    let run_id = shell.id;
    let workspace_id = context.actor.current_workspace_id;
    let actor_user_id = context.user.id;

    state
        .runtime_event_stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await?;
    let persister_handle = spawn_debug_event_persister(
        state.store.clone(),
        state.runtime_event_stream.clone(),
        run_id,
    );
    state
        .runtime_event_stream
        .append(run_id, debug_stream_events::flow_accepted(run_id))
        .await?;
    state
        .runtime_event_stream
        .append(run_id, debug_stream_events::heartbeat())
        .await?;

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(debug_run_stream::send_runtime_event_stream(
        state.runtime_event_stream.clone(),
        run_id,
        debug_run_stream_from_sequence(run_id, &stream_query, &headers),
        sender,
    ));

    let background_state = state.clone();
    tokio::spawn(async move {
        let background_service = OrchestrationRuntimeService::new(
            background_state.store.clone(),
            ApiProviderRuntime::new(background_state.provider_runtime.clone()),
            background_state.runtime_engine.clone(),
            background_state.provider_secret_master_key.clone(),
        )
        .with_runtime_event_stream(background_state.runtime_event_stream.clone());
        let prepare_result = background_service
            .prepare_flow_debug_run_from_shell(PrepareFlowDebugRunCommand {
                actor_user_id,
                application_id: id,
                flow_run_id: run_id,
                input_payload: body.input_payload,
                document_snapshot: body.document,
                debug_session_id: body.debug_session_id.unwrap_or_default(),
            })
            .await;
        let result = match prepare_result {
            Ok(_) => {
                background_service
                    .continue_flow_debug_run(ContinueFlowDebugRunCommand {
                        application_id: id,
                        flow_run_id: run_id,
                        workspace_id,
                    })
                    .await
            }
            Err(error) => Err(error),
        };

        match result {
            Ok(detail) => {
                if let Err(error) = offload_application_run_detail_artifacts(
                    background_state.clone(),
                    workspace_id,
                    id,
                    detail,
                )
                .await
                {
                    error!(
                        application_id = %id,
                        flow_run_id = %run_id,
                        error = %error.0,
                        "failed to offload streamed flow debug artifacts"
                    );
                }
            }
            Err(error) => {
                fail_runtime_event_stream_if_missing_terminal(
                    background_state.runtime_event_stream.clone(),
                    run_id,
                    &error,
                )
                .await;
                error!(
                    application_id = %id,
                    flow_run_id = %run_id,
                    error = %error,
                    "failed to prepare and continue streamed flow debug run"
                );
            }
        }
        wait_for_debug_event_persister(persister_handle, id, run_id).await;
    });

    tracing::info!(
        application_id = %id,
        flow_run_id = %run_id,
        http_to_sse_open_ms = request_received_at.elapsed().as_millis() as u64,
        "flow debug stream opened"
    );

    Ok(Sse::new(debug_run_stream::DebugRunSseStream::new(receiver))
        .keep_alive(KeepAlive::default()))
}

pub async fn subscribe_flow_debug_run_stream(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Query(stream_query): Query<DebugRunStreamQuery>,
) -> Result<Sse<debug_run_stream::DebugRunSseStream>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let flow_run = state
        .store
        .get_flow_run(id, run_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    if flow_run.created_by != context.user.id {
        return Err(ControlPlaneError::NotFound("flow_run").into());
    }

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(debug_run_stream::send_runtime_event_stream(
        state.runtime_event_stream.clone(),
        run_id,
        debug_run_stream_from_sequence(run_id, &stream_query, &headers),
        sender,
    ));

    Ok(Sse::new(debug_run_stream::DebugRunSseStream::new(receiver))
        .keep_alive(KeepAlive::default()))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/runs/{run_id}/cancel",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn cancel_flow_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<ApplicationRunDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;

    let runtime_service = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_runtime_event_stream(state.runtime_event_stream.clone());

    let detail = runtime_service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: context.user.id,
            application_id: id,
            flow_run_id: run_id,
        })
        .await?;
    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        detail,
    )
    .await?;

    Ok(Json(ApiSuccess::new(to_application_run_detail_response(
        &application,
        detail,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/runs/{run_id}/resume",
    request_body = ResumeFlowRunBody,
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn resume_flow_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ResumeFlowRunBody>,
) -> Result<Json<ApiSuccess<ApplicationRunDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;

    let checkpoint_id = Uuid::parse_str(&body.checkpoint_id)
        .map_err(|_| ControlPlaneError::InvalidInput("checkpoint_id"))?;
    let detail = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .resume_flow_run(ResumeFlowRunCommand {
        actor_user_id: context.user.id,
        application_id: id,
        flow_run_id: run_id,
        checkpoint_id,
        input_payload: body.input_payload,
    })
    .await?;
    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        detail,
    )
    .await?;

    Ok(Json(ApiSuccess::new(to_application_run_detail_response(
        &application,
        detail,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/callback-tasks/{callback_task_id}/complete",
    request_body = CompleteCallbackTaskBody,
    params(
        ("id" = String, Path, description = "Application id"),
        ("callback_task_id" = String, Path, description = "Callback task id")
    ),
    responses(
        (status = 200, body = ApplicationRunDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn complete_callback_task(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, callback_task_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CompleteCallbackTaskBody>,
) -> Result<Json<ApiSuccess<ApplicationRunDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;

    let detail = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .complete_callback_task(CompleteCallbackTaskCommand {
        actor_user_id: context.user.id,
        application_id: id,
        callback_task_id,
        response_payload: body.response_payload,
    })
    .await?;
    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        detail,
    )
    .await?;

    Ok(Json(ApiSuccess::new(to_application_run_detail_response(
        &application,
        detail,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/nodes/{node_id}/debug-runs",
    request_body = StartNodeDebugPreviewBody,
    params(
        ("id" = String, Path, description = "Application id"),
        ("node_id" = String, Path, description = "Node id")
    ),
    responses(
        (status = 201, body = NodeLastRunResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn start_node_debug_preview(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, node_id)): Path<(Uuid, String)>,
    Json(body): Json<StartNodeDebugPreviewBody>,
) -> Result<(StatusCode, Json<ApiSuccess<NodeLastRunResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;

    let outcome = OrchestrationRuntimeService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .start_node_debug_preview(StartNodeDebugPreviewCommand {
        actor_user_id: context.user.id,
        application_id: id,
        node_id,
        input_payload: body.input_payload,
        document_snapshot: body.document,
        debug_session_id: body.debug_session_id,
    })
    .await?;

    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        domain::ApplicationRunDetail {
            flow_run: outcome.flow_run,
            node_runs: vec![outcome.node_run],
            checkpoints: Vec::new(),
            callback_tasks: Vec::new(),
            events: outcome.events,
        },
    )
    .await?;
    let node_run = detail
        .node_runs
        .into_iter()
        .next()
        .ok_or(ControlPlaneError::NotFound("node_run"))?;
    let response = to_node_last_run_response(domain::NodeLastRun {
        flow_run: detail.flow_run,
        node_run,
        checkpoints: Vec::new(),
        events: detail.events,
    });

    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration/debug-artifacts/{artifact_id}",
    params(
        ("id" = String, Path, description = "Application id"),
        ("artifact_id" = String, Path, description = "Runtime debug artifact id")
    ),
    responses(
        (status = 200, body = serde_json::Value),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_runtime_debug_artifact(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, artifact_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    load_runtime_debug_artifact_response(state, context.actor.current_workspace_id, id, artifact_id)
        .await
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs",
    params(
        ("id" = String, Path, description = "Application id"),
        ("page" = Option<i64>, Query, description = "1-based page number"),
        ("page_size" = Option<i64>, Query, description = "Page size"),
        ("time_range_days" = Option<i64>, Query, description = "Optional created-at day window"),
        ("sort_by" = Option<String>, Query, description = "Sort field: created_at, started_at, finished_at or updated_at"),
        ("sort_order" = Option<String>, Query, description = "Sort direction: asc or desc")
    ),
    responses(
        (status = 200, body = FlowRunSummaryPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_runs(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<ApplicationRunsQuery>,
) -> Result<Json<ApiSuccess<FlowRunSummaryPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;

    let runs_page =
        <MainDurableStore as OrchestrationRuntimeRepository>::list_application_runs_page(
            &state.store,
            id,
            control_plane::ports::ListApplicationRunsPageInput {
                page: query.page.unwrap_or(1),
                page_size: query.page_size.unwrap_or(20),
                created_after: application_runs_created_after(&query),
                sort_by: Some(
                    normalize_application_run_sort_by(query.sort_by.as_deref()).to_string(),
                ),
                sort_order: Some(
                    normalize_application_run_sort_order(query.sort_order.as_deref()).to_string(),
                ),
            },
        )
        .await?;

    Ok(Json(ApiSuccess::new(FlowRunSummaryPageResponse {
        items: runs_page
            .items
            .into_iter()
            .map(|summary| to_flow_run_summary_response(&application, summary))
            .collect(),
        total: runs_page.total,
        page: runs_page.page,
        page_size: runs_page.page_size,
    })))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/conversations/{conversation_id}/messages",
    params(
        ("id" = String, Path, description = "Application id"),
        ("conversation_id" = String, Path, description = "External conversation id"),
        ("around_run_id" = Option<String>, Query, description = "Flow run id to center the page around"),
        ("before" = Option<String>, Query, description = "Load runs before this cursor run id"),
        ("after" = Option<String>, Query, description = "Load runs after this cursor run id"),
        ("limit" = Option<i64>, Query, description = "Page size, defaults to 5")
    ),
    responses(
        (status = 200, body = ApplicationConversationMessagesPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_conversation_messages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, conversation_id)): Path<(Uuid, String)>,
    Query(query): Query<ApplicationConversationMessagesQuery>,
) -> Result<Json<ApiSuccess<ApplicationConversationMessagesPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    let page =
        <MainDurableStore as OrchestrationRuntimeRepository>::list_application_conversation_runs_page(
            &state.store,
            id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: conversation_id,
                around_run_id: query.around_run_id,
                before_run_id: parse_optional_uuid_cursor(query.before.as_deref()),
                after_run_id: parse_optional_uuid_cursor(query.after.as_deref()),
                limit: query.limit.unwrap_or(5),
            },
        )
        .await?;
    let current_run_id = query.around_run_id;

    Ok(Json(ApiSuccess::new(
        ApplicationConversationMessagesPageResponse {
            items: page
                .items
                .into_iter()
                .map(|run| to_application_conversation_message_response(run, current_run_id))
                .collect(),
            page: ApplicationConversationMessagesPageInfoResponse {
                has_before: page.has_before,
                has_after: page.has_after,
                before_cursor: page.before_cursor.map(|value| value.to_string()),
                after_cursor: page.after_cursor.map(|value| value.to_string()),
            },
        },
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/conversation/messages",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("before" = Option<String>, Query, description = "Load messages before this cursor"),
        ("after" = Option<String>, Query, description = "Load messages after this cursor"),
        ("limit" = Option<i64>, Query, description = "Page size, defaults to 5")
    ),
    responses(
        (status = 200, body = ApplicationConversationMessagesPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_run_conversation_messages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ApplicationConversationMessagesQuery>,
) -> Result<Json<ApiSuccess<ApplicationConversationMessagesPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    let detail = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
        &state.store,
        id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;

    if let Some(conversation_id) = detail.flow_run.external_conversation_id.clone() {
        let page = <MainDurableStore as OrchestrationRuntimeRepository>::list_application_conversation_runs_page(
            &state.store,
            id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: conversation_id,
                around_run_id: query.around_run_id.or(Some(run_id)),
                before_run_id: parse_optional_uuid_cursor(query.before.as_deref()),
                after_run_id: parse_optional_uuid_cursor(query.after.as_deref()),
                limit: query.limit.unwrap_or(5),
            },
        )
        .await?;

        return Ok(Json(ApiSuccess::new(
            ApplicationConversationMessagesPageResponse {
                items: page
                    .items
                    .into_iter()
                    .map(|run| to_application_conversation_message_response(run, Some(run_id)))
                    .collect(),
                page: ApplicationConversationMessagesPageInfoResponse {
                    has_before: page.has_before,
                    has_after: page.has_after,
                    before_cursor: page.before_cursor.map(|value| value.to_string()),
                    after_cursor: page.after_cursor.map(|value| value.to_string()),
                },
            },
        )));
    }

    Ok(Json(ApiSuccess::new(
        fallback_conversation_messages_from_run(&detail.flow_run, &query),
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunDetailResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_detail(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<ApplicationRunDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let application = ensure_application_visible(&state, context.user.id, id).await?;

    let detail = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
        &state.store,
        id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        detail,
    )
    .await?;

    Ok(Json(ApiSuccess::new(to_application_run_detail_response(
        &application,
        detail,
    ))))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/nodes/{node_id}",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id"),
        ("node_id" = String, Path, description = "Flow node id")
    ),
    responses(
        (status = 200, body = NodeLastRunResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_run_node_last_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id, node_id)): Path<(Uuid, Uuid, String)>,
) -> Result<Json<ApiSuccess<Option<NodeLastRunResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    let detail = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
        &state.store,
        id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    let detail = offload_application_run_detail_artifacts(
        state.clone(),
        context.actor.current_workspace_id,
        id,
        detail,
    )
    .await?;

    let Some(node_run) = detail
        .node_runs
        .into_iter()
        .rev()
        .find(|candidate| candidate.node_id == node_id)
    else {
        return Ok(Json(ApiSuccess::new(None)));
    };

    let node_run_id = node_run.id;
    let checkpoints = detail
        .checkpoints
        .into_iter()
        .filter(|checkpoint| checkpoint.node_run_id == Some(node_run_id))
        .collect();
    let events = detail
        .events
        .into_iter()
        .filter(|event| event.node_run_id == Some(node_run_id))
        .collect();

    Ok(Json(ApiSuccess::new(Some(to_node_last_run_response(
        domain::NodeLastRun {
            flow_run: detail.flow_run,
            node_run,
            checkpoints,
            events,
        },
    )))))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/debug-stream",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = RuntimeDebugStreamResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_runtime_debug_stream(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<RuntimeDebugStreamResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    <MainDurableStore as OrchestrationRuntimeRepository>::get_flow_run(&state.store, id, run_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;

    let parts = <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_events(
        &state.store,
        run_id,
        0,
    )
    .await?
    .iter()
    .filter_map(|event| {
        control_plane::runtime_observability::debug_read_model::fold_event_to_debug_part(
            run_id, event,
        )
    })
    .map(to_runtime_debug_stream_part_response)
    .collect();

    Ok(Json(ApiSuccess::new(RuntimeDebugStreamResponse { parts })))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration/nodes/{node_id}/last-run",
    params(
        ("id" = String, Path, description = "Application id"),
        ("node_id" = String, Path, description = "Node id")
    ),
    responses(
        (status = 200, body = NodeLastRunResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_node_last_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, node_id)): Path<(Uuid, String)>,
) -> Result<Json<ApiSuccess<Option<NodeLastRunResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;

    let last_run = <MainDurableStore as OrchestrationRuntimeRepository>::get_latest_node_run(
        &state.store,
        id,
        &node_id,
    )
    .await?;
    let last_run = if let Some(last_run) = last_run {
        let checkpoints = last_run.checkpoints;
        let detail = offload_application_run_detail_artifacts(
            state.clone(),
            context.actor.current_workspace_id,
            id,
            domain::ApplicationRunDetail {
                flow_run: last_run.flow_run,
                node_runs: vec![last_run.node_run],
                checkpoints: checkpoints.clone(),
                callback_tasks: Vec::new(),
                events: last_run.events,
            },
        )
        .await?;
        let node_run = detail
            .node_runs
            .into_iter()
            .next()
            .ok_or(ControlPlaneError::NotFound("node_run"))?;
        Some(to_node_last_run_response(domain::NodeLastRun {
            flow_run: detail.flow_run,
            node_run,
            checkpoints,
            events: detail.events,
        }))
    } else {
        None
    };

    Ok(Json(ApiSuccess::new(last_run)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn runtime_event_cursor_accepts_numeric_and_run_scoped_event_ids() {
        let run_id = Uuid::now_v7();

        assert_eq!(parse_runtime_event_cursor(run_id, "7"), Some(7));
        assert_eq!(
            parse_runtime_event_cursor(run_id, &format!("{run_id}:8")),
            Some(8)
        );
        assert_eq!(
            parse_runtime_event_cursor(Uuid::now_v7(), &format!("{run_id}:8")),
            None
        );
        assert_eq!(parse_runtime_event_cursor(run_id, "not-a-cursor"), None);
    }

    #[test]
    fn debug_run_stream_cursor_prefers_query_before_last_event_id_header() {
        let run_id = Uuid::now_v7();
        let mut headers = HeaderMap::new();
        headers.insert(
            "last-event-id",
            HeaderValue::from_str(&format!("{run_id}:11")).unwrap(),
        );

        assert_eq!(
            debug_run_stream_from_sequence(
                run_id,
                &DebugRunStreamQuery {
                    from_sequence: Some(9),
                    last_event_id: Some(format!("{run_id}:10")),
                },
                &headers,
            ),
            Some(9)
        );
        assert_eq!(
            debug_run_stream_from_sequence(
                run_id,
                &DebugRunStreamQuery {
                    from_sequence: None,
                    last_event_id: Some(format!("{run_id}:10")),
                },
                &headers,
            ),
            Some(10)
        );
        assert_eq!(
            debug_run_stream_from_sequence(
                run_id,
                &DebugRunStreamQuery {
                    from_sequence: None,
                    last_event_id: None,
                },
                &headers,
            ),
            Some(11)
        );
    }

    #[test]
    fn start_node_response_moves_legacy_output_payload_into_input() {
        let run = domain::NodeRunRecord {
            id: Uuid::now_v7(),
            flow_run_id: Uuid::now_v7(),
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            node_alias: "Start".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: serde_json::json!({}),
            output_payload: serde_json::json!({
                "query": "ping",
                "tools": [
                    {
                        "name": "read_file",
                        "source": "openai_compatible"
                    }
                ]
            }),
            error_payload: None,
            metrics_payload: serde_json::json!({}),
            debug_payload: serde_json::json!({}),
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        };

        let response = to_node_run_response(run);

        assert_eq!(response.input_payload["query"], serde_json::json!("ping"));
        assert_eq!(
            response.input_payload["tools"][0]["name"],
            serde_json::json!("read_file")
        );
        assert_eq!(response.output_payload, serde_json::json!({}));
    }

    #[test]
    fn start_node_response_exposes_input_payload_summary_view() {
        let artifact_ref = Uuid::now_v7().to_string();
        let run = domain::NodeRunRecord {
            id: Uuid::now_v7(),
            flow_run_id: Uuid::now_v7(),
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            node_alias: "Start".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: serde_json::json!({
                "__runtime_debug_artifact": true,
                "artifact_ref": artifact_ref,
                "is_truncated": true,
                "original_size_bytes": 12598,
                "preview_size_bytes": 2048,
                "preview": "{\"query\":\"say",
                "query": "say hello",
                "model": "deepseek-chat",
                "files": [{ "name": "brief.md" }],
                "history": ["..."],
                "tools": ["..."]
            }),
            output_payload: serde_json::json!({ "query": "say hello" }),
            error_payload: None,
            metrics_payload: serde_json::json!({}),
            debug_payload: serde_json::json!({}),
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        };

        let response = to_node_run_response(run);

        assert_eq!(
            response.input_payload["__runtime_debug_artifact"],
            serde_json::json!(true)
        );
        assert_eq!(response.input_payload_view["kind"], "start_input_summary");
        assert_eq!(response.input_payload_view["artifact_ref"], artifact_ref);
        assert_eq!(response.input_payload_view["is_truncated"], true);
        assert_eq!(response.input_payload_view["original_size_bytes"], 12598);
        assert_eq!(response.input_payload_view["preview_size_bytes"], 2048);
        assert_eq!(response.input_payload_view["preview"]["query"], "say hello");
        assert_eq!(
            response.input_payload_view["preview"]["model"],
            "deepseek-chat"
        );
        assert_eq!(
            response.input_payload_view["preview"]["files"],
            serde_json::json!([{ "name": "brief.md" }])
        );
        assert_eq!(
            response.input_payload_view["preview"]["history"],
            serde_json::json!(["..."])
        );
        assert_eq!(
            response.input_payload_view["preview"]["tools"],
            serde_json::json!(["..."])
        );
        assert!(response.input_payload_view.get("query").is_none());
    }

    #[test]
    fn flow_run_response_exposes_query_and_model_short_fields() {
        let run = domain::FlowRunRecord {
            id: Uuid::now_v7(),
            application_id: Uuid::now_v7(),
            flow_id: Uuid::now_v7(),
            draft_id: Uuid::now_v7(),
            compiled_plan_id: None,
            debug_session_id: "debug-session".to_string(),
            flow_schema_version: "1flowbase.flow/v2".to_string(),
            document_hash: "hash".to_string(),
            run_mode: domain::FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "say hello".to_string(),
            status: domain::FlowRunStatus::Succeeded,
            input_payload: serde_json::json!({
                "node-start": {
                    "query": "say hello",
                    "model": "deepseek-chat"
                }
            }),
            output_payload: serde_json::json!({ "answer": "hello" }),
            error_payload: None,
            created_by: Uuid::now_v7(),
            authorized_account: Some("root".to_string()),
            api_key_id: None,
            publication_version_id: None,
            external_user: Some("user-1".to_string()),
            external_conversation_id: Some("conversation-1".to_string()),
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };

        let response = to_flow_run_response(run);

        assert_eq!(response.query.as_deref(), Some("say hello"));
        assert_eq!(response.model.as_deref(), Some("deepseek-chat"));
        assert_eq!(
            response.external_conversation_id.as_deref(),
            Some("conversation-1")
        );
    }

    #[test]
    fn run_conversation_fallback_reads_recent_history_and_current_turn() {
        let run_id = Uuid::now_v7();
        let run = domain::FlowRunRecord {
            id: run_id,
            application_id: Uuid::now_v7(),
            flow_id: Uuid::now_v7(),
            draft_id: Uuid::now_v7(),
            compiled_plan_id: None,
            debug_session_id: "debug-session".to_string(),
            flow_schema_version: "1flowbase.flow/v2".to_string(),
            document_hash: "hash".to_string(),
            run_mode: domain::FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "current question".to_string(),
            status: domain::FlowRunStatus::Succeeded,
            input_payload: serde_json::json!({
                "node-start": {
                    "query": "current question",
                    "model": "deepseek-chat",
                    "history": [
                        { "role": "system", "content": "hidden" },
                        { "role": "user", "content": "old question 1" },
                        { "role": "assistant", "content": "old answer 1" },
                        { "role": "tool", "content": "tool payload" },
                        { "role": "user", "content": "old question 2" },
                        { "role": "assistant", "content": "old answer 2" }
                    ]
                }
            }),
            output_payload: serde_json::json!({ "answer": "current answer" }),
            error_payload: None,
            created_by: Uuid::now_v7(),
            authorized_account: Some("root".to_string()),
            api_key_id: None,
            publication_version_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };

        let page = fallback_conversation_messages_from_run(
            &run,
            &ApplicationConversationMessagesQuery {
                around_run_id: None,
                before: None,
                after: None,
                limit: Some(2),
            },
        );

        assert_eq!(page.items.len(), 2);
        assert!(page.page.has_before);
        assert_eq!(page.items[0].query.as_deref(), Some("old question 2"));
        assert_eq!(page.items[0].answer.as_deref(), Some("old answer 2"));
        assert_eq!(page.items[1].run_id, run_id.to_string());
        assert_eq!(page.items[1].query.as_deref(), Some("current question"));
        assert_eq!(page.items[1].answer.as_deref(), Some("current answer"));
    }
}
