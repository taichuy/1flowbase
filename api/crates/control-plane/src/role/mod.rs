use std::collections::{BTreeMap, BTreeSet};

use access_control::{
    ensure_permission, ConsoleAuthorization, ConsoleLocaleCatalog,
    ConsoleOperationCompiledInventory, ConsoleOperationInventoryEntry,
    ConsolePolicyGroup as RegisteredConsolePolicyGroup, ResourceAccessRegistration,
    ResourceAccessScopeKind, SettingsFeatureLifecycle, SYSTEM_ROLES_SETTINGS_FEATURE_ID,
    SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION,
};
use anyhow::Result;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        AuthRepository, CreateWorkspaceRoleInput, FrontstagePageRepository,
        ReplaceRoleDataPolicyInput, RoleConsolePolicyReader, RoleDataModelPolicyInput,
        RoleDataPolicyDefaultsInput, RoleDataPolicyView, RoleRepository, UpdateWorkspaceRoleInput,
    },
};

pub mod console_policy_migration;
mod console_policy_validation;

use console_policy_validation::{
    complete_stored_console_policy, role_console_policy_groups_from_input,
    CompiledConsolePolicyOperationIndex, ConsolePolicyGroupKey,
};
pub use console_policy_validation::{
    ConsolePolicyAuthorization, ConsolePolicyCatalog, ConsolePolicyCatalogAction,
    ConsolePolicyCatalogFullProfile, ConsolePolicyCatalogGroup, ConsolePolicyCatalogOperation,
    ConsolePolicyCatalogOption, ConsolePolicyCatalogResource,
};

const ROLES_CONSOLE_POLICY_CATALOG_VIEW_OPERATION_ID: &str = "roles.console_policy_catalog.view";
const ROLES_CONSOLE_POLICY_VIEW_OPERATION_ID: &str = "roles.console_policy.view";
const ROLES_CONSOLE_POLICY_REPLACE_OPERATION_ID: &str = "roles.console_policy.replace";
const ROLES_CREATE_OPERATION_ID: &str = "roles.create";
const ROLES_DATA_POLICY_REPLACE_OPERATION_ID: &str = "roles.data_policy.replace";
const ROLES_DATA_POLICY_VIEW_OPERATION_ID: &str = "roles.data_policy.view";
const ROLES_DELETE_OPERATION_ID: &str = "roles.delete";
const ROLES_LIST_OPERATION_ID: &str = "roles.list";
const ROLES_PERMISSION_OPTIONS_LIST_OPERATION_ID: &str = "roles.permission_options.list";
const ROLES_PERMISSIONS_REPLACE_OPERATION_ID: &str = "roles.permissions.replace";
const ROLES_PERMISSIONS_VIEW_OPERATION_ID: &str = "roles.permissions.view";
const ROLES_UPDATE_OPERATION_ID: &str = "roles.update";

pub struct CreateRoleCommand {
    pub actor_user_id: Uuid,
    pub code: String,
    pub name: String,
    pub introduction: String,
    pub auto_grant_new_permissions: bool,
    pub is_default_member_role: bool,
}

pub struct UpdateRoleCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub name: String,
    pub introduction: String,
    pub auto_grant_new_permissions: Option<bool>,
    pub is_default_member_role: Option<bool>,
}

pub struct DeleteRoleCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
}

pub struct ReplaceRolePermissionsCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub permission_codes: Vec<String>,
}

pub struct ReplaceRoleDataPolicyCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub default_policy: RoleDataPolicyDefaultsInput,
    pub model_policies: Vec<RoleDataModelPolicyInput>,
}

pub struct ReplaceRoleFrontstageRoutesCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub page_ids: Vec<Uuid>,
    pub tab_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct ReplaceRoleConsolePolicyCommand {
    pub actor_user_id: Uuid,
    pub role_code: String,
    pub groups: Vec<ConsolePolicyGroupInput>,
}

#[derive(Debug, Clone)]
pub struct ConsolePolicyGroupInput {
    pub kind: String,
    pub group_id: String,
    pub mode: String,
    pub operations: Vec<ConsolePolicyOperationInput>,
}

#[derive(Debug, Clone)]
pub enum ConsolePolicyOperationInput {
    Simple { operation_id: String, enabled: bool },
    Row { operation_id: String, scope: String },
}

fn localized_reference(
    locale_catalog: &ConsoleLocaleCatalog,
    reference: &str,
    locale: &str,
) -> Result<String, ControlPlaneError> {
    locale_catalog
        .text(locale, reference)
        .map(str::to_string)
        .ok_or(ControlPlaneError::InvalidInput(
            "console_policy_translation",
        ))
}

fn domain_console_policy_group(
    registered: &RegisteredConsolePolicyGroup,
) -> Result<domain::ConsolePolicyGroup, ControlPlaneError> {
    match registered {
        RegisteredConsolePolicyGroup::SettingsFeature(group_id) => {
            domain::ConsolePolicyGroup::settings_feature(group_id)
                .map_err(|_| ControlPlaneError::InvalidInput("console_policy_group"))
        }
        RegisteredConsolePolicyGroup::Other(group_id) => {
            domain::ConsolePolicyGroup::other(group_id)
                .map_err(|_| ControlPlaneError::InvalidInput("console_policy_group"))
        }
    }
}

fn console_policy_group_key(group: &domain::ConsolePolicyGroup) -> ConsolePolicyGroupKey {
    (
        group.kind().as_str().to_string(),
        group.group_id().as_str().to_string(),
    )
}

fn console_policy_group_text(
    locale_catalog: &ConsoleLocaleCatalog,
    group: &RegisteredConsolePolicyGroup,
    locale: &str,
) -> Result<(String, String), ControlPlaneError> {
    let display = locale_catalog
        .policy_group_display(group, locale)
        .map_err(|_| ControlPlaneError::InvalidInput("console_policy_group_translation"))?;
    Ok((display.label, display.description))
}

fn compiled_console_policy_operations(
    inventory: &ConsoleOperationCompiledInventory,
) -> Result<CompiledConsolePolicyOperationIndex, ControlPlaneError> {
    let mut resources = BTreeMap::<String, &ResourceAccessRegistration>::new();
    for resource in &inventory.resources {
        if resource.lifecycle != SettingsFeatureLifecycle::Active {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_resource_inactive",
            ));
        }
        if resources
            .insert(resource.resource_code.clone(), resource)
            .is_some()
        {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_resource_duplicate",
            ));
        }
        let mut actions = BTreeSet::new();
        for action in &resource.actions {
            if !actions.insert(action.action_code.as_str()) {
                return Err(ControlPlaneError::InvalidInput(
                    "console_policy_action_duplicate",
                ));
            }
        }
    }

    let mut groups = BTreeMap::new();
    for operation in &inventory.operations {
        if operation.lifecycle != SettingsFeatureLifecycle::Active {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_operation_inactive",
            ));
        }
        let group = domain_console_policy_group(&operation.policy_group)?;
        let group_key = console_policy_group_key(&group);
        let Some(full_profile) = (match &operation.authorization {
            ConsoleAuthorization::Authenticated => None,
            ConsoleAuthorization::Simple => {
                Some(ConsolePolicyCatalogFullProfile::Simple { enabled: true })
            }
            ConsoleAuthorization::ResourceAction {
                resource_code,
                action_code,
            } => {
                let resource = resources
                    .get(resource_code)
                    .ok_or(ControlPlaneError::InvalidInput("console_policy_resource"))?;
                if resource.scope_kind != ResourceAccessScopeKind::Workspace
                    || resource.scope_field.as_deref() != Some("scope_id")
                    || resource.owner_field.as_deref() != Some("created_by")
                {
                    return Err(ControlPlaneError::InvalidInput(
                        "console_policy_resource_scope",
                    ));
                }
                if !resource
                    .actions
                    .iter()
                    .any(|action| action.action_code == *action_code)
                {
                    return Err(ControlPlaneError::InvalidInput("console_policy_action"));
                }
                Some(ConsolePolicyCatalogFullProfile::Row {
                    scope: domain::ConsoleOperationRowScope::ScopeAll,
                })
            }
        }) else {
            continue;
        };
        if domain::ConsoleOperationId::try_from(operation.operation_id.as_str()).is_err() {
            return Err(ControlPlaneError::InvalidInput("console_policy_operation"));
        }
        let operations = groups.entry(group_key).or_insert_with(BTreeMap::new);
        if operations
            .insert(operation.operation_id.clone(), full_profile)
            .is_some()
        {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_operation_duplicate",
            ));
        }
    }
    Ok(groups)
}

fn operation_text(
    locale_catalog: &ConsoleLocaleCatalog,
    operation: &ConsoleOperationInventoryEntry,
    locale: &str,
) -> Result<(String, String), ControlPlaneError> {
    let label = localized_reference(locale_catalog, &operation.label_ref, locale)?;
    let description = operation
        .description_ref
        .as_deref()
        .map(|reference| localized_reference(locale_catalog, reference, locale))
        .transpose()?
        .ok_or(ControlPlaneError::InvalidInput(
            "console_policy_description",
        ))?;
    Ok((label, description))
}

fn build_console_policy_catalog_for_locale(
    inventory: &ConsoleOperationCompiledInventory,
    locale_catalog: &ConsoleLocaleCatalog,
    locale: &str,
) -> Result<ConsolePolicyCatalog, ControlPlaneError> {
    let operation_index = compiled_console_policy_operations(inventory)?;
    let group_mode_options = locale_catalog
        .group_mode_options(locale)
        .map_err(|_| ControlPlaneError::InvalidInput("console_policy_translation"))?
        .into_iter()
        .map(|option| ConsolePolicyCatalogOption {
            value: option.value,
            label: option.label,
            description: option.description,
        })
        .collect::<Vec<_>>();
    let row_scope_options = locale_catalog
        .row_scope_options(locale)
        .map_err(|_| ControlPlaneError::InvalidInput("console_policy_translation"))?
        .into_iter()
        .map(|option| ConsolePolicyCatalogOption {
            value: option.value,
            label: option.label,
            description: option.description,
        })
        .collect::<Vec<_>>();
    let mut groups = Vec::with_capacity(operation_index.len());
    for ((kind, group_id), operations) in operation_index {
        let registered_group = if kind == "settings_feature" {
            RegisteredConsolePolicyGroup::SettingsFeature(group_id.clone())
        } else {
            RegisteredConsolePolicyGroup::Other(group_id.clone())
        };
        let group = domain_console_policy_group(&registered_group)?;
        let (label, description) =
            console_policy_group_text(locale_catalog, &registered_group, locale)?;
        let mut operation_views = inventory
            .operations
            .iter()
            .filter(|operation| {
                operation.policy_group == registered_group
                    && !matches!(operation.authorization, ConsoleAuthorization::Authenticated)
            })
            .map(|operation| {
                let (label, description) = operation_text(locale_catalog, operation, locale)?;
                let full_profile = operations
                    .get(&operation.operation_id)
                    .ok_or(ControlPlaneError::InvalidInput("console_policy_operation"))?;
                let authorization = match &operation.authorization {
                    ConsoleAuthorization::Simple => ConsolePolicyAuthorization::Simple,
                    ConsoleAuthorization::ResourceAction {
                        resource_code,
                        action_code,
                    } => ConsolePolicyAuthorization::ResourceAction {
                        resource_code: resource_code.clone(),
                        action_code: action_code.clone(),
                    },
                    ConsoleAuthorization::Authenticated => {
                        return Err(ControlPlaneError::InvalidInput("console_policy_type"));
                    }
                };
                let allowed_row_scopes = match full_profile {
                    ConsolePolicyCatalogFullProfile::Simple { .. } => Vec::new(),
                    ConsolePolicyCatalogFullProfile::Row { .. } => row_scope_options.clone(),
                };
                Ok((
                    operation.order,
                    operation.operation_id.clone(),
                    ConsolePolicyCatalogOperation {
                        operation_id: operation.operation_id.clone(),
                        label,
                        description,
                        order: operation.order,
                        full_profile: full_profile.clone(),
                        allowed_row_scopes,
                        authorization,
                    },
                ))
            })
            .collect::<Result<Vec<_>, ControlPlaneError>>()?;
        operation_views.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
        groups.push(ConsolePolicyCatalogGroup {
            kind: group.kind(),
            group_id,
            label,
            description,
            operations: operation_views
                .into_iter()
                .map(|(_, _, operation)| operation)
                .collect(),
        });
    }
    groups.sort_by(|left, right| {
        let left_kind_order = (left.kind == domain::ConsolePolicyGroupKind::Other) as u8;
        let right_kind_order = (right.kind == domain::ConsolePolicyGroupKind::Other) as u8;
        left_kind_order
            .cmp(&right_kind_order)
            .then(left.group_id.cmp(&right.group_id))
    });

    let resources = inventory
        .resources
        .iter()
        .map(|resource| {
            let label = localized_reference(locale_catalog, &resource.label_ref, locale)?;
            let description = resource
                .description_ref
                .as_deref()
                .map(|reference| localized_reference(locale_catalog, reference, locale))
                .transpose()?
                .ok_or(ControlPlaneError::InvalidInput(
                    "console_policy_description",
                ))?;
            let mut actions = resource
                .actions
                .iter()
                .map(|action| {
                    let label = localized_reference(locale_catalog, &action.label_ref, locale)?;
                    let description = action
                        .description_ref
                        .as_deref()
                        .map(|reference| localized_reference(locale_catalog, reference, locale))
                        .transpose()?
                        .ok_or(ControlPlaneError::InvalidInput(
                            "console_policy_description",
                        ))?;
                    Ok(ConsolePolicyCatalogAction {
                        action_code: action.action_code.clone(),
                        label,
                        description,
                    })
                })
                .collect::<Result<Vec<_>, ControlPlaneError>>()?;
            actions.sort_by(|left, right| left.action_code.cmp(&right.action_code));
            Ok(ConsolePolicyCatalogResource {
                resource_code: resource.resource_code.clone(),
                label,
                description,
                actions,
            })
        })
        .collect::<Result<Vec<_>, ControlPlaneError>>()?;

    Ok(ConsolePolicyCatalog {
        schema_version: inventory.schema_version.to_string(),
        locale: locale.to_string(),
        group_mode_options,
        groups,
        resources,
    })
}

fn validate_complete_console_policy_catalog(
    inventory: &ConsoleOperationCompiledInventory,
) -> Result<CompiledConsolePolicyOperationIndex, ControlPlaneError> {
    compiled_console_policy_operations(inventory)
}

pub struct RoleFrontstageRoutesView {
    pub pages: Vec<domain::FrontstagePageRecord>,
    pub tabs: Vec<domain::frontstage::FrontstagePageTabRecord>,
    pub rules: Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>,
}

pub struct RoleService<R> {
    repository: R,
}

fn ensure_workspace_role_data_policy_scope(
    scope: domain::RoleDataPolicyScope,
) -> Result<(), ControlPlaneError> {
    if scope == domain::RoleDataPolicyScope::SystemAll {
        return Err(ControlPlaneError::InvalidInput(
            "system_all_requires_system_role",
        ));
    }

    Ok(())
}

fn ensure_workspace_role_data_policy_allowed(
    default_policy: &RoleDataPolicyDefaultsInput,
    model_policies: &[RoleDataModelPolicyInput],
) -> Result<(), ControlPlaneError> {
    ensure_workspace_role_data_policy_scope(default_policy.default_view_scope)?;
    ensure_workspace_role_data_policy_scope(default_policy.default_update_scope)?;
    ensure_workspace_role_data_policy_scope(default_policy.default_delete_scope)?;

    for policy in model_policies {
        for scope in [
            policy.view_scope_override,
            policy.update_scope_override,
            policy.delete_scope_override,
        ]
        .into_iter()
        .flatten()
        {
            ensure_workspace_role_data_policy_scope(scope)?;
        }
    }

    Ok(())
}

impl<R> RoleService<R>
where
    R: RoleRepository + RoleConsolePolicyReader,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn get_console_policy_catalog(
        &self,
        actor_user_id: Uuid,
        inventory: &ConsoleOperationCompiledInventory,
        locale: &str,
    ) -> Result<ConsolePolicyCatalog> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_CONSOLE_POLICY_CATALOG_VIEW_OPERATION_ID)
            .await?;
        validate_complete_console_policy_catalog(inventory)?;
        let locale_catalog =
            inventory
                .locale_catalog
                .as_ref()
                .ok_or(ControlPlaneError::InvalidInput(
                    "console_policy_locale_catalog",
                ))?;
        build_console_policy_catalog_for_locale(inventory, locale_catalog, locale)
            .map_err(Into::into)
    }

    pub async fn get_console_policy(
        &self,
        actor_user_id: Uuid,
        role_code: &str,
        inventory: &ConsoleOperationCompiledInventory,
    ) -> Result<domain::RoleConsolePolicy> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_CONSOLE_POLICY_VIEW_OPERATION_ID)
            .await?;
        let operation_index = validate_complete_console_policy_catalog(inventory)?;
        let policy = self
            .repository
            .get_role_console_policy(actor.current_workspace_id, role_code)
            .await?;
        complete_stored_console_policy(policy, &operation_index).map_err(Into::into)
    }

    pub async fn replace_console_policy(
        &self,
        command: ReplaceRoleConsolePolicyCommand,
        inventory: &ConsoleOperationCompiledInventory,
    ) -> Result<domain::RoleConsolePolicy> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_CONSOLE_POLICY_REPLACE_OPERATION_ID)
            .await?;

        if self
            .repository
            .list_roles(actor.current_workspace_id)
            .await?
            .into_iter()
            .find(|role| role.code == command.role_code)
            .is_some_and(|role| role.is_builtin || !role.is_editable)
        {
            return Err(ControlPlaneError::PermissionDenied("builtin_role_immutable").into());
        }

        let operation_index = validate_complete_console_policy_catalog(inventory)?;
        let groups = role_console_policy_groups_from_input(&command.groups, &operation_index)?;
        let role_code = command.role_code.clone();
        let policy = self
            .repository
            .replace_role_console_policy(&crate::ports::ReplaceRoleConsolePolicyInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                role_code: role_code.clone(),
                groups,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.console_policy_replaced",
                serde_json::json!({
                    "code": role_code,
                    "schema_version": inventory.schema_version,
                }),
            ))
            .await?;
        Ok(policy)
    }

    pub async fn list_roles(&self, actor_user_id: Uuid) -> Result<Vec<domain::RoleTemplate>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_LIST_OPERATION_ID)
            .await?;
        self.repository.list_roles(actor.current_workspace_id).await
    }

    pub async fn get_role_permissions(
        &self,
        actor_user_id: Uuid,
        role_code: &str,
    ) -> Result<Vec<String>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_PERMISSIONS_VIEW_OPERATION_ID)
            .await?;
        self.repository
            .list_role_permissions(actor.current_workspace_id, role_code)
            .await
    }

    pub async fn get_role_data_policy(
        &self,
        actor_user_id: Uuid,
        role_code: &str,
    ) -> Result<RoleDataPolicyView> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_DATA_POLICY_VIEW_OPERATION_ID)
            .await?;
        self.repository
            .get_role_data_policy(actor.current_workspace_id, role_code)
            .await
    }

    pub async fn create_role(&self, command: CreateRoleCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_CREATE_OPERATION_ID)
            .await?;
        self.repository
            .create_team_role(&CreateWorkspaceRoleInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                code: command.code.clone(),
                name: command.name.clone(),
                introduction: command.introduction.clone(),
                auto_grant_new_permissions: command.auto_grant_new_permissions,
                is_default_member_role: command.is_default_member_role,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.created",
                serde_json::json!({ "code": command.code }),
            ))
            .await?;
        Ok(())
    }

    pub async fn update_role(&self, command: UpdateRoleCommand) -> Result<()> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_UPDATE_OPERATION_ID)
            .await?;
        self.repository
            .update_team_role(&UpdateWorkspaceRoleInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                role_code: command.role_code.clone(),
                name: command.name.clone(),
                introduction: command.introduction.clone(),
                auto_grant_new_permissions: command.auto_grant_new_permissions,
                is_default_member_role: command.is_default_member_role,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.updated",
                serde_json::json!({ "code": command.role_code }),
            ))
            .await?;
        Ok(())
    }

    pub async fn delete_role(&self, command: DeleteRoleCommand) -> Result<()> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_DELETE_OPERATION_ID)
            .await?;
        self.repository
            .delete_team_role(
                command.actor_user_id,
                actor.current_workspace_id,
                &command.role_code,
            )
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.deleted",
                serde_json::json!({ "code": command.role_code }),
            ))
            .await?;
        Ok(())
    }

    pub async fn replace_permissions(&self, command: ReplaceRolePermissionsCommand) -> Result<()> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_PERMISSIONS_REPLACE_OPERATION_ID)
            .await?;

        self.repository
            .replace_role_permissions(
                command.actor_user_id,
                actor.current_workspace_id,
                &command.role_code,
                &command.permission_codes,
            )
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.permissions_replaced",
                serde_json::json!({
                    "code": command.role_code,
                    "permission_codes": command.permission_codes,
                }),
            ))
            .await?;
        Ok(())
    }

    pub async fn replace_data_policy(
        &self,
        command: ReplaceRoleDataPolicyCommand,
    ) -> Result<RoleDataPolicyView> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        self.ensure_console_operation(&actor, ROLES_DATA_POLICY_REPLACE_OPERATION_ID)
            .await?;
        ensure_workspace_role_data_policy_allowed(
            &command.default_policy,
            &command.model_policies,
        )?;

        let policy = self
            .repository
            .replace_role_data_policy(&ReplaceRoleDataPolicyInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                role_code: command.role_code.clone(),
                default_policy: command.default_policy,
                model_policies: command.model_policies,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.data_policy_replaced",
                serde_json::json!({
                    "code": command.role_code,
                }),
            ))
            .await?;
        Ok(policy)
    }

    async fn ensure_console_operation(
        &self,
        actor: &domain::ActorContext,
        operation_id: &str,
    ) -> Result<()> {
        if actor.is_root {
            return Ok(());
        }
        let policies = self
            .repository
            .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
            .await?;
        let operation_id = domain::ConsoleOperationId::try_from(operation_id)
            .expect("compiled roles operation id must be valid");
        if domain::effective_console_simple_operation(
            &policies,
            &roles_console_group(),
            &operation_id,
        ) {
            Ok(())
        } else {
            Err(ControlPlaneError::PermissionDenied("permission_denied").into())
        }
    }
}

impl<R> RoleService<R>
where
    R: RoleRepository + RoleConsolePolicyReader + AuthRepository,
{
    pub async fn list_permission_options(
        &self,
        actor_user_id: Uuid,
    ) -> Result<Vec<domain::PermissionDefinition>> {
        let actor =
            RoleRepository::load_actor_context_for_user(&self.repository, actor_user_id).await?;
        self.ensure_console_operation(&actor, ROLES_PERMISSION_OPTIONS_LIST_OPERATION_ID)
            .await?;
        AuthRepository::list_permissions(&self.repository).await
    }
}

fn roles_console_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::settings_feature(SYSTEM_ROLES_SETTINGS_FEATURE_ID)
        .expect("compiled roles settings feature id must be valid")
}

impl<R> RoleService<R>
where
    R: RoleRepository + FrontstagePageRepository,
{
    pub async fn get_frontstage_routes(
        &self,
        actor_user_id: Uuid,
        role_code: &str,
    ) -> Result<RoleFrontstageRoutesView> {
        let actor =
            RoleRepository::load_actor_context_for_user(&self.repository, actor_user_id).await?;
        // Frontstage route visibility is explicitly outside this console-policy cutover.
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        RoleRepository::list_role_permissions(
            &self.repository,
            actor.current_workspace_id,
            role_code,
        )
        .await?;

        let pages = FrontstagePageRepository::list_frontstage_pages(
            &self.repository,
            actor.current_workspace_id,
        )
        .await?;
        let mut tabs = Vec::new();
        for page in pages
            .iter()
            .filter(|page| page.kind == domain::FrontstagePageKind::Page)
        {
            tabs.extend(
                FrontstagePageRepository::list_frontstage_page_tabs(
                    &self.repository,
                    actor.current_workspace_id,
                    page.id,
                )
                .await?,
            );
        }
        let rules = FrontstagePageRepository::list_frontstage_page_visibility_rules_for_role(
            &self.repository,
            actor.current_workspace_id,
            role_code,
        )
        .await?;

        Ok(RoleFrontstageRoutesView { pages, tabs, rules })
    }

    pub async fn replace_frontstage_routes(
        &self,
        command: ReplaceRoleFrontstageRoutesCommand,
    ) -> Result<()> {
        if command.role_code == "root" {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        let actor =
            RoleRepository::load_actor_context_for_user(&self.repository, command.actor_user_id)
                .await?;
        // Frontstage route visibility is explicitly outside this console-policy cutover.
        ensure_permission(&actor, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        RoleRepository::list_role_permissions(
            &self.repository,
            actor.current_workspace_id,
            &command.role_code,
        )
        .await?;

        FrontstagePageRepository::replace_frontstage_page_visibility_rules_for_role(
            &self.repository,
            actor.current_workspace_id,
            &command.role_code,
            &command.page_ids,
            &command.tab_ids,
            command.actor_user_id,
        )
        .await?;
        RoleRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "role",
                None,
                "role.frontstage_routes_replaced",
                serde_json::json!({
                    "code": command.role_code,
                    "page_ids": command.page_ids,
                    "tab_ids": command.tab_ids,
                }),
            ),
        )
        .await?;
        Ok(())
    }
}
