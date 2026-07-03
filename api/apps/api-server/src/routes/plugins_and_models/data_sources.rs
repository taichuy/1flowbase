use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
    Json, Router,
};
use control_plane::data_source::{
    CreateDataSourceInstanceCommand, DataSourceCatalogEntryView, DataSourceInstanceView,
    DataSourceService, MapDataSourceResourceToModelCommand, PreviewDataSourceReadCommand,
    PreviewDataSourceReadResult, RotateDataSourceSecretCommand, UpdateDataSourceDefaultsCommand,
    ValidateDataSourceInstanceCommand, ValidateDataSourceInstanceResult,
};
use serde::{Deserialize, Serialize};
use storage_durable::MainDurableStore;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
};

use super::model_definitions::{to_model_definition_response, ModelDefinitionResponse};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDataSourceInstanceBody {
    pub installation_id: String,
    pub source_code: String,
    pub display_name: String,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    #[schema(value_type = Object)]
    pub secret_json: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PreviewDataSourceReadBody {
    pub resource_key: String,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    #[schema(value_type = Object)]
    pub options_json: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RotateDataSourceSecretBody {
    #[schema(value_type = Object)]
    pub secret_json: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDataSourceDefaultsBody {
    pub default_data_model_status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MapDataSourceResourceToModelBody {
    pub resource_key: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceCatalogEntryResponse {
    pub installation_id: String,
    pub source_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub display_name: String,
    pub protocol: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceCatalogResponse {
    pub entries: Vec<DataSourceCatalogEntryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceCatalogCacheResponse {
    pub refresh_status: String,
    #[schema(value_type = Object)]
    pub catalog_json: serde_json::Value,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceInstanceResponse {
    pub id: String,
    pub source_kind: String,
    pub installation_id: String,
    pub source_code: String,
    pub display_name: String,
    pub status: String,
    pub default_data_model_status: String,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    pub secret_ref: Option<String>,
    pub secret_version: Option<i32>,
    pub catalog_refresh_status: Option<String>,
    pub catalog_last_error_message: Option<String>,
    pub catalog_refreshed_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ValidateDataSourceResponse {
    pub instance: DataSourceInstanceResponse,
    pub catalog: DataSourceCatalogCacheResponse,
    #[schema(value_type = Object)]
    pub output: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourcePreviewOutputResponse {
    #[schema(value_type = [Object])]
    pub rows: Vec<serde_json::Value>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PreviewDataSourceReadResponse {
    pub preview_session_id: String,
    pub expires_at: String,
    pub output: DataSourcePreviewOutputResponse,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/data-sources/catalog", get(list_catalog))
        .route(
            "/data-sources/instances",
            get(list_instances).post(create_instance),
        )
        .route(
            "/data-sources/instances/:instance_id/defaults",
            patch(update_defaults),
        )
        .route(
            "/data-sources/instances/:instance_id/validate",
            post(validate_instance),
        )
        .route(
            "/data-sources/instances/:instance_id/secret/rotate",
            post(rotate_secret),
        )
        .route(
            "/data-sources/instances/:instance_id/preview-read",
            post(preview_read),
        )
        .route(
            "/data-sources/instances/:instance_id/resources/map-to-model",
            post(map_resource_to_model),
        )
}

fn service(state: &ApiState) -> DataSourceService<MainDurableStore, ApiProviderRuntime> {
    DataSourceService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn format_time(value: time::OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap()
}

fn format_optional_time(value: Option<time::OffsetDateTime>) -> Option<String> {
    value.map(format_time)
}

fn to_catalog_entry_response(entry: DataSourceCatalogEntryView) -> DataSourceCatalogEntryResponse {
    DataSourceCatalogEntryResponse {
        installation_id: entry.installation_id.to_string(),
        source_code: entry.source_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        display_name: entry.display_name,
        protocol: entry.protocol,
    }
}

fn to_cache_response(
    cache: domain::DataSourceCatalogCacheRecord,
) -> DataSourceCatalogCacheResponse {
    DataSourceCatalogCacheResponse {
        refresh_status: cache.refresh_status.as_str().to_string(),
        catalog_json: cache.catalog_json,
        last_error_message: cache.last_error_message,
        refreshed_at: format_optional_time(cache.refreshed_at),
    }
}

fn to_instance_response(view: DataSourceInstanceView) -> DataSourceInstanceResponse {
    let catalog = view.catalog;
    DataSourceInstanceResponse {
        id: view.instance.id.to_string(),
        source_kind: domain::DataModelSourceKind::ExternalSource
            .as_str()
            .to_string(),
        installation_id: view.instance.installation_id.to_string(),
        source_code: view.instance.source_code,
        display_name: view.instance.display_name,
        status: view.instance.status.as_str().to_string(),
        default_data_model_status: view
            .instance
            .defaults
            .data_model_status
            .as_str()
            .to_string(),
        config_json: view.instance.config_json,
        secret_ref: view.instance.secret_ref,
        secret_version: view.instance.secret_version,
        catalog_refresh_status: catalog
            .as_ref()
            .map(|cache| cache.refresh_status.as_str().to_string()),
        catalog_last_error_message: catalog
            .as_ref()
            .and_then(|cache| cache.last_error_message.clone()),
        catalog_refreshed_at: catalog.and_then(|cache| format_optional_time(cache.refreshed_at)),
    }
}

fn main_source_response(defaults: domain::DataSourceDefaults) -> DataSourceInstanceResponse {
    DataSourceInstanceResponse {
        id: "main_source".to_string(),
        source_kind: domain::DataModelSourceKind::MainSource.as_str().to_string(),
        installation_id: "main_source".to_string(),
        source_code: "main_source".to_string(),
        display_name: "主数据源".to_string(),
        status: "ready".to_string(),
        default_data_model_status: defaults.data_model_status.as_str().to_string(),
        config_json: serde_json::json!({}),
        secret_ref: None,
        secret_version: None,
        catalog_refresh_status: None,
        catalog_last_error_message: None,
        catalog_refreshed_at: None,
    }
}

fn parse_model_status(raw: &str) -> Result<domain::DataModelStatus, ApiError> {
    match raw {
        "draft" => Ok(domain::DataModelStatus::Draft),
        "published" => Ok(domain::DataModelStatus::Published),
        "disabled" => Ok(domain::DataModelStatus::Disabled),
        "broken" => Ok(domain::DataModelStatus::Broken),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "default_data_model_status",
        )
        .into()),
    }
}

fn to_validate_response(result: ValidateDataSourceInstanceResult) -> ValidateDataSourceResponse {
    ValidateDataSourceResponse {
        instance: to_instance_response(DataSourceInstanceView {
            instance: result.instance,
            catalog: Some(result.catalog.clone()),
        }),
        catalog: to_cache_response(result.catalog),
        output: result.output,
    }
}

fn to_preview_response(result: PreviewDataSourceReadResult) -> PreviewDataSourceReadResponse {
    PreviewDataSourceReadResponse {
        preview_session_id: result.preview_session.id.to_string(),
        expires_at: format_time(result.preview_session.expires_at),
        output: DataSourcePreviewOutputResponse {
            rows: result.output.rows,
            next_cursor: result.output.next_cursor,
        },
    }
}

#[utoipa::path(
    get,
    path = "/api/console/data-sources/catalog",
    operation_id = "data_source_list_catalog",
    responses((status = 200, body = DataSourceCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<DataSourceCatalogResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let entries = service(&state)
        .list_catalog(context.user.id, context.actor.current_workspace_id)
        .await?;
    Ok(Json(ApiSuccess::new(DataSourceCatalogResponse {
        entries: entries.into_iter().map(to_catalog_entry_response).collect(),
    })))
}

#[utoipa::path(
    get,
    path = "/api/console/data-sources/instances",
    operation_id = "data_source_list_instances",
    responses((status = 200, body = [DataSourceInstanceResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_instances(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<DataSourceInstanceResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let main_source_defaults = service(&state)
        .get_main_source_defaults(context.user.id, context.actor.current_workspace_id)
        .await?;
    let mut sources = vec![main_source_response(main_source_defaults)];
    sources.extend(
        service(&state)
            .list_instances(context.user.id, context.actor.current_workspace_id)
            .await?
            .into_iter()
            .map(to_instance_response),
    );
    Ok(Json(ApiSuccess::new(sources)))
}

#[utoipa::path(
    post,
    path = "/api/console/data-sources/instances",
    operation_id = "data_source_create_instance",
    request_body = CreateDataSourceInstanceBody,
    responses((status = 201, body = DataSourceInstanceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateDataSourceInstanceBody>,
) -> Result<(StatusCode, Json<ApiSuccess<DataSourceInstanceResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let created = service(&state)
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            installation_id: parse_uuid(&body.installation_id, "installation_id")?,
            source_code: body.source_code,
            display_name: body.display_name,
            config_json: body.config_json,
            secret_json: body.secret_json,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_instance_response(created))),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/console/data-sources/instances/{instance_id}/defaults",
    operation_id = "data_source_update_defaults",
    request_body = UpdateDataSourceDefaultsBody,
    responses((status = 200, body = DataSourceInstanceResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_defaults(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateDataSourceDefaultsBody>,
) -> Result<Json<ApiSuccess<DataSourceInstanceResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let defaults = domain::DataSourceDefaults {
        data_model_status: parse_model_status(&body.default_data_model_status)?,
    };
    if instance_id == "main_source" {
        let defaults = service(&state)
            .update_main_source_defaults(UpdateDataSourceDefaultsCommand {
                actor_user_id: context.user.id,
                workspace_id: context.actor.current_workspace_id,
                instance_id: Uuid::nil(),
                defaults,
            })
            .await?;
        return Ok(Json(ApiSuccess::new(main_source_response(defaults))));
    }

    let instance = service(&state)
        .update_defaults(UpdateDataSourceDefaultsCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            instance_id: parse_uuid(&instance_id, "instance_id")?,
            defaults,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_instance_response(
        DataSourceInstanceView {
            instance,
            catalog: None,
        },
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/data-sources/instances/{instance_id}/validate",
    operation_id = "data_source_validate_instance",
    responses((status = 200, body = ValidateDataSourceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn validate_instance(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ValidateDataSourceResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let result = service(&state)
        .validate_instance(ValidateDataSourceInstanceCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            instance_id: parse_uuid(&instance_id, "instance_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_validate_response(result))))
}

#[utoipa::path(
    post,
    path = "/api/console/data-sources/instances/{instance_id}/secret/rotate",
    operation_id = "data_source_rotate_secret",
    request_body = RotateDataSourceSecretBody,
    responses((status = 200, body = DataSourceInstanceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn rotate_secret(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<RotateDataSourceSecretBody>,
) -> Result<Json<ApiSuccess<DataSourceInstanceResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let result = service(&state)
        .rotate_secret(RotateDataSourceSecretCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            instance_id: parse_uuid(&instance_id, "instance_id")?,
            secret_json: body.secret_json,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_instance_response(result))))
}

#[utoipa::path(
    post,
    path = "/api/console/data-sources/instances/{instance_id}/preview-read",
    operation_id = "data_source_preview_read",
    request_body = PreviewDataSourceReadBody,
    responses((status = 200, body = PreviewDataSourceReadResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn preview_read(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<PreviewDataSourceReadBody>,
) -> Result<Json<ApiSuccess<PreviewDataSourceReadResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let result = service(&state)
        .preview_read(PreviewDataSourceReadCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            instance_id: parse_uuid(&instance_id, "instance_id")?,
            resource_key: body.resource_key,
            limit: body.limit,
            cursor: body.cursor,
            options_json: body.options_json,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_preview_response(result))))
}

#[utoipa::path(
    post,
    path = "/api/console/data-sources/instances/{instance_id}/resources/map-to-model",
    operation_id = "data_source_map_resource_to_model",
    request_body = MapDataSourceResourceToModelBody,
    responses((status = 201, body = ModelDefinitionResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn map_resource_to_model(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<MapDataSourceResourceToModelBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ModelDefinitionResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let result = service(&state)
        .map_resource_to_model(MapDataSourceResourceToModelCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            instance_id: parse_uuid(&instance_id, "instance_id")?,
            resource_key: body.resource_key,
        })
        .await?;
    let mut model = result.model;
    model.fields = result.fields;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_model_definition_response(model))),
    ))
}
