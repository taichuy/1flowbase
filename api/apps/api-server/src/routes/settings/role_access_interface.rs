use std::sync::Arc;

use control_plane::{
    model_definition::ModelDefinitionService,
    role::{
        CreateRoleCommand, DeleteRoleCommand, ReplaceConsoleSettingsOrderCommand,
        ReplaceRoleConsolePolicyCommand, ReplaceRoleDataPolicyCommand,
        ReplaceRoleFrontstageRoutesCommand, ReplaceRolePermissionsCommand, RoleService,
        UpdateRoleCommand,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::{permissions, roles};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum RoleAccessInput {
    ListDataModelOptions,
    GetConsolePolicyCatalog {
        locale: ConsoleLocaleHints,
    },
    ReplaceConsoleSettingsOrder {
        locale: ConsoleLocaleHints,
        body: roles::ReplaceConsoleSettingsOrderBody,
    },
    GetRoleConsolePolicy {
        role_code: String,
    },
    ReplaceRoleConsolePolicy {
        role_code: String,
        body: roles::ReplaceRoleConsolePolicyBody,
    },
    ListRoles,
    CreateRole(roles::CreateRoleBody),
    UpdateRole {
        role_code: String,
        body: roles::UpdateRoleBody,
    },
    DeleteRole {
        role_code: String,
    },
    GetRolePermissions {
        role_code: String,
    },
    ReplaceRolePermissions {
        role_code: String,
        body: roles::ReplaceRolePermissionsBody,
    },
    GetRoleFrontstageRoutes {
        role_code: String,
    },
    ReplaceRoleFrontstageRoutes {
        role_code: String,
        body: roles::ReplaceRoleFrontstageRoutesBody,
    },
    GetRoleDataPolicy {
        role_code: String,
    },
    ReplaceRoleDataPolicy {
        role_code: String,
        body: roles::ReplaceRoleDataPolicyBody,
    },
    ListPermissionOptions,
}

impl InterfaceContract for RoleAccessInput {
    const CONTRACT_ID: &'static str = "console-role-access-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum RoleAccessOutput {
    DataModelOptions(Vec<roles::RoleDataModelOptionResponse>),
    ConsolePolicyCatalog(roles::ConsolePolicyCatalogResponse),
    RoleConsolePolicy(roles::RoleConsolePolicyResponse),
    Roles(Vec<roles::RoleResponse>),
    Role(roles::RoleResponse),
    RolePermissions(roles::RolePermissionsResponse),
    RoleFrontstageRoutes(roles::RoleFrontstageRoutesResponse),
    RoleDataPolicy(roles::RoleDataPolicyResponse),
    PermissionOptions(Vec<permissions::PermissionResponse>),
    NoContent,
}

impl InterfaceContract for RoleAccessOutput {
    const CONTRACT_ID: &'static str = "console-role-access-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct RoleAccessAdapter {
    store: MainDurableStore,
    console_inventory: access_control::ConsoleOperationCompiledInventory,
    settings_features: Vec<access_control::SettingsFeatureInventoryEntry>,
    bootstrap_workspace_id: uuid::Uuid,
}

pub(crate) fn role_access_port(
    store: MainDurableStore,
    console_inventory: access_control::ConsoleOperationCompiledInventory,
    settings_features: Vec<access_control::SettingsFeatureInventoryEntry>,
    bootstrap_workspace_id: uuid::Uuid,
) -> Arc<dyn ConsoleInterfacePort<RoleAccessInput, RoleAccessOutput>> {
    Arc::new(RoleAccessAdapter {
        store,
        console_inventory,
        settings_features,
        bootstrap_workspace_id,
    })
}

impl RoleAccessAdapter {
    async fn preferred_locale(
        &self,
        principal: &UserPrincipal,
    ) -> Result<Option<String>, ApiError> {
        Ok(self
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale)
    }

    async fn localized_catalog(
        &self,
        principal: &UserPrincipal,
        hints: ConsoleLocaleHints,
        mut catalog: control_plane::role::ConsolePolicyCatalog,
    ) -> Result<roles::ConsolePolicyCatalogResponse, ApiError> {
        let locale = hints.resolve(self.preferred_locale(principal).await?);
        roles::resolve_console_policy_catalog_display_with(
            &self.store,
            self.bootstrap_workspace_id,
            &locale,
            &mut catalog,
        )
        .await?;
        Ok(roles::to_console_policy_catalog_response(catalog))
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: RoleAccessInput,
    ) -> Result<RoleAccessOutput, ApiError> {
        let actor = principal.actor();
        match input {
            RoleAccessInput::ListDataModelOptions => {
                let models = ModelDefinitionService::for_console_operation(
                    self.store.clone(),
                    domain::ConsolePolicyGroup::settings_feature("system.roles")
                        .expect("compiled roles settings group must be valid"),
                    "roles.data_model_options.list",
                )
                .list_role_settings_data_model_options(actor.user_id)
                .await?;
                Ok(RoleAccessOutput::DataModelOptions(
                    models
                        .into_iter()
                        .map(|model| roles::RoleDataModelOptionResponse {
                            id: model.id,
                            code: model.code,
                            title: model.title,
                        })
                        .collect(),
                ))
            }
            RoleAccessInput::GetConsolePolicyCatalog { locale } => {
                let catalog = RoleService::new(self.store.for_actor(actor.clone()))
                    .get_console_policy_catalog(
                        actor.user_id,
                        &self.console_inventory,
                        &self.settings_features,
                        locale
                            .resolve(self.preferred_locale(principal).await?)
                            .as_str(),
                    )
                    .await?;
                Ok(RoleAccessOutput::ConsolePolicyCatalog(
                    self.localized_catalog(principal, locale, catalog).await?,
                ))
            }
            RoleAccessInput::ReplaceConsoleSettingsOrder { locale, body } => {
                let resolved = locale.resolve(self.preferred_locale(principal).await?);
                let catalog = RoleService::new(self.store.for_actor(actor.clone()))
                    .replace_console_settings_order(
                        ReplaceConsoleSettingsOrderCommand {
                            actor_user_id: actor.user_id,
                            expected_revision: body.expected_revision,
                            group_ids: body.group_ids,
                        },
                        &self.console_inventory,
                        &self.settings_features,
                        resolved.as_str(),
                    )
                    .await?;
                Ok(RoleAccessOutput::ConsolePolicyCatalog(
                    self.localized_catalog(principal, locale, catalog).await?,
                ))
            }
            RoleAccessInput::GetRoleConsolePolicy { role_code } => {
                let policy = RoleService::new(self.store.for_actor(actor.clone()))
                    .get_console_policy(actor.user_id, &role_code, &self.console_inventory)
                    .await?;
                Ok(RoleAccessOutput::RoleConsolePolicy(
                    roles::to_role_console_policy_response(role_code, policy),
                ))
            }
            RoleAccessInput::ReplaceRoleConsolePolicy { role_code, body } => {
                RoleService::new(self.store.for_actor(actor.clone()))
                    .replace_console_policy(
                        ReplaceRoleConsolePolicyCommand {
                            actor_user_id: actor.user_id,
                            role_code,
                            groups: body
                                .groups
                                .into_iter()
                                .map(roles::to_console_policy_group_input)
                                .collect(),
                        },
                        &self.console_inventory,
                    )
                    .await?;
                Ok(RoleAccessOutput::NoContent)
            }
            RoleAccessInput::ListRoles => {
                let records = RoleService::new(self.store.for_actor(actor.clone()))
                    .list_roles(actor.user_id)
                    .await?;
                Ok(RoleAccessOutput::Roles(
                    records.into_iter().map(roles::to_role_response).collect(),
                ))
            }
            RoleAccessInput::CreateRole(body) => {
                RoleService::new(self.store.for_actor(actor.clone()))
                    .create_role(CreateRoleCommand {
                        actor_user_id: actor.user_id,
                        code: body.code.clone(),
                        name: body.name.clone(),
                        introduction: body.introduction.clone(),
                        auto_grant_new_permissions: body
                            .auto_grant_new_permissions
                            .unwrap_or(false),
                        is_default_member_role: body.is_default_member_role.unwrap_or(false),
                    })
                    .await?;
                Ok(RoleAccessOutput::Role(roles::RoleResponse {
                    code: body.code,
                    name: body.name,
                    introduction: body.introduction,
                    scope_kind: "workspace".to_string(),
                    is_builtin: false,
                    is_editable: true,
                    auto_grant_new_permissions: body.auto_grant_new_permissions.unwrap_or(false),
                    is_default_member_role: body.is_default_member_role.unwrap_or(false),
                    permission_codes: Vec::new(),
                }))
            }
            RoleAccessInput::UpdateRole { role_code, body } => {
                RoleService::new(self.store.for_actor(actor.clone()))
                    .update_role(UpdateRoleCommand {
                        actor_user_id: actor.user_id,
                        role_code,
                        name: body.name,
                        introduction: body.introduction,
                        auto_grant_new_permissions: body.auto_grant_new_permissions,
                        is_default_member_role: body.is_default_member_role,
                    })
                    .await?;
                Ok(RoleAccessOutput::NoContent)
            }
            RoleAccessInput::DeleteRole { role_code } => {
                RoleService::new(self.store.for_actor(actor.clone()))
                    .delete_role(DeleteRoleCommand {
                        actor_user_id: actor.user_id,
                        role_code,
                    })
                    .await?;
                Ok(RoleAccessOutput::NoContent)
            }
            RoleAccessInput::GetRolePermissions { role_code } => {
                let permission_codes = RoleService::new(self.store.for_actor(actor.clone()))
                    .get_role_permissions(actor.user_id, &role_code)
                    .await?;
                Ok(RoleAccessOutput::RolePermissions(
                    roles::RolePermissionsResponse {
                        role_code,
                        permission_codes,
                    },
                ))
            }
            RoleAccessInput::ReplaceRolePermissions { role_code, body } => {
                RoleService::new(self.store.for_actor(actor.clone()))
                    .replace_permissions(ReplaceRolePermissionsCommand {
                        actor_user_id: actor.user_id,
                        role_code,
                        permission_codes: body.permission_codes,
                    })
                    .await?;
                Ok(RoleAccessOutput::NoContent)
            }
            RoleAccessInput::GetRoleFrontstageRoutes { role_code } => {
                let view = RoleService::new(self.store.for_actor(actor.clone()))
                    .get_frontstage_routes(actor.user_id, &role_code)
                    .await?;
                Ok(RoleAccessOutput::RoleFrontstageRoutes(
                    roles::RoleFrontstageRoutesResponse {
                        role_code,
                        checked_page_ids: view
                            .rules
                            .iter()
                            .filter_map(|rule| rule.page_id)
                            .collect(),
                        checked_tab_ids: view.rules.iter().filter_map(|rule| rule.tab_id).collect(),
                        tree: roles::build_frontstage_route_tree(view.pages, view.tabs),
                    },
                ))
            }
            RoleAccessInput::ReplaceRoleFrontstageRoutes { role_code, body } => {
                RoleService::new(self.store.for_actor(actor.clone()))
                    .replace_frontstage_routes(ReplaceRoleFrontstageRoutesCommand {
                        actor_user_id: actor.user_id,
                        role_code,
                        page_ids: body.page_ids,
                        tab_ids: body.tab_ids,
                    })
                    .await?;
                Ok(RoleAccessOutput::NoContent)
            }
            RoleAccessInput::GetRoleDataPolicy { role_code } => {
                let policy = RoleService::new(self.store.for_actor(actor.clone()))
                    .get_role_data_policy(actor.user_id, &role_code)
                    .await?;
                Ok(RoleAccessOutput::RoleDataPolicy(
                    roles::to_role_data_policy_response(policy),
                ))
            }
            RoleAccessInput::ReplaceRoleDataPolicy { role_code, body } => {
                let default_policy = roles::to_default_policy_input(body.default_policy)?;
                let model_policies = body
                    .model_policies
                    .into_iter()
                    .map(roles::to_model_policy_input)
                    .collect::<Result<Vec<_>, _>>()?;
                let policy = RoleService::new(self.store.for_actor(actor.clone()))
                    .replace_data_policy(ReplaceRoleDataPolicyCommand {
                        actor_user_id: actor.user_id,
                        role_code,
                        default_policy,
                        model_policies,
                    })
                    .await?;
                Ok(RoleAccessOutput::RoleDataPolicy(
                    roles::to_role_data_policy_response(policy),
                ))
            }
            RoleAccessInput::ListPermissionOptions => {
                let options = RoleService::new(self.store.for_actor(actor.clone()))
                    .list_permission_options(actor.user_id)
                    .await?;
                Ok(RoleAccessOutput::PermissionOptions(
                    permissions::permission_responses(options, &self.settings_features),
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<RoleAccessInput, RoleAccessOutput> for RoleAccessAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: RoleAccessInput,
    ) -> ConsoleInterfaceFuture<'a, RoleAccessOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "roles.data_model_options.list",
        binding_id: "http.console.roles.data-model-options.list.v1",
        method: "GET",
        path: "/api/console/settings/roles/data-model-options",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.console_policy_catalog.view",
        binding_id: "http.console.roles.console-policy-catalog.view.v1",
        method: "GET",
        path: "/api/console/settings/roles/console-policy-catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.console_settings_order.replace",
        binding_id: "http.console.roles.console-settings-order.replace.v1",
        method: "PUT",
        path: "/api/console/settings/roles/console-policy-catalog/order",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.console_policy.view",
        binding_id: "http.console.roles.console-policy.view.v1",
        method: "GET",
        path: "/api/console/settings/roles/:id/console-policy",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.console_policy.replace",
        binding_id: "http.console.roles.console-policy.replace.v1",
        method: "PUT",
        path: "/api/console/settings/roles/:id/console-policy",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.list",
        binding_id: "http.console.roles.list.v1",
        method: "GET",
        path: "/api/console/settings/roles",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.create",
        binding_id: "http.console.roles.create.v1",
        method: "POST",
        path: "/api/console/settings/roles",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.update",
        binding_id: "http.console.roles.update.v1",
        method: "PATCH",
        path: "/api/console/settings/roles/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.delete",
        binding_id: "http.console.roles.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/roles/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.permissions.view",
        binding_id: "http.console.roles.permissions.view.v1",
        method: "GET",
        path: "/api/console/settings/roles/:id/permissions",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.permissions.replace",
        binding_id: "http.console.roles.permissions.replace.v1",
        method: "PUT",
        path: "/api/console/settings/roles/:id/permissions",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.frontstage_routes.view",
        binding_id: "http.console.roles.frontstage-routes.view.v1",
        method: "GET",
        path: "/api/console/settings/roles/:id/frontstage-routes",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.frontstage_routes.replace",
        binding_id: "http.console.roles.frontstage-routes.replace.v1",
        method: "PUT",
        path: "/api/console/settings/roles/:id/frontstage-routes",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.data_policy.view",
        binding_id: "http.console.roles.data-policy.view.v1",
        method: "GET",
        path: "/api/console/settings/roles/:id/data-policy",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.data_policy.replace",
        binding_id: "http.console.roles.data-policy.replace.v1",
        method: "PUT",
        path: "/api/console/settings/roles/:id/data-policy",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.permission_options.list",
        binding_id: "http.console.roles.permission-options.list.v1",
        method: "GET",
        path: "/api/console/settings/roles/permission-options",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<RoleAccessInput, RoleAccessOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-role-access",
        "graph:console-role-access-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableRoleAccessPort;

#[cfg(test)]
impl ConsoleInterfacePort<RoleAccessInput, RoleAccessOutput> for UnavailableRoleAccessPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: RoleAccessInput,
    ) -> ConsoleInterfaceFuture<'a, RoleAccessOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("role access fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f08c_registry_freezes_role_access_bindings() {
        let registry = compile_registry(Arc::new(UnavailableRoleAccessPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
