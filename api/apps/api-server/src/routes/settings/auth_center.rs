use std::sync::Arc;

use access_control::ensure_permission;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post, put},
    Json, Router,
};
use control_plane::errors::ControlPlaneError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_csrf::require_csrf,
    middleware::require_session::require_session, response::ApiSuccess,
};

const AUTH_CENTER_OVERVIEW_PERMISSION: &str = "user.view.all";
const AUTH_CENTER_MANAGE_PERMISSION: &str = "user.manage.all";

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AuthCenterConfigFieldResponse {
    pub key: String,
    pub label: String,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAuthCenterAuthenticatorConfigBody {
    pub name: Option<String>,
    pub title: String,
    pub enabled: bool,
    pub description: Option<Option<String>>,
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
        .route(
            "/settings/auth-center/authenticators/:name/config",
            put(update_auth_center_authenticator_config),
        )
}

fn config_field_type(value: &Value) -> String {
    match value {
        Value::Bool(_) => "boolean".to_string(),
        Value::Number(_) => "number".to_string(),
        _ => "string".to_string(),
    }
}

fn auth_center_extension_config(options: &Value) -> Map<String, Value> {
    options
        .get("extension_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
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
            control: None,
            read_only: None,
            required: None,
            pattern: None,
        })
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| left.key.cmp(&right.key));
    fields
}

fn auth_center_config_schema_from_options(
    options: &Value,
    extension_config: &Map<String, Value>,
) -> Vec<AuthCenterConfigFieldResponse> {
    options
        .get("config_form_schema")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_else(|| auth_center_config_schema(extension_config))
}

fn auth_center_description(options: &Value) -> Option<String> {
    options
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn upsert_auth_center_description(options: &mut Value, description: Option<String>) {
    if !options.is_object() {
        *options = Value::Object(Map::new());
    }
    let Some(values) = options.as_object_mut() else {
        return;
    };
    match description {
        Some(description) => {
            values.insert("description".to_string(), Value::String(description));
        }
        None => {
            values.remove("description");
        }
    }
}

fn auth_center_config_response_values(
    authenticator: &domain::AuthenticatorRecord,
    description: Option<String>,
    extension_config: Map<String, Value>,
) -> Map<String, Value> {
    let mut values = Map::new();
    values.insert(
        "name".to_string(),
        Value::String(authenticator.name.clone()),
    );
    values.insert(
        "title".to_string(),
        Value::String(authenticator.title.clone()),
    );
    values.insert("enabled".to_string(), Value::Bool(authenticator.enabled));
    values.insert(
        "description".to_string(),
        description.map(Value::String).unwrap_or(Value::Null),
    );
    values.insert(
        "extension_config".to_string(),
        Value::Object(extension_config),
    );
    values
}

fn to_auth_center_authenticator_response(
    authenticator: domain::AuthenticatorRecord,
) -> AuthCenterAuthenticatorResponse {
    let description = auth_center_description(&authenticator.options);
    let extension_config = auth_center_extension_config(&authenticator.options);
    let config_schema =
        auth_center_config_schema_from_options(&authenticator.options, &extension_config);
    let config_values =
        auth_center_config_response_values(&authenticator, description, extension_config);
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
    state
        .store
        .update_authenticator_config(&authenticator)
        .await?;

    Ok(Json(ApiSuccess::new(
        to_auth_center_authenticator_response(authenticator),
    )))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/auth-center/authenticators/{name}/config",
    request_body = UpdateAuthCenterAuthenticatorConfigBody,
    responses(
        (status = 200, body = AuthCenterAuthenticatorResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn update_auth_center_authenticator_config(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateAuthCenterAuthenticatorConfigBody>,
) -> Result<Json<ApiSuccess<AuthCenterAuthenticatorResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    ensure_permission(&context.actor, AUTH_CENTER_MANAGE_PERMISSION)
        .map_err(ControlPlaneError::PermissionDenied)?;

    if body
        .name
        .as_deref()
        .is_some_and(|body_name| body_name != name)
    {
        return Err(ControlPlaneError::InvalidInput("authenticator_name").into());
    }
    if body.title.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("title").into());
    }

    let mut authenticator = state
        .store
        .find_authenticator(&name)
        .await?
        .ok_or(ControlPlaneError::NotFound("authenticator"))?;
    authenticator.title = body.title;
    authenticator.enabled = body.enabled;
    if let Some(description) = body.description {
        upsert_auth_center_description(&mut authenticator.options, description);
    }
    state
        .store
        .update_authenticator_config(&authenticator)
        .await?;

    Ok(Json(ApiSuccess::new(
        to_auth_center_authenticator_response(authenticator),
    )))
}
