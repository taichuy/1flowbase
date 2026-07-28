use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json, Router};
use control_plane::role::RoleService;
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
    pub label: String,
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
    let locale = crate::app_state::request_catalog_locale(&headers, context.user.preferred_locale);
    let permission_options = RoleService::new(state.store.clone())
        .list_permission_options(context.user.id)
        .await?;
    let mut permissions = Vec::with_capacity(permission_options.len());
    for permission in permission_options {
        let settings_feature = state
            .settings_feature_registry
            .inventory()
            .features
            .iter()
            .find(|feature| feature.permission_code == permission.code);
        let settings_feature = if let Some(feature) = settings_feature {
            Some(SettingsFeaturePermissionResponse {
                feature_id: feature.feature_id.clone(),
                label: super::super::core_console_i18n::resolve_core_console_display(
                    &state,
                    &locale,
                    &feature.console_surface.label_key,
                )
                .await?,
                order: feature.console_surface.order,
            })
        } else {
            None
        };

        permissions.push(PermissionResponse {
            code: permission.code,
            resource: permission.resource,
            action: permission.action,
            scope: permission.scope,
            name: permission.name,
            settings_feature,
        });
    }

    Ok(Json(ApiSuccess::new(permissions)))
}
