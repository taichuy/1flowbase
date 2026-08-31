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
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
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

pub(crate) fn hash_password(password: &str) -> Result<String, ApiError> {
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

pub(crate) fn to_me_response(
    profile: control_plane::profile::MeProfile,
    actor: &domain::ActorContext,
) -> MeResponse {
    let mut permissions = actor.permissions.iter().cloned().collect::<Vec<_>>();
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
        effective_display_role: actor.effective_display_role.clone(),
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
    let output = super::console_identity_interface::invoke(
        Arc::clone(&state),
        "http.console.identity.me.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        super::console_identity_interface::ConsoleIdentityInput::GetMe,
    )
    .await?;
    let super::console_identity_interface::ConsoleIdentityOutput::Me(response) = output else {
        return Err(anyhow::anyhow!("console me output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(response)))
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
    let output = super::console_identity_interface::invoke(
        Arc::clone(&state),
        "http.console.identity.me.patch.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        super::console_identity_interface::ConsoleIdentityInput::PatchMe(body),
    )
    .await?;
    let super::console_identity_interface::ConsoleIdentityOutput::Me(response) = output else {
        return Err(anyhow::anyhow!("console me output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(response)))
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
    let output = super::console_identity_interface::invoke(
        Arc::clone(&state),
        "http.console.identity.me.meta.patch.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        super::console_identity_interface::ConsoleIdentityInput::PatchMeMeta(body),
    )
    .await?;
    let super::console_identity_interface::ConsoleIdentityOutput::Me(response) = output else {
        return Err(anyhow::anyhow!("console me output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(response)))
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
    super::console_identity_interface::invoke(
        Arc::clone(&state),
        "http.console.identity.me.change-password.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        super::console_identity_interface::ConsoleIdentityInput::ChangePassword(body),
    )
    .await?;

    Ok((
        CookieJar::new().remove(expired_session_cookie(
            &state.cookie_name,
            state.cookie_secure,
        )),
        StatusCode::NO_CONTENT,
    ))
}
