use control_plane::ports::AuthRepository;
use std::future::Future;

use axum::http::HeaderMap;
use control_plane::auth::ApiKeyService;
use control_plane::ports::SessionStore;
use domain::{ActorContext, SessionRecord, UserRecord, UserStatus};
use uuid::Uuid;

use crate::{app_state::ApiState, error_response::ApiError};

#[derive(Clone)]
pub enum RequestCredential {
    CookieSession(SessionRecord),
    UserApiKey { api_key_id: Uuid },
    ServerDelegation,
}

#[derive(Clone)]
pub struct RequestContext {
    pub credential: RequestCredential,
    pub user: UserRecord,
    pub actor: ActorContext,
}

impl RequestContext {
    pub fn server_delegation(user: UserRecord, actor: ActorContext) -> Self {
        Self {
            credential: RequestCredential::ServerDelegation,
            user,
            actor,
        }
    }

    pub fn cookie_session(
        &self,
    ) -> Result<&SessionRecord, control_plane::errors::ControlPlaneError> {
        match &self.credential {
            RequestCredential::CookieSession(session) => Ok(session),
            RequestCredential::UserApiKey { .. } => {
                Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                    "cookie_session_required",
                ))
            }
            RequestCredential::ServerDelegation => {
                Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                    "cookie_session_required",
                ))
            }
        }
    }

    pub fn csrf_required(&self) -> bool {
        matches!(self.credential, RequestCredential::CookieSession(_))
    }

    pub fn credential_kind(&self) -> &'static str {
        match &self.credential {
            RequestCredential::CookieSession(_) => "session",
            RequestCredential::UserApiKey { .. } => "user_api_key",
            RequestCredential::ServerDelegation => "server_delegation",
        }
    }

    pub fn interface_principal(&self) -> interface_runtime::UserPrincipal {
        if let RequestCredential::CookieSession(session) = &self.credential {
            return interface_runtime::UserPrincipal::with_authenticated_session(
                self.actor.clone(),
                interface_runtime::AuthenticatedSessionIdentity::new(session.session_id.clone())
                    .expect("authenticated session identity is non-empty"),
            );
        }
        let credential_kind = match &self.credential {
            RequestCredential::CookieSession(_) => unreachable!("cookie session returned above"),
            RequestCredential::UserApiKey { api_key_id } => {
                interface_runtime::UserCredentialKind::UserApiKey {
                    api_key_id: *api_key_id,
                }
            }
            RequestCredential::ServerDelegation => {
                interface_runtime::UserCredentialKind::ServerDelegation
            }
        };
        interface_runtime::UserPrincipal::new(self.actor.clone(), credential_kind)
    }
}

tokio::task_local! {
    static SERVER_DELEGATED_REQUEST_CONTEXT: RequestContext;
}

/// Trusted in-process delegation only; request headers cannot populate this context.
pub fn server_delegated_request_context() -> Option<RequestContext> {
    SERVER_DELEGATED_REQUEST_CONTEXT.try_with(Clone::clone).ok()
}

pub async fn with_server_delegated_request_context<F>(
    context: RequestContext,
    future: F,
) -> F::Output
where
    F: Future,
{
    SERVER_DELEGATED_REQUEST_CONTEXT
        .scope(context, future)
        .await
}

fn extract_cookie(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == cookie_name).then(|| value.to_string())
    })
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    raw.strip_prefix("Bearer ")
}

struct IdentityRequestSnapshot {
    user: UserRecord,
    session: Option<SessionRecord>,
}

tokio::task_local! {
    static IDENTITY_REQUEST_CONTEXT: std::cell::OnceCell<IdentityRequestSnapshot>;
}

/// Reuse the validated user and session only within this identity invocation.
pub async fn with_identity_request_context<F: Future>(future: F) -> F::Output {
    IDENTITY_REQUEST_CONTEXT
        .scope(std::cell::OnceCell::new(), future)
        .await
}

pub fn identity_request_user(user_id: Uuid) -> Option<UserRecord> {
    IDENTITY_REQUEST_CONTEXT
        .try_with(|snapshot| {
            snapshot
                .get()
                .filter(|entry| entry.user.id == user_id)
                .map(|entry| entry.user.clone())
        })
        .ok()
        .flatten()
}

pub fn identity_request_session(session_id: &str) -> Option<SessionRecord> {
    IDENTITY_REQUEST_CONTEXT
        .try_with(|snapshot| {
            snapshot
                .get()?
                .session
                .as_ref()
                .filter(|session| session.session_id == session_id)
                .cloned()
        })
        .ok()
        .flatten()
}

pub async fn require_session(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<RequestContext, ApiError> {
    let context = authenticate_request(state, headers).await?;
    let _ = IDENTITY_REQUEST_CONTEXT.try_with(|snapshot| {
        snapshot.get_or_init(|| IdentityRequestSnapshot {
            user: context.user.clone(),
            session: context.cookie_session().ok().cloned(),
        });
    });
    Ok(context)
}

async fn authenticate_request(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<RequestContext, ApiError> {
    if let Ok(context) = SERVER_DELEGATED_REQUEST_CONTEXT.try_with(Clone::clone) {
        state
            .store
            .verify_workspace_access(
                context.actor.user_id,
                context.actor.tenant_id,
                context.actor.current_workspace_id,
                context.actor.is_root,
            )
            .await?;
        return apply_console_policy_permissions(state, context).await;
    }

    if let Some(token) = extract_bearer_token(headers) {
        let mut user_api_key = ApiKeyService::new(state.store.clone())
            .authenticate_console_api_key(token)
            .await?;
        state
            .console_policy_projector
            .project(&state.store, &mut user_api_key.actor)
            .await?;
        return Ok(RequestContext {
            credential: RequestCredential::UserApiKey {
                api_key_id: user_api_key.api_key.id,
            },
            user: user_api_key.user,
            actor: user_api_key.actor,
        });
    }

    let session_id = extract_cookie(headers, &state.cookie_name)
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;
    let session = state
        .session_store
        .get(&session_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;
    let user = state
        .store
        .find_user_by_id(session.user_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;

    if user.session_version != session.session_version
        || matches!(user.status, UserStatus::Disabled)
        || !user.has_effective_role_in_workspace(
            session.current_workspace_id,
            &session.active_role_code,
        )
    {
        state.session_store.delete(&session.session_id).await?;
        return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
    }

    let mut actor = state
        .store
        .resolve_loaded_user_role(
            &user,
            session.tenant_id,
            session.current_workspace_id,
            Some(&session.active_role_code),
        )
        .await?;

    state
        .console_policy_projector
        .project(&state.store, &mut actor)
        .await?;
    Ok(RequestContext {
        credential: RequestCredential::CookieSession(session),
        user,
        actor,
    })
}

/// Navigation, design-mode visibility and the existing frontstage service guard are
/// projections of current canonical policies, never persisted legacy feature grants.
async fn apply_console_policy_permissions(
    state: &ApiState,
    mut context: RequestContext,
) -> Result<RequestContext, ApiError> {
    apply_console_policy_permissions_to_actor(state, &mut context.actor).await?;
    Ok(context)
}

pub(crate) async fn apply_console_policy_permissions_to_actor(
    state: &ApiState,
    actor: &mut ActorContext,
) -> Result<(), ApiError> {
    Ok(state
        .console_policy_projector
        .project_existing(actor)
        .await?)
}
