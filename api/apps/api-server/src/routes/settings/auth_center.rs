use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::auth::settings::{
    AuthCenterSettingsOverview, AuthCenterSettingsService, CopyAuthCenterAuthenticatorCommand,
    CreateAuthCenterAuthenticatorCommand, UpdateAuthCenterAuthenticatorCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_csrf::require_csrf,
    middleware::require_session::require_session,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_post, console_put, ConsoleRouteAssembly,
    },
};

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
#[serde(rename_all = "snake_case")]
pub enum AuthCenterContextVariableGroupResponse {
    Configuration,
    Runtime,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthCenterContextVariableResponse {
    pub group: AuthCenterContextVariableGroupResponse,
    pub label: String,
    pub member_path: String,
    #[schema(value_type = Object)]
    pub schema: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthCenterAuthenticatorResponse {
    pub id: Uuid,
    pub auth_type: String,
    pub title: String,
    pub enabled: bool,
    pub is_builtin: bool,
    pub sort_order: i32,
    pub interface_path_prefixes: Vec<String>,
    pub public_variables: Option<Map<String, Value>>,
    pub context_variables: Vec<AuthCenterContextVariableResponse>,
    pub config_schema: Vec<AuthCenterConfigFieldResponse>,
    pub config_values: Map<String, Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthCenterOverviewResponse {
    pub default_authenticator_id: Uuid,
    pub supported_auth_types: Vec<String>,
    pub authenticators: Vec<AuthCenterAuthenticatorResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAuthCenterAuthenticatorBody {
    pub auth_type: String,
    pub title: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CopyAuthCenterAuthenticatorBody {
    pub title: String,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderAuthCenterAuthenticatorsBody {
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAuthCenterAuthenticatorConfigBody {
    pub title: String,
    pub enabled: bool,
    pub description: Option<Option<String>>,
    pub self_registration_enabled: bool,
    pub public_ui_block: String,
    pub extension_config: Option<Map<String, Value>>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/auth-center/overview",
            console_get(
                get_auth_center_overview,
                ConsoleOperation("auth_center.overview.view".to_string()),
            ),
        )
        .route(
            "/settings/auth-center/authenticators",
            console_post(
                create_auth_center_authenticator,
                ConsoleOperation("auth_center.authenticators.create".to_string()),
            ),
        )
        .route(
            "/settings/auth-center/authenticators/order",
            console_put(
                reorder_auth_center_authenticators,
                ConsoleOperation("auth_center.authenticators.order".to_string()),
            ),
        )
        .route(
            "/settings/auth-center/authenticators/:id/actions/enable",
            console_post(
                enable_auth_center_authenticator,
                ConsoleOperation("auth_center.authenticators.enable".to_string()),
            ),
        )
        .route(
            "/settings/auth-center/authenticators/:id/copy",
            console_post(
                copy_auth_center_authenticator,
                ConsoleOperation("auth_center.authenticators.copy".to_string()),
            ),
        )
        .route(
            "/settings/auth-center/authenticators/:id/config",
            console_put(
                update_auth_center_authenticator_config,
                ConsoleOperation("auth_center.authenticators.update".to_string()),
            ),
        )
        .route(
            "/settings/auth-center/authenticators/:id",
            console_delete(
                delete_auth_center_authenticator,
                ConsoleOperation("auth_center.authenticators.delete".to_string()),
            ),
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
    let provider_fields = options
        .get("config_form_schema")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_else(|| auth_center_config_schema(extension_config));
    let mut fields: Vec<AuthCenterConfigFieldResponse> =
        serde_json::from_value(control_plane::auth::public_ui::auth_common_config_form_schema())
            .expect("core auth center config schema must be valid");
    for field in provider_fields {
        if !fields.iter().any(|existing| existing.key == field.key) {
            fields.push(field);
        }
    }
    fields
}

fn auth_center_description(options: &Value) -> Option<String> {
    options
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn auth_center_config_response_values(
    authenticator: &domain::AuthenticatorRecord,
    description: Option<String>,
    extension_config: Map<String, Value>,
) -> Map<String, Value> {
    let mut values = Map::new();
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
        "self_registration_enabled".to_string(),
        extension_config
            .get("self_registration_enabled")
            .cloned()
            .unwrap_or(Value::Bool(false)),
    );
    values.insert(
        "public_ui_block".to_string(),
        Value::String(authenticator.public_ui_block.clone()),
    );
    values.insert(
        "extension_config".to_string(),
        Value::Object(extension_config.clone()),
    );
    values.extend(extension_config);
    values
}

fn to_auth_center_authenticator_response(
    authenticator: domain::AuthenticatorRecord,
    registry: &control_plane::auth::AuthenticatorRegistry,
) -> AuthCenterAuthenticatorResponse {
    let description = auth_center_description(&authenticator.options);
    let extension_config = auth_center_extension_config(&authenticator.options);
    let config_schema =
        auth_center_config_schema_from_options(&authenticator.options, &extension_config);
    let config_values =
        auth_center_config_response_values(&authenticator, description, extension_config);
    let public_variables = registry.public_variables(&authenticator);
    let context_variables = registry
        .context_variables(&authenticator.auth_type)
        .into_iter()
        .map(|variable| AuthCenterContextVariableResponse {
            group: match variable.group {
                control_plane::auth::AuthenticatorContextVariableGroup::Configuration => {
                    AuthCenterContextVariableGroupResponse::Configuration
                }
                control_plane::auth::AuthenticatorContextVariableGroup::Runtime => {
                    AuthCenterContextVariableGroupResponse::Runtime
                }
            },
            label: variable.label,
            member_path: variable.member_path,
            schema: variable.schema,
        })
        .collect();
    AuthCenterAuthenticatorResponse {
        id: authenticator.id,
        auth_type: authenticator.auth_type,
        title: authenticator.title,
        enabled: authenticator.enabled,
        is_builtin: authenticator.is_builtin,
        sort_order: authenticator.sort_order,
        interface_path_prefixes: vec![crate::routes::PUBLIC_API_PATH_PREFIX.to_string()],
        public_variables,
        context_variables,
        config_schema,
        config_values,
    }
}

fn auth_center_overview_response(
    overview: AuthCenterSettingsOverview,
    registry: &control_plane::auth::AuthenticatorRegistry,
) -> AuthCenterOverviewResponse {
    let authenticators = overview
        .authenticators
        .into_iter()
        .map(|authenticator| to_auth_center_authenticator_response(authenticator, registry))
        .collect();

    AuthCenterOverviewResponse {
        default_authenticator_id: overview.default_authenticator_id,
        supported_auth_types: overview.supported_auth_types,
        authenticators,
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
    let overview = AuthCenterSettingsService::with_registry(
        state.store.clone(),
        state.authenticator_registry.clone(),
    )
    .overview(&context.actor)
    .await?;
    Ok(Json(ApiSuccess::new(auth_center_overview_response(
        overview,
        state.authenticator_registry.as_ref(),
    ))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let authenticator = AuthCenterSettingsService::with_registry(
        state.store.clone(),
        state.authenticator_registry.clone(),
    )
    .create_authenticator(
        &context.actor,
        CreateAuthCenterAuthenticatorCommand {
            auth_type: body.auth_type,
            title: body.title,
            description: body.description,
            enabled: body.enabled,
            sort_order: body.sort_order,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_auth_center_authenticator_response(
            authenticator,
            state.authenticator_registry.as_ref(),
        ))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/auth-center/authenticators/{id}/copy",
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
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<CopyAuthCenterAuthenticatorBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<AuthCenterAuthenticatorResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let authenticator = AuthCenterSettingsService::with_registry(
        state.store.clone(),
        state.authenticator_registry.clone(),
    )
    .copy_authenticator(
        &context.actor,
        CopyAuthCenterAuthenticatorCommand {
            source_id: id,
            title: body.title,
            sort_order: body.sort_order,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_auth_center_authenticator_response(
            authenticator,
            state.authenticator_registry.as_ref(),
        ))),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/auth-center/authenticators/{id}",
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
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    AuthCenterSettingsService::with_registry(
        state.store.clone(),
        state.authenticator_registry.clone(),
    )
    .delete_authenticator(&context.actor, id)
    .await?;
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let overview = AuthCenterSettingsService::with_registry(
        state.store.clone(),
        state.authenticator_registry.clone(),
    )
    .reorder_authenticators(&context.actor, &body.ids)
    .await?;
    Ok(Json(ApiSuccess::new(auth_center_overview_response(
        overview,
        state.authenticator_registry.as_ref(),
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/auth-center/authenticators/{id}/actions/enable",
    responses(
        (status = 200, body = AuthCenterAuthenticatorResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn enable_auth_center_authenticator(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<AuthCenterAuthenticatorResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let authenticator = AuthCenterSettingsService::with_registry(
        state.store.clone(),
        state.authenticator_registry.clone(),
    )
    .enable_authenticator(&context.actor, id)
    .await?;

    Ok(Json(ApiSuccess::new(
        to_auth_center_authenticator_response(authenticator, state.authenticator_registry.as_ref()),
    )))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/auth-center/authenticators/{id}/config",
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
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<UpdateAuthCenterAuthenticatorConfigBody>,
) -> Result<Json<ApiSuccess<AuthCenterAuthenticatorResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let authenticator = AuthCenterSettingsService::with_registry(
        state.store.clone(),
        state.authenticator_registry.clone(),
    )
    .update_authenticator(
        &context.actor,
        UpdateAuthCenterAuthenticatorCommand {
            authenticator_id: id,
            title: body.title,
            enabled: body.enabled,
            description: body.description,
            self_registration_enabled: body.self_registration_enabled,
            public_ui_block: body.public_ui_block,
            extension_config: body.extension_config,
        },
    )
    .await?;

    Ok(Json(ApiSuccess::new(
        to_auth_center_authenticator_response(authenticator, state.authenticator_registry.as_ref()),
    )))
}
