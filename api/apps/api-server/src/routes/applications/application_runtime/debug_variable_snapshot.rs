use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use control_plane::flow::FlowService;
use control_plane::ports::{DebugVariableCacheEntry, OrchestrationRuntimeRepository};
use serde::Serialize;
use sha2::{Digest, Sha256};
use storage_durable::MainDurableStore;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
    response::ApiSuccess,
};

use super::ensure_application_visible;

const DEBUG_VARIABLE_SNAPSHOT_SCHEMA_VERSION: &str = "1flowbase.debug-variable-snapshot/v1";

#[derive(Debug, Serialize, ToSchema)]
pub struct DebugVariableSnapshotResponse {
    pub snapshot_schema_version: String,
    pub workspace_id: String,
    pub actor_user_id: String,
    pub draft_id: String,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub debug_session_id: String,
    pub latest_run_scope: Option<DebugVariableSnapshotRunScopeResponse>,
    pub snapshot_completeness: String,
    pub source_flow_run_ids: serde_json::Value,
    pub source_node_run_ids: serde_json::Value,
    pub variable_cache: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DebugVariableSnapshotRunScopeResponse {
    pub flow_run_id: String,
    pub run_mode: String,
    pub status: String,
    pub target_node_id: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration/debug-variable-snapshot",
    params(("id" = String, Path, description = "Application id")),
    responses(
        (status = 200, body = DebugVariableSnapshotResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_debug_variable_snapshot(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<DebugVariableSnapshotResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let editor_state = FlowService::new(state.store.clone())
        .get_or_create_editor_state(context.user.id, id)
        .await?;

    let snapshot = build_debug_variable_snapshot(
        &state.store,
        id,
        context.actor.current_workspace_id,
        context.actor.user_id,
        &editor_state,
    )
    .await?;

    Ok(Json(ApiSuccess::new(snapshot)))
}

fn insert_variable_value(
    variable_cache: &mut serde_json::Map<String, serde_json::Value>,
    node_id: &str,
    key: &str,
    value: &serde_json::Value,
) {
    let node_entry = variable_cache
        .entry(node_id.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(node_entry) = node_entry.as_object_mut() else {
        return;
    };
    node_entry.insert(key.to_string(), value.clone());
}

fn merge_debug_variable_cache_entries(
    variable_cache: &mut serde_json::Map<String, serde_json::Value>,
    entries: Vec<DebugVariableCacheEntry>,
) {
    for entry in entries {
        insert_variable_value(
            variable_cache,
            &entry.node_id,
            &entry.variable_key,
            &entry.value,
        );
    }
}

async fn load_debug_variable_cache_entries(
    store: &MainDurableStore,
    application_id: Uuid,
    draft_id: Uuid,
    actor_user_id: Uuid,
) -> Result<Vec<DebugVariableCacheEntry>, ApiError> {
    Ok(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_debug_variable_cache_entries(
            store,
            application_id,
            draft_id,
            actor_user_id,
        )
        .await?,
    )
}

fn debug_snapshot_document_hash(document: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(document).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn to_debug_snapshot_run_scope(
    run: &domain::FlowRunRecord,
) -> DebugVariableSnapshotRunScopeResponse {
    DebugVariableSnapshotRunScopeResponse {
        flow_run_id: run.id.to_string(),
        run_mode: run.run_mode.as_str().to_string(),
        status: run.status.as_str().to_string(),
        target_node_id: run.target_node_id.clone(),
    }
}

fn debug_snapshot_completeness(status: domain::FlowRunStatus) -> &'static str {
    match status {
        domain::FlowRunStatus::Succeeded => "complete",
        domain::FlowRunStatus::Queued
        | domain::FlowRunStatus::Running
        | domain::FlowRunStatus::WaitingCallback
        | domain::FlowRunStatus::WaitingHuman
        | domain::FlowRunStatus::Paused
        | domain::FlowRunStatus::Failed
        | domain::FlowRunStatus::Cancelled => "partial",
    }
}

fn debug_snapshot_flow_schema_version(editor_state: &domain::FlowEditorState) -> String {
    editor_state
        .draft
        .document
        .get("schemaVersion")
        .and_then(|value| value.as_str())
        .unwrap_or(&editor_state.draft.schema_version)
        .to_string()
}

fn run_matches_snapshot_key(
    run: &domain::FlowRunRecord,
    actor_user_id: Uuid,
    editor_state: &domain::FlowEditorState,
) -> bool {
    run.created_by == actor_user_id && run.draft_id == editor_state.draft.id
}

pub(super) async fn build_debug_variable_snapshot(
    store: &MainDurableStore,
    application_id: Uuid,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    editor_state: &domain::FlowEditorState,
) -> Result<DebugVariableSnapshotResponse, ApiError> {
    let document_hash = debug_snapshot_document_hash(&editor_state.draft.document);
    let flow_schema_version = debug_snapshot_flow_schema_version(editor_state);
    let runs = <MainDurableStore as OrchestrationRuntimeRepository>::list_application_runs(
        store,
        application_id,
    )
    .await?;
    let mut variable_cache = serde_json::Map::new();
    let source_flow_run_ids = serde_json::Map::new();
    let source_node_run_ids = serde_json::Map::new();
    let mut latest_run_scope = None;
    let mut snapshot_completeness = "empty";
    let mut snapshot_draft_id = editor_state.draft.id.to_string();
    let mut snapshot_flow_schema_version = flow_schema_version;
    let mut snapshot_document_hash = document_hash;

    for run in runs {
        let Some(detail) =
            <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
                store,
                application_id,
                run.id,
            )
            .await?
        else {
            continue;
        };

        if !run_matches_snapshot_key(&detail.flow_run, actor_user_id, editor_state) {
            continue;
        }

        latest_run_scope = Some(to_debug_snapshot_run_scope(&detail.flow_run));
        snapshot_completeness = debug_snapshot_completeness(detail.flow_run.status);
        snapshot_draft_id = detail.flow_run.draft_id.to_string();
        snapshot_flow_schema_version = detail.flow_run.flow_schema_version.clone();
        snapshot_document_hash = detail.flow_run.document_hash.clone();
        break;
    }
    merge_debug_variable_cache_entries(
        &mut variable_cache,
        load_debug_variable_cache_entries(
            store,
            application_id,
            editor_state.draft.id,
            actor_user_id,
        )
        .await?,
    );

    Ok(DebugVariableSnapshotResponse {
        snapshot_schema_version: DEBUG_VARIABLE_SNAPSHOT_SCHEMA_VERSION.to_string(),
        workspace_id: workspace_id.to_string(),
        actor_user_id: actor_user_id.to_string(),
        draft_id: snapshot_draft_id,
        flow_schema_version: snapshot_flow_schema_version,
        document_hash: snapshot_document_hash,
        debug_session_id: String::new(),
        latest_run_scope,
        snapshot_completeness: snapshot_completeness.to_string(),
        source_flow_run_ids: serde_json::Value::Object(source_flow_run_ids),
        source_node_run_ids: serde_json::Value::Object(source_node_run_ids),
        variable_cache: serde_json::Value::Object(variable_cache),
    })
}
