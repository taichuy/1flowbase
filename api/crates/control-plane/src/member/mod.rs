use access_control::{ensure_permission, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION};
use anyhow::Result;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{CreateMemberInput, MemberRepository, RoleRepository, UpdateMemberInput},
};

pub struct CreateMemberCommand {
    pub actor_user_id: Uuid,
    pub account: String,
    pub email: String,
    pub phone: Option<String>,
    pub password_hash: String,
    pub name: String,
    pub nickname: String,
    pub introduction: String,
    pub email_login_enabled: bool,
    pub phone_login_enabled: bool,
}

pub struct DisableMemberCommand {
    pub actor_user_id: Uuid,
    pub target_user_id: Uuid,
}

pub struct EnableMemberCommand {
    pub actor_user_id: Uuid,
    pub target_user_id: Uuid,
}

pub struct DeleteMemberCommand {
    pub actor_user_id: Uuid,
    pub target_user_id: Uuid,
}

pub struct UpdateMemberCommand {
    pub actor_user_id: Uuid,
    pub target_user_id: Uuid,
    pub name: String,
    pub nickname: String,
    pub email: String,
    pub phone: Option<String>,
    pub introduction: String,
}

pub struct ResetMemberPasswordCommand {
    pub actor_user_id: Uuid,
    pub target_user_id: Uuid,
    pub password_hash: String,
}

pub struct ReplaceMemberRolesCommand {
    pub actor_user_id: Uuid,
    pub target_user_id: Uuid,
    pub role_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignableRoleOption {
    pub code: String,
    pub name: String,
}

pub struct MemberService<R> {
    repository: R,
}

impl<R> MemberService<R>
where
    R: MemberRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_members(&self, actor_user_id: Uuid) -> Result<Vec<domain::UserRecord>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .list_members(actor.current_workspace_id)
            .await
    }

    pub async fn create_member(&self, command: CreateMemberCommand) -> Result<domain::UserRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;

        let user = self
            .repository
            .create_member_with_default_role(&CreateMemberInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                account: command.account,
                email: command.email,
                phone: command.phone,
                password_hash: command.password_hash,
                name: command.name,
                nickname: command.nickname,
                introduction: command.introduction,
                email_login_enabled: command.email_login_enabled,
                phone_login_enabled: command.phone_login_enabled,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "user",
                Some(user.id),
                "member.created",
                serde_json::json!({ "account": user.account }),
            ))
            .await?;

        Ok(user)
    }

    pub async fn update_member(&self, command: UpdateMemberCommand) -> Result<domain::UserRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;

        let user = self
            .repository
            .update_member_profile(&UpdateMemberInput {
                actor_user_id: command.actor_user_id,
                user_id: command.target_user_id,
                name: command.name,
                nickname: command.nickname,
                email: command.email,
                phone: command.phone,
                introduction: command.introduction,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "user",
                Some(command.target_user_id),
                "member.updated",
                serde_json::json!({ "account": user.account }),
            ))
            .await?;

        Ok(user)
    }

    pub async fn disable_member(&self, command: DisableMemberCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .disable_member(command.actor_user_id, command.target_user_id)
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "user",
                Some(command.target_user_id),
                "member.disabled",
                serde_json::json!({}),
            ))
            .await?;
        Ok(())
    }

    pub async fn enable_member(&self, command: EnableMemberCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .enable_member(command.actor_user_id, command.target_user_id)
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "user",
                Some(command.target_user_id),
                "member.enabled",
                serde_json::json!({}),
            ))
            .await?;
        Ok(())
    }

    pub async fn delete_member(&self, command: DeleteMemberCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .delete_member(command.actor_user_id, command.target_user_id)
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "user",
                Some(command.target_user_id),
                "member.deleted",
                serde_json::json!({}),
            ))
            .await?;
        Ok(())
    }

    pub async fn reset_member_password(&self, command: ResetMemberPasswordCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .reset_member_password(
                command.actor_user_id,
                command.target_user_id,
                &command.password_hash,
            )
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "user",
                Some(command.target_user_id),
                "member.password_reset",
                serde_json::json!({}),
            ))
            .await?;
        Ok(())
    }

    pub async fn replace_member_roles(&self, command: ReplaceMemberRolesCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;
        self.repository
            .replace_member_roles(
                command.actor_user_id,
                actor.current_workspace_id,
                command.target_user_id,
                &command.role_codes,
            )
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "user",
                Some(command.target_user_id),
                "member.roles_replaced",
                serde_json::json!({ "role_codes": command.role_codes }),
            ))
            .await?;
        Ok(())
    }
}

impl<R> MemberService<R>
where
    R: MemberRepository + RoleRepository,
{
    pub async fn list_assignable_role_options(
        &self,
        actor_user_id: Uuid,
    ) -> Result<Vec<AssignableRoleOption>> {
        let actor =
            MemberRepository::load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_permission(&actor, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION)
            .map_err(ControlPlaneError::PermissionDenied)?;

        Ok(
            RoleRepository::list_roles(&self.repository, actor.current_workspace_id)
                .await?
                .into_iter()
                .filter(|role| {
                    role.scope_kind == domain::RoleScopeKind::Workspace && role.code != "root"
                })
                .map(|role| AssignableRoleOption {
                    code: role.code,
                    name: role.name,
                })
                .collect(),
        )
    }
}
