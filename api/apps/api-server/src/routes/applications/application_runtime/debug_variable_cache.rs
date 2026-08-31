use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};

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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.debug-variables.cache.upsert.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::interface_debug_variables::ApplicationRuntimeDebugVariablesInput::Upsert {
            application_id: id,
            body,
        },
    )
    .await?;
    let super::interface_debug_variables::ApplicationRuntimeDebugVariablesOutput::Updated = output
    else {
        unreachable!("debug variable cache upsert binding returned a different output")
    };
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.debug-variables.cache.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::interface_debug_variables::ApplicationRuntimeDebugVariablesInput::Delete {
            application_id: id,
            body,
        },
    )
    .await?;
    let super::interface_debug_variables::ApplicationRuntimeDebugVariablesOutput::Updated = output
    else {
        unreachable!("debug variable cache delete binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(serde_json::json!({ "ok": true }))))
}
