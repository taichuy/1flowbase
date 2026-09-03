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
use control_plane::ports::SessionStore;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use storage_durable_postgres::MainDurableStore;
use utoipa::ToSchema;
use uuid::Uuid;

use interface_runtime::{
    InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol, InvocationEnvelope,
    InvocationId, InvocationLineage, PublicPrincipal,
};

use super::login_entries_interface::{
    self, PublicLoginEntriesFuture, PublicLoginEntriesInput, PublicLoginEntriesOutput,
    PublicLoginEntriesPort, PublicLoginEntriesTargetError,
};
use super::public_residual_interface::{
    self, PublicProvidersFuture, PublicProvidersInput, PublicProvidersOutput, PublicProvidersPort,
    PublicResidualTargetError, PublicSignUpFuture, PublicSignUpInput, PublicSignUpOutput,
    PublicSignUpPort,
};
use super::sign_in_interface::{
    self, PublicSignInInput, PublicSignInOutput, PublicSignInTargetError,
};
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthProviderResponse {
    pub id: Uuid,
    pub auth_type: String,
    pub title: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PublicLoginEntryResponse {
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
pub struct PublicLoginEntriesResponse {
    pub default_login_entry_id: Uuid,
    pub login_entries: Vec<PublicLoginEntryResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginBody {
    pub login_entry_id: Option<Uuid>,
    pub identifier: String,
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SignUpBody {
    pub login_entry_id: Uuid,
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
        .route("/login-entries", get(list_login_entries))
        .route("/sign-in", post(sign_in))
        .route("/sign-up", post(sign_up))
}

fn public_login_entry_description(options: &Value) -> Option<String> {
    options
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn to_public_login_entry(
    entry: domain::LoginEntryRecord,
    registry: &AuthenticatorRegistry,
) -> Option<PublicLoginEntryResponse> {
    if !entry.enabled {
        return None;
    }
    let public_variables = registry.public_variables(&entry)?;

    Some(PublicLoginEntryResponse {
        id: entry.id,
        auth_type: entry.auth_type,
        is_builtin: entry.is_builtin,
        title: entry.title,
        description: public_login_entry_description(&entry.options),
        sort_order: entry.sort_order,
        public_ui_block: entry.public_ui_block,
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
    let locale = crate::app_state::request_catalog_locale(&headers, None);
    let output = invoke_public_residual::<PublicProvidersInput, PublicProvidersOutput>(
        &state,
        public_residual_interface::PROVIDERS_BINDING_ID,
        PublicProvidersInput { locale },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

struct PublicProvidersAdapter {
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
}

impl PublicProvidersPort for PublicProvidersAdapter {
    fn list(&self, input: PublicProvidersInput) -> PublicProvidersFuture<'_> {
        let store = self.store.clone();
        let bootstrap_workspace_id = self.bootstrap_workspace_id;
        Box::pin(async move {
            let mut provider = store
                .find_login_entry(domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID)
                .await
                .map_err(ApiError::from)
                .map_err(PublicResidualTargetError)?
                .map(|entry| AuthProviderResponse {
                    id: entry.id,
                    auth_type: entry.auth_type,
                    title: entry.title,
                });
            if let Some(provider) = &mut provider {
                provider.title = crate::app_state::project_canonical_display_with(
                    &store,
                    bootstrap_workspace_id,
                    &input.locale,
                    "Password",
                    &provider.title,
                )
                .await
                .map_err(PublicResidualTargetError)?;
            }
            Ok(PublicProvidersOutput(provider.into_iter().collect()))
        })
    }
}

#[utoipa::path(
    get,
    path = "/api/public/auth/login-entries",
    responses((status = 200, body = PublicLoginEntriesResponse))
)]
pub async fn list_login_entries(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PublicLoginEntriesResponse>>, ApiError> {
    let locale = crate::app_state::request_catalog_locale(&headers, None);
    let boot_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("extension boot snapshot is unavailable"))?;
    let snapshot = boot_snapshot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry is unavailable"))?
        .snapshot();
    let binding_id = interface_runtime::BindingId::new("http.public.auth.login-entries.v1")
        .expect("static binding id is valid");
    let activated_authentication = snapshot
        .authentication(&binding_id)
        .ok_or_else(|| anyhow::anyhow!("public authentication activation is unavailable"))?;
    let principal: PublicPrincipal = boot_snapshot
        .authenticate(
            activated_authentication,
            crate::extension_bus::PublicAuthenticationCredential,
        )
        .await?;
    let authentication_activation = activated_authentication.activation().clone();
    let outcome = InterfaceInvocationKernel::new(Arc::new(
        login_entries_interface::PublicLoginEntriesAuthorization,
    ))
    .invoke::<PublicLoginEntriesInput, PublicLoginEntriesOutput, PublicLoginEntriesTargetError>(
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
            PublicLoginEntriesInput { locale },
        ),
    )
    .await
    .map_err(|failure| public_login_entries_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(Json(ApiSuccess::new(outcome.into_value().0)))
}

struct PublicLoginEntriesAdapter {
    store: MainDurableStore,
    authenticator_registry: Arc<AuthenticatorRegistry>,
    bootstrap_workspace_id: Uuid,
}

impl PublicLoginEntriesPort for PublicLoginEntriesAdapter {
    fn list(&self, input: PublicLoginEntriesInput) -> PublicLoginEntriesFuture<'_> {
        let store = self.store.clone();
        let registry = Arc::clone(&self.authenticator_registry);
        let bootstrap_workspace_id = self.bootstrap_workspace_id;
        Box::pin(async move {
            let mut login_entries = store
                .list_login_entries()
                .await
                .map_err(ApiError::from)?
                .into_iter()
                .filter_map(|entry| to_public_login_entry(entry, &registry))
                .collect::<Vec<_>>();
            for entry in &mut login_entries {
                entry.title = crate::app_state::project_canonical_display_with(
                    &store,
                    bootstrap_workspace_id,
                    &input.locale,
                    "Password",
                    &entry.title,
                )
                .await?;
            }
            let default_login_entry_id = login_entries
                .first()
                .map(|entry| entry.id)
                .unwrap_or(domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID);
            Ok(PublicLoginEntriesOutput(PublicLoginEntriesResponse {
                default_login_entry_id,
                login_entries,
            }))
        })
    }
}

pub(crate) fn compile_public_login_entries_registry(
    port: Arc<dyn PublicLoginEntriesPort>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    login_entries_interface::compile_registry(port)
}

pub(crate) fn public_login_entries_port(
    store: MainDurableStore,
    authenticator_registry: Arc<AuthenticatorRegistry>,
    bootstrap_workspace_id: Uuid,
) -> Arc<dyn PublicLoginEntriesPort> {
    Arc::new(PublicLoginEntriesAdapter {
        store,
        authenticator_registry,
        bootstrap_workspace_id,
    })
}

fn public_login_entries_error(error: InterfaceInvocationError) -> ApiError {
    match error {
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<PublicLoginEntriesTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| anyhow::anyhow!("public login entries failed").into()),
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
    let boot_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("extension boot snapshot is unavailable"))?;
    let snapshot = boot_snapshot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry is unavailable"))?
        .snapshot();
    let binding_id = interface_runtime::BindingId::new(sign_in_interface::BINDING_ID)
        .expect("static binding id is valid");
    let activated = snapshot
        .authentication(&binding_id)
        .ok_or_else(|| anyhow::anyhow!("public authentication activation is unavailable"))?;
    let principal: PublicPrincipal = boot_snapshot
        .authenticate(
            activated,
            crate::extension_bus::PublicAuthenticationCredential,
        )
        .await?;
    let authentication_activation = activated.activation().clone();
    let outcome =
        InterfaceInvocationKernel::new(Arc::new(sign_in_interface::PublicSignInAuthorization))
            .invoke::<PublicSignInInput, PublicSignInOutput, PublicSignInTargetError>(
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
                    PublicSignInInput(LoginCommand {
                        login_entry_id: body
                            .login_entry_id
                            .unwrap_or(domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID),
                        identifier: body.identifier,
                        password: body.password,
                    }),
                ),
            )
            .await
            .map_err(|failure| public_sign_in_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    let result = outcome.into_value().0;

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

fn public_sign_in_error(error: InterfaceInvocationError) -> ApiError {
    match error {
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<PublicSignInTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| anyhow::anyhow!("public sign-in failed").into()),
        error => anyhow::anyhow!(error.to_string()).into(),
    }
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
    let result = invoke_public_residual::<PublicSignUpInput, PublicSignUpOutput>(
        &state,
        public_residual_interface::SIGN_UP_BINDING_ID,
        PublicSignUpInput(SignUpCommand {
            login_entry_id: body.login_entry_id,
            account: body.account,
            email: body.email,
            password: body.password,
        }),
    )
    .await?
    .0;

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

struct PublicSignUpAdapter {
    store: MainDurableStore,
    session_store: Arc<dyn SessionStore>,
    session_ttl_days: i64,
}

impl PublicSignUpPort for PublicSignUpAdapter {
    fn sign_up(&self, input: PublicSignUpInput) -> PublicSignUpFuture<'_> {
        let store = self.store.clone();
        let session_store = Arc::clone(&self.session_store);
        let session_ttl_days = self.session_ttl_days;
        Box::pin(async move {
            AuthKernel::new(store, SessionIssuer::new(session_store, session_ttl_days))
                .sign_up(input.0)
                .await
                .map(PublicSignUpOutput)
                .map_err(ApiError::from)
                .map_err(PublicResidualTargetError)
        })
    }
}

pub(crate) fn compile_public_residual_registry(
    providers: Arc<dyn PublicProvidersPort>,
    sign_up: Arc<dyn PublicSignUpPort>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    public_residual_interface::compile_registry(providers, sign_up)
}

pub(crate) fn public_providers_port(
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
) -> Arc<dyn PublicProvidersPort> {
    Arc::new(PublicProvidersAdapter {
        store,
        bootstrap_workspace_id,
    })
}

pub(crate) fn public_sign_up_port(
    store: MainDurableStore,
    session_store: Arc<dyn SessionStore>,
    session_ttl_days: i64,
) -> Arc<dyn PublicSignUpPort> {
    Arc::new(PublicSignUpAdapter {
        store,
        session_store,
        session_ttl_days,
    })
}

async fn invoke_public_residual<I, O>(
    state: &Arc<ApiState>,
    binding: &str,
    input: I,
) -> Result<O, ApiError>
where
    I: interface_runtime::InterfaceContract,
    O: interface_runtime::InterfaceContract,
{
    let boot_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("extension boot snapshot is unavailable"))?;
    let snapshot = boot_snapshot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry is unavailable"))?
        .snapshot();
    let binding_id = interface_runtime::BindingId::new(binding).expect("static binding is valid");
    let activated = snapshot
        .authentication(&binding_id)
        .ok_or_else(|| anyhow::anyhow!("public authentication activation is unavailable"))?;
    let principal: PublicPrincipal = boot_snapshot
        .authenticate(
            activated,
            crate::extension_bus::PublicAuthenticationCredential,
        )
        .await?;
    let authentication_activation = activated.activation().clone();
    let outcome = InterfaceInvocationKernel::new(Arc::new(
        public_residual_interface::PublicResidualAuthorization,
    ))
    .invoke::<I, O, PublicResidualTargetError>(
        snapshot,
        InvocationEnvelope::with_principal(
            InvocationLineage::root(InvocationId::now_v7()),
            binding_id,
            InterfaceProtocol::Http,
            interface_runtime::AuthenticationAdapterReference::new("api-server.public")
                .expect("static authentication adapter reference is valid"),
            authentication_activation,
            principal,
            None,
            input,
        ),
    )
    .await
    .map_err(|failure| match failure.into_error() {
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<PublicResidualTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| anyhow::anyhow!("public interface failed").into()),
        error => anyhow::anyhow!(error.to_string()).into(),
    })?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(outcome.into_value())
}
