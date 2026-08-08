use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json, Router};
use control_plane::frontend_block_catalog::{
    FrontendBlockCatalogService, ListFrontendBlockCatalogQuery,
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::require_session,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
};

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendBlockPermissionsResponse {
    pub network: String,
    pub storage: String,
    pub secrets: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendBlockContextContractResponse {
    pub primitives: Vec<String>,
    #[schema(value_type = Object)]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendBlockModuleAssetResponse {
    pub role: String,
    pub media_type: String,
    pub sha256: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendBlockCodeModuleResponse {
    pub source: String,
    pub version: String,
    pub exports: Vec<String>,
    pub binding: String,
    pub assets: Vec<FrontendBlockModuleAssetResponse>,
    pub type_declarations: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendBlockCatalogResponse {
    pub installation_id: String,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub title: String,
    pub runtime: String,
    pub entry: String,
    pub code_template: Option<String>,
    pub code_template_version: Option<String>,
    pub code_template_language: Option<String>,
    pub code_modules: Vec<FrontendBlockCodeModuleResponse>,
    pub context_contract: FrontendBlockContextContractResponse,
    pub permissions: FrontendBlockPermissionsResponse,
    pub ui_capabilities: Vec<String>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new().route(
        "/frontend-blocks",
        console_get(
            list_frontend_blocks,
            ConsoleOperation("frontend_blocks.view".to_string()),
        ),
    )
}

fn to_response(entry: domain::FrontendBlockCatalogEntry) -> FrontendBlockCatalogResponse {
    FrontendBlockCatalogResponse {
        installation_id: entry.installation_id.to_string(),
        provider_code: entry.provider_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        contribution_code: entry.contribution_code,
        title: entry.title,
        runtime: entry.runtime,
        entry: entry.entry,
        code_template: entry.code_template,
        code_template_version: entry.code_template_version,
        code_template_language: entry.code_template_language,
        code_modules: entry
            .code_modules
            .into_iter()
            .map(|code_module| {
                let type_declarations = code_module.resolved_type_declarations();
                FrontendBlockCodeModuleResponse {
                    source: code_module.source,
                    version: code_module.version,
                    exports: code_module.exports,
                    binding: match code_module.binding {
                        domain::FrontendModuleBinding::Host => "host".to_string(),
                        domain::FrontendModuleBinding::Fetched => "fetched".to_string(),
                    },
                    assets: code_module
                        .assets
                        .into_iter()
                        .map(|asset| FrontendBlockModuleAssetResponse {
                            role: match asset.role {
                                domain::FrontendModuleAssetRole::BrowserModule => {
                                    "browser_module".to_string()
                                }
                                domain::FrontendModuleAssetRole::ShadowStyle => {
                                    "shadow_style".to_string()
                                }
                                domain::FrontendModuleAssetRole::Support => "support".to_string(),
                            },
                            media_type: asset.media_type,
                            sha256: asset.sha256,
                        })
                        .collect(),
                    type_declarations,
                }
            })
            .collect(),
        context_contract: FrontendBlockContextContractResponse {
            primitives: entry.context_contract.primitives,
            input_schema: entry.context_contract.input_schema,
        },
        permissions: FrontendBlockPermissionsResponse {
            network: entry.permissions.network,
            storage: entry.permissions.storage,
            secrets: entry.permissions.secrets,
        },
        ui_capabilities: entry.ui_capabilities,
    }
}

#[utoipa::path(
    get,
    path = "/api/console/frontend-blocks",
    responses(
        (status = 200, body = [FrontendBlockCatalogResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontend_blocks(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<FrontendBlockCatalogResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let entries = FrontendBlockCatalogService::new(
        state.store.for_actor(context.actor.clone()),
        state.api_node_id.clone(),
    )
    .list_frontend_blocks(ListFrontendBlockCatalogQuery {
        actor_user_id: context.user.id,
    })
    .await?;

    Ok(Json(ApiSuccess::new(
        entries.entries.into_iter().map(to_response).collect(),
    )))
}
