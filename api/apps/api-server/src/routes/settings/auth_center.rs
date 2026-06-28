use std::sync::Arc;

use access_control::ensure_permission;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use control_plane::errors::ControlPlaneError;
use serde::Serialize;
use serde_json::{Map, Value};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_csrf::require_csrf,
    middleware::require_session::require_session, response::ApiSuccess,
};

const AUTH_CENTER_OVERVIEW_PERMISSION: &str = "user.view.all";
const AUTH_CENTER_MANAGE_PERMISSION: &str = "user.manage.all";

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthCenterConfigFieldResponse {
    pub key: String,
    pub label: String,
    pub r#type: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthCenterAuthenticatorResponse {
    pub name: String,
    pub auth_type: String,
    pub title: String,
    pub enabled: bool,
    pub is_builtin: bool,
    pub config_schema: Vec<AuthCenterConfigFieldResponse>,
    pub config_values: Map<String, Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthCenterOverviewResponse {
    pub default_authenticator_name: String,
    pub authenticators: Vec<AuthCenterAuthenticatorResponse>,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/settings/auth-center/overview",
            get(get_auth_center_overview),
        )
        .route(
            "/settings/auth-center/authenticators/:name/actions/enable",
            post(enable_auth_center_authenticator),
        )
}

fn config_field_type(value: &Value) -> String {
    match value {
        Value::Bool(_) => "boolean".to_string(),
        Value::Number(_) => "number".to_string(),
        _ => "string".to_string(),
    }
}

fn is_sensitive_config_key(key: &str) -> bool {
    let normalized_key = key.to_ascii_lowercase();
    normalized_key.contains("secret")
        || normalized_key.contains("password")
        || normalized_key.contains("token")
        || normalized_key.contains("api_key")
}

fn auth_center_config_values(options: Value) -> Map<String, Value> {
    match options {
        Value::Object(values) => values
            .into_iter()
            .filter(|(key, value)| {
                !is_sensitive_config_key(key)
                    && matches!(
                        value,
                        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null
                    )
            })
            .collect(),
        _ => Map::new(),
    }
}

fn auth_center_config_schema(
    config_values: &Map<String, Value>,
) -> Vec<AuthCenterConfigFieldResponse> {
    let mut fields = config_values
        .iter()
        .map(|(key, value)| AuthCenterConfigFieldResponse {
            key: key.clone(),
            label: key.clone(),
            r#type: config_field_type(value),
        })
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| left.key.cmp(&right.key));
    fields
}

fn to_auth_center_authenticator_response(
    authenticator: domain::AuthenticatorRecord,
) -> AuthCenterAuthenticatorResponse {
    let config_values = auth_center_config_values(authenticator.options);
    let config_schema = auth_center_config_schema(&config_values);
    AuthCenterAuthenticatorResponse {
        name: authenticator.name,
        auth_type: authenticator.auth_type,
        title: authenticator.title,
        enabled: authenticator.enabled,
        is_builtin: authenticator.is_builtin,
        config_schema,
        config_values,
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

#[utoipa::path(
    post,
    path = "/api/console/settings/auth-center/authenticators/{name}/actions/enable",
    responses(
        (status = 200, body = AuthCenterAuthenticatorResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn enable_auth_center_authenticator(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<AuthCenterAuthenticatorResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    ensure_permission(&context.actor, AUTH_CENTER_MANAGE_PERMISSION)
        .map_err(ControlPlaneError::PermissionDenied)?;

    let mut authenticator = state
        .store
        .find_authenticator(&name)
        .await?
        .ok_or(ControlPlaneError::NotFound("authenticator"))?;
    authenticator.enabled = true;
    state.store.upsert_authenticator(&authenticator).await?;

    Ok(Json(ApiSuccess::new(
        to_auth_center_authenticator_response(authenticator),
    )))
}
