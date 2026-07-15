use std::sync::Arc;

use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use axum_extra::extract::cookie::CookieJar;
use control_plane::profile::{ProfileService, UpdateMeCommand, UpdateMeMetaCommand};
use control_plane::session_security::{ChangeOwnPasswordCommand, SessionSecurityService};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_post, ConsoleRouteAssembly,
    },
    routes::session::expired_session_cookie,
};

#[derive(Debug, Serialize, ToSchema)]
pub struct MeResponse {
    pub id: String,
    pub account: String,
    pub email: String,
    pub phone: Option<String>,
    pub nickname: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub introduction: String,
    pub preferred_locale: Option<String>,
    pub meta: serde_json::Value,
    pub effective_display_role: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordBody {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum PreferredLocalePatch {
    Value(String),
    Null,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchMeBody {
    pub name: String,
    pub nickname: String,
    pub email: String,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
    pub introduction: String,
    pub preferred_locale: PreferredLocalePatch,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchMeMetaBody {
    pub meta: serde_json::Value,
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!("failed to hash password: {err}"))?
        .to_string())
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new()
        .route(
            "/me",
            console_get(get_me, Authenticated).patch(patch_me, Authenticated),
        )
        .route("/me/meta", console_patch(patch_me_meta, Authenticated))
        .route(
            "/me/actions/change-password",
            console_post(change_password, Authenticated),
        )
}

fn to_me_response(profile: control_plane::profile::MeProfile) -> MeResponse {
    let mut permissions = profile.actor.permissions.into_iter().collect::<Vec<_>>();
    permissions.sort();

    MeResponse {
        id: profile.user.id.to_string(),
        account: profile.user.account,
        email: profile.user.email,
        phone: profile.user.phone,
        nickname: profile.user.nickname,
        name: profile.user.name,
        avatar_url: profile.user.avatar_url,
        introduction: profile.user.introduction,
        preferred_locale: profile.user.preferred_locale,
        meta: profile.user.meta,
        effective_display_role: profile.actor.effective_display_role,
        permissions,
    }
}

#[utoipa::path(
    get,
    path = "/api/console/me",
    responses((status = 200, body = MeResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn get_me(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<MeResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let profile = ProfileService::new(state.store.clone())
        .get_me(
            context.user.id,
            context.actor.tenant_id,
            context.actor.current_workspace_id,
        )
        .await?;
    Ok(Json(ApiSuccess::new(to_me_response(profile))))
}

#[utoipa::path(
    patch,
    path = "/api/console/me",
    request_body = PatchMeBody,
    responses((status = 200, body = MeResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn patch_me(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PatchMeBody>,
) -> Result<Json<ApiSuccess<MeResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let profile = ProfileService::new(state.store.clone())
        .update_me(UpdateMeCommand {
            actor_user_id: context.user.id,
            tenant_id: context.actor.tenant_id,
            workspace_id: context.actor.current_workspace_id,
            name: body.name,
            nickname: body.nickname,
            email: body.email,
            phone: body.phone,
            avatar_url: body.avatar_url,
            introduction: body.introduction,
            preferred_locale: match body.preferred_locale {
                PreferredLocalePatch::Value(value) => Some(value),
                PreferredLocalePatch::Null => None,
            },
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_me_response(profile))))
}

#[utoipa::path(
    patch,
    path = "/api/console/me/meta",
    request_body = PatchMeMetaBody,
    responses((status = 200, body = MeResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn patch_me_meta(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PatchMeMetaBody>,
) -> Result<Json<ApiSuccess<MeResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let profile = ProfileService::new(state.store.clone())
        .update_me_meta(UpdateMeMetaCommand {
            actor_user_id: context.user.id,
            tenant_id: context.actor.tenant_id,
            workspace_id: context.actor.current_workspace_id,
            meta_patch: body.meta,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_me_response(profile))))
}

#[utoipa::path(
    post,
    path = "/api/console/me/actions/change-password",
    request_body = ChangePasswordBody,
    responses((status = 204), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn change_password(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordBody>,
) -> Result<(CookieJar, StatusCode), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let session = context.cookie_session()?;

    SessionSecurityService::new(state.store.clone(), state.session_store.clone())
        .change_own_password(ChangeOwnPasswordCommand {
            actor_user_id: context.user.id,
            session_id: session.session_id.clone(),
            old_password: body.old_password,
            new_password_hash: hash_password(&body.new_password)?,
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
