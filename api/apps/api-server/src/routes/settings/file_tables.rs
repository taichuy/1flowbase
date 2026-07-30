use std::sync::Arc;

use access_control::{
    FILE_TABLES_CREATE_OPERATION_ID, FILE_TABLES_DELETE_OPERATION_ID,
    FILE_TABLES_LIST_OPERATION_ID, FILE_TABLES_STORAGE_BIND_OPERATION_ID,
};
use axum::{
    extract::{Path, State},
    http::{header::ACCEPT_LANGUAGE, HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::file_management::{
    project_builtin_file_table_title, BindFileTableStorageCommand, CreateFileTableCommand,
    DeleteFileTableCommand, FileTableService, FileTableWithStorageTitle,
};
use control_plane::i18n_catalog::CatalogResolver;
use control_plane::ports::RuntimeRegistrySync;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_put, ConsoleRouteAssembly,
    },
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

fn request_catalog_locale(
    headers: &HeaderMap,
    preferred_locale: Option<String>,
) -> domain::CatalogLocale {
    let resolved = runtime_profile::resolve_locale(runtime_profile::LocaleResolutionInput {
        query_locale: None,
        explicit_header_locale: headers
            .get("x-1flowbase-locale")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        user_preferred_locale: preferred_locale,
        accept_language: headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        fallback_locale: runtime_profile::FALLBACK_LOCALE,
        supported_locales: runtime_profile::SUPPORTED_LOCALES
            .iter()
            .map(|value| value.to_string())
            .collect(),
    });
    domain::CatalogLocale::new(resolved.resolved_locale)
        .expect("runtime profile must resolve a supported catalog locale")
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
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/files/tables",
            console_get(
                list_file_tables,
                ConsoleOperation(FILE_TABLES_LIST_OPERATION_ID.to_string()),
            )
            .post(
                create_file_table,
                ConsoleOperation(FILE_TABLES_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/files/tables/:id",
            console_delete(
                delete_file_table,
                ConsoleOperation(FILE_TABLES_DELETE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/files/tables/:id/binding",
            console_put(
                bind_file_table_storage,
                ConsoleOperation(FILE_TABLES_STORAGE_BIND_OPERATION_ID.to_string()),
            ),
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
    let mut tables = FileTableService::new(state.store.clone())
        .list_tables(context.user.id)
        .await?;
    let locale = request_catalog_locale(&headers, context.user.preferred_locale);
    let resolver = CatalogResolver::new(state.store.clone(), state.bootstrap_workspace_id);
    for result in &mut tables {
        project_builtin_file_table_title(
            &resolver,
            state.bootstrap_workspace_id,
            &locale,
            &mut result.table,
        )
        .await?;
    }

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
