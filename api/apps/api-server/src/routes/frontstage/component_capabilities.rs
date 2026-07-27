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
    frontend_block_catalog::{
        FrontendComponentCapability, FrontendComponentCatalogService,
        GetFrontendComponentCapabilityQuery, GetFrontendModuleAssetQuery,
        ListFrontendComponentCapabilitiesQuery,
    },
    ports::FrontstagePageRepository,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
    response::ApiSuccess,
};

const COMPONENT_CAPABILITY_PAGE_SIZE: usize = 20;

#[derive(Debug, Deserialize, IntoParams)]
pub struct FrontstageComponentCapabilityQuery {
    pub installation_id: Option<Uuid>,
    pub contribution_code: Option<String>,
    pub query: Option<String>,
    pub module_source: Option<String>,
    #[param(minimum = 0)]
    pub offset: Option<usize>,
    #[param(minimum = 1, maximum = 20)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontendComponentUpstreamResponse {
    pub package: String,
    pub component: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontendComponentPropResponse {
    pub name: String,
    pub r#type: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontendComponentExampleResponse {
    pub title: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontendModuleBrowserAssetResponse {
    pub sha256: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontendModuleAssetResponse {
    pub role: String,
    pub media_type: String,
    pub sha256: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontstageComponentCapabilitySummaryResponse {
    pub component_id: String,
    pub installation_id: String,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub module_source: String,
    pub module_version: String,
    pub exports: Vec<String>,
    pub binding: String,
    pub assets: Vec<FrontendModuleAssetResponse>,
    pub browser_asset: Option<FrontendModuleBrowserAssetResponse>,
    pub export_name: String,
    pub upstream: Option<FrontendComponentUpstreamResponse>,
    pub description: String,
    pub insert_snippet: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageComponentCapabilityPageResponse {
    pub items: Vec<FrontstageComponentCapabilitySummaryResponse>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub module_sources: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageComponentCapabilityResponse {
    #[serde(flatten)]
    pub summary: FrontstageComponentCapabilitySummaryResponse,
    pub props: Vec<FrontendComponentPropResponse>,
    pub limitations: Vec<String>,
    pub examples: Vec<FrontendComponentExampleResponse>,
    pub typescript_declaration: String,
    pub api_documentation: String,
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/{workspace_id}/component-capabilities",
    params(
        FrontstageComponentCapabilityQuery,
        ("workspace_id" = String, Path, description = "Workspace id")
    ),
    responses(
        (status = 200, body = FrontstageComponentCapabilityPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontstage_component_capabilities(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<FrontstageComponentCapabilityQuery>,
    Path(workspace_id): Path<String>,
) -> Result<Json<ApiSuccess<FrontstageComponentCapabilityPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    require_design_permission(&state, context.user.id, workspace_id).await?;
    let page = FrontendComponentCatalogService::new(state.store.clone())
        .list_component_capabilities(ListFrontendComponentCapabilitiesQuery {
            workspace_id,
            installation_id: query.installation_id,
            contribution_code: query.contribution_code,
            query: query.query,
            module_source: query.module_source,
            offset: query.offset.unwrap_or(0),
            limit: query
                .limit
                .unwrap_or(COMPONENT_CAPABILITY_PAGE_SIZE)
                .clamp(1, COMPONENT_CAPABILITY_PAGE_SIZE),
        })
        .await?;

    Ok(Json(ApiSuccess::new(
        FrontstageComponentCapabilityPageResponse {
            items: page
                .items
                .into_iter()
                .map(|entry| to_summary_response(entry, workspace_id))
                .collect(),
            total: page.total,
            offset: page.offset,
            limit: page.limit,
            has_more: page.has_more,
            next_offset: page.next_offset,
            module_sources: page.module_sources,
        },
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/{workspace_id}/component-capabilities/{component_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace id"),
        ("component_id" = String, Path, description = "Component capability id")
    ),
    responses(
        (status = 200, body = FrontstageComponentCapabilityResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_frontstage_component_capability(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, component_id)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<FrontstageComponentCapabilityResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    require_design_permission(&state, context.user.id, workspace_id).await?;
    let entry = FrontendComponentCatalogService::new(state.store.clone())
        .get_component_capability(GetFrontendComponentCapabilityQuery {
            workspace_id,
            component_id,
        })
        .await?
        .ok_or(ControlPlaneError::NotFound("frontend_component_capability"))?;

    Ok(Json(ApiSuccess::new(to_detail_response(
        entry,
        workspace_id,
    ))))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/{workspace_id}/component-module-assets/{sha256}",
    params(
        ("workspace_id" = String, Path, description = "Workspace id"),
        ("sha256" = String, Path, description = "Registered module asset SHA-256")
    ),
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
    Path((workspace_id, sha256)): Path<(String, String)>,
) -> Result<Response<Body>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    require_design_permission(&state, context.user.id, workspace_id).await?;
    let asset = FrontendComponentCatalogService::new(state.store.clone())
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

async fn require_design_permission(
    state: &ApiState,
    actor_user_id: Uuid,
    workspace_id: Uuid,
) -> Result<(), ApiError> {
    let actor = state
        .store
        .load_actor_context_for_workspace(actor_user_id, workspace_id)
        .await?;
    if !actor.has_permission("frontstage.page.design") {
        return Err(ControlPlaneError::PermissionDenied("frontstage.page.design").into());
    }
    Ok(())
}

fn to_summary_response(
    entry: FrontendComponentCapability,
    workspace_id: Uuid,
) -> FrontstageComponentCapabilitySummaryResponse {
    let assets = entry
        .assets
        .iter()
        .map(|asset| FrontendModuleAssetResponse {
            role: frontend_asset_role(asset.role).to_string(),
            media_type: asset.media_type.clone(),
            sha256: asset.sha256.clone(),
            url: module_asset_url(workspace_id, &asset.sha256),
        })
        .collect::<Vec<_>>();
    let browser_asset = entry
        .assets
        .iter()
        .find(|asset| asset.role == domain::FrontendModuleAssetRole::BrowserModule)
        .map(|asset| FrontendModuleBrowserAssetResponse {
            sha256: asset.sha256.clone(),
            url: module_asset_url(workspace_id, &asset.sha256),
        });
    FrontstageComponentCapabilitySummaryResponse {
        component_id: entry.component_id,
        installation_id: entry.installation_id.to_string(),
        provider_code: entry.provider_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        contribution_code: entry.contribution_code,
        module_source: entry.module_source,
        module_version: entry.module_version,
        exports: entry.exports,
        binding: match entry.binding {
            domain::FrontendModuleBinding::Host => "host".to_owned(),
            domain::FrontendModuleBinding::Fetched => "fetched".to_owned(),
        },
        assets,
        browser_asset,
        export_name: entry.contract.export_name,
        upstream: entry
            .contract
            .upstream
            .map(|upstream| FrontendComponentUpstreamResponse {
                package: upstream.package,
                component: upstream.component,
                version: upstream.version,
            }),
        description: entry.contract.description,
        insert_snippet: entry.contract.insert_snippet,
    }
}

fn module_asset_url(workspace_id: Uuid, sha256: &str) -> String {
    format!("/api/console/frontstage/{workspace_id}/component-module-assets/{sha256}")
}

fn frontend_asset_role(role: domain::FrontendModuleAssetRole) -> &'static str {
    match role {
        domain::FrontendModuleAssetRole::BrowserModule => "browser_module",
        domain::FrontendModuleAssetRole::ShadowStyle => "shadow_style",
        domain::FrontendModuleAssetRole::Support => "support",
    }
}

fn to_detail_response(
    entry: FrontendComponentCapability,
    workspace_id: Uuid,
) -> FrontstageComponentCapabilityResponse {
    let declaration = entry.contract.typescript_declaration(&entry.module_source);
    let import_statement = if entry.contract.export_name == "default" {
        format!("import DefaultExport from '{}';", entry.module_source)
    } else {
        format!(
            "import {{ {} }} from '{}';",
            entry.contract.export_name, entry.module_source
        )
    };
    let api_documentation = format!("{import_statement}\n\n{declaration}");
    let props = entry
        .contract
        .props
        .iter()
        .map(|prop| FrontendComponentPropResponse {
            name: prop.name.clone(),
            r#type: prop.type_name.clone(),
            required: prop.required,
            description: prop.description.clone(),
        })
        .collect();
    let limitations = entry.contract.limitations.clone();
    let examples = entry
        .contract
        .examples
        .iter()
        .map(|example| FrontendComponentExampleResponse {
            title: example.title.clone(),
            code: example.code.clone(),
        })
        .collect();
    FrontstageComponentCapabilityResponse {
        summary: to_summary_response(entry, workspace_id),
        props,
        limitations,
        examples,
        typescript_declaration: declaration,
        api_documentation,
    }
}
