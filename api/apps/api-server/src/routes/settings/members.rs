use std::sync::Arc;

use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::member::AssignableRoleOption;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_post, console_put, ConsoleRouteAssembly,
    },
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMemberBody {
    pub account: String,
    pub email: String,
    pub phone: Option<String>,
    pub password: String,
    pub name: String,
    pub nickname: String,
    pub introduction: String,
    pub email_login_enabled: bool,
    pub phone_login_enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMemberBody {
    pub email: String,
    pub phone: Option<String>,
    pub name: String,
    pub nickname: String,
    pub introduction: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetMemberPasswordBody {
    pub new_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceMemberRolesBody {
    pub role_codes: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MemberRoleOptionResponse {
    pub code: String,
    pub name: String,
}

impl From<AssignableRoleOption> for MemberRoleOptionResponse {
    fn from(option: AssignableRoleOption) -> Self {
        Self {
            code: option.code,
            name: option.name,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MemberResponse {
    pub id: String,
    pub account: String,
    pub email: String,
    pub phone: Option<String>,
    pub name: String,
    pub nickname: String,
    pub introduction: String,
    pub default_display_role: Option<String>,
    pub email_login_enabled: bool,
    pub phone_login_enabled: bool,
    pub status: String,
    pub role_codes: Vec<String>,
}

pub(crate) fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!("failed to hash password: {err}"))?
        .to_string())
}

pub(crate) fn to_member_response(user: domain::UserRecord) -> MemberResponse {
    let resolved_display_role = user.resolved_display_role();
    let domain::UserRecord {
        id,
        account,
        email,
        phone,
        name,
        nickname,
        introduction,
        email_login_enabled,
        phone_login_enabled,
        status,
        roles,
        ..
    } = user;
    let mut role_codes = roles.into_iter().map(|role| role.code).collect::<Vec<_>>();
    role_codes.sort();
    role_codes.dedup();

    MemberResponse {
        id: id.to_string(),
        account,
        email,
        phone,
        name,
        nickname,
        introduction,
        default_display_role: resolved_display_role,
        email_login_enabled,
        phone_login_enabled,
        status: match status {
            domain::UserStatus::Active => "active".to_string(),
            domain::UserStatus::Disabled => "disabled".to_string(),
        },
        role_codes,
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/members",
            console_get(list_members, ConsoleOperation("members.list".to_string())).post(
                create_member,
                ConsoleOperation("members.create".to_string()),
            ),
        )
        .route(
            "/settings/members/role-options",
            console_get(
                list_member_role_options,
                ConsoleOperation("members.role_options.list".to_string()),
            ),
        )
        .route(
            "/settings/members/:id",
            console_patch(
                update_member,
                ConsoleOperation("members.update".to_string()),
            )
            .delete(
                delete_member,
                ConsoleOperation("members.delete".to_string()),
            ),
        )
        .route(
            "/settings/members/:id/disable",
            console_post(
                disable_member,
                ConsoleOperation("members.disable".to_string()),
            ),
        )
        .route(
            "/settings/members/:id/enable",
            console_post(
                enable_member,
                ConsoleOperation("members.enable".to_string()),
            ),
        )
        .route(
            "/settings/members/:id/reset-password",
            console_post(
                reset_member,
                ConsoleOperation("members.password.reset".to_string()),
            ),
        )
        .route(
            "/settings/members/:id/roles",
            console_put(
                replace_member_roles,
                ConsoleOperation("members.roles.replace".to_string()),
            ),
        )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/members/role-options",
    responses((status = 200, body = [MemberRoleOptionResponse]), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_member_role_options(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<MemberRoleOptionResponse>>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.members.role-options.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::ListMemberRoleOptions,
    )
    .await?;
    let crate::routes::membership_interface::MembershipOutput::MemberRoleOptions(items) = output
    else {
        return Err(anyhow::anyhow!("member role options output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(items)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/members",
    responses((status = 200, body = [MemberResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_members(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<MemberResponse>>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.members.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::ListMembers,
    )
    .await?;
    let crate::routes::membership_interface::MembershipOutput::Members(items) = output else {
        return Err(anyhow::anyhow!("members output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(items)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/members",
    request_body = CreateMemberBody,
    responses((status = 201, body = MemberResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateMemberBody>,
) -> Result<(StatusCode, Json<ApiSuccess<MemberResponse>>), ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.members.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::CreateMember(body),
    )
    .await?;
    let crate::routes::membership_interface::MembershipOutput::Member(member) = output else {
        return Err(anyhow::anyhow!("member create output contract mismatch").into());
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(member))))
}

#[utoipa::path(
    patch,
    path = "/api/console/settings/members/{id}",
    request_body = UpdateMemberBody,
    params(("id" = String, Path, description = "Member user id")),
    responses((status = 200, body = MemberResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn update_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(member_id): Path<String>,
    Json(body): Json<UpdateMemberBody>,
) -> Result<Json<ApiSuccess<MemberResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.members.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::UpdateMember { member_id, body },
    )
    .await?;
    let crate::routes::membership_interface::MembershipOutput::Member(member) = output else {
        return Err(anyhow::anyhow!("member update output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(member)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/members/{id}/disable",
    params(("id" = String, Path, description = "Member user id")),
    responses((status = 204), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn disable_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(member_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::membership_interface::MembershipOutput,
    >(
        Arc::clone(&state),
        "http.console.members.disable.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::DisableMember { member_id },
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/console/settings/members/{id}/enable",
    params(("id" = String, Path, description = "Member user id")),
    responses((status = 204), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn enable_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(member_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::membership_interface::MembershipOutput,
    >(
        Arc::clone(&state),
        "http.console.members.enable.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::EnableMember { member_id },
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/members/{id}",
    params(("id" = String, Path, description = "Member user id")),
    responses((status = 204), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn delete_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(member_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::membership_interface::MembershipOutput,
    >(
        Arc::clone(&state),
        "http.console.members.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::DeleteMember { member_id },
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/console/settings/members/{id}/reset-password",
    request_body = ResetMemberPasswordBody,
    params(("id" = String, Path, description = "Member user id")),
    responses((status = 204), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn reset_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(member_id): Path<String>,
    Json(body): Json<ResetMemberPasswordBody>,
) -> Result<StatusCode, ApiError> {
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::membership_interface::MembershipOutput,
    >(
        Arc::clone(&state),
        "http.console.members.password.reset.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::ResetMember { member_id, body },
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/api/console/settings/members/{id}/roles",
    request_body = ReplaceMemberRolesBody,
    params(("id" = String, Path, description = "Member user id")),
    responses((status = 204), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn replace_member_roles(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(member_id): Path<String>,
    Json(body): Json<ReplaceMemberRolesBody>,
) -> Result<StatusCode, ApiError> {
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::membership_interface::MembershipOutput,
    >(
        Arc::clone(&state),
        "http.console.members.roles.replace.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::ReplaceMemberRoles {
            member_id,
            body,
        },
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
