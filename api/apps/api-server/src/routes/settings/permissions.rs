use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use control_plane::role::RoleService;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
    response::ApiSuccess,
};

#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionResponse {
    pub code: String,
    pub resource: String,
    pub action: String,
    pub scope: String,
    pub name: String,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new().route("/settings/roles/permission-options", get(list_permissions))
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
        .map(|permission| PermissionResponse {
            code: permission.code,
            resource: permission.resource,
            action: permission.action,
            scope: permission.scope,
            name: permission.name,
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiSuccess::new(permissions)))
}
