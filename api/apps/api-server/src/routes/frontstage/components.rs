use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::Response,
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    frontend_block_catalog::{FrontendModuleAssetService, GetFrontendModuleAssetQuery},
    frontstage::FrontstagePageService,
    ui_management::{ListUiComponentRecordsQuery, UiManagementService},
};
use domain::{UiComponentRecord, UiComponentRecordOrigin};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
    response::ApiSuccess,
};

const COMPONENT_PAGE_SIZE: usize = 20;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolveFrontstageComponentDependencyLockBody {
    pub source_code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageComponentDependencyLockResponse {
    #[schema(value_type = Vec<Object>)]
    pub dependency_lock: Value,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct FrontstageComponentQuery {
    pub query: Option<String>,
    #[param(minimum = 0)]
    pub offset: Option<usize>,
    #[param(minimum = 1, maximum = 20)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontstageComponentUpstreamResponse {
    pub identity: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontstageComponentResponse {
    pub id: String,
    pub scope_id: String,
    pub component_code: String,
    pub name: String,
    pub description: String,
    pub import_code: String,
    pub source_code: String,
    #[schema(value_type = String)]
    pub origin: UiComponentRecordOrigin,
    pub source: String,
    pub group: String,
    pub upstream: FrontstageComponentUpstreamResponse,
    pub version: String,
    pub keywords: Vec<String>,
    pub catalog_updated_at: Option<String>,
    pub source_locator: Option<String>,
    pub source_checksum: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageComponentPageResponse {
    pub items: Vec<FrontstageComponentResponse>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/components",
    params(FrontstageComponentQuery),
    responses(
        (status = 200, body = FrontstageComponentPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontstage_components(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<FrontstageComponentQuery>,
) -> Result<Json<ApiSuccess<FrontstageComponentPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_design_permission(&context.actor)?;
    let page = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .list_component_records_page(ListUiComponentRecordsQuery {
            query: query.query,
            offset: query.offset.unwrap_or(0),
            limit: query
                .limit
                .unwrap_or(COMPONENT_PAGE_SIZE)
                .clamp(1, COMPONENT_PAGE_SIZE),
        })
        .await?;
    let items = page
        .items
        .into_iter()
        .map(component_response)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ApiSuccess::new(FrontstageComponentPageResponse {
        items,
        total: page.total,
        offset: page.offset,
        limit: page.limit,
        has_more: page.has_more,
        next_offset: page.next_offset,
    })))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/components/{component_id}",
    params(("component_id" = Uuid, Path, description = "Persisted component record id")),
    responses(
        (status = 200, body = FrontstageComponentResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_frontstage_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(component_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<FrontstageComponentResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_design_permission(&context.actor)?;
    let record = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .get_component_record(component_id)
        .await?;
    Ok(Json(ApiSuccess::new(component_response(record)?)))
}

#[utoipa::path(
    post,
    path = "/api/console/frontstage/component-dependency-lock",
    request_body = ResolveFrontstageComponentDependencyLockBody,
    responses(
        (status = 200, body = FrontstageComponentDependencyLockResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn resolve_frontstage_component_dependency_lock(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ResolveFrontstageComponentDependencyLockBody>,
) -> Result<Json<ApiSuccess<FrontstageComponentDependencyLockResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = context.actor.current_workspace_id;
    require_design_permission(&context.actor)?;
    let dependency_lock = FrontstagePageService::for_actor(state.store.clone(), context.actor)
        .with_node_id(state.api_node_id.clone())
        .resolve_component_dependency_lock(workspace_id, &body.source_code)
        .await?;
    Ok(Json(ApiSuccess::new(
        FrontstageComponentDependencyLockResponse { dependency_lock },
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/component-module-assets/{sha256}",
    params(("sha256" = String, Path, description = "Registered module asset SHA-256")),
    responses(
        (status = 200, description = "Digest-verified module asset with its declared Content-Type", body = Vec<u8>),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody),
        (status = 502, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_frontstage_component_module_asset(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<Response<Body>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = context.actor.current_workspace_id;
    require_design_permission(&context.actor)?;
    let asset = FrontendModuleAssetService::new(state.store.clone(), state.api_node_id.clone())
        .get_module_asset(GetFrontendModuleAssetQuery {
            workspace_id,
            sha256,
        })
        .await?
        .ok_or(ControlPlaneError::NotFound(
            "frontend_component_module_asset",
        ))?;
    let mut response = Response::new(Body::from(asset.bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.media_type)
            .map_err(|_| ControlPlaneError::InvalidInput("media_type"))?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"sha256-{}\"", asset.sha256))
            .map_err(|_| ControlPlaneError::InvalidInput("sha256"))?,
    );
    Ok(response)
}

fn require_design_permission(actor: &domain::ActorContext) -> Result<(), ApiError> {
    if !actor.has_permission("frontstage.page.design") {
        return Err(ControlPlaneError::PermissionDenied("frontstage.page.design").into());
    }
    Ok(())
}

fn component_response(value: UiComponentRecord) -> Result<FrontstageComponentResponse, ApiError> {
    use time::format_description::well_known::Rfc3339;
    Ok(FrontstageComponentResponse {
        id: value.id.to_string(),
        scope_id: value.scope_id.to_string(),
        component_code: value.component_code,
        name: value.name,
        description: value.description,
        import_code: value.import_code,
        source_code: value.source_code,
        origin: value.origin,
        source: value.source,
        group: value.group,
        upstream: FrontstageComponentUpstreamResponse {
            identity: value.upstream.identity,
            version: value.upstream.version,
        },
        version: value.version,
        keywords: value.keywords,
        catalog_updated_at: value
            .catalog_updated_at
            .map(|timestamp| timestamp.format(&Rfc3339))
            .transpose()?,
        source_locator: value.source_locator,
        source_checksum: value.source_checksum,
        created_at: value.created_at.format(&Rfc3339)?,
        updated_at: value.updated_at.format(&Rfc3339)?,
    })
}
