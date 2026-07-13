use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, put},
    Json, Router,
};
use control_plane::file_management::{
    BindFileTableStorageCommand, CreateFileTableCommand, DeleteFileTableCommand, FileTableService,
    FileTableWithStorageTitle,
};
use control_plane::ports::RuntimeRegistrySync;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    runtime_registry_sync::ApiRuntimeRegistrySync,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFileTableBody {
    pub code: String,
    pub title: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BindFileTableStorageBody {
    pub bound_storage_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileTableResponse {
    pub id: String,
    pub code: String,
    pub title: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub model_definition_id: String,
    pub bound_storage_id: String,
    pub bound_storage_title: Option<String>,
    pub is_builtin: bool,
    pub is_default: bool,
    pub status: String,
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn to_response(result: FileTableWithStorageTitle) -> FileTableResponse {
    let record = result.table;
    FileTableResponse {
        id: record.id.to_string(),
        code: record.code,
        title: record.title,
        scope_kind: match record.scope_kind {
            domain::FileTableScopeKind::System => "system".into(),
            domain::FileTableScopeKind::Workspace => "workspace".into(),
        },
        scope_id: record.scope_id.to_string(),
        model_definition_id: record.model_definition_id.to_string(),
        bound_storage_id: record.bound_storage_id.to_string(),
        bound_storage_title: result.bound_storage_title,
        is_builtin: record.is_builtin,
        is_default: record.is_default,
        status: record.status,
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/settings/files/tables",
            get(list_file_tables).post(create_file_table),
        )
        .route("/settings/files/tables/:id", delete(delete_file_table))
        .route(
            "/settings/files/tables/:id/binding",
            put(bind_file_table_storage),
        )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/files/tables",
    responses((status = 200, body = [FileTableResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_file_tables(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<FileTableResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let tables = FileTableService::new(state.store.clone())
        .list_tables(context.user.id)
        .await?;

    Ok(Json(ApiSuccess::new(
        tables.into_iter().map(to_response).collect(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/files/tables",
    request_body = CreateFileTableBody,
    responses((status = 201, body = FileTableResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn create_file_table(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateFileTableBody>,
) -> Result<(StatusCode, Json<ApiSuccess<FileTableResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let created = FileTableService::new(state.store.clone())
        .create_table(CreateFileTableCommand {
            actor_user_id: context.user.id,
            code: body.code,
            title: body.title,
        })
        .await?;
    ApiRuntimeRegistrySync::new(state.store.clone(), state.runtime_engine.registry().clone())
        .rebuild()
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_response(created))),
    ))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/files/tables/{id}/binding",
    request_body = BindFileTableStorageBody,
    params(("id" = String, Path, description = "File table id")),
    responses((status = 200, body = FileTableResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn bind_file_table_storage(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(file_table_id): Path<String>,
    Json(body): Json<BindFileTableStorageBody>,
) -> Result<Json<ApiSuccess<FileTableResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let updated = FileTableService::new(state.store.clone())
        .bind_storage(BindFileTableStorageCommand {
            actor_user_id: context.user.id,
            file_table_id: parse_uuid(&file_table_id, "file_table_id")?,
            bound_storage_id: parse_uuid(&body.bound_storage_id, "bound_storage_id")?,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_response(updated))))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/files/tables/{id}",
    params(("id" = String, Path, description = "File table id")),
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_file_table(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(file_table_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    FileTableService::new(state.store.clone())
        .delete_table(DeleteFileTableCommand {
            actor_user_id: context.user.id,
            file_table_id: parse_uuid(&file_table_id, "file_table_id")?,
        })
        .await?;
    ApiRuntimeRegistrySync::new(state.store.clone(), state.runtime_engine.registry().clone())
        .rebuild()
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
