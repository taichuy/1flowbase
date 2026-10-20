//! Authenticated self-service methods. Business state comes from the Control Plane.
mod dto;
pub(crate) mod interface;
mod proof;
use crate::routes::external_auth::transaction::{self, BindingIntent, Purpose};
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::cookie::CookieJar;
use control_plane::{
    auth::AuthenticationMethodsService,
    errors::ControlPlaneError as Error,
    ports::{AuthRepository, ExternalIdentityLifecycleRepository, PluginRepository},
};
pub use dto::*;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

pub(crate) fn route_assembly(
) -> crate::routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>> {
    use crate::routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly};
    use access_control::ConsoleRouteOwnership::Authenticated;
    ConsoleRouteAssembly::new()
        .route(
            "/me/authentication-methods",
            console_get(get_methods, Authenticated),
        )
        .route(
            "/me/authentication-methods/external-identities/:identity_id/unlink",
            console_post(unlink, Authenticated),
        )
        .route(
            "/me/authentication-methods/password/authorize",
            console_post(authorize_password, Authenticated),
        )
        .route(
            "/me/authentication-methods/password",
            console_post(initialize_password, Authenticated),
        )
}
fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}
async fn invoke(
    state: &Arc<ApiState>,
    headers: HeaderMap,
    index: usize,
    input: interface::Input,
) -> Result<interface::Output, ApiError> {
    let credential = if index == 0 {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: state.clone(),
            headers,
        }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: state.clone(),
            headers,
        }
    };
    crate::routes::console_interface::invoke(
        state.clone(),
        interface::BINDING_IDS[index],
        credential,
        input,
    )
    .await
}
#[utoipa::path(get,path="/api/console/me/authentication-methods",responses((status=200,body=AuthenticationMethodsResponse),(status=401)))]
pub async fn get_methods(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let interface::Output::Methods(data) =
        invoke(&state, headers, 0, interface::Input::Read).await?
    else {
        return Err(Error::InvalidInput("authentication_methods_output").into());
    };
    Ok(no_store(Json(ApiSuccess::new(data)).into_response()))
}
#[utoipa::path(post,path="/api/console/me/authentication-methods/external-identities/{identity_id}/unlink",params(("identity_id"=Uuid,Path)),responses((status=204),(status=401),(status=403),(status=409)))]
pub async fn unlink(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(identity_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    invoke(&state, headers, 1, interface::Input::Unlink(identity_id)).await?;
    Ok(no_store(StatusCode::NO_CONTENT.into_response()))
}
#[utoipa::path(post,path="/api/console/me/authentication-methods/password/authorize",request_body=crate::routes::external_auth::AuthorizeBody,responses((status=200,body=crate::routes::external_auth::AuthorizeResponse),(status=401),(status=403)))]
pub async fn authorize_password(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<crate::routes::external_auth::AuthorizeBody>,
) -> Result<Response, ApiError> {
    let interface::Output::Authorization(cookies, url) = invoke(
        &state,
        headers,
        2,
        interface::Input::AuthorizePassword(body.login_entry_id),
    )
    .await?
    else {
        return Err(Error::InvalidInput("authentication_methods_output").into());
    };
    Ok(no_store(
        (
            cookies,
            Json(ApiSuccess::new(
                crate::routes::external_auth::AuthorizeResponse {
                    authorization_url: url,
                },
            )),
        )
            .into_response(),
    ))
}
#[utoipa::path(post,path="/api/console/me/authentication-methods/password",request_body=InitializePasswordBody,responses((status=204),(status=401),(status=403),(status=409)))]
pub async fn initialize_password(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InitializePasswordBody>,
) -> Result<Response, ApiError> {
    if body.new_password.chars().count() < 8 || body.new_password.len() > 1024 {
        return Err(Error::InvalidInput("new_password").into());
    }
    invoke(
        &state,
        headers,
        3,
        interface::Input::InitializePassword(body.new_password),
    )
    .await?;
    Ok(no_store(
        (
            CookieJar::new().add(crate::routes::session::expired_session_cookie(
                &state.cookie_name,
                state.cookie_secure,
            )),
            StatusCode::NO_CONTENT,
        )
            .into_response(),
    ))
}

struct AvailableMethods {
    providers: Vec<crate::authentication_runtime::AuthenticationProvider>,
    entries: Vec<domain::LoginEntryRecord>,
    authorities: Vec<String>,
    leases: Vec<Box<dyn control_plane::ports::ContributionAuthorityLease>>,
}
async fn available(
    state: &super::external_auth::ExternalAuthentication,
) -> Result<AvailableMethods, ApiError> {
    let entries = state.runtime.store.list_login_entries().await?;
    let providers = crate::authentication_runtime::active_providers(&state.runtime).await?;
    let (authorities, lease) = crate::authentication_runtime::authorize_available_providers(
        &state.runtime,
        &providers,
        &entries,
    )
    .await?;
    let leases = lease.into_iter().collect();
    Ok(AvailableMethods {
        providers,
        entries,
        authorities,
        leases,
    })
}
async fn release(
    leases: Vec<Box<dyn control_plane::ports::ContributionAuthorityLease>>,
) -> Result<(), ApiError> {
    for lease in leases {
        lease.release().await?;
    }
    Ok(())
}
fn profile_text(profile: &serde_json::Value, key: &str, max: usize) -> Option<String> {
    profile
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= max && !s.chars().any(char::is_control))
        .map(str::to_owned)
}
fn profile_url(profile: &serde_json::Value, key: &str) -> Option<String> {
    let value = profile_text(profile, key, 2048)?;
    let url = url::Url::parse(&value).ok()?;
    (url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(value)
}
fn time_text(value: time::OffsetDateTime) -> Result<String, ApiError> {
    Ok(value.format(&time::format_description::well_known::Rfc3339)?)
}
async fn read_methods(
    state: &super::external_auth::ExternalAuthentication,
    actor: &domain::ActorContext,
    session_id: &str,
) -> Result<AuthenticationMethodsResponse, ApiError> {
    let available = available(state).await?;
    let methods = AuthenticationMethodsService::new(state.runtime.store.clone())
        .read(actor.user_id, &available.authorities)
        .await?;
    let mut groups = BTreeMap::<String, ProviderResponse>::new();
    for provider in &available.providers {
        let entries = available
            .entries
            .iter()
            .filter(|entry| entry.enabled && entry.auth_type == provider.descriptor.auth_type)
            .map(|entry| LoginEntrySummary {
                id: entry.id,
                title: entry.title.clone(),
            })
            .collect();
        groups.insert(
            provider.descriptor.auth_type.clone(),
            ProviderResponse {
                auth_type: provider.descriptor.auth_type.clone(),
                display_name: provider.descriptor.display_name.clone(),
                provider_available: available
                    .authorities
                    .contains(&provider.authority_namespace),
                login_entries: entries,
                bindings: Vec::new(),
            },
        );
    }
    let installations = state.runtime.store.list_installations().await?;
    for binding in methods.bindings {
        let name = installations
            .iter()
            .find_map(|install| {
                if format!(
                    "managed:{}/{}:{}",
                    install.organization, install.provider_code, binding.auth_type
                ) != binding.authority_namespace
                {
                    return None;
                }
                install
                    .metadata_json
                    .get("authentication_providers")?
                    .as_array()?
                    .iter()
                    .find(|value| value["auth_type"].as_str() == Some(&binding.auth_type))
                    .and_then(|value| value["display_name"].as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "External account".into());
        let group = groups
            .entry(binding.auth_type.clone())
            .or_insert_with(|| ProviderResponse {
                auth_type: binding.auth_type.clone(),
                display_name: name,
                provider_available: false,
                login_entries: Vec::new(),
                bindings: Vec::new(),
            });
        group.bindings.push(BindingResponse {
            identity_id: binding.identity_id,
            state: binding.state,
            subject_type: binding.subject_type,
            subject_value: binding.subject_value,
            issuer: binding.issuer,
            account: profile_text(&binding.profile, "account", 256),
            nickname: profile_text(&binding.profile, "nickname", 512),
            avatar_url: profile_url(&binding.profile, "avatar_url"),
            profile_url: profile_url(&binding.profile, "profile_url"),
            bound_at: time_text(binding.bound_at)?,
            unlinked_at: binding.unlinked_at.map(time_text).transpose()?,
            provider_available: binding.provider_available,
            can_unlink: binding.can_unlink,
            unlink_blocked_reason: binding.unlink_blocked_reason,
        });
    }
    let mut verification = Vec::new();
    for group in groups.values() {
        if group.provider_available
            && group
                .bindings
                .iter()
                .any(|b| b.state == "bound" && b.provider_available)
        {
            verification.extend(group.login_entries.clone());
        }
    }
    let password_setup_verified = proof::read(state.cache.as_ref(), actor.user_id, session_id)
        .await?
        .is_some();
    release(available.leases).await?;
    Ok(AuthenticationMethodsResponse {
        password_configured: methods.password_configured,
        password_usable: methods.password_usable,
        password_setup_verified,
        password_verification_entries: verification,
        providers: groups.into_values().collect(),
    })
}

pub(crate) async fn issue_password_proof(
    state: &super::external_auth::ExternalAuthentication,
    actor: &domain::ActorContext,
    session_hash: String,
    login_entry_id: Uuid,
    identity: &domain::VerifiedExternalIdentity,
) -> Result<(), ApiError> {
    let binding = state
        .runtime
        .store
        .find_external_identity_binding(identity)
        .await?
        .ok_or(Error::PermissionDenied("password_setup_identity_required"))?;
    if binding.user_id != actor.user_id || binding.state != "bound" {
        return Err(Error::PermissionDenied("password_setup_identity_required").into());
    }
    let entry = state
        .runtime
        .store
        .find_login_entry(login_entry_id)
        .await?
        .ok_or(Error::NotFound("login_entry"))?;
    let connection = state
        .runtime
        .store
        .find_authentication_connection(entry.connection_id)
        .await?
        .ok_or(Error::NotFound("authentication_connection"))?;
    let provider =
        crate::authentication_runtime::provider(&state.runtime, &entry.auth_type).await?;
    proof::issue(
        state.cache.as_ref(),
        proof::PasswordSetupProof {
            actor_id: actor.user_id,
            session_hash,
            identity_id: binding.identity_id,
            login_entry_id,
            provider_fingerprint: provider.fingerprint,
            authority_namespace: provider.authority_namespace,
            entry_fingerprint: super::external_auth::fingerprint(&entry)?,
            connection_fingerprint: super::external_auth::fingerprint(&connection)?,
            nonce: transaction::random_token(),
            expires_at: proof::expires_at(),
        },
    )
    .await
}

#[cfg(test)]
#[path = "_tests/api.rs"]
mod tests;
