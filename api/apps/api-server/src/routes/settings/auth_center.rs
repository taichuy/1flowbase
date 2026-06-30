use std::{collections::HashSet, sync::Arc};

use access_control::ensure_permission;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use control_plane::auth::AuthenticatorRegistry;
use control_plane::errors::ControlPlaneError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
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
    pub sort_order: i32,
    pub config_schema: Vec<AuthCenterConfigFieldResponse>,
    pub config_values: Map<String, Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthCenterOverviewResponse {
    pub default_authenticator_name: String,
    pub supported_auth_types: Vec<String>,
    pub authenticators: Vec<AuthCenterAuthenticatorResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAuthCenterAuthenticatorBody {
    pub name: String,
    pub auth_type: String,
    pub title: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CopyAuthCenterAuthenticatorBody {
    pub name: String,
    pub title: String,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderAuthCenterAuthenticatorsBody {
    pub names: Vec<String>,
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
            "/settings/auth-center/authenticators",
            post(create_auth_center_authenticator),
        )
        .route(
            "/settings/auth-center/authenticators/order",
            put(reorder_auth_center_authenticators),
        )
        .route(
            "/settings/auth-center/authenticators/:name/actions/enable",
            post(enable_auth_center_authenticator),
        )
        .route(
            "/settings/auth-center/authenticators/:name/copy",
            post(copy_auth_center_authenticator),
        )
        .route(
            "/settings/auth-center/authenticators/:name/config",
            put(update_auth_center_authenticator_config),
        )
        .route(
            "/settings/auth-center/authenticators/:name",
            delete(delete_auth_center_authenticator),
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

fn auth_center_default_config_form_schema() -> Value {
    json!([
        {
            "key": "name",
            "label": "Authenticator identifier",
            "type": "string",
            "read_only": true,
            "required": true,
            "pattern": "^[A-Za-z0-9_]+$"
        },
        {
            "key": "title",
            "label": "Authenticator title",
            "type": "string",
            "required": true
        },
        {
            "key": "description",
            "label": "Description",
            "type": "string",
            "control": "textarea",
            "read_only": false,
            "required": false
        },
        {
            "key": "enabled",
            "label": "Enabled",
            "type": "boolean",
            "control": "switch"
        }
    ])
}

fn auth_center_new_authenticator_options(description: Option<String>) -> Value {
    let mut options = Map::new();
    if let Some(description) = description {
        options.insert("description".to_string(), Value::String(description));
    }
    options.insert(
        "config_form_schema".to_string(),
        auth_center_default_config_form_schema(),
    );
    options.insert("extension_config".to_string(), Value::Object(Map::new()));
    Value::Object(options)
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
        sort_order: authenticator.sort_order,
        config_schema,
        config_values,
    }
}

fn supported_auth_types() -> Vec<String> {
    AuthenticatorRegistry::new().supported_auth_types()
}

fn validate_supported_auth_type(auth_type: &str) -> Result<(), ControlPlaneError> {
    if supported_auth_types()
        .iter()
        .any(|supported| supported == auth_type)
    {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput("auth_type"))
    }
}

fn validate_new_authenticator_name(name: &str) -> Result<(), ControlPlaneError> {
    if name.is_empty()
        || name
            .chars()
            .any(|character| !character.is_ascii_alphanumeric() && character != '_')
    {
        return Err(ControlPlaneError::InvalidInput("authenticator_name"));
    }
    Ok(())
}

fn validate_authenticator_title(title: &str) -> Result<(), ControlPlaneError> {
    if title.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("title"));
    }
    Ok(())
}

fn validate_reorder_names(
    requested_names: &[String],
    existing_authenticators: &[domain::AuthenticatorRecord],
) -> Result<(), ControlPlaneError> {
    let mut seen = HashSet::new();
    for name in requested_names {
        if !seen.insert(name.as_str()) {
            return Err(ControlPlaneError::InvalidInput(
                "authenticator_order_duplicate",
            ));
        }
    }

    let existing_names = existing_authenticators
        .iter()
        .map(|authenticator| authenticator.name.as_str())
        .collect::<HashSet<_>>();
    if requested_names
        .iter()
        .any(|name| !existing_names.contains(name.as_str()))
    {
        return Err(ControlPlaneError::InvalidInput(
            "authenticator_order_unknown",
        ));
    }
    if requested_names.len() != existing_authenticators.len() {
        return Err(ControlPlaneError::InvalidInput(
            "authenticator_order_missing",
        ));
    }

    Ok(())
}

async fn next_authenticator_sort_order(state: &Arc<ApiState>) -> Result<i32, ApiError> {
    let next = state
        .store
        .list_authenticators()
        .await?
        .into_iter()
        .map(|authenticator| authenticator.sort_order)
        .max()
        .unwrap_or(0)
        + 10;
    Ok(next)
}

async fn auth_center_overview_response(
    state: &Arc<ApiState>,
) -> Result<AuthCenterOverviewResponse, ApiError> {
    let authenticators = state
        .store
        .list_authenticators()
        .await?
        .into_iter()
        .map(to_auth_center_authenticator_response)
        .collect();

    Ok(AuthCenterOverviewResponse {
        default_authenticator_name: domain::PASSWORD_LOCAL_AUTHENTICATOR_NAME.to_string(),
        supported_auth_types: supported_auth_types(),
        authenticators,
    })
}

async fn require_auth_center_manage(
    state: &Arc<ApiState>,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let context = require_session(state, headers).await?;
    require_csrf(headers, &context)?;
    ensure_permission(&context.actor, AUTH_CENTER_MANAGE_PERMISSION)
        .map_err(ControlPlaneError::PermissionDenied)?;
    Ok(())
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

    Ok(Json(ApiSuccess::new(
        auth_center_overview_response(&state).await?,
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/auth-center/authenticators",
    request_body = CreateAuthCenterAuthenticatorBody,
    responses(
        (status = 201, body = AuthCenterAuthenticatorResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 409, body = crate::error_response::ErrorBody)
    )
)]
pub async fn create_auth_center_authenticator(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateAuthCenterAuthenticatorBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<AuthCenterAuthenticatorResponse>>,
    ),
    ApiError,
> {
    require_auth_center_manage(&state, &headers).await?;
    validate_new_authenticator_name(&body.name)?;
    validate_supported_auth_type(&body.auth_type)?;
    validate_authenticator_title(&body.title)?;
    if state.store.find_authenticator(&body.name).await?.is_some() {
        return Err(ControlPlaneError::Conflict("authenticator").into());
    }

    let authenticator = domain::AuthenticatorRecord {
        name: body.name,
        auth_type: body.auth_type,
        title: body.title,
        enabled: body.enabled,
        is_builtin: false,
        sort_order: match body.sort_order {
            Some(sort_order) => sort_order,
            None => next_authenticator_sort_order(&state).await?,
        },
        options: auth_center_new_authenticator_options(body.description),
    };
    state.store.create_authenticator(&authenticator).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_auth_center_authenticator_response(
            authenticator,
        ))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/auth-center/authenticators/{name}/copy",
    request_body = CopyAuthCenterAuthenticatorBody,
    responses(
        (status = 201, body = AuthCenterAuthenticatorResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody),
        (status = 409, body = crate::error_response::ErrorBody)
    )
)]
pub async fn copy_auth_center_authenticator(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(body): Json<CopyAuthCenterAuthenticatorBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<AuthCenterAuthenticatorResponse>>,
    ),
    ApiError,
> {
    require_auth_center_manage(&state, &headers).await?;
    validate_new_authenticator_name(&body.name)?;
    validate_authenticator_title(&body.title)?;
    if state.store.find_authenticator(&body.name).await?.is_some() {
        return Err(ControlPlaneError::Conflict("authenticator").into());
    }

    let source = state
        .store
        .find_authenticator(&name)
        .await?
        .ok_or(ControlPlaneError::NotFound("authenticator"))?;
    validate_supported_auth_type(&source.auth_type)?;
    let authenticator = domain::AuthenticatorRecord {
        name: body.name,
        auth_type: source.auth_type,
        title: body.title,
        enabled: false,
        is_builtin: false,
        sort_order: match body.sort_order {
            Some(sort_order) => sort_order,
            None => next_authenticator_sort_order(&state).await?,
        },
        options: source.options,
    };
    state.store.create_authenticator(&authenticator).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_auth_center_authenticator_response(
            authenticator,
        ))),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/auth-center/authenticators/{name}",
    responses(
        (status = 204),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody),
        (status = 409, body = crate::error_response::ErrorBody)
    )
)]
pub async fn delete_auth_center_authenticator(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    require_auth_center_manage(&state, &headers).await?;
    state.store.delete_authenticator_if_unbound(&name).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/api/console/settings/auth-center/authenticators/order",
    request_body = ReorderAuthCenterAuthenticatorsBody,
    responses(
        (status = 200, body = AuthCenterOverviewResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn reorder_auth_center_authenticators(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ReorderAuthCenterAuthenticatorsBody>,
) -> Result<Json<ApiSuccess<AuthCenterOverviewResponse>>, ApiError> {
    require_auth_center_manage(&state, &headers).await?;
    let existing_authenticators = state.store.list_authenticators().await?;
    validate_reorder_names(&body.names, &existing_authenticators)?;
    state.store.update_authenticator_order(&body.names).await?;
    Ok(Json(ApiSuccess::new(
        auth_center_overview_response(&state).await?,
    )))
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
