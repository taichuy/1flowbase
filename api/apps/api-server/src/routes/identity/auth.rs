use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use control_plane::auth::{AuthKernel, LoginCommand, SessionIssuer};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthProviderResponse {
    pub id: Uuid,
    pub auth_type: String,
    pub title: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PublicLoginInstanceResponse {
    pub id: Uuid,
    pub auth_type: String,
    pub title: String,
    pub description: Option<String>,
    pub sort_order: i32,
    pub flow: String,
    pub sign_in_path: String,
    pub public_options: Map<String, Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PublicLoginInstancesResponse {
    pub default_authenticator_id: Uuid,
    pub login_instances: Vec<PublicLoginInstanceResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginBody {
    pub authenticator_id: Option<Uuid>,
    pub identifier: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub csrf_token: String,
    pub effective_display_role: String,
    pub current_workspace_id: String,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/providers", get(list_providers))
        .route("/login-instances", get(list_login_instances))
        .route("/sign-in", post(sign_in))
}

fn public_authenticator_description(options: &Value) -> Option<String> {
    options
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn public_authenticator_options(options: &Value) -> Map<String, Value> {
    options
        .get("public_options")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn to_public_login_instance(
    authenticator: domain::AuthenticatorRecord,
) -> Option<PublicLoginInstanceResponse> {
    if !authenticator.enabled || authenticator.auth_type != "password-local" {
        return None;
    }

    Some(PublicLoginInstanceResponse {
        id: authenticator.id,
        auth_type: authenticator.auth_type,
        title: authenticator.title,
        description: public_authenticator_description(&authenticator.options),
        sort_order: authenticator.sort_order,
        flow: "password".to_string(),
        sign_in_path: "/api/public/auth/sign-in".to_string(),
        public_options: public_authenticator_options(&authenticator.options),
    })
}

#[utoipa::path(
    get,
    path = "/api/public/auth/providers",
    responses((status = 200, body = [AuthProviderResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_providers(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiSuccess<Vec<AuthProviderResponse>>>, ApiError> {
    let provider = state
        .store
        .find_authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
        .await?
        .map(|authenticator| AuthProviderResponse {
            id: authenticator.id,
            auth_type: authenticator.auth_type,
            title: authenticator.title,
        });

    Ok(Json(ApiSuccess::new(provider.into_iter().collect())))
}

#[utoipa::path(
    get,
    path = "/api/public/auth/login-instances",
    responses((status = 200, body = PublicLoginInstancesResponse))
)]
pub async fn list_login_instances(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiSuccess<PublicLoginInstancesResponse>>, ApiError> {
    let login_instances = state
        .store
        .list_authenticators()
        .await?
        .into_iter()
        .filter_map(to_public_login_instance)
        .collect::<Vec<_>>();
    let default_authenticator_id = login_instances
        .first()
        .map(|instance| instance.id)
        .unwrap_or(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID);

    Ok(Json(ApiSuccess::new(PublicLoginInstancesResponse {
        default_authenticator_id,
        login_instances,
    })))
}

#[utoipa::path(
    post,
    path = "/api/public/auth/sign-in",
    request_body = LoginBody,
    responses((status = 200, body = LoginResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn sign_in(
    State(state): State<Arc<ApiState>>,
    Json(body): Json<LoginBody>,
) -> Result<(CookieJar, Json<ApiSuccess<LoginResponse>>), ApiError> {
    let kernel = AuthKernel::new(
        state.store.clone(),
        SessionIssuer::new(state.session_store.clone(), state.session_ttl_days),
    );
    let result = kernel
        .login(LoginCommand {
            authenticator_id: body
                .authenticator_id
                .unwrap_or(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID),
            identifier: body.identifier,
            password: body.password,
        })
        .await?;

    let cookie = Cookie::build((state.cookie_name.clone(), result.session.session_id.clone()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.cookie_secure)
        .path("/")
        .build();
    let jar = CookieJar::new().add(cookie);

    Ok((
        jar,
        Json(ApiSuccess::new(LoginResponse {
            csrf_token: result.session.csrf_token,
            effective_display_role: result.actor.effective_display_role,
            current_workspace_id: result.session.current_workspace_id.to_string(),
        })),
    ))
}
