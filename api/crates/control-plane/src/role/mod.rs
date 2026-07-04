use access_control::ensure_permission;
use anyhow::Result;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        CreateWorkspaceRoleInput, ReplaceRoleDataPolicyInput, RoleDataModelPolicyInput,
        RoleDataPolicyDefaultsInput, RoleDataPolicyView, RoleRepository, UpdateWorkspaceRoleInput,
    },
};

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
    R: RoleRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_roles(&self, actor_user_id: Uuid) -> Result<Vec<domain::RoleTemplate>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_permission(&actor, "role_permission.view.all")
            .map_err(ControlPlaneError::PermissionDenied)?;
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
        ensure_permission(&actor, "role_permission.view.all")
            .map_err(ControlPlaneError::PermissionDenied)?;
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
        ensure_permission(&actor, "role_permission.view.all")
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .get_role_data_policy(actor.current_workspace_id, role_code)
            .await
    }

    pub async fn create_role(&self, command: CreateRoleCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, "role_permission.manage.all")
            .map_err(ControlPlaneError::PermissionDenied)?;
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
        ensure_permission(&actor, "role_permission.manage.all")
            .map_err(ControlPlaneError::PermissionDenied)?;
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
        ensure_permission(&actor, "role_permission.manage.all")
            .map_err(ControlPlaneError::PermissionDenied)?;
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
        ensure_permission(&actor, "role_permission.manage.all")
            .map_err(ControlPlaneError::PermissionDenied)?;

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
        ensure_permission(&actor, "role_permission.manage.all")
            .map_err(ControlPlaneError::PermissionDenied)?;
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
}
