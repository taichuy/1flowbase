use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
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
    pub label_key: String,
    pub order: i32,
}

pub(crate) fn permission_responses(
    permission_options: Vec<domain::PermissionDefinition>,
    features: &[access_control::SettingsFeatureInventoryEntry],
) -> Vec<PermissionResponse> {
    permission_options
        .into_iter()
        .map(|permission| {
            let settings_feature = features
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
        .collect()
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.permission-options.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::ListPermissionOptions,
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::PermissionOptions(items) = output
    else {
        return Err(anyhow::anyhow!("permission options output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(items)))
}
