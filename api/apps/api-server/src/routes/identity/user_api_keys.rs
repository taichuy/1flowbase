use std::{collections::BTreeSet, sync::Arc};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::auth::{
    ApiKeyService, CreateUserApiKeyCommand, ListUserApiKeysCommand, RevokeUserApiKeyCommand,
    UserApiKeyExpirationPolicy,
};
use control_plane::ports::RoleRepository;
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserApiKeyRequest {
    pub name: String,
    pub role_code: Option<String>,
    pub expiration_policy: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserApiKeyRoleOptionResponse {
    pub code: String,
    pub name: String,
    pub scope_kind: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserApiKeyRoleOptionsResponse {
    pub items: Vec<UserApiKeyRoleOptionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserApiKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub token: Option<String>,
    pub token_prefix: String,
    pub key_kind: String,
    pub role_code: Option<String>,
    pub creator_user_id: Uuid,
    pub tenant_id: Uuid,
    pub scope_kind: String,
    pub scope_id: Uuid,
    pub enabled: bool,
    pub revoked: bool,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserApiKeyListResponse {
    pub items: Vec<UserApiKeyResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RevokeUserApiKeyResponse {
    pub id: Uuid,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/user-api-keys",
            console_get(
                list_user_api_keys,
                ConsoleOperation("user_api_keys.manage".to_string()),
            )
            .post(
                create_user_api_key,
                ConsoleOperation("user_api_keys.manage".to_string()),
            ),
        )
        .route(
            "/user-api-keys/role-options",
            console_get(
                list_user_api_key_role_options,
                ConsoleOperation("user_api_keys.manage".to_string()),
            ),
        )
        .route(
            "/user-api-keys/:api_key_id/revoke",
            console_post(
                revoke_user_api_key,
                ConsoleOperation("user_api_keys.manage".to_string()),
            ),
        )
}

fn is_role_bound_to_current_workspace(
    role: &domain::BoundRole,
    current_workspace_id: Uuid,
) -> bool {
    match role.scope_kind {
        domain::RoleScopeKind::System => true,
        domain::RoleScopeKind::Workspace => role.workspace_id == Some(current_workspace_id),
    }
}

fn role_scope_kind_text(role: &domain::BoundRole) -> String {
    match role.scope_kind {
        domain::RoleScopeKind::System => "system".to_string(),
        domain::RoleScopeKind::Workspace => "workspace".to_string(),
    }
}

fn parse_expiration_policy(raw: &str) -> Result<UserApiKeyExpirationPolicy, ApiError> {
    match raw {
        "30d" => Ok(UserApiKeyExpirationPolicy::ThirtyDays),
        "1y" => Ok(UserApiKeyExpirationPolicy::OneYear),
        "3y" => Ok(UserApiKeyExpirationPolicy::ThreeYears),
        "never" => Ok(UserApiKeyExpirationPolicy::Never),
        _ => {
            Err(control_plane::errors::ControlPlaneError::InvalidInput("expiration_policy").into())
        }
    }
}

fn format_time(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("RFC3339 formatting should support OffsetDateTime")
}

fn format_optional_time(value: Option<OffsetDateTime>) -> Option<String> {
    value.map(format_time)
}

fn user_api_key_response(
    api_key: domain::ApiKeyRecord,
    token: Option<String>,
) -> UserApiKeyResponse {
    UserApiKeyResponse {
        id: api_key.id,
        name: api_key.name,
        token,
        token_prefix: api_key.token_prefix,
        key_kind: api_key.key_kind.as_str().to_string(),
        role_code: api_key.role_code,
        creator_user_id: api_key.creator_user_id,
        tenant_id: api_key.tenant_id,
        scope_kind: api_key.scope_kind.as_str().to_string(),
        scope_id: api_key.scope_id,
        enabled: api_key.enabled,
        revoked: !api_key.enabled,
        expires_at: format_optional_time(api_key.expires_at),
        last_used_at: format_optional_time(api_key.last_used_at),
        created_at: format_time(api_key.created_at),
        updated_at: format_time(api_key.updated_at),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/user-api-keys",
    responses((status = 200, body = UserApiKeyListResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_user_api_keys(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<UserApiKeyListResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let items = ApiKeyService::new(state.store.clone())
        .list_user_api_keys(ListUserApiKeysCommand {
            actor_user_id: context.actor.user_id,
            tenant_id: context.actor.tenant_id,
            current_workspace_id: context.actor.current_workspace_id,
        })
        .await?
        .into_iter()
        .map(|api_key| user_api_key_response(api_key, None))
        .collect();

    Ok(Json(ApiSuccess::new(UserApiKeyListResponse { items })))
}

#[utoipa::path(
    get,
    path = "/api/console/user-api-keys/role-options",
    responses((status = 200, body = UserApiKeyRoleOptionsResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_user_api_key_role_options(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<UserApiKeyRoleOptionsResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_roles = state
        .store
        .list_roles(context.actor.current_workspace_id)
        .await?;
    let role_names = workspace_roles
        .into_iter()
        .map(|role| (role.code, role.name))
        .collect::<std::collections::HashMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut items = context
        .user
        .roles
        .iter()
        .filter(|role| is_role_bound_to_current_workspace(role, context.actor.current_workspace_id))
        .filter(|role| seen.insert(role.code.clone()))
        .map(|role| UserApiKeyRoleOptionResponse {
            code: role.code.clone(),
            name: role_names
                .get(&role.code)
                .cloned()
                .unwrap_or_else(|| role.code.clone()),
            scope_kind: role_scope_kind_text(role),
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.code.cmp(&right.code));

    Ok(Json(ApiSuccess::new(UserApiKeyRoleOptionsResponse {
        items,
    })))
}

#[utoipa::path(
    post,
    path = "/api/console/user-api-keys",
    request_body = CreateUserApiKeyRequest,
    responses((status = 201, body = UserApiKeyResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_user_api_key(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(payload): Json<CreateUserApiKeyRequest>,
) -> Result<(StatusCode, Json<ApiSuccess<UserApiKeyResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let role_code = payload
        .role_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(context.actor.effective_display_role.as_str())
        .to_string();
    let role_allowed = context.user.roles.iter().any(|role| {
        role.code == role_code
            && is_role_bound_to_current_workspace(role, context.actor.current_workspace_id)
    });
    if !role_allowed {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("role_code").into());
    }
    let result = ApiKeyService::new(state.store.clone())
        .create_user_api_key(CreateUserApiKeyCommand {
            actor_user_id: context.actor.user_id,
            tenant_id: context.actor.tenant_id,
            current_workspace_id: context.actor.current_workspace_id,
            name: payload.name,
            role_code,
            expiration_policy: parse_expiration_policy(&payload.expiration_policy)?,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(user_api_key_response(
            result.api_key,
            Some(result.token),
        ))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/user-api-keys/{api_key_id}/revoke",
    params(("api_key_id" = Uuid, Path, description = "User API key id")),
    responses((status = 200, body = RevokeUserApiKeyResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn revoke_user_api_key(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(api_key_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<RevokeUserApiKeyResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    ApiKeyService::new(state.store.clone())
        .revoke_user_api_key(RevokeUserApiKeyCommand {
            actor_user_id: context.actor.user_id,
            tenant_id: context.actor.tenant_id,
            current_workspace_id: context.actor.current_workspace_id,
            api_key_id,
        })
        .await?;

    Ok(Json(ApiSuccess::new(RevokeUserApiKeyResponse {
        id: api_key_id,
    })))
}
