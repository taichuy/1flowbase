//! Host OAuth protocol boundary. Raw code, PKCE and browser/session cookies never
//! enter the canonical business input or managed plugin hook projections.
pub(crate) mod interface;
pub(crate) mod transaction;

use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
use axum::{
    extract::{RawQuery, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use control_plane::ports::{AuthRepository, ExternalIdentityLifecycleRepository};
use domain::{LoginEntryRecord, VerifiedExternalIdentity};
use extension_contracts::{AuthenticationRequest, AuthenticationResponse};
use interface::{PublicInput, PublicOutput};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use transaction::{BindingIntent, Transaction};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct SafeFailure(&'static str);

fn safe_failure(code: &'static str) -> ApiError {
    SafeFailure(code).into()
}

/// Finite UI vocabulary. Neither plugin reason text nor error strings can become
/// a query parameter; every redirect target below is a host-owned constant.
fn failure_redirect(error: &ApiError) -> &'static str {
    use control_plane::errors::ControlPlaneError as E;
    let safe = error.0.downcast_ref::<SafeFailure>().map(|error| error.0);
    let code = safe.or_else(|| match error.0.downcast_ref::<E>() {
        Some(E::PermissionDenied("self_registration_disabled")) => Some("registration_disabled"),
        Some(E::PermissionDenied("external_registration_ineligible")) => {
            Some("registration_ineligible")
        }
        Some(E::InvalidInput("external_registration_email_required")) => Some("email_required"),
        Some(E::Conflict(
            "external_identity_already_bound" | "external_registration_identity_exists",
        )) => Some("identity_exists"),
        Some(E::PermissionDenied("external_identity_unlinked")) => Some("identity_unlinked"),
        Some(E::PermissionDenied("external_auth_invalid_state")) => Some("invalid_state"),
        Some(E::PermissionDenied("login_entry_disabled")) => Some("entry_unavailable"),
        Some(E::PermissionDenied("user_disabled")) => Some("user_disabled"),
        Some(E::NotAuthenticated) => Some("session_expired"),
        Some(
            E::PluginUnavailable
            | E::NotFound("auth_provider")
            | E::PermissionDenied("authentication_provider_changed"),
        ) => Some("provider_unavailable"),
        _ => None,
    });
    match code {
        Some("registration_disabled") => "/sign-in?auth_error=registration_disabled",
        Some("account_too_new") => "/sign-in?auth_error=account_too_new",
        Some("email_required") => "/sign-in?auth_error=email_required",
        Some("identity_exists") => "/sign-in?auth_error=identity_exists",
        Some("identity_unlinked") => "/sign-in?auth_error=identity_unlinked",
        Some("registration_ineligible") => "/sign-in?auth_error=registration_ineligible",
        Some("cancelled") => "/sign-in?auth_error=cancelled",
        Some("invalid_state") => "/sign-in?auth_error=invalid_state",
        Some("entry_unavailable") => "/sign-in?auth_error=entry_unavailable",
        Some("provider_unavailable") => "/sign-in?auth_error=provider_unavailable",
        Some("session_expired") => "/sign-in?auth_error=session_expired",
        Some("user_disabled") => "/sign-in?auth_error=user_disabled",
        _ => "/sign-in?auth_error=external_auth_failed",
    }
}

fn security_failure_redirect(error: &ApiError) -> &'static str {
    match failure_redirect(error) {
        "/sign-in?auth_error=identity_exists" => "/me/security?auth_error=identity_exists",
        "/sign-in?auth_error=provider_unavailable" => {
            "/me/security?auth_error=provider_unavailable"
        }
        "/sign-in?auth_error=invalid_state" => "/me/security?auth_error=invalid_state",
        "/sign-in?auth_error=session_expired" => "/me/security?auth_error=session_expired",
        "/sign-in?auth_error=cancelled" => "/me/security?auth_error=cancelled",
        _ => "/me/security?auth_error=verification_failed",
    }
}

fn eligibility_failure(reason_code: Option<&str>) -> ApiError {
    safe_failure(match reason_code {
        Some("account_too_new" | "github_account_too_new") => "account_too_new",
        Some("email_required" | "github_email_required") => "email_required",
        Some("registration_disabled") => "registration_disabled",
        _ => "registration_ineligible",
    })
}

pub(crate) fn console_route_assembly(
) -> crate::routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>> {
    crate::routes::console_route_assembly::ConsoleRouteAssembly::new().route(
        "/me/external-identities/authorize",
        crate::routes::console_route_assembly::console_post(
            authorize,
            access_control::ConsoleRouteOwnership::Authenticated,
        ),
    )
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorizeBody {
    pub login_entry_id: Uuid,
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct AuthorizeResponse {
    pub authorization_url: String,
}

pub(super) fn validate_entry(entry: &LoginEntryRecord) -> Result<(), ApiError> {
    if !entry.enabled {
        return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
            "login_entry_disabled",
        )
        .into());
    }
    if entry.is_builtin
        || entry.auth_type == "password-local"
        || entry.connection_id == domain::PASSWORD_LOCAL_CONNECTION_ID
    {
        return Err(transaction::denied());
    }
    Ok(())
}

pub(crate) fn fingerprint<T: Serialize>(value: &T) -> Result<String, ApiError> {
    Ok(transaction::digest(&serde_json::to_string(value)?))
}

fn unique_query(raw: Option<&str>) -> Result<BTreeMap<String, String>, ApiError> {
    let mut query = BTreeMap::new();
    for (key, value) in url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        if query.insert(key.into_owned(), value.into_owned()).is_some() {
            return Err(transaction::denied());
        }
    }
    Ok(query)
}

fn start_entry_id(raw: Option<&str>) -> Result<Uuid, ApiError> {
    let query = unique_query(raw)?;
    if query.len() != 1 {
        return Err(transaction::denied());
    }
    query
        .get("login_entry_id")
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(transaction::denied)
}

fn no_store(response: &mut Response) {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        header::HeaderValue::from_static("no-referrer"),
    );
}

/// Start external sign-in.
///
/// Start a browser-bound external authentication transaction for an enabled login entry.
#[utoipa::path(get, path = "/api/public/auth/external/start", operation_id = "public.auth.external.start", params(("login_entry_id" = Uuid, Query)), responses((status = 303)))]
pub async fn start(State(state): State<Arc<ApiState>>, RawQuery(raw): RawQuery) -> Response {
    let result = async {
        let login_entry_id = start_entry_id(raw.as_deref())?;
        let PublicOutput::Authorization(prepared) = interface::invoke_public(
            &state,
            interface::START_BINDING,
            PublicInput::Start { login_entry_id },
        )
        .await?
        else {
            return Err(transaction::denied());
        };
        Ok::<_, ApiError>(
            (prepared.cookies, Redirect::to(&prepared.authorization_url)).into_response(),
        )
    }
    .await;
    let mut response =
        result.unwrap_or_else(|error| Redirect::to(failure_redirect(&error)).into_response());
    no_store(&mut response);
    response
}

/// Authorize an external identity binding.
///
/// Create an external authentication transaction bound to the current cookie session and return its authorization URL.
#[utoipa::path(post, path = "/api/console/me/external-identities/authorize", operation_id = "console.identity.external-authorize", request_body = AuthorizeBody, responses((status = 200, body = AuthorizeResponse), (status = 401), (status = 403)))]
pub async fn authorize(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<AuthorizeBody>,
) -> Result<Response, ApiError> {
    let prepared = interface::invoke_authorize(&state, headers, body.login_entry_id).await?;
    let mut response = (
        prepared.cookies,
        Json(ApiSuccess::new(AuthorizeResponse {
            authorization_url: prepared.authorization_url,
        })),
    )
        .into_response();
    no_store(&mut response);
    Ok(response)
}

fn authorization_url(
    value: String,
    state: &str,
    challenge: &str,
    callback_url: &str,
) -> Result<String, ApiError> {
    let url = url::Url::parse(&value).map_err(|_| transaction::denied())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(transaction::denied());
    }
    let query = unique_query(url.query())?;
    for (key, expected) in [
        ("state", state),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("redirect_uri", callback_url),
        ("response_type", "code"),
    ] {
        if query.get(key).map(String::as_str) != Some(expected) {
            return Err(transaction::denied());
        }
    }
    Ok(url.to_string())
}

async fn begin(
    state: &ApiState,
    entry: LoginEntryRecord,
    binding: Option<BindingIntent>,
) -> Result<(CookieJar, String), ApiError> {
    begin_for_purpose(state, entry, binding, transaction::Purpose::Bind).await
}

pub(crate) async fn begin_for_purpose(
    state: &ApiState,
    entry: LoginEntryRecord,
    binding: Option<BindingIntent>,
    purpose: transaction::Purpose,
) -> Result<(CookieJar, String), ApiError> {
    validate_entry(&entry)?;
    let provider = crate::authentication_runtime::provider(state, &entry.auth_type).await?;
    let callback_url = crate::authentication_runtime::callback_url()?;
    let connection = state
        .store
        .find_authentication_connection(entry.connection_id)
        .await?
        .ok_or_else(transaction::denied)?;
    if connection.auth_type != entry.auth_type {
        return Err(transaction::denied());
    }
    let oauth_state = transaction::random_token();
    let browser = transaction::random_token();
    let verifier = transaction::random_token();
    let challenge = transaction::challenge(&verifier);
    let transaction = Transaction {
        login_entry_id: entry.id,
        connection_id: entry.connection_id,
        auth_type: entry.auth_type.clone(),
        installation_id: provider.installation_id,
        contribution_id: provider.descriptor.contribution_id.as_str().to_owned(),
        provider_fingerprint: provider.fingerprint.clone(),
        entry_fingerprint: fingerprint(&entry)?,
        connection_fingerprint: fingerprint(&connection)?,
        callback_url: callback_url.clone(),
        browser_hash: transaction::digest(&browser),
        code_verifier: verifier,
        expires_at_unix: time::OffsetDateTime::now_utc().unix_timestamp()
            + transaction::TTL_SECONDS,
        binding,
        purpose,
    };
    let response = provider
        .invoke(
            state,
            &entry,
            AuthenticationRequest::Begin {
                callback_url: callback_url.clone(),
                state: oauth_state.clone(),
                code_challenge: challenge.clone(),
            },
        )
        .await?;
    let AuthenticationResponse::Authorization {
        authorization_url: value,
    } = response
    else {
        return Err(transaction::denied());
    };
    let value = authorization_url(value, &oauth_state, &challenge, &callback_url)?;
    transaction::store(
        state.infrastructure.cache_store().as_ref(),
        &oauth_state,
        &transaction,
    )
    .await?;
    let cookie = Cookie::build((transaction::browser_cookie_name(&oauth_state), browser))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.cookie_secure || callback_url.starts_with("https://"))
        .path("/api/public/auth/external/callback")
        .max_age(time::Duration::seconds(transaction::TTL_SECONDS))
        .build();
    Ok((CookieJar::new().add(cookie), value))
}

pub(super) struct VerifiedCallback {
    login_entry_id: Uuid,
    identity: VerifiedExternalIdentity,
    registration_eligible: bool,
    binding_actor: Option<domain::ActorContext>,
    authority: CompletionAuthority,
    purpose: transaction::Purpose,
    binding_session_hash: Option<String>,
}

/// The lease is Send; this one-shot Host holder also makes the canonical sealed
/// facts Sync without exposing the authority lock to any managed projection.
pub(super) struct CompletionAuthority(
    Mutex<Option<Box<dyn control_plane::ports::ContributionAuthorityLease>>>,
);
impl CompletionAuthority {
    fn new(lease: Box<dyn control_plane::ports::ContributionAuthorityLease>) -> Self {
        Self(Mutex::new(Some(lease)))
    }
    fn take(self) -> Result<Box<dyn control_plane::ports::ContributionAuthorityLease>, ApiError> {
        self.0
            .into_inner()
            .map_err(|_| transaction::denied())?
            .ok_or_else(transaction::denied)
    }
}

#[derive(Default)]
struct CallbackAdmissionState {
    verified: Option<VerifiedCallback>,
    cookie_name: Option<String>,
    security_return: bool,
}

/// Host-only one-shot side port, analogous to ConsoleProtocolAdmission. The
/// PublicPrincipal stays anonymous; it never fabricates a user for an unbound identity.
#[derive(Clone, Default)]
pub(super) struct CallbackAdmission(Arc<Mutex<CallbackAdmissionState>>);
impl CallbackAdmission {
    fn take(&self) -> Result<VerifiedCallback, ApiError> {
        self.0
            .lock()
            .map_err(|_| transaction::denied())?
            .verified
            .take()
            .ok_or_else(transaction::denied)
    }
    fn security_return(&self) -> bool {
        self.0.lock().map(|s| s.security_return).unwrap_or(false)
    }
    fn cookie_name(&self) -> Option<String> {
        self.0.lock().ok()?.cookie_name.take()
    }
}

/// Only the activated BuiltIn factory may consume the original OAuth carrier.
/// Neither this credential nor its side port is serializable or hook-projectable.
pub(crate) struct ExternalCallbackCredential {
    state: Arc<ApiState>,
    headers: HeaderMap,
    jar: CookieJar,
    raw_query: Option<String>,
    admission: CallbackAdmission,
}
impl ExternalCallbackCredential {
    pub(crate) async fn authenticate(self) -> Result<interface_runtime::PublicPrincipal, ApiError> {
        let query = unique_query(self.raw_query.as_deref())?;
        self.admission
            .0
            .lock()
            .map_err(|_| transaction::denied())?
            .cookie_name = query
            .get("state")
            .filter(|value| transaction::valid_token(value))
            .map(|value| transaction::browser_cookie_name(value));
        let verified = verify_callback(
            &self.state,
            &self.headers,
            &self.jar,
            &query,
            Some(&self.admission),
        )
        .await?;
        let mut admission = self.admission.0.lock().map_err(|_| transaction::denied())?;
        if admission.verified.is_some() {
            return Err(transaction::denied());
        }
        admission.verified = Some(verified);
        Ok(interface_runtime::PublicPrincipal::new())
    }
}

fn callback_code(query: &BTreeMap<String, String>) -> Result<&str, ApiError> {
    if let Some(error) = query.get("error") {
        return Err(safe_failure(
            if error == "access_denied" && !query.contains_key("code") {
                "cancelled"
            } else {
                "external_auth_failed"
            },
        ));
    }
    query
        .get("code")
        .filter(|code| !code.is_empty())
        .map(String::as_str)
        .ok_or_else(transaction::denied)
}

async fn frozen_context(
    state: &ApiState,
    transaction: &Transaction,
) -> Result<
    (
        LoginEntryRecord,
        crate::authentication_runtime::AuthenticationProvider,
    ),
    ApiError,
> {
    let entry = state
        .store
        .find_login_entry(transaction.login_entry_id)
        .await?
        .ok_or_else(transaction::denied)?;
    validate_entry(&entry)?;
    let connection = state
        .store
        .find_authentication_connection(entry.connection_id)
        .await?
        .ok_or_else(transaction::denied)?;
    let provider = crate::authentication_runtime::provider(state, &entry.auth_type).await?;
    if entry.connection_id != transaction.connection_id
        || entry.auth_type != transaction.auth_type
        || provider.installation_id != transaction.installation_id
        || provider.descriptor.contribution_id.as_str() != transaction.contribution_id
        || provider.fingerprint != transaction.provider_fingerprint
        || fingerprint(&entry)? != transaction.entry_fingerprint
        || fingerprint(&connection)? != transaction.connection_fingerprint
        || crate::authentication_runtime::callback_url()? != transaction.callback_url
    {
        return Err(transaction::denied());
    }
    Ok((entry, provider))
}

async fn bound_actor(
    state: &ApiState,
    headers: &HeaderMap,
    binding: Option<&BindingIntent>,
) -> Result<Option<domain::ActorContext>, ApiError> {
    if let Some(binding) = binding {
        let context = crate::middleware::require_session::require_session(state, headers).await?;
        if !transaction::binding_matches(
            binding,
            context.cookie_session()?,
            &context.actor,
            time::OffsetDateTime::now_utc().unix_timestamp(),
        ) {
            return Err(transaction::denied());
        }
        Ok(Some(context.actor))
    } else {
        Ok(None)
    }
}

/// Trusted Host credential verification. This function is the only callback owner
/// of the original authorization code and persisted PKCE verifier.
async fn verify_callback(
    state: &ApiState,
    headers: &HeaderMap,
    jar: &CookieJar,
    query: &BTreeMap<String, String>,
    admission: Option<&CallbackAdmission>,
) -> Result<VerifiedCallback, ApiError> {
    let oauth_state = query.get("state").ok_or_else(transaction::denied)?;
    let browser = jar
        .get(&transaction::browser_cookie_name(oauth_state))
        .ok_or_else(transaction::denied)?
        .value();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let transaction = transaction::claim(
        state.infrastructure.cache_store().as_ref(),
        oauth_state,
        browser,
        now,
    )
    .await?;
    if let Some(admission) = admission {
        admission
            .0
            .lock()
            .map_err(|_| transaction::denied())?
            .security_return = transaction.binding.is_some();
    }
    // Cancellation and malformed callbacks consume a browser-validated state too.
    let code = callback_code(query)?;
    let (entry, provider) = frozen_context(state, &transaction).await?;
    let binding_actor = bound_actor(state, headers, transaction.binding.as_ref()).await?;
    let verified = provider
        .invoke(
            state,
            &entry,
            AuthenticationRequest::Complete {
                callback_url: transaction.callback_url.clone(),
                code: code.to_owned(),
                code_verifier: transaction.code_verifier.clone(),
            },
        )
        .await?;
    let AuthenticationResponse::VerifiedIdentity {
        subject_type,
        subject_value,
        issuer,
        realm,
        profile,
    } = verified
    else {
        return Err(transaction::denied());
    };
    if issuer.trim().is_empty() || !profile.is_object() {
        return Err(transaction::denied());
    }
    let identity: VerifiedExternalIdentity = domain::ExternalIdentityClaim {
        connection_id: entry.connection_id,
        subject_type,
        subject_value,
        issuer: Some(issuer),
        realm,
        profile,
        verified: true,
        metadata: serde_json::json!({
            "installation_id": provider.installation_id,
            "contribution_id": provider.descriptor.contribution_id.as_str(),
        }),
    }
    .try_into()
    .map_err(|_| transaction::denied())?;
    let identity = identity
        .with_authority_namespace(provider.authority_namespace.clone())
        .map_err(|_| transaction::denied())?;

    // Existing bindings bypass registration policy; the kernel rechecks entry,
    // identity binding and registration authorization before issuing a session.
    let registration_enabled = entry
        .options
        .get("extension_config")
        .and_then(|v| v.get("self_registration_enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let registration_eligible = if binding_actor.is_none()
        && registration_enabled
        && state
            .store
            .find_external_identity_binding(&identity)
            .await?
            .is_none()
    {
        match provider
            .invoke(
                state,
                &entry,
                AuthenticationRequest::RegistrationEligibility {
                    profile: identity.profile().clone(),
                    now_unix: now,
                },
            )
            .await?
        {
            AuthenticationResponse::RegistrationEligibility { allowed: true, .. } => true,
            AuthenticationResponse::RegistrationEligibility {
                allowed: false,
                reason_code,
            } => return Err(eligibility_failure(reason_code.as_deref())),
            _ => return Err(transaction::denied()),
        }
    } else {
        false
    };
    // No call may silently move to new entry/config/provider authority while the
    // external verifier is in flight. Recheck the original binding session too.
    frozen_context(state, &transaction).await?;
    let binding_actor = bound_actor(state, headers, transaction.binding.as_ref()).await?;
    // Last live authority check happens after every worker call. Keep the same
    // lock as grant/revoke until the Host business effect has completed.
    let authority = CompletionAuthority::new(provider.authorize_completion(state, &entry).await?);
    Ok(VerifiedCallback {
        login_entry_id: entry.id,
        identity,
        registration_eligible,
        binding_actor,
        authority,
        purpose: transaction.purpose,
        binding_session_hash: transaction
            .binding
            .as_ref()
            .map(|binding| binding.session_hash.clone()),
    })
}

/// Complete external sign-in.
///
/// Verify the one-time browser transaction and complete sign-in or explicit identity binding.
#[utoipa::path(get, path = "/api/public/auth/external/callback", operation_id = "public.auth.external.callback", responses((status = 303)))]
pub async fn callback(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    jar: CookieJar,
    RawQuery(raw): RawQuery,
) -> Response {
    let (result, cookie_name, security_return) =
        interface::invoke_callback(&state, headers, jar, raw).await;
    let mut cookies = CookieJar::new();
    if let Some(name) = cookie_name {
        cookies = cookies.remove(
            Cookie::build((name, ""))
                .http_only(true)
                .same_site(SameSite::Lax)
                .secure(state.cookie_secure)
                .path("/api/public/auth/external/callback")
                .build(),
        );
    }
    let target = match result {
        Ok(PublicOutput::Login(result)) => {
            cookies = cookies.add(
                Cookie::build((state.cookie_name.clone(), result.session.session_id))
                    .http_only(true)
                    .same_site(SameSite::Lax)
                    .secure(state.cookie_secure)
                    .path("/")
                    .build(),
            );
            "/"
        }
        Ok(PublicOutput::Bound) => "/me/security?auth_bound=true",
        Ok(PublicOutput::PasswordVerified) => {
            "/me/security?auth_verified=true&security_tab=password"
        }
        Err(error) if security_return => security_failure_redirect(&error),
        Err(error) => failure_redirect(&error),
        _ => "/sign-in?auth_error=external_auth_failed",
    };
    let mut response = (cookies, Redirect::to(target)).into_response();
    no_store(&mut response);
    response
}

#[cfg(test)]
#[path = "_tests/protocol_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "_tests/authentication_tests.rs"]
mod authentication_tests;

#[cfg(test)]
#[path = "_tests/authority_tests.rs"]
pub(crate) mod authority_tests;
