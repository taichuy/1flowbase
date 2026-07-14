use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use control_plane::session_security::{
    LogoutCurrentSessionCommand, RevokeAllSessionsCommand, SessionSecurityService,
};
use control_plane::workspace_session::{SwitchWorkspaceCommand, WorkspaceSessionService};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionResponse {
    pub actor: serde_json::Value,
    pub session: serde_json::Value,
    pub csrf_token: String,
    pub cookie_name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SwitchWorkspaceBody {
    pub workspace_id: String,
}

fn parse_workspace_id(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("workspace_id").into())
}

fn to_session_response(
    account: &str,
    actor: &domain::ActorContext,
    session: &domain::SessionRecord,
    cookie_name: &str,
) -> SessionResponse {
    SessionResponse {
        actor: serde_json::json!({
            "id": actor.user_id,
            "account": account,
            "effective_display_role": actor.effective_display_role,
            "current_workspace_id": actor.current_workspace_id,
        }),
        session: serde_json::json!({
            "id": session.session_id,
            "user_id": session.user_id,
            "tenant_id": session.tenant_id,
            "current_workspace_id": session.current_workspace_id,
        }),
        csrf_token: session.csrf_token.clone(),
        cookie_name: cookie_name.to_string(),
    }
}

pub(crate) fn expired_session_cookie(cookie_name: &str, cookie_secure: bool) -> Cookie<'static> {
    Cookie::build((cookie_name.to_string(), String::new()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(cookie_secure)
        .path("/")
        .build()
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new()
        .route(
            "/session",
            console_get(get_session, Authenticated).delete(delete_session, Authenticated),
        )
        .route(
            "/session/actions/revoke-all",
            console_post(revoke_all_sessions, Authenticated),
        )
        .route(
            "/session/actions/switch-workspace",
            console_post(switch_workspace, Authenticated),
        )
}

#[utoipa::path(
    get,
    path = "/api/console/session",
    responses((status = 200, body = SessionResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn get_session(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<SessionResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let session = context.cookie_session()?;

    Ok(Json(ApiSuccess::new(to_session_response(
        &context.user.account,
        &context.actor,
        session,
        &state.cookie_name,
    ))))
}

#[utoipa::path(
    delete,
    path = "/api/console/session",
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn delete_session(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<(CookieJar, StatusCode), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let session = context.cookie_session()?;

    SessionSecurityService::new(state.store.clone(), state.session_store.clone())
        .logout_current_session(LogoutCurrentSessionCommand {
            session_id: session.session_id.clone(),
        })
        .await?;

    Ok((
        CookieJar::new().remove(expired_session_cookie(
            &state.cookie_name,
            state.cookie_secure,
        )),
        StatusCode::NO_CONTENT,
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/session/actions/revoke-all",
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn revoke_all_sessions(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<(CookieJar, StatusCode), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let session = context.cookie_session()?;

    SessionSecurityService::new(state.store.clone(), state.session_store.clone())
        .revoke_all_sessions(RevokeAllSessionsCommand {
            actor_user_id: context.user.id,
            session_id: session.session_id.clone(),
        })
        .await?;

    Ok((
        CookieJar::new().remove(expired_session_cookie(
            &state.cookie_name,
            state.cookie_secure,
        )),
        StatusCode::NO_CONTENT,
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/session/actions/switch-workspace",
    request_body = SwitchWorkspaceBody,
    responses(
        (status = 200, body = SessionResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn switch_workspace(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<SwitchWorkspaceBody>,
) -> Result<Json<ApiSuccess<SessionResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let session = context.cookie_session()?;
    let workspace_id = parse_workspace_id(&body.workspace_id)?;

    let result = WorkspaceSessionService::new(
        state.store.clone(),
        state.store.clone(),
        state.session_store.clone(),
    )
    .switch_workspace(SwitchWorkspaceCommand {
        actor_user_id: context.user.id,
        session_id: session.session_id.clone(),
        target_workspace_id: workspace_id,
    })
    .await?;

    Ok(Json(ApiSuccess::new(to_session_response(
        &context.user.account,
        &result.actor,
        &result.session,
        &state.cookie_name,
    ))))
}
