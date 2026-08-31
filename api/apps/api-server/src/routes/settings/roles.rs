use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::ports::{RoleDataModelPolicyInput, RoleDataPolicyDefaultsInput};
use control_plane::role::{ConsolePolicyGroupInput, ConsolePolicyOperationInput};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_put, ConsoleRouteAssembly,
    },
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

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleDataModelOptionResponse {
    pub id: Uuid,
    pub code: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConsolePolicyGroupKindBody {
    SettingsFeature,
    Other,
}

impl ConsolePolicyGroupKindBody {
    fn as_str(self) -> &'static str {
        match self {
            Self::SettingsFeature => "settings_feature",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConsolePolicyStrategyBody {
    Full,
    Custom,
}

impl ConsolePolicyStrategyBody {
    fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConsolePolicyRowScopeBody {
    Disabled,
    Own,
    ScopeAll,
}

impl ConsolePolicyRowScopeBody {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Own => "own",
            Self::ScopeAll => "scope_all",
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceRoleConsolePolicyBody {
    pub groups: Vec<ConsoleRoleConsolePolicyGroupBody>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConsoleRoleConsolePolicyGroupBody {
    pub kind: ConsolePolicyGroupKindBody,
    pub group_id: String,
    pub enabled: bool,
    pub strategy: ConsolePolicyStrategyBody,
    pub operations: Vec<ConsoleRoleConsolePolicyOperationBody>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsoleRoleConsolePolicyOperationBody {
    Simple {
        operation_id: String,
        enabled: bool,
    },
    Row {
        operation_id: String,
        scope: ConsolePolicyRowScopeBody,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsolePolicyCatalogResponse {
    pub schema_version: String,
    pub locale: String,
    pub settings_order_revision: i64,
    pub group_strategy_options: Vec<ConsolePolicyCatalogOptionResponse>,
    pub groups: Vec<ConsolePolicyCatalogGroupResponse>,
    pub resources: Vec<ConsolePolicyCatalogResourceResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceConsoleSettingsOrderBody {
    pub expected_revision: i64,
    pub group_ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsolePolicyCatalogOptionResponse {
    pub value: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsolePolicyCatalogGroupResponse {
    pub kind: ConsolePolicyGroupKindBody,
    pub group_id: String,
    pub label: String,
    pub description: String,
    pub operations: Vec<ConsolePolicyCatalogOperationResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsolePolicyCatalogOperationResponse {
    pub operation_id: String,
    pub summary: String,
    pub description: String,
    pub order: i32,
    pub route: ConsolePolicyCatalogRouteResponse,
    pub full_profile: ConsolePolicyCatalogOperationFullProfileResponse,
    pub allowed_row_scopes: Vec<ConsolePolicyCatalogOptionResponse>,
    pub authorization: ConsolePolicyOperationAuthorizationResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsolePolicyCatalogRouteResponse {
    pub method: String,
    pub path: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsolePolicyCatalogOperationFullProfileResponse {
    Simple { enabled: bool },
    Row { scope: ConsolePolicyRowScopeBody },
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsolePolicyOperationAuthorizationResponse {
    Simple,
    ResourceAction {
        resource_code: String,
        action_code: String,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsolePolicyCatalogResourceResponse {
    pub resource_code: String,
    pub label: String,
    pub description: String,
    pub actions: Vec<ConsolePolicyCatalogResourceActionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsolePolicyCatalogResourceActionResponse {
    pub action_code: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleConsolePolicyResponse {
    pub role_code: String,
    pub groups: Vec<RoleConsolePolicyGroupResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleConsolePolicyGroupResponse {
    pub kind: ConsolePolicyGroupKindBody,
    pub group_id: String,
    pub enabled: bool,
    pub strategy: ConsolePolicyStrategyBody,
    pub operations: Vec<RoleConsolePolicyOperationResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RoleConsolePolicyOperationResponse {
    Simple {
        operation_id: String,
        enabled: bool,
    },
    Row {
        operation_id: String,
        scope: ConsolePolicyRowScopeBody,
    },
}

pub(crate) fn to_role_response(role: domain::RoleTemplate) -> RoleResponse {
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

fn to_console_policy_group_kind_body(
    kind: domain::ConsolePolicyGroupKind,
) -> ConsolePolicyGroupKindBody {
    match kind {
        domain::ConsolePolicyGroupKind::SettingsFeature => {
            ConsolePolicyGroupKindBody::SettingsFeature
        }
        domain::ConsolePolicyGroupKind::Other => ConsolePolicyGroupKindBody::Other,
    }
}

fn to_console_policy_strategy_body(
    strategy: domain::ConsolePolicyStrategy,
) -> ConsolePolicyStrategyBody {
    match strategy {
        domain::ConsolePolicyStrategy::Full => ConsolePolicyStrategyBody::Full,
        domain::ConsolePolicyStrategy::Custom => ConsolePolicyStrategyBody::Custom,
    }
}

fn to_console_policy_row_scope_body(
    scope: domain::ConsoleOperationRowScope,
) -> ConsolePolicyRowScopeBody {
    match scope {
        domain::ConsoleOperationRowScope::Disabled => ConsolePolicyRowScopeBody::Disabled,
        domain::ConsoleOperationRowScope::Own => ConsolePolicyRowScopeBody::Own,
        domain::ConsoleOperationRowScope::ScopeAll => ConsolePolicyRowScopeBody::ScopeAll,
    }
}

pub(crate) fn to_console_policy_catalog_response(
    catalog: control_plane::role::ConsolePolicyCatalog,
) -> ConsolePolicyCatalogResponse {
    ConsolePolicyCatalogResponse {
        schema_version: catalog.schema_version,
        locale: catalog.locale,
        settings_order_revision: catalog.settings_order_revision,
        group_strategy_options: catalog
            .group_strategy_options
            .into_iter()
            .map(|option| ConsolePolicyCatalogOptionResponse {
                value: option.value,
                label: option.label,
                description: option.description,
            })
            .collect(),
        groups: catalog
            .groups
            .into_iter()
            .map(|group| ConsolePolicyCatalogGroupResponse {
                kind: to_console_policy_group_kind_body(group.kind),
                group_id: group.group_id,
                label: group.label,
                description: group.description,
                operations: group
                    .operations
                    .into_iter()
                    .map(|operation| ConsolePolicyCatalogOperationResponse {
                        operation_id: operation.operation_id,
                        summary: operation.summary,
                        description: operation.description,
                        order: operation.order,
                        route: ConsolePolicyCatalogRouteResponse {
                            method: operation.route.method,
                            path: operation.route.path,
                        },
                        full_profile: match operation.full_profile {
                            control_plane::role::ConsolePolicyCatalogFullProfile::Simple {
                                enabled,
                            } => {
                                ConsolePolicyCatalogOperationFullProfileResponse::Simple { enabled }
                            }
                            control_plane::role::ConsolePolicyCatalogFullProfile::Row { scope } => {
                                ConsolePolicyCatalogOperationFullProfileResponse::Row {
                                    scope: to_console_policy_row_scope_body(scope),
                                }
                            }
                        },
                        allowed_row_scopes: operation
                            .allowed_row_scopes
                            .into_iter()
                            .map(|option| ConsolePolicyCatalogOptionResponse {
                                value: option.value,
                                label: option.label,
                                description: option.description,
                            })
                            .collect(),
                        authorization: match operation.authorization {
                            control_plane::role::ConsolePolicyAuthorization::Simple => {
                                ConsolePolicyOperationAuthorizationResponse::Simple
                            }
                            control_plane::role::ConsolePolicyAuthorization::ResourceAction {
                                resource_code,
                                action_code,
                            } => ConsolePolicyOperationAuthorizationResponse::ResourceAction {
                                resource_code,
                                action_code,
                            },
                        },
                    })
                    .collect(),
            })
            .collect(),
        resources: catalog
            .resources
            .into_iter()
            .map(|resource| ConsolePolicyCatalogResourceResponse {
                resource_code: resource.resource_code,
                label: resource.label,
                description: resource.description,
                actions: resource
                    .actions
                    .into_iter()
                    .map(|action| ConsolePolicyCatalogResourceActionResponse {
                        action_code: action.action_code,
                        label: action.label,
                        description: action.description,
                    })
                    .collect(),
            })
            .collect(),
    }
}

async fn resolve_console_policy_catalog_display(
    state: &ApiState,
    locale: &domain::CatalogLocale,
    catalog: &mut control_plane::role::ConsolePolicyCatalog,
) -> Result<(), ApiError> {
    catalog.locale = locale.as_str().to_string();
    for option in &mut catalog.group_strategy_options {
        option.label = super::super::core_console_i18n::resolve_core_console_display(
            state,
            locale,
            &option.label,
        )
        .await?;
        option.description = super::super::core_console_i18n::resolve_core_console_display(
            state,
            locale,
            &option.description,
        )
        .await?;
    }
    for group in &mut catalog.groups {
        group.label = super::super::core_console_i18n::resolve_core_console_display(
            state,
            locale,
            &group.label,
        )
        .await?;
        group.description = super::super::core_console_i18n::resolve_core_console_display(
            state,
            locale,
            &group.description,
        )
        .await?;
        for operation in &mut group.operations {
            operation.summary =
                crate::app_state::resolve_request_text(state, locale, &operation.summary).await?;
            operation.description =
                crate::app_state::resolve_request_text(state, locale, &operation.description)
                    .await?;
            for option in &mut operation.allowed_row_scopes {
                option.label = super::super::core_console_i18n::resolve_core_console_display(
                    state,
                    locale,
                    &option.label,
                )
                .await?;
                option.description = super::super::core_console_i18n::resolve_core_console_display(
                    state,
                    locale,
                    &option.description,
                )
                .await?;
            }
        }
    }
    for resource in &mut catalog.resources {
        resource.label = super::super::core_console_i18n::resolve_core_console_display(
            state,
            locale,
            &resource.label,
        )
        .await?;
        resource.description = super::super::core_console_i18n::resolve_core_console_display(
            state,
            locale,
            &resource.description,
        )
        .await?;
        for action in &mut resource.actions {
            action.label = super::super::core_console_i18n::resolve_core_console_display(
                state,
                locale,
                &action.label,
            )
            .await?;
            action.description = super::super::core_console_i18n::resolve_core_console_display(
                state,
                locale,
                &action.description,
            )
            .await?;
        }
    }
    Ok(())
}

pub(crate) async fn resolve_console_policy_catalog_display_with(
    store: &storage_durable_postgres::MainDurableStore,
    bootstrap_workspace_id: Uuid,
    locale: &domain::CatalogLocale,
    catalog: &mut control_plane::role::ConsolePolicyCatalog,
) -> Result<(), ApiError> {
    catalog.locale = locale.as_str().to_string();
    for option in &mut catalog.group_strategy_options {
        option.label = super::super::core_console_i18n::resolve_core_console_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            &option.label,
        )
        .await?;
        option.description = super::super::core_console_i18n::resolve_core_console_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            &option.description,
        )
        .await?;
    }
    for group in &mut catalog.groups {
        group.label = super::super::core_console_i18n::resolve_core_console_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            &group.label,
        )
        .await?;
        group.description = super::super::core_console_i18n::resolve_core_console_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            &group.description,
        )
        .await?;
        for operation in &mut group.operations {
            operation.summary = crate::app_state::resolve_request_text_with(
                store,
                bootstrap_workspace_id,
                locale,
                &operation.summary,
            )
            .await?;
            operation.description = crate::app_state::resolve_request_text_with(
                store,
                bootstrap_workspace_id,
                locale,
                &operation.description,
            )
            .await?;
            for option in &mut operation.allowed_row_scopes {
                option.label = super::super::core_console_i18n::resolve_core_console_display_with(
                    store,
                    bootstrap_workspace_id,
                    locale,
                    &option.label,
                )
                .await?;
                option.description =
                    super::super::core_console_i18n::resolve_core_console_display_with(
                        store,
                        bootstrap_workspace_id,
                        locale,
                        &option.description,
                    )
                    .await?;
            }
        }
    }
    for resource in &mut catalog.resources {
        resource.label = super::super::core_console_i18n::resolve_core_console_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            &resource.label,
        )
        .await?;
        resource.description = super::super::core_console_i18n::resolve_core_console_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            &resource.description,
        )
        .await?;
        for action in &mut resource.actions {
            action.label = super::super::core_console_i18n::resolve_core_console_display_with(
                store,
                bootstrap_workspace_id,
                locale,
                &action.label,
            )
            .await?;
            action.description =
                super::super::core_console_i18n::resolve_core_console_display_with(
                    store,
                    bootstrap_workspace_id,
                    locale,
                    &action.description,
                )
                .await?;
        }
    }
    Ok(())
}

pub(crate) fn to_role_console_policy_response(
    role_code: String,
    policy: domain::RoleConsolePolicy,
) -> RoleConsolePolicyResponse {
    RoleConsolePolicyResponse {
        role_code,
        groups: policy
            .groups()
            .iter()
            .map(|group| RoleConsolePolicyGroupResponse {
                kind: to_console_policy_group_kind_body(group.group().kind()),
                group_id: group.group().group_id().as_str().to_string(),
                enabled: group.enabled(),
                strategy: to_console_policy_strategy_body(group.strategy()),
                operations: group
                    .operations()
                    .iter()
                    .map(|operation| match operation {
                        domain::ConsoleOperationPolicy::Simple {
                            operation_id,
                            enabled,
                        } => RoleConsolePolicyOperationResponse::Simple {
                            operation_id: operation_id.as_str().to_string(),
                            enabled: *enabled,
                        },
                        domain::ConsoleOperationPolicy::Row {
                            operation_id,
                            scope,
                        } => RoleConsolePolicyOperationResponse::Row {
                            operation_id: operation_id.as_str().to_string(),
                            scope: to_console_policy_row_scope_body(*scope),
                        },
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub(crate) fn to_console_policy_group_input(
    group: ConsoleRoleConsolePolicyGroupBody,
) -> ConsolePolicyGroupInput {
    ConsolePolicyGroupInput {
        kind: group.kind.as_str().to_string(),
        group_id: group.group_id,
        enabled: group.enabled,
        strategy: group.strategy.as_str().to_string(),
        operations: group
            .operations
            .into_iter()
            .map(|operation| match operation {
                ConsoleRoleConsolePolicyOperationBody::Simple {
                    operation_id,
                    enabled,
                } => ConsolePolicyOperationInput::Simple {
                    operation_id,
                    enabled,
                },
                ConsoleRoleConsolePolicyOperationBody::Row {
                    operation_id,
                    scope,
                } => ConsolePolicyOperationInput::Row {
                    operation_id,
                    scope: scope.as_str().to_string(),
                },
            })
            .collect(),
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/roles",
            console_get(list_roles, ConsoleOperation("roles.list".to_string()))
                .post(create_role, ConsoleOperation("roles.create".to_string())),
        )
        .route(
            "/settings/roles/data-model-options",
            console_get(
                list_data_model_options,
                ConsoleOperation("roles.data_model_options.list".to_string()),
            ),
        )
        .route(
            "/settings/roles/console-policy-catalog",
            console_get(
                get_console_policy_catalog,
                ConsoleOperation("roles.console_policy_catalog.view".to_string()),
            ),
        )
        .route(
            "/settings/roles/console-policy-catalog/order",
            console_put(
                replace_console_settings_order,
                ConsoleOperation("roles.console_settings_order.replace".to_string()),
            ),
        )
        .route(
            "/settings/roles/:id",
            console_patch(update_role, ConsoleOperation("roles.update".to_string()))
                .delete(delete_role, ConsoleOperation("roles.delete".to_string())),
        )
        .route(
            "/settings/roles/:id/console-policy",
            console_get(
                get_role_console_policy,
                ConsoleOperation("roles.console_policy.view".to_string()),
            )
            .put(
                replace_role_console_policy,
                ConsoleOperation("roles.console_policy.replace".to_string()),
            ),
        )
        .route(
            "/settings/roles/:id/permissions",
            console_get(
                get_role_permissions,
                ConsoleOperation("roles.permissions.view".to_string()),
            )
            .put(
                replace_role_permissions,
                ConsoleOperation("roles.permissions.replace".to_string()),
            ),
        )
        .route(
            "/settings/roles/:id/frontstage-routes",
            console_get(
                get_role_frontstage_routes,
                ConsoleOperation("roles.frontstage_routes.view".to_string()),
            )
            .put(
                replace_role_frontstage_routes,
                ConsoleOperation("roles.frontstage_routes.replace".to_string()),
            ),
        )
        .route(
            "/settings/roles/:id/data-policy",
            console_get(
                get_role_data_policy,
                ConsoleOperation("roles.data_policy.view".to_string()),
            )
            .put(
                replace_role_data_policy,
                ConsoleOperation("roles.data_policy.replace".to_string()),
            ),
        )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/roles/data-model-options",
    responses((status = 200, body = [RoleDataModelOptionResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_data_model_options(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<RoleDataModelOptionResponse>>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.data-model-options.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::ListDataModelOptions,
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::DataModelOptions(items) = output
    else {
        return Err(anyhow::anyhow!("role data model options output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(items)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/roles/console-policy-catalog",
    responses(
        (status = 200, body = ConsolePolicyCatalogResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_console_policy_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ConsolePolicyCatalogResponse>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.console-policy-catalog.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::GetConsolePolicyCatalog { locale },
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::ConsolePolicyCatalog(catalog) =
        output
    else {
        return Err(anyhow::anyhow!("console policy catalog output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(catalog)))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/roles/console-policy-catalog/order",
    request_body = ReplaceConsoleSettingsOrderBody,
    responses(
        (status = 200, body = ConsolePolicyCatalogResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 409, body = crate::error_response::ErrorBody)
    )
)]
pub async fn replace_console_settings_order(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ReplaceConsoleSettingsOrderBody>,
) -> Result<Json<ApiSuccess<ConsolePolicyCatalogResponse>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.console-settings-order.replace.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::ReplaceConsoleSettingsOrder {
            locale,
            body,
        },
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::ConsolePolicyCatalog(catalog) =
        output
    else {
        return Err(anyhow::anyhow!("console policy catalog output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(catalog)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/roles/{id}/console-policy",
    params(("id" = String, Path, description = "Role code")),
    responses(
        (status = 200, body = RoleConsolePolicyResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_role_console_policy(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
) -> Result<Json<ApiSuccess<RoleConsolePolicyResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.console-policy.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::GetRoleConsolePolicy { role_code },
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::RoleConsolePolicy(policy) = output
    else {
        return Err(anyhow::anyhow!("role console policy output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(policy)))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/roles/{id}/console-policy",
    request_body = ReplaceRoleConsolePolicyBody,
    params(("id" = String, Path, description = "Role code")),
    responses(
        (status = 204),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn replace_role_console_policy(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
    Json(body): Json<ReplaceRoleConsolePolicyBody>,
) -> Result<StatusCode, ApiError> {
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::role_access_interface::RoleAccessOutput,
    >(
        Arc::clone(&state),
        "http.console.roles.console-policy.replace.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::ReplaceRoleConsolePolicy {
            role_code,
            body,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
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

pub(crate) fn to_default_policy_input(
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

pub(crate) fn to_model_policy_input(
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

pub(crate) fn to_role_data_policy_response(
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::ListRoles,
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::Roles(roles) = output else {
        return Err(anyhow::anyhow!("roles output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(roles)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::CreateRole(body),
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::Role(role) = output else {
        return Err(anyhow::anyhow!("role create output contract mismatch").into());
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(role))))
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
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::role_access_interface::RoleAccessOutput,
    >(
        Arc::clone(&state),
        "http.console.roles.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::UpdateRole { role_code, body },
    )
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
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::role_access_interface::RoleAccessOutput,
    >(
        Arc::clone(&state),
        "http.console.roles.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::DeleteRole { role_code },
    )
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.permissions.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::GetRolePermissions { role_code },
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::RolePermissions(permissions) =
        output
    else {
        return Err(anyhow::anyhow!("role permissions output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(permissions)))
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
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::role_access_interface::RoleAccessOutput,
    >(
        Arc::clone(&state),
        "http.console.roles.permissions.replace.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::ReplaceRolePermissions {
            role_code,
            body,
        },
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn build_frontstage_route_tree(
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.frontstage-routes.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::GetRoleFrontstageRoutes {
            role_code,
        },
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::RoleFrontstageRoutes(routes) =
        output
    else {
        return Err(anyhow::anyhow!("role frontstage routes output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(routes)))
}

pub async fn replace_role_frontstage_routes(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(role_code): Path<String>,
    Json(body): Json<ReplaceRoleFrontstageRoutesBody>,
) -> Result<StatusCode, ApiError> {
    crate::routes::console_interface::invoke::<
        _,
        crate::routes::role_access_interface::RoleAccessOutput,
    >(
        Arc::clone(&state),
        "http.console.roles.frontstage-routes.replace.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::ReplaceRoleFrontstageRoutes {
            role_code,
            body,
        },
    )
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.data-policy.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::GetRoleDataPolicy { role_code },
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::RoleDataPolicy(policy) = output
    else {
        return Err(anyhow::anyhow!("role data policy output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(policy)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.roles.data-policy.replace.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::role_access_interface::RoleAccessInput::ReplaceRoleDataPolicy {
            role_code,
            body,
        },
    )
    .await?;
    let crate::routes::role_access_interface::RoleAccessOutput::RoleDataPolicy(policy) = output
    else {
        return Err(anyhow::anyhow!("role data policy output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(policy)))
}
