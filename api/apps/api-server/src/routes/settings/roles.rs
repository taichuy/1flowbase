use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch},
    Json, Router,
};
use control_plane::ports::{RoleDataModelPolicyInput, RoleDataPolicyDefaultsInput};
use control_plane::role::{
    CreateRoleCommand, DeleteRoleCommand, ReplaceRoleDataPolicyCommand,
    ReplaceRoleFrontstageRoutesCommand, ReplaceRolePermissionsCommand, RoleService,
    UpdateRoleCommand,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRoleBody {
    pub code: String,
    pub name: String,
    pub introduction: String,
    pub auto_grant_new_permissions: Option<bool>,
    pub is_default_member_role: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRoleBody {
    pub name: String,
    pub introduction: String,
    pub auto_grant_new_permissions: Option<bool>,
    pub is_default_member_role: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceRolePermissionsBody {
    pub permission_codes: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceRoleDataPolicyBody {
    pub default_policy: RoleDataPolicyBody,
    pub model_policies: Vec<RoleDataModelPolicyBody>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct RoleDataPolicyBody {
    pub can_view: bool,
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub default_view_scope: String,
    pub default_update_scope: String,
    pub default_delete_scope: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct RoleDataModelPolicyBody {
    pub data_model_id: Uuid,
    pub can_create_override: Option<bool>,
    pub view_scope_override: Option<String>,
    pub update_scope_override: Option<String>,
    pub delete_scope_override: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleResponse {
    pub code: String,
    pub name: String,
    pub introduction: String,
    pub scope_kind: String,
    pub is_builtin: bool,
    pub is_editable: bool,
    pub auto_grant_new_permissions: bool,
    pub is_default_member_role: bool,
    pub permission_codes: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RolePermissionsResponse {
    pub role_code: String,
    pub permission_codes: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceRoleFrontstageRoutesBody {
    pub page_ids: Vec<Uuid>,
    pub tab_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleFrontstageRouteNodeResponse {
    pub id: Uuid,
    pub kind: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub children: Vec<RoleFrontstageRouteNodeResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleFrontstageRoutesResponse {
    pub role_code: String,
    pub checked_page_ids: Vec<Uuid>,
    pub checked_tab_ids: Vec<Uuid>,
    pub tree: Vec<RoleFrontstageRouteNodeResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleDataPolicyResponse {
    pub role_code: String,
    pub default_policy: RoleDataPolicyBody,
    pub model_policies: Vec<RoleDataModelPolicyBody>,
}

fn to_role_response(role: domain::RoleTemplate) -> RoleResponse {
    RoleResponse {
        code: role.code,
        name: role.name,
        introduction: role.introduction,
        scope_kind: match role.scope_kind {
            domain::RoleScopeKind::System => "system".to_string(),
            domain::RoleScopeKind::Workspace => "workspace".to_string(),
        },
        is_builtin: role.is_builtin,
        is_editable: role.is_editable,
        auto_grant_new_permissions: role.auto_grant_new_permissions,
        is_default_member_role: role.is_default_member_role,
        permission_codes: role.permissions,
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/settings/roles", get(list_roles).post(create_role))
        .route(
            "/settings/roles/:id",
            patch(update_role).delete(delete_role),
        )
        .route(
            "/settings/roles/:id/permissions",
            get(get_role_permissions).put(replace_role_permissions),
        )
        .route(
            "/settings/roles/:id/frontstage-routes",
            get(get_role_frontstage_routes).put(replace_role_frontstage_routes),
        )
        .route(
            "/settings/roles/:id/data-policy",
            get(get_role_data_policy).put(replace_role_data_policy),
        )
}

fn parse_data_policy_scope(value: &str) -> Result<domain::RoleDataPolicyScope, ApiError> {
    domain::RoleDataPolicyScope::parse(value)
        .ok_or(control_plane::errors::ControlPlaneError::InvalidInput("data_policy_scope").into())
        .map_err(ApiError)
}

fn parse_optional_data_policy_scope(
    value: Option<String>,
) -> Result<Option<domain::RoleDataPolicyScope>, ApiError> {
    value.as_deref().map(parse_data_policy_scope).transpose()
}

fn to_default_policy_input(
    policy: RoleDataPolicyBody,
) -> Result<RoleDataPolicyDefaultsInput, ApiError> {
    Ok(RoleDataPolicyDefaultsInput {
        can_view: policy.can_view,
        can_create: policy.can_create,
        can_update: policy.can_update,
        can_delete: policy.can_delete,
        default_view_scope: parse_data_policy_scope(&policy.default_view_scope)?,
        default_update_scope: parse_data_policy_scope(&policy.default_update_scope)?,
        default_delete_scope: parse_data_policy_scope(&policy.default_delete_scope)?,
    })
}

fn to_model_policy_input(
    policy: RoleDataModelPolicyBody,
) -> Result<RoleDataModelPolicyInput, ApiError> {
    Ok(RoleDataModelPolicyInput {
        data_model_id: policy.data_model_id,
        can_create_override: policy.can_create_override,
        view_scope_override: parse_optional_data_policy_scope(policy.view_scope_override)?,
        update_scope_override: parse_optional_data_policy_scope(policy.update_scope_override)?,
        delete_scope_override: parse_optional_data_policy_scope(policy.delete_scope_override)?,
    })
}

fn to_role_data_policy_response(
    policy: control_plane::ports::RoleDataPolicyView,
) -> RoleDataPolicyResponse {
    RoleDataPolicyResponse {
        role_code: policy.role_code,
        default_policy: RoleDataPolicyBody {
            can_view: policy.default_policy.can_view,
            can_create: policy.default_policy.can_create,
            can_update: policy.default_policy.can_update,
            can_delete: policy.default_policy.can_delete,
            default_view_scope: policy
                .default_policy
                .default_view_scope
                .as_str()
                .to_string(),
            default_update_scope: policy
                .default_policy
                .default_update_scope
                .as_str()
                .to_string(),
            default_delete_scope: policy
                .default_policy
                .default_delete_scope
                .as_str()
                .to_string(),
        },
        model_policies: policy
            .model_policies
            .into_iter()
            .map(|model_policy| RoleDataModelPolicyBody {
                data_model_id: model_policy.data_model_id,
                can_create_override: model_policy.can_create_override,
                view_scope_override: model_policy
                    .view_scope_override
                    .map(|scope| scope.as_str().to_string()),
                update_scope_override: model_policy
                    .update_scope_override
                    .map(|scope| scope.as_str().to_string()),
                delete_scope_override: model_policy
                    .delete_scope_override
                    .map(|scope| scope.as_str().to_string()),
            })
            .collect(),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/settings/roles",
    responses((status = 200, body = [RoleResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_roles(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<RoleResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let roles = RoleService::new(state.store.clone())
        .list_roles(context.user.id)
        .await?;

    Ok(Json(ApiSuccess::new(
        roles.into_iter().map(to_role_response).collect::<Vec<_>>(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/roles",
    request_body = CreateRoleBody,
    responses((status = 201, body = RoleResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_role(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateRoleBody>,
) -> Result<(StatusCode, Json<ApiSuccess<RoleResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    RoleService::new(state.store.clone())
        .create_role(CreateRoleCommand {
            actor_user_id: context.user.id,
            code: body.code.clone(),
            name: body.name.clone(),
            introduction: body.introduction.clone(),
            auto_grant_new_permissions: body.auto_grant_new_permissions.unwrap_or(false),
            is_default_member_role: body.is_default_member_role.unwrap_or(false),
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(RoleResponse {
            code: body.code,
            name: body.name,
            introduction: body.introduction,
            scope_kind: "workspace".to_string(),
            is_builtin: false,
            is_editable: true,
            auto_grant_new_permissions: body.auto_grant_new_permissions.unwrap_or(false),
            is_default_member_role: body.is_default_member_role.unwrap_or(false),
            permission_codes: Vec::new(),
        })),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/console/settings/roles/{id}",
    request_body = UpdateRoleBody,
    params(("id" = String, Path, description = "Role code")),
    responses((status = 204), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn update_role(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
    Json(body): Json<UpdateRoleBody>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    RoleService::new(state.store.clone())
        .update_role(UpdateRoleCommand {
            actor_user_id: context.user.id,
            role_code,
            name: body.name,
            introduction: body.introduction,
            auto_grant_new_permissions: body.auto_grant_new_permissions,
            is_default_member_role: body.is_default_member_role,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/roles/{id}",
    params(("id" = String, Path, description = "Role code")),
    responses((status = 204), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn delete_role(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    RoleService::new(state.store.clone())
        .delete_role(DeleteRoleCommand {
            actor_user_id: context.user.id,
            role_code,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/console/settings/roles/{id}/permissions",
    params(("id" = String, Path, description = "Role code")),
    responses((status = 200, body = RolePermissionsResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_role_permissions(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
) -> Result<Json<ApiSuccess<RolePermissionsResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let permission_codes = RoleService::new(state.store.clone())
        .get_role_permissions(context.user.id, &role_code)
        .await?;

    Ok(Json(ApiSuccess::new(RolePermissionsResponse {
        role_code,
        permission_codes,
    })))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/roles/{id}/permissions",
    request_body = ReplaceRolePermissionsBody,
    params(("id" = String, Path, description = "Role code")),
    responses((status = 204), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn replace_role_permissions(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
    Json(body): Json<ReplaceRolePermissionsBody>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    RoleService::new(state.store.clone())
        .replace_permissions(ReplaceRolePermissionsCommand {
            actor_user_id: context.user.id,
            role_code,
            permission_codes: body.permission_codes,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn build_frontstage_route_tree(
    pages: Vec<domain::FrontstagePageRecord>,
    tabs: Vec<domain::frontstage::FrontstagePageTabRecord>,
) -> Vec<RoleFrontstageRouteNodeResponse> {
    use std::collections::HashMap;
    fn build(
        parent: Option<Uuid>,
        pages: &HashMap<Option<Uuid>, Vec<domain::FrontstagePageRecord>>,
        tabs: &HashMap<Uuid, Vec<domain::frontstage::FrontstagePageTabRecord>>,
    ) -> Vec<RoleFrontstageRouteNodeResponse> {
        pages
            .get(&parent)
            .into_iter()
            .flatten()
            .filter(|page| {
                parent.is_some()
                    || page.placement == domain::frontstage::FrontstageNavigationPlacement::Topbar
            })
            .map(|page| {
                let mut children = build(Some(page.id), pages, tabs);
                if page.kind == domain::FrontstagePageKind::Page {
                    children.extend(tabs.get(&page.id).into_iter().flatten().map(|tab| {
                        RoleFrontstageRouteNodeResponse {
                            id: tab.id,
                            kind: "tab".into(),
                            title: tab.title.clone(),
                            slug: None,
                            children: vec![],
                        }
                    }));
                }
                RoleFrontstageRouteNodeResponse {
                    id: page.id,
                    kind: page.kind.as_str().into(),
                    title: page.title.clone(),
                    slug: page.slug.clone(),
                    children,
                }
            })
            .collect()
    }
    let mut by_parent: HashMap<Option<Uuid>, Vec<_>> = HashMap::new();
    for page in pages {
        by_parent.entry(page.parent_id).or_default().push(page);
    }
    for nodes in by_parent.values_mut() {
        nodes.sort_by(|a, b| a.rank.cmp(&b.rank).then(a.id.cmp(&b.id)));
    }
    let mut by_page: HashMap<Uuid, Vec<_>> = HashMap::new();
    for tab in tabs {
        by_page.entry(tab.page_id).or_default().push(tab);
    }
    for nodes in by_page.values_mut() {
        nodes.sort_by(|a, b| a.rank.cmp(&b.rank).then(a.id.cmp(&b.id)));
    }
    build(None, &by_parent, &by_page)
}

pub async fn get_role_frontstage_routes(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
) -> Result<Json<ApiSuccess<RoleFrontstageRoutesResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let view = RoleService::new(state.store.clone())
        .get_frontstage_routes(context.user.id, &role_code)
        .await?;
    Ok(Json(ApiSuccess::new(RoleFrontstageRoutesResponse {
        role_code,
        checked_page_ids: view.rules.iter().filter_map(|rule| rule.page_id).collect(),
        checked_tab_ids: view.rules.iter().filter_map(|rule| rule.tab_id).collect(),
        tree: build_frontstage_route_tree(view.pages, view.tabs),
    })))
}

pub async fn replace_role_frontstage_routes(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
    Json(body): Json<ReplaceRoleFrontstageRoutesBody>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    RoleService::new(state.store.clone())
        .replace_frontstage_routes(ReplaceRoleFrontstageRoutesCommand {
            actor_user_id: context.user.id,
            role_code,
            page_ids: body.page_ids,
            tab_ids: body.tab_ids,
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/console/settings/roles/{id}/data-policy",
    params(("id" = String, Path, description = "Role code")),
    responses((status = 200, body = RoleDataPolicyResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_role_data_policy(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
) -> Result<Json<ApiSuccess<RoleDataPolicyResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let policy = RoleService::new(state.store.clone())
        .get_role_data_policy(context.user.id, &role_code)
        .await?;

    Ok(Json(ApiSuccess::new(to_role_data_policy_response(policy))))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/roles/{id}/data-policy",
    request_body = ReplaceRoleDataPolicyBody,
    params(("id" = String, Path, description = "Role code")),
    responses((status = 200, body = RoleDataPolicyResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn replace_role_data_policy(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
    Json(body): Json<ReplaceRoleDataPolicyBody>,
) -> Result<Json<ApiSuccess<RoleDataPolicyResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let default_policy = to_default_policy_input(body.default_policy)?;
    let model_policies = body
        .model_policies
        .into_iter()
        .map(to_model_policy_input)
        .collect::<Result<Vec<_>, _>>()?;
    let policy = RoleService::new(state.store.clone())
        .replace_data_policy(ReplaceRoleDataPolicyCommand {
            actor_user_id: context.user.id,
            role_code,
            default_policy,
            model_policies,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_role_data_policy_response(policy))))
}
