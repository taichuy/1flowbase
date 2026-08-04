use std::{collections::HashSet, convert::Infallible, sync::Arc};

use access_control::{
    APPLICATIONS_LOGS_EXPORT_OPERATION_ID, APPLICATIONS_LOGS_IMPORT_OPERATION_ID,
    APPLICATIONS_RUN_OPERATION_ID, APPLICATIONS_UPDATE_OPERATION_ID,
    APPLICATIONS_VIEW_OPERATION_ID,
};
use axum::{
    extract::{Path, Query, RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use control_plane::{
    application::{ApplicationNonCrudConsoleOperation, ApplicationService},
    errors::ControlPlaneError,
    orchestration_runtime::{
        debug_stream_events, project_runtime_event_stream_terminal,
        spawn_runtime_debug_event_persister,
        trace_projection::{
            build_application_run_trace_projection, merge_trace_node_run_detail,
            projection_status_needs_lazy_rebuild, APPLICATION_RUN_TRACE_PROJECTION_VERSION,
        },
        wait_for_runtime_debug_event_persister, CancelFlowRunCommand, CompleteCallbackTaskCommand,
        ContinueFlowDebugRunCommand, OrchestrationRuntimeService, PrepareFlowDebugRunCommand,
        ResumeFlowRunCommand, StartFlowDebugRunCommand, StartNodeDebugPreviewCommand,
    },
    ports::{
        ApplicationRepository, ApplicationRunTraceChildrenCursor,
        ApplicationRunTraceProjectionStatistics, ListApplicationConversationRunsPageInput,
        ListApplicationRunConversationMessageItemsPageInput,
        ListApplicationRunTraceChildrenPageInput, OrchestrationRuntimeRepository,
        RuntimeEventStreamPolicy,
    },
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use storage_durable::MainDurableStore;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::error;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_post, console_put, ConsoleRouteAssembly,
    },
    runtime_activity::{scope_application_activity, ApplicationActivityKind},
};

use super::debug_run_stream;
mod application_log_cache;
mod application_logs;
pub(crate) mod application_monitoring;
pub(crate) mod archive;
pub(crate) mod debug_variable_cache;
pub(crate) mod debug_variable_snapshot;
mod runtime_debug_artifacts;

use archive::{
    complete_run_archive_upload_session, create_run_archive_upload_session,
    export_application_run_archive, export_application_runs_archive, get_run_archive_import_job,
    upload_run_archive_chunk,
};
pub use debug_variable_cache::{
    delete_debug_variable_cache_entries, upsert_debug_variable_cache_entry,
};
pub use debug_variable_snapshot::{get_debug_variable_snapshot, DebugVariableSnapshotResponse};
use runtime_debug_artifacts::{
    application_run_model, application_run_query, count_llm_tool_callback_trace_items,
    enrich_application_run_detail_visible_internal_llm_route_traces,
    enrich_node_last_run_visible_internal_llm_route_traces, load_runtime_debug_artifact_json_value,
    load_runtime_debug_artifact_response, offload_application_run_detail_artifacts,
    offload_trace_node_content_artifacts, offload_trace_node_run_detail_artifacts,
    RuntimeDebugArtifactPreviewRequest,
};

pub(super) const APPLICATION_RUN_LOG_DEFAULT_TIME_RANGE_DAYS: i64 = 7;
pub(super) const RUNTIME_DEBUG_STREAM_DEFAULT_PAGE_SIZE: usize = 500;
pub(super) const RUNTIME_DEBUG_STREAM_MAX_PAGE_SIZE: usize = 1_000;
pub(super) const RUNTIME_DEBUG_ARTIFACT_RESOLVE_MAX_REFS: usize = 50;

fn api_provider_runtime(state: &ApiState) -> ApiProviderRuntime {
    ApiProviderRuntime::new_with_activity(
        state.provider_runtime.clone(),
        state.runtime_activity.clone(),
    )
}

include!("application_runtime/types.rs");

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

#[allow(deprecated)]
pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/applications/:id/orchestration/debug-runs",
            console_post(
                start_flow_debug_run,
                ConsoleOperation(APPLICATIONS_RUN_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/debug-runs/stream",
            console_post(
                start_flow_debug_run_stream,
                ConsoleOperation(APPLICATIONS_RUN_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/runs/:run_id/debug-stream",
            console_get(
                subscribe_flow_debug_run_stream,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/runs/:run_id/debug-snapshot",
            console_get(
                get_flow_debug_run_snapshot,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/runs/:run_id/resume",
            console_post(
                resume_flow_run,
                ConsoleOperation(APPLICATIONS_RUN_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/runs/:run_id/cancel",
            console_post(
                cancel_flow_run,
                ConsoleOperation(APPLICATIONS_RUN_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/callback-tasks/:callback_task_id/complete",
            console_post(
                complete_callback_task,
                ConsoleOperation(APPLICATIONS_RUN_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/nodes/:node_id/debug-runs",
            console_post(
                start_node_debug_preview,
                ConsoleOperation(APPLICATIONS_RUN_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/debug-variable-snapshot",
            console_get(
                get_debug_variable_snapshot,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/debug-variable-cache",
            console_put(
                upsert_debug_variable_cache_entry,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            )
            .delete(
                delete_debug_variable_cache_entries,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/debug-artifacts/resolve",
            console_post(
                resolve_runtime_debug_artifacts,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/debug-artifacts/:artifact_id",
            console_get(
                get_runtime_debug_artifact,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs",
            console_get(
                list_application_runs,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/export",
            console_post(
                export_application_runs_zip,
                ConsoleOperation(APPLICATIONS_LOGS_EXPORT_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/archive",
            console_post(
                export_application_runs_archive,
                ConsoleOperation(APPLICATIONS_LOGS_EXPORT_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/archive/import-sessions",
            console_post(
                create_run_archive_upload_session,
                ConsoleOperation(APPLICATIONS_LOGS_IMPORT_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/archive/import-sessions/:session_id/chunks/:chunk_index",
            console_put(
                upload_run_archive_chunk,
                ConsoleOperation(APPLICATIONS_LOGS_IMPORT_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/archive/import-sessions/:session_id/complete",
            console_post(
                complete_run_archive_upload_session,
                ConsoleOperation(APPLICATIONS_LOGS_IMPORT_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/archive/import-jobs/:job_id",
            console_get(
                get_run_archive_import_job,
                ConsoleOperation(APPLICATIONS_LOGS_IMPORT_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/monitoring/run-metrics",
            console_get(
                application_monitoring::get_application_run_monitoring_report,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/monitoring/runtime-activity",
            console_get(
                application_monitoring::get_application_runtime_activity,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/conversations/:conversation_id/messages",
            console_get(
                list_application_conversation_messages,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/conversation/messages",
            console_get(
                list_application_run_conversation_messages,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/overview",
            console_get(
                get_application_run_overview,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/trace-tree",
            console_get(
                get_application_run_trace_tree,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/export",
            console_get(
                export_application_run_trace_dump,
                ConsoleOperation(APPLICATIONS_LOGS_EXPORT_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/archive",
            console_get(
                export_application_run_archive,
                ConsoleOperation(APPLICATIONS_LOGS_EXPORT_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/trace-tree/nodes",
            console_get(
                get_application_run_trace_node_children,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/trace-tree/nodes/:trace_node_id/content",
            console_get(
                get_application_run_trace_node_content,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/trace-tree/nodes/:trace_node_id/details/:detail_ref_id",
            console_get(
                get_application_run_trace_node_detail,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/trace-tree/nodes/:trace_node_id/tool-callbacks/:tool_call_id/content",
            console_get(
                get_application_run_trace_tool_callback_content,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/resume-timeline",
            console_get(
                get_application_run_resume_timeline,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/nodes/:node_id",
            console_get(
                get_application_run_node_last_run,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/logs/runs/:run_id/debug-stream",
            console_get(
                get_runtime_debug_stream,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/nodes/:node_id/last-run",
            console_get(
                get_node_last_run,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
}

include!("application_runtime/summary_responses.rs");

include!("application_runtime/conversation_helpers.rs");

include!("application_runtime/detail_responses.rs");

include!("application_runtime/debug_handlers.rs");

include!("application_runtime/log_handlers.rs");

include!("application_runtime/export_handlers.rs");

#[cfg(test)]
use archive::{build_archive_from_trace_exports, parse_run_archive_v1};

#[cfg(test)]
mod tests;
