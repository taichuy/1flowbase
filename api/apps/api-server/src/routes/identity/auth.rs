use std::sync::Arc;

use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use control_plane::auth::{
    AuthKernel, AuthenticatorRegistry, LoginCommand, SessionIssuer, SignUpCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;
use uuid::Uuid;

use interface_runtime::{
    InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol, InvocationEnvelope,
    InvocationId, InvocationLineage, PublicPrincipal,
};

use super::login_instances_interface::{
    self, PublicLoginInstancesFuture, PublicLoginInstancesInput, PublicLoginInstancesOutput,
    PublicLoginInstancesPort, PublicLoginInstancesTargetError,
};
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
    pub is_builtin: bool,
    pub title: String,
    pub description: Option<String>,
    pub sort_order: i32,
    pub public_ui_block: String,
    pub public_variables: Map<String, Value>,
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

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SignUpBody {
    pub authenticator_id: Uuid,
    pub account: String,
    pub email: String,
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
        .route("/sign-up", post(sign_up))
}

fn public_authenticator_description(options: &Value) -> Option<String> {
    options
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn to_public_login_instance(
    authenticator: domain::AuthenticatorRecord,
    registry: &AuthenticatorRegistry,
) -> Option<PublicLoginInstanceResponse> {
    if !authenticator.enabled {
        return None;
    }
    let public_variables = registry.public_variables(&authenticator)?;

    Some(PublicLoginInstanceResponse {
        id: authenticator.id,
        auth_type: authenticator.auth_type,
        is_builtin: authenticator.is_builtin,
        title: authenticator.title,
        description: public_authenticator_description(&authenticator.options),
        sort_order: authenticator.sort_order,
        public_ui_block: authenticator.public_ui_block,
        public_variables,
    })
}

#[utoipa::path(
    get,
    path = "/api/public/auth/providers",
    responses((status = 200, body = [AuthProviderResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_providers(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<AuthProviderResponse>>>, ApiError> {
    let mut provider = state
        .store
        .find_authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
        .await?
        .map(|authenticator| AuthProviderResponse {
            id: authenticator.id,
            auth_type: authenticator.auth_type,
            title: authenticator.title,
        });
    if let Some(provider) = &mut provider {
        let locale = crate::app_state::request_catalog_locale(&headers, None);
        provider.title = crate::app_state::project_canonical_display(
            &state,
            &locale,
            "Password",
            &provider.title,
        )
        .await?;
    }

    Ok(Json(ApiSuccess::new(provider.into_iter().collect())))
}

#[utoipa::path(
    get,
    path = "/api/public/auth/login-instances",
    responses((status = 200, body = PublicLoginInstancesResponse))
)]
pub async fn list_login_instances(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PublicLoginInstancesResponse>>, ApiError> {
    let locale = crate::app_state::request_catalog_locale(&headers, None);
    let boot_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("extension boot snapshot is unavailable"))?;
    let snapshot = boot_snapshot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry is unavailable"))?
        .snapshot();
    let binding_id = interface_runtime::BindingId::new("http.public.auth.login-instances.v1")
        .expect("static binding id is valid");
    let activated_authentication = snapshot
        .authentication(&binding_id)
        .ok_or_else(|| anyhow::anyhow!("public authentication activation is unavailable"))?;
    let principal =
        boot_snapshot.establish_principal(activated_authentication, PublicPrincipal::new())?;
    let authentication_activation = activated_authentication.activation().clone();
    let outcome = InterfaceInvocationKernel::new(Arc::new(
        login_instances_interface::PublicLoginInstancesAuthorization,
    ))
    .invoke::<
        PublicLoginInstancesInput,
        PublicLoginInstancesOutput,
        PublicLoginInstancesTargetError,
    >(
        snapshot,
        InvocationEnvelope::with_principal(
            InvocationLineage::root(InvocationId::now_v7()),
            binding_id,
            InterfaceProtocol::Http,
            interface_runtime::AuthenticationAdapterReference::new("api-server.public")
                .expect("static adapter is valid"),
            authentication_activation,
            principal,
            None,
            PublicLoginInstancesInput { locale },
        ),
    )
    .await
    .map_err(|failure| public_login_instances_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(Json(ApiSuccess::new(outcome.into_value().0)))
}

struct PublicLoginInstancesAdapter {
    state: std::sync::Weak<ApiState>,
}

impl PublicLoginInstancesPort for PublicLoginInstancesAdapter {
    fn list(&self, input: PublicLoginInstancesInput) -> PublicLoginInstancesFuture<'_> {
        let state = self.state.clone();
        Box::pin(async move {
            let state = state
                .upgrade()
                .ok_or_else(|| ApiError::from(anyhow::anyhow!("API state is unavailable")))?;
            let registry = state.authenticator_registry.as_ref();
            let mut login_instances = state
                .store
                .list_authenticators()
                .await
                .map_err(ApiError::from)?
                .into_iter()
                .filter_map(|authenticator| to_public_login_instance(authenticator, registry))
                .collect::<Vec<_>>();
            for instance in &mut login_instances {
                instance.title = crate::app_state::project_canonical_display(
                    &state,
                    &input.locale,
                    "Password",
                    &instance.title,
                )
                .await?;
            }
            let default_authenticator_id = login_instances
                .first()
                .map(|instance| instance.id)
                .unwrap_or(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID);
            Ok(PublicLoginInstancesOutput(PublicLoginInstancesResponse {
                default_authenticator_id,
                login_instances,
            }))
        })
    }
}

pub(crate) fn compile_public_login_instances_registry(
    state: std::sync::Weak<ApiState>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    login_instances_interface::compile_registry(Arc::new(PublicLoginInstancesAdapter { state }))
}

fn public_login_instances_error(error: InterfaceInvocationError) -> ApiError {
    match error {
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<PublicLoginInstancesTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| anyhow::anyhow!("public login instances failed").into()),
        error => anyhow::anyhow!(error.to_string()).into(),
    }
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

#[utoipa::path(
    post,
    path = "/api/public/auth/sign-up",
    request_body = SignUpBody,
    responses(
        (status = 200, body = LoginResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 409, body = crate::error_response::ErrorBody)
    )
)]
pub async fn sign_up(
    State(state): State<Arc<ApiState>>,
    Json(body): Json<SignUpBody>,
) -> Result<(CookieJar, Json<ApiSuccess<LoginResponse>>), ApiError> {
    let result = AuthKernel::new(
        state.store.clone(),
        SessionIssuer::new(state.session_store.clone(), state.session_ttl_days),
    )
    .sign_up(SignUpCommand {
        authenticator_id: body.authenticator_id,
        account: body.account,
        email: body.email,
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
