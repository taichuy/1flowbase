use anyhow::Result;
use domain::{ActorContext, SessionRecord};
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{AuthRepository, SessionStore, WorkspaceRepository},
};

#[derive(Debug)]
pub struct SwitchWorkspaceCommand {
    pub actor_user_id: Uuid,
    pub session_id: String,
    pub target_workspace_id: Uuid,
}

#[derive(Debug)]
pub struct SwitchWorkspaceResult {
    pub actor: ActorContext,
    pub session: SessionRecord,
}

#[derive(Debug)]
pub struct SwitchActiveRoleCommand {
    pub actor_user_id: Uuid,
    pub session_id: String,
    pub active_role_code: String,
}

pub type SwitchActiveRoleResult = SwitchWorkspaceResult;

pub struct WorkspaceSessionService<R, T, S> {
    auth_repository: R,
    workspace_repository: T,
    session_store: S,
}

impl<R, T, S> WorkspaceSessionService<R, T, S>
where
    R: AuthRepository,
    T: WorkspaceRepository,
    S: SessionStore,
{
    pub fn new(auth_repository: R, workspace_repository: T, session_store: S) -> Self {
        Self {
            auth_repository,
            workspace_repository,
            session_store,
        }
    }

    pub async fn switch_workspace(
        &self,
        command: SwitchWorkspaceCommand,
    ) -> Result<SwitchWorkspaceResult> {
        let current_session = self
            .session_store
            .get(&command.session_id)
            .await?
            .ok_or(ControlPlaneError::NotAuthenticated)?;
        if current_session.user_id != command.actor_user_id {
            return Err(ControlPlaneError::NotAuthenticated.into());
        }

        if command.target_workspace_id == current_session.current_workspace_id {
            let actor = self
                .auth_repository
                .load_actor_context(
                    command.actor_user_id,
                    current_session.tenant_id,
                    current_session.current_workspace_id,
                    Some(&current_session.active_role_code),
                )
                .await?;

            return Ok(SwitchWorkspaceResult {
                actor,
                session: current_session,
            });
        }

        let target_workspace = self
            .workspace_repository
            .get_accessible_workspace(command.actor_user_id, command.target_workspace_id)
            .await?
            .ok_or(ControlPlaneError::PermissionDenied(
                "workspace_access_denied",
            ))?;

        let actor = self
            .auth_repository
            .load_actor_context(
                command.actor_user_id,
                target_workspace.tenant_id,
                target_workspace.id,
                None,
            )
            .await?;

        let next_session = SessionRecord {
            session_id: current_session.session_id.clone(),
            user_id: current_session.user_id,
            tenant_id: target_workspace.tenant_id,
            current_workspace_id: target_workspace.id,
            active_role_code: actor.effective_display_role.clone(),
            session_version: current_session.session_version,
            csrf_token: Uuid::now_v7().to_string(),
            expires_at_unix: current_session.expires_at_unix,
        };

        self.session_store.put(next_session.clone()).await?;
        self.auth_repository
            .append_audit_log(&audit_log(
                Some(next_session.current_workspace_id),
                Some(command.actor_user_id),
                "session",
                None,
                "session.switch_workspace",
                serde_json::json!({
                    "from_workspace_id": current_session.current_workspace_id,
                    "to_workspace_id": next_session.current_workspace_id,
                }),
            ))
            .await?;

        Ok(SwitchWorkspaceResult {
            actor,
            session: next_session,
        })
    }

    pub async fn switch_active_role(
        &self,
        command: SwitchActiveRoleCommand,
    ) -> Result<SwitchActiveRoleResult> {
        let current_session = self
            .session_store
            .get(&command.session_id)
            .await?
            .ok_or(ControlPlaneError::NotAuthenticated)?;
        if current_session.user_id != command.actor_user_id {
            return Err(ControlPlaneError::NotAuthenticated.into());
        }

        let actor = self
            .auth_repository
            .load_actor_context(
                command.actor_user_id,
                current_session.tenant_id,
                current_session.current_workspace_id,
                Some(&command.active_role_code),
            )
            .await?;
        if actor.effective_display_role != command.active_role_code {
            return Err(ControlPlaneError::PermissionDenied("role_not_bound").into());
        }

        let next_session = SessionRecord {
            active_role_code: actor.effective_display_role.clone(),
            csrf_token: Uuid::now_v7().to_string(),
            ..current_session.clone()
        };
        self.session_store.put(next_session.clone()).await?;
        self.auth_repository
            .append_audit_log(&audit_log(
                Some(next_session.current_workspace_id),
                Some(command.actor_user_id),
                "session",
                None,
                "session.switch_active_role",
                serde_json::json!({
                    "from_role_code": current_session.active_role_code,
                    "to_role_code": next_session.active_role_code,
                }),
            ))
            .await?;

        Ok(SwitchActiveRoleResult {
            actor,
            session: next_session,
        })
    }
}
