use std::sync::Arc;

use access_control::{
    ConsoleRouteOwnership::ConsoleOperation, SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION,
};
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    openapi_docs::{DocsCatalog, DocsCatalogCategoryOperationsPage, DOCS_OPERATIONS_PAGE_SIZE},
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
};

#[path = "data_model_openapi_interface.rs"]
pub(crate) mod data_model_openapi_interface;
#[path = "docs_interface.rs"]
pub(crate) mod interface;

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

async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: interface::DocsInput,
) -> Result<interface::DocsOutput, ApiError> {
    crate::routes::console_interface::invoke(
        Arc::clone(&state),
        binding_id,
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        input,
    )
    .await
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

pub async fn get_docs_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<DocsCatalog>>, ApiError> {
    let interface::DocsOutput::Catalog(catalog) = invoke(
        state,
        headers,
        "http.console.docs.catalog.get.v1",
        interface::DocsInput::Catalog,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(catalog)))
}

pub async fn get_category_operations(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<DocsCategoryOperationsQuery>,
    Path(category_id): Path<String>,
) -> Result<Json<ApiSuccess<DocsCatalogCategoryOperationsPage>>, ApiError> {
    let interface::DocsOutput::CategoryOperations(page) = invoke(
        state,
        headers,
        "http.console.docs.category-operations.get.v1",
        interface::DocsInput::CategoryOperations { category_id, query },
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(page)))
}

pub async fn get_category_openapi(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let interface::DocsOutput::OpenApi(spec) = invoke(
        state,
        headers,
        "http.console.docs.category-openapi.get.v1",
        interface::DocsInput::CategoryOpenApi { category_id },
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(spec))
}

pub async fn get_operation_openapi(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(operation_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let interface::DocsOutput::OpenApi(spec) = invoke(
        state,
        headers,
        "http.console.docs.operation-openapi.get.v1",
        interface::DocsInput::OperationOpenApi { operation_id },
    )
    .await?
    else {
        unreachable!()
    };
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
    let output: data_model_openapi_interface::DataModelOpenApiOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.model-definitions.openapi.view.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
            data_model_openapi_interface::DataModelOpenApiInput { model_id },
        )
        .await?;
    Ok(Json(output.0))
}
