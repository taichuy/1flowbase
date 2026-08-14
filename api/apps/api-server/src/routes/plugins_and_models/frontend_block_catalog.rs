use std::{collections::BTreeMap, sync::Arc};

use axum::{extract::State, http::HeaderMap, Json, Router};
use control_plane::frontend_block_catalog::{
    FrontendBlockCatalogService, FrontendContributionBinding, ListFrontendBlockCatalogQuery,
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
    pub url: String,
    pub integrity: String,
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
    pub frontend_contribution_id: String,
    pub frontend_block_id: String,
    pub frontend_block_version: String,
    pub runtime_kind: String,
    pub execution_kind: String,
    pub isolation_requirement: String,
    pub requested_permissions: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub workspace_id: String,
    pub lifecycle_kind: String,
    pub graph_fingerprint: String,
    pub provenance: FrontendContributionProvenanceResponse,
    pub disable_reason: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendContributionProvenanceResponse {
    pub module_id: String,
    pub module_version: String,
    pub module_kind: String,
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

fn to_response(
    binding: FrontendContributionBinding,
) -> Result<FrontendBlockCatalogResponse, ApiError> {
    let asset_bindings = binding
        .assets
        .into_iter()
        .map(|asset| (asset.digest.clone(), asset))
        .collect::<BTreeMap<_, _>>();
    let entry = binding.catalog_entry;
    Ok(FrontendBlockCatalogResponse {
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
                Ok(FrontendBlockCodeModuleResponse {
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
                        .map(|asset| {
                            let projected = asset_bindings.get(&asset.sha256).ok_or(
                                control_plane::errors::ControlPlaneError::UpstreamUnavailable(
                                    "frontend_contribution_asset_binding",
                                ),
                            )?;
                            Ok(FrontendBlockModuleAssetResponse {
                                role: match asset.role {
                                    domain::FrontendModuleAssetRole::BrowserModule => {
                                        "browser_module".to_string()
                                    }
                                    domain::FrontendModuleAssetRole::ShadowStyle => {
                                        "shadow_style".to_string()
                                    }
                                    domain::FrontendModuleAssetRole::Support => {
                                        "support".to_string()
                                    }
                                },
                                media_type: asset.media_type,
                                sha256: asset.sha256,
                                url: projected.url.clone(),
                                integrity: projected.integrity.as_str().to_string(),
                            })
                        })
                        .collect::<Result<Vec<_>, ApiError>>()?,
                    type_declarations,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?,
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
        frontend_contribution_id: binding.contribution_id,
        frontend_block_id: binding.block_id,
        frontend_block_version: binding.block_version,
        runtime_kind: binding.runtime_kind.as_str().to_string(),
        execution_kind: binding.execution_kind.as_str().to_string(),
        isolation_requirement: binding.isolation_requirement.as_str().to_string(),
        requested_permissions: binding.requested_permissions,
        granted_permissions: binding.granted_permissions,
        workspace_id: binding.workspace_id.to_string(),
        lifecycle_kind: binding.lifecycle.as_str().to_string(),
        graph_fingerprint: binding.graph_fingerprint,
        provenance: FrontendContributionProvenanceResponse {
            module_id: binding.provenance.module_id().as_str().to_string(),
            module_version: binding.provenance.module_version().as_str().to_string(),
            module_kind: binding.provenance.module_kind().as_str().to_string(),
        },
        disable_reason: binding
            .disable_reason
            .map(|reason| reason.as_str().to_string()),
    })
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
    let graph = state
        .extension_boot_snapshot
        .as_ref()
        .map(|snapshot| Arc::clone(snapshot.graph_arc()))
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_boot_snapshot",
        ))?;
    let entries = FrontendBlockCatalogService::new(
        state.store.for_actor(context.actor.clone()),
        state.api_node_id.clone(),
        graph,
    )?
    .list_frontend_blocks(ListFrontendBlockCatalogQuery {
        actor_user_id: context.user.id,
    })
    .await?;

    Ok(Json(ApiSuccess::new(
        entries
            .entries
            .into_iter()
            .map(to_response)
            .collect::<Result<Vec<_>, _>>()?,
    )))
}
