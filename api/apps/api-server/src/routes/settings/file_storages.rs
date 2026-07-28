use std::sync::Arc;

use access_control::{
    FILE_STORAGES_CREATE_OPERATION_ID, FILE_STORAGES_DELETE_OPERATION_ID,
    FILE_STORAGES_LIST_OPERATION_ID, FILE_STORAGES_UPDATE_OPERATION_ID,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::file_management::{
    CreateFileStorageCommand, DeleteFileStorageCommand, FileStorageService,
    UpdateFileStorageCommand,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_put, ConsoleRouteAssembly},
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFileStorageBody {
    pub code: String,
    pub title: String,
    pub driver_type: String,
    pub enabled: bool,
    pub is_default: bool,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    #[schema(value_type = Object)]
    pub rule_json: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateFileStorageBody {
    pub title: String,
    pub enabled: bool,
    pub is_default: bool,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    #[schema(value_type = Object)]
    pub rule_json: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileStorageResponse {
    pub id: String,
    pub code: String,
    pub title: String,
    pub driver_type: String,
    pub enabled: bool,
    pub is_default: bool,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    #[schema(value_type = Object)]
    pub rule_json: serde_json::Value,
    pub health_status: String,
    pub last_health_error: Option<String>,
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn to_response(record: domain::FileStorageRecord) -> FileStorageResponse {
    FileStorageResponse {
        id: record.id.to_string(),
        code: record.code,
        title: record.title,
        driver_type: record.driver_type,
        enabled: record.enabled,
        is_default: record.is_default,
        config_json: record.config_json,
        rule_json: record.rule_json,
        health_status: match record.health_status {
            domain::FileStorageHealthStatus::Unknown => "unknown".into(),
            domain::FileStorageHealthStatus::Ready => "ready".into(),
            domain::FileStorageHealthStatus::Failed => "failed".into(),
        },
        last_health_error: record.last_health_error,
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/files/storages",
            console_get(
                list_file_storages,
                ConsoleOperation(FILE_STORAGES_LIST_OPERATION_ID.to_string()),
            )
            .post(
                create_file_storage,
                ConsoleOperation(FILE_STORAGES_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/files/storages/:id",
            console_put(
                update_file_storage,
                ConsoleOperation(FILE_STORAGES_UPDATE_OPERATION_ID.to_string()),
            )
            .delete(
                delete_file_storage,
                ConsoleOperation(FILE_STORAGES_DELETE_OPERATION_ID.to_string()),
            ),
        )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/files/storages",
    responses((status = 200, body = [FileStorageResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_file_storages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<FileStorageResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let mut storages = FileStorageService::new(state.store.clone())
        .list_storages(context.user.id)
        .await?;
    let locale = crate::app_state::request_catalog_locale(&headers, context.user.preferred_locale);
    for storage in &mut storages {
        if storage.code == "local_default" && storage.is_default {
            storage.title = crate::app_state::project_canonical_display(
                &state,
                &locale,
                "@taichuy/platform/authentication",
                "Local",
                &storage.title,
            )
            .await?;
        }
    }

    Ok(Json(ApiSuccess::new(
        storages.into_iter().map(to_response).collect(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/files/storages",
    request_body = CreateFileStorageBody,
    responses((status = 201, body = FileStorageResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_file_storage(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateFileStorageBody>,
) -> Result<(StatusCode, Json<ApiSuccess<FileStorageResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let created = FileStorageService::new(state.store.clone())
        .create_storage(CreateFileStorageCommand {
            actor_user_id: context.user.id,
            code: body.code,
            title: body.title,
            driver_type: body.driver_type,
            enabled: body.enabled,
            is_default: body.is_default,
            config_json: body.config_json,
            rule_json: body.rule_json,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_response(created))),
    ))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/files/storages/{id}",
    request_body = UpdateFileStorageBody,
    params(("id" = String, Path, description = "File storage id")),
    responses((status = 200, body = FileStorageResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_file_storage(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(file_storage_id): Path<String>,
    Json(body): Json<UpdateFileStorageBody>,
) -> Result<Json<ApiSuccess<FileStorageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let updated = FileStorageService::new(state.store.clone())
        .update_storage(UpdateFileStorageCommand {
            actor_user_id: context.user.id,
            file_storage_id: parse_uuid(&file_storage_id, "file_storage_id")?,
            title: body.title,
            enabled: body.enabled,
            is_default: body.is_default,
            config_json: body.config_json,
            rule_json: body.rule_json,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_response(updated))))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/files/storages/{id}",
    params(("id" = String, Path, description = "File storage id")),
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_file_storage(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(file_storage_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    FileStorageService::new(state.store.clone())
        .delete_storage(DeleteFileStorageCommand {
            actor_user_id: context.user.id,
            file_storage_id: parse_uuid(&file_storage_id, "file_storage_id")?,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
