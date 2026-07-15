use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    flow::FlowService,
    ports::{
        DebugVariableCacheKey, DeleteDebugVariableCacheEntriesInput,
        OrchestrationRuntimeRepository, UpsertDebugVariableCacheEntryInput,
    },
};
use serde::Deserialize;
use storage_durable::MainDurableStore;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
};

use super::ensure_application_visible;

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertDebugVariableCacheEntryBody {
    pub node_id: String,
    pub variable_key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DebugVariableCacheKeyBody {
    pub node_id: String,
    pub variable_key: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteDebugVariableCacheEntriesBody {
    pub keys: Option<Vec<DebugVariableCacheKeyBody>>,
}

#[utoipa::path(
    put,
    path = "/api/console/applications/{id}/orchestration/debug-variable-cache",
    params(("id" = String, Path, description = "Application id")),
    request_body = UpsertDebugVariableCacheEntryBody,
    responses(
        (status = 200, body = serde_json::Value),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn upsert_debug_variable_cache_entry(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UpsertDebugVariableCacheEntryBody>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let editor_state = FlowService::new(state.store.clone())
        .get_or_create_editor_state(context.user.id, id)
        .await?;
    let node_id = body.node_id.trim().to_string();
    let variable_key = body.variable_key.trim().to_string();
    if node_id.is_empty() || variable_key.is_empty() {
        return Err(ControlPlaneError::InvalidInput("debug_variable_cache_key").into());
    }

    <MainDurableStore as OrchestrationRuntimeRepository>::upsert_debug_variable_cache_entry(
        &state.store,
        &UpsertDebugVariableCacheEntryInput {
            workspace_id: context.actor.current_workspace_id,
            application_id: id,
            draft_id: editor_state.draft.id,
            actor_user_id: context.actor.user_id,
            node_id,
            variable_key,
            value: body.value,
        },
    )
    .await?;

    Ok(Json(ApiSuccess::new(serde_json::json!({ "ok": true }))))
}

#[utoipa::path(
    delete,
    path = "/api/console/applications/{id}/orchestration/debug-variable-cache",
    params(("id" = String, Path, description = "Application id")),
    request_body = DeleteDebugVariableCacheEntriesBody,
    responses(
        (status = 200, body = serde_json::Value),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn delete_debug_variable_cache_entries(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<DeleteDebugVariableCacheEntriesBody>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    ensure_application_visible(&state, context.user.id, id).await?;
    let editor_state = FlowService::new(state.store.clone())
        .get_or_create_editor_state(context.user.id, id)
        .await?;
    let keys = body.keys.map(|keys| {
        keys.into_iter()
            .filter_map(|key| {
                let node_id = key.node_id.trim().to_string();
                let variable_key = key.variable_key.trim().to_string();
                if node_id.is_empty() || variable_key.is_empty() {
                    return None;
                }
                Some(DebugVariableCacheKey {
                    node_id,
                    variable_key,
                })
            })
            .collect::<Vec<_>>()
    });

    <MainDurableStore as OrchestrationRuntimeRepository>::delete_debug_variable_cache_entries(
        &state.store,
        &DeleteDebugVariableCacheEntriesInput {
            application_id: id,
            draft_id: editor_state.draft.id,
            actor_user_id: context.actor.user_id,
            keys,
        },
    )
    .await?;

    Ok(Json(ApiSuccess::new(serde_json::json!({ "ok": true }))))
}
