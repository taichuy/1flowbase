use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::auth::settings::{AuthCenterSettingsOverview, AUTH_CENTER_HOST_CONFIG_KEYS};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
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
    pub public_ui_block: String,
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
    pub extension_config: Option<Map<String, Value>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAuthCenterAuthenticatorPublicUiBlockBody {
    pub public_ui_block: String,
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
            "/settings/auth-center/authenticators/:id/public-ui-block",
            console_put(
                update_auth_center_authenticator_public_ui_block,
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
    for field in provider_fields
        .into_iter()
        .filter(|field| field.key != "public_ui_block")
    {
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
    config_schema: &[AuthCenterConfigFieldResponse],
) -> Map<String, Value> {
    let writable_extension_config = extension_config
        .iter()
        .filter(|(key, _)| {
            !AUTH_CENTER_HOST_CONFIG_KEYS.contains(&key.as_str())
                && config_schema.iter().any(|field| field.key == key.as_str())
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Map<_, _>>();
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
        "extension_config".to_string(),
        Value::Object(writable_extension_config.clone()),
    );
    values.extend(writable_extension_config);
    values
}

pub(crate) fn to_auth_center_authenticator_response(
    authenticator: domain::AuthenticatorRecord,
    registry: &control_plane::auth::AuthenticatorRegistry,
) -> AuthCenterAuthenticatorResponse {
    let description = auth_center_description(&authenticator.options);
    let extension_config = auth_center_extension_config(&authenticator.options);
    let config_schema =
        auth_center_config_schema_from_options(&authenticator.options, &extension_config);
    let config_values = auth_center_config_response_values(
        &authenticator,
        description,
        extension_config,
        &config_schema,
    );
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
        public_ui_block: authenticator.public_ui_block,
        interface_path_prefixes: vec![crate::routes::PUBLIC_API_PATH_PREFIX.to_string()],
        public_variables,
        context_variables,
        config_schema,
        config_values,
    }
}

pub(crate) fn auth_center_overview_response(
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

async fn localize_authenticator_response(
    state: &ApiState,
    locale: &domain::CatalogLocale,
    response: &mut AuthCenterAuthenticatorResponse,
) -> Result<(), ApiError> {
    if response.id == domain::PASSWORD_LOCAL_AUTHENTICATOR_ID {
        response.title =
            crate::app_state::project_canonical_display(state, locale, "Password", &response.title)
                .await?;
        response
            .config_values
            .insert("title".to_owned(), Value::String(response.title.clone()));
        if let Some(public_variables) = &mut response.public_variables {
            public_variables.insert("title".to_owned(), Value::String(response.title.clone()));
        }
    }
    for variable in &mut response.context_variables {
        let key = match variable.member_path.as_str() {
            "inputs.authenticator_id" => Some("Authenticator ID"),
            "inputs.authenticator_selection_available" => Some("Authenticator selection available"),
            "inputs.auth_event" => Some("Authentication event"),
            "api" => Some("API"),
            _ => None,
        };
        if let Some(key) = key {
            variable.label = crate::app_state::resolve_request_text(state, locale, key).await?;
        }
    }
    Ok(())
}

async fn localize_overview_response(
    state: &ApiState,
    locale: &domain::CatalogLocale,
    response: &mut AuthCenterOverviewResponse,
) -> Result<(), ApiError> {
    for authenticator in &mut response.authenticators {
        localize_authenticator_response(state, locale, authenticator).await?;
    }
    Ok(())
}

pub(crate) async fn localize_authenticator_response_with(
    store: &storage_durable_postgres::MainDurableStore,
    bootstrap_workspace_id: Uuid,
    locale: &domain::CatalogLocale,
    response: &mut AuthCenterAuthenticatorResponse,
) -> Result<(), ApiError> {
    if response.id == domain::PASSWORD_LOCAL_AUTHENTICATOR_ID {
        response.title = crate::app_state::project_canonical_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            "Password",
            &response.title,
        )
        .await?;
        response
            .config_values
            .insert("title".to_owned(), Value::String(response.title.clone()));
        if let Some(public_variables) = &mut response.public_variables {
            public_variables.insert("title".to_owned(), Value::String(response.title.clone()));
        }
    }
    for variable in &mut response.context_variables {
        let key = match variable.member_path.as_str() {
            "inputs.authenticator_id" => Some("Authenticator ID"),
            "inputs.authenticator_selection_available" => Some("Authenticator selection available"),
            "inputs.auth_event" => Some("Authentication event"),
            "api" => Some("API"),
            _ => None,
        };
        if let Some(key) = key {
            variable.label = crate::app_state::resolve_request_text_with(
                store,
                bootstrap_workspace_id,
                locale,
                key,
            )
            .await?;
        }
    }
    Ok(())
}

pub(crate) async fn localize_overview_response_with(
    store: &storage_durable_postgres::MainDurableStore,
    bootstrap_workspace_id: Uuid,
    locale: &domain::CatalogLocale,
    response: &mut AuthCenterOverviewResponse,
) -> Result<(), ApiError> {
    for authenticator in &mut response.authenticators {
        localize_authenticator_response_with(store, bootstrap_workspace_id, locale, authenticator)
            .await?;
    }
    Ok(())
}

async fn localized_authenticator_response(
    state: &ApiState,
    headers: &HeaderMap,
    preferred_locale: Option<String>,
    authenticator: domain::AuthenticatorRecord,
) -> Result<AuthCenterAuthenticatorResponse, ApiError> {
    let locale = crate::app_state::request_catalog_locale(headers, preferred_locale);
    let mut response =
        to_auth_center_authenticator_response(authenticator, state.authenticator_registry.as_ref());
    localize_authenticator_response(state, &locale, &mut response).await?;
    Ok(response)
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.auth-center.overview.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::auth_center_interface::AuthCenterInput::Overview { locale },
    )
    .await?;
    let crate::routes::auth_center_interface::AuthCenterOutput::Overview(response) = output else {
        return Err(anyhow::anyhow!("auth center overview output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(response)))
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.auth-center.authenticators.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::auth_center_interface::AuthCenterInput::Create { locale, body },
    )
    .await?;
    let crate::routes::auth_center_interface::AuthCenterOutput::Authenticator(response) = output
    else {
        return Err(anyhow::anyhow!("auth center create output contract mismatch").into());
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.auth-center.authenticators.copy.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::auth_center_interface::AuthCenterInput::Copy { locale, id, body },
    )
    .await?;
    let crate::routes::auth_center_interface::AuthCenterOutput::Authenticator(response) = output
    else {
        return Err(anyhow::anyhow!("auth center copy output contract mismatch").into());
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
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
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::auth_center_interface::AuthCenterOutput,
    >(
        Arc::clone(&state),
        "http.console.auth-center.authenticators.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::auth_center_interface::AuthCenterInput::Delete { id },
    )
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.auth-center.authenticators.order.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::auth_center_interface::AuthCenterInput::Reorder { locale, body },
    )
    .await?;
    let crate::routes::auth_center_interface::AuthCenterOutput::Overview(response) = output else {
        return Err(anyhow::anyhow!("auth center reorder output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(response)))
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.auth-center.authenticators.enable.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::auth_center_interface::AuthCenterInput::Enable { locale, id },
    )
    .await?;
    let crate::routes::auth_center_interface::AuthCenterOutput::Authenticator(response) = output
    else {
        return Err(anyhow::anyhow!("auth center enable output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(response)))
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.auth-center.authenticators.update.config.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::auth_center_interface::AuthCenterInput::UpdateConfig { locale, id, body },
    )
    .await?;
    let crate::routes::auth_center_interface::AuthCenterOutput::Authenticator(response) = output
    else {
        return Err(anyhow::anyhow!("auth center update output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/auth-center/authenticators/{id}/public-ui-block",
    request_body = UpdateAuthCenterAuthenticatorPublicUiBlockBody,
    responses(
        (status = 200, body = AuthCenterAuthenticatorResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn update_auth_center_authenticator_public_ui_block(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<UpdateAuthCenterAuthenticatorPublicUiBlockBody>,
) -> Result<Json<ApiSuccess<AuthCenterAuthenticatorResponse>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.auth-center.authenticators.update.public-ui-block.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::auth_center_interface::AuthCenterInput::UpdatePublicUiBlock {
            locale,
            id,
            body,
        },
    )
    .await?;
    let crate::routes::auth_center_interface::AuthCenterOutput::Authenticator(response) = output
    else {
        return Err(anyhow::anyhow!("auth center public UI output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(response)))
}
