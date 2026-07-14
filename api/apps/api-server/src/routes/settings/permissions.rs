use std::sync::Arc;

use axum::{Json, Router, extract::State, http::HeaderMap};
use control_plane::role::RoleService;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::require_session,
    response::ApiSuccess,
    routes::console_route_assembly::{ConsoleRouteAssembly, console_get},
};

#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionResponse {
    pub code: String,
    pub resource: String,
    pub action: String,
    pub scope: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings_feature: Option<SettingsFeaturePermissionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SettingsFeaturePermissionResponse {
    pub feature_id: String,
    pub label_key: String,
    pub order: i32,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new().route(
        "/settings/roles/permission-options",
        console_get(
            list_permissions,
            ConsoleOperation("roles.permission_options.list".to_string()),
        ),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/roles/permission-options",
    responses((status = 200, body = [PermissionResponse]), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_permissions(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<PermissionResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let permissions = RoleService::new(state.store.clone())
        .list_permission_options(context.user.id)
        .await?
        .into_iter()
        .map(|permission| {
            let settings_feature = state
                .settings_feature_registry
                .inventory()
                .features
                .iter()
                .find(|feature| feature.permission_code == permission.code)
                .map(|feature| SettingsFeaturePermissionResponse {
                    feature_id: feature.feature_id.clone(),
                    label_key: feature.console_surface.label_key.clone(),
                    order: feature.console_surface.order,
                });

            PermissionResponse {
                code: permission.code,
                resource: permission.resource,
                action: permission.action,
                scope: permission.scope,
                name: permission.name,
                settings_feature,
            }
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiSuccess::new(permissions)))
}
