use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json, Router};
use control_plane::js_dependency::{JsDependencyService, ListWorkspaceJsDependenciesQuery};
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
pub struct JsDependencyPermissionsResponse {
    pub network: String,
    pub filesystem: String,
    pub env: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct JsDependencyCatalogEntryResponse {
    pub installation_id: String,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub alias: String,
    pub package: String,
    pub version: String,
    pub target: String,
    pub artifact_path: String,
    pub integrity: String,
    pub permissions: JsDependencyPermissionsResponse,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new().route(
        "/js-dependencies",
        console_get(
            list_js_dependencies,
            ConsoleOperation("js_dependencies.view".to_string()),
        ),
    )
}

fn to_response(entry: domain::JsDependencyRegistryEntry) -> JsDependencyCatalogEntryResponse {
    JsDependencyCatalogEntryResponse {
        installation_id: entry.installation_id.to_string(),
        provider_code: entry.provider_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        alias: entry.alias,
        package: entry.package,
        version: entry.version,
        target: entry.target,
        artifact_path: entry.artifact_path,
        integrity: entry.integrity,
        permissions: JsDependencyPermissionsResponse {
            network: entry.permissions.network,
            filesystem: entry.permissions.filesystem,
            env: entry.permissions.env,
        },
    }
}

#[utoipa::path(
    get,
    path = "/api/console/js-dependencies",
    responses(
        (status = 200, body = [JsDependencyCatalogEntryResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_js_dependencies(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<JsDependencyCatalogEntryResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let entries = JsDependencyService::new(state.store.clone())
        .list_workspace_js_dependencies(ListWorkspaceJsDependenciesQuery {
            actor_user_id: context.user.id,
        })
        .await?;

    Ok(Json(ApiSuccess::new(
        entries.entries.into_iter().map(to_response).collect(),
    )))
}
