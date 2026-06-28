use std::sync::Arc;

use access_control::ensure_permission;
use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use control_plane::errors::ControlPlaneError;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
    response::ApiSuccess,
};

const AUTH_CENTER_OVERVIEW_PERMISSION: &str = "user.view.all";

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthCenterAuthenticatorResponse {
    pub name: String,
    pub auth_type: String,
    pub title: String,
    pub enabled: bool,
    pub is_builtin: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthCenterOverviewResponse {
    pub default_authenticator_name: String,
    pub authenticators: Vec<AuthCenterAuthenticatorResponse>,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new().route(
        "/settings/auth-center/overview",
        get(get_auth_center_overview),
    )
}

fn to_auth_center_authenticator_response(
    authenticator: domain::AuthenticatorRecord,
) -> AuthCenterAuthenticatorResponse {
    AuthCenterAuthenticatorResponse {
        name: authenticator.name,
        auth_type: authenticator.auth_type,
        title: authenticator.title,
        enabled: authenticator.enabled,
        is_builtin: authenticator.is_builtin,
    }
}

#[utoipa::path(
    get,
    path = "/api/console/settings/auth-center/overview",
    responses(
        (status = 200, body = AuthCenterOverviewResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_auth_center_overview(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<AuthCenterOverviewResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ensure_permission(&context.actor, AUTH_CENTER_OVERVIEW_PERMISSION)
        .map_err(ControlPlaneError::PermissionDenied)?;

    let authenticators = state
        .store
        .list_authenticators()
        .await?
        .into_iter()
        .map(to_auth_center_authenticator_response)
        .collect();

    Ok(Json(ApiSuccess::new(AuthCenterOverviewResponse {
        default_authenticator_name: domain::PASSWORD_LOCAL_AUTHENTICATOR_NAME.to_string(),
        authenticators,
    })))
}
