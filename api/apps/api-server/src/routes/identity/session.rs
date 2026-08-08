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
use control_plane::workspace_session::{
    SwitchActiveRoleCommand, SwitchWorkspaceCommand, WorkspaceSessionService,
};
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
    pub available_roles: Vec<AvailableRoleResponse>,
    pub active_role_permissions: Vec<String>,
    pub csrf_token: String,
    pub cookie_name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AvailableRoleResponse {
    pub code: String,
    pub name: String,
    pub scope_kind: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SwitchWorkspaceBody {
    pub workspace_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SwitchActiveRoleBody {
    pub role_code: String,
}

fn parse_workspace_id(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("workspace_id").into())
}

fn to_session_response(
    user: &domain::UserRecord,
    actor: &domain::ActorContext,
    session: &domain::SessionRecord,
    cookie_name: &str,
) -> SessionResponse {
    let mut available_roles = user
        .roles
        .iter()
        .filter(|role| {
            role.scope_kind == domain::RoleScopeKind::System
                || role.workspace_id == Some(session.current_workspace_id)
        })
        .map(|role| AvailableRoleResponse {
            code: role.code.clone(),
            name: role.name.clone(),
            scope_kind: match role.scope_kind {
                domain::RoleScopeKind::System => "system",
                domain::RoleScopeKind::Workspace => "workspace",
            }
            .to_string(),
        })
        .collect::<Vec<_>>();
    available_roles.dedup_by(|left, right| left.code == right.code);
    SessionResponse {
        actor: serde_json::json!({
            "id": actor.user_id,
            "account": user.account,
            "effective_display_role": actor.effective_display_role,
            "current_workspace_id": actor.current_workspace_id,
        }),
        session: serde_json::json!({
            "id": session.session_id,
            "user_id": session.user_id,
            "tenant_id": session.tenant_id,
            "current_workspace_id": session.current_workspace_id,
            "active_role_code": session.active_role_code,
        }),
        available_roles,
        active_role_permissions: actor.permissions.iter().cloned().collect(),
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
        .route(
            "/session/actions/switch-role",
            console_post(switch_active_role, Authenticated),
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
        &context.user,
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
        &context.user,
        &result.actor,
        &result.session,
        &state.cookie_name,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/session/actions/switch-role",
    request_body = SwitchActiveRoleBody,
    responses(
        (status = 200, body = SessionResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn switch_active_role(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<SwitchActiveRoleBody>,
) -> Result<Json<ApiSuccess<SessionResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let session = context.cookie_session()?;
    let role_code = body.role_code.trim();
    if role_code.is_empty() {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("role_code").into());
    }

    let result = WorkspaceSessionService::new(
        state.store.clone(),
        state.store.clone(),
        state.session_store.clone(),
    )
    .switch_active_role(SwitchActiveRoleCommand {
        actor_user_id: context.user.id,
        session_id: session.session_id.clone(),
        active_role_code: role_code.to_string(),
    })
    .await?;

    Ok(Json(ApiSuccess::new(to_session_response(
        &context.user,
        &result.actor,
        &result.session,
        &state.cookie_name,
    ))))
}
