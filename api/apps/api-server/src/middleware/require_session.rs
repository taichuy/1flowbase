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
}

tokio::task_local! {
    static SERVER_DELEGATED_REQUEST_CONTEXT: RequestContext;
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

pub async fn require_session(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<RequestContext, ApiError> {
    if let Ok(context) = SERVER_DELEGATED_REQUEST_CONTEXT.try_with(Clone::clone) {
        return Ok(context);
    }

    if let Some(token) = extract_bearer_token(headers) {
        let user_api_key = ApiKeyService::new(state.store.clone())
            .authenticate_user_api_key(token)
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
    {
        state.session_store.delete(&session.session_id).await?;
        return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
    }

    let actor = state
        .store
        .load_actor_context(
            user.id,
            session.tenant_id,
            session.current_workspace_id,
            Some(&session.active_role_code),
        )
        .await?;
    if actor.effective_display_role != session.active_role_code {
        state.session_store.delete(&session.session_id).await?;
        return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
    }

    Ok(RequestContext {
        credential: RequestCredential::CookieSession(session),
        user,
        actor,
    })
}
