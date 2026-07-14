use std::sync::Arc;

use access_control::{
    ConsoleRouteOwnership::ConsoleOperation, SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use control_plane::errors::ControlPlaneError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::require_session,
    openapi_docs::{
        DOCS_OPERATIONS_PAGE_SIZE, DocsCatalog, DocsCatalogCategoryOperationsPage,
        filter_category_operations, paginate_category_operations,
    },
    response::ApiSuccess,
    routes::console_route_assembly::{ConsoleRouteAssembly, console_get},
    runtime_data_model_docs,
};

#[derive(Debug, Deserialize, IntoParams)]
pub struct DocsCategoryOperationsQuery {
    #[param(minimum = 0)]
    pub offset: Option<usize>,
    #[param(minimum = 1, maximum = 20)]
    pub limit: Option<usize>,
    pub q: Option<String>,
}

impl DocsCategoryOperationsQuery {
    fn offset(&self) -> usize {
        self.offset.unwrap_or(0)
    }

    fn limit(&self) -> usize {
        self.limit
            .unwrap_or(DOCS_OPERATIONS_PAGE_SIZE)
            .clamp(1, DOCS_OPERATIONS_PAGE_SIZE)
    }

    fn search_query(&self) -> Option<&str> {
        self.q.as_deref()
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataModelOpenApiDocumentResponse {
    pub openapi: String,
    #[schema(value_type = Object)]
    pub info: Value,
    #[schema(value_type = Object)]
    pub paths: Value,
    #[schema(value_type = Object)]
    pub components: Value,
    #[serde(rename = "x-data-model")]
    #[schema(value_type = Object)]
    pub data_model: Value,
    #[serde(rename = "x-scope-permission-note")]
    pub scope_permission_note: String,
    #[serde(rename = "x-external-source-safety-limits")]
    pub external_source_safety_limits: String,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/docs/catalog",
            console_get(
                get_docs_catalog,
                ConsoleOperation(SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION.to_string()),
            ),
        )
        .route(
            "/docs/categories/:category_id/operations",
            console_get(
                get_category_operations,
                ConsoleOperation(SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION.to_string()),
            ),
        )
        .route(
            "/docs/categories/:category_id/openapi.json",
            console_get(
                get_category_openapi,
                ConsoleOperation(SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION.to_string()),
            ),
        )
        .route(
            "/docs/operations/:operation_id/openapi.json",
            console_get(
                get_operation_openapi,
                ConsoleOperation(SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION.to_string()),
            ),
        )
}

fn parse_data_model_docs_operation_id(
    operation_id: &str,
) -> Result<
    Option<(
        uuid::Uuid,
        runtime_data_model_docs::RuntimeDataModelDocsOperationKind,
    )>,
    ApiError,
> {
    runtime_data_model_docs::parse_operation_id(operation_id)
        .map_err(|_| ControlPlaneError::InvalidInput("operation_id").into())
}

pub async fn get_docs_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<DocsCatalog>>, ApiError> {
    let context = require_session(&state, &headers).await?;

    let mut catalog = state.api_docs.catalog().clone();
    let models = runtime_data_model_docs::ready_models(&state, context.user.id).await?;
    if let Some(category) = runtime_data_model_docs::build_category(&models) {
        catalog.categories.push(category);
    }

    Ok(Json(ApiSuccess::new(catalog)))
}

pub async fn get_category_operations(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<DocsCategoryOperationsQuery>,
    Path(category_id): Path<String>,
) -> Result<Json<ApiSuccess<DocsCatalogCategoryOperationsPage>>, ApiError> {
    let context = require_session(&state, &headers).await?;

    if category_id == runtime_data_model_docs::DATA_MODEL_DOCS_CATEGORY_ID {
        let models = runtime_data_model_docs::ready_models(&state, context.user.id).await?;
        if models.is_empty() {
            return Err(ControlPlaneError::NotFound("category_id").into());
        }
        let operations = runtime_data_model_docs::build_category_operations(&models);
        let filtered_operations = filter_category_operations(&operations, query.search_query());
        return Ok(Json(ApiSuccess::new(paginate_category_operations(
            &filtered_operations,
            query.offset(),
            query.limit(),
        ))));
    }

    let operations = state
        .api_docs
        .category_operations(&category_id)
        .ok_or(ControlPlaneError::NotFound("category_id"))?;
    let filtered_operations = filter_category_operations(operations, query.search_query());

    Ok(Json(ApiSuccess::new(paginate_category_operations(
        &filtered_operations,
        query.offset(),
        query.limit(),
    ))))
}

pub async fn get_category_openapi(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let context = require_session(&state, &headers).await?;

    if category_id == runtime_data_model_docs::DATA_MODEL_DOCS_CATEGORY_ID {
        let models = runtime_data_model_docs::ready_models(&state, context.user.id).await?;
        if models.is_empty() {
            return Err(ControlPlaneError::NotFound("category_id").into());
        }
        return Ok(Json(runtime_data_model_docs::build_category_openapi(
            &models,
        )));
    }

    let spec = state
        .api_docs
        .category_spec(&category_id)
        .cloned()
        .ok_or(ControlPlaneError::NotFound("category_id"))?;

    Ok(Json(spec))
}

pub async fn get_operation_openapi(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(operation_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let context = require_session(&state, &headers).await?;

    if let Some((model_id, kind)) = parse_data_model_docs_operation_id(&operation_id)? {
        let Some(model) =
            runtime_data_model_docs::ready_model(&state, context.user.id, model_id).await?
        else {
            return Err(ControlPlaneError::NotFound("operation_id").into());
        };
        return Ok(Json(runtime_data_model_docs::build_operation_openapi(
            &model, kind,
        )));
    }

    let spec = state
        .api_docs
        .operation_spec(&operation_id)
        .cloned()
        .ok_or(ControlPlaneError::NotFound("operation_id"))?;

    Ok(Json(spec))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/data-models/model-definitions/{model_id}/openapi.json",
    params(("model_id" = String, Path, description = "Data Model id")),
    responses((status = 200, body = DataModelOpenApiDocumentResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn get_data_model_openapi(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let context = require_session(&state, &headers).await?;

    let model_id =
        Uuid::parse_str(&model_id).map_err(|_| ControlPlaneError::InvalidInput("model_id"))?;
    let Some(model) =
        runtime_data_model_docs::ready_model(&state, context.user.id, model_id).await?
    else {
        return Err(ControlPlaneError::NotFound("model_id").into());
    };

    Ok(Json(runtime_data_model_docs::build_model_openapi(&model)))
}
