use anyhow::Result;
use domain::{ActorContext, WorkspaceRecord};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{RoleConsolePolicyReader, WorkspaceRepository},
};

const WORKSPACE_UPDATE_OPERATION_ID: &str = "workspace.update";

pub struct UpdateWorkspaceCommand {
    pub actor: ActorContext,
    pub workspace_id: Uuid,
    pub name: String,
    pub logo_url: Option<String>,
    pub introduction: String,
}

pub struct WorkspaceService<R> {
    repository: R,
}

impl<R> WorkspaceService<R>
where
    R: WorkspaceRepository + RoleConsolePolicyReader,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn get_workspace(&self, workspace_id: Uuid) -> Result<WorkspaceRecord> {
        self.repository
            .get_workspace(workspace_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("workspace").into())
    }

    pub async fn list_accessible_workspaces(&self, user_id: Uuid) -> Result<Vec<WorkspaceRecord>> {
        self.repository.list_accessible_workspaces(user_id).await
    }

    pub async fn get_accessible_workspace(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<WorkspaceRecord> {
        self.repository
            .get_accessible_workspace(user_id, workspace_id)
            .await?
            .ok_or(ControlPlaneError::PermissionDenied("workspace_access_denied").into())
    }

    pub async fn update_workspace(
        &self,
        command: UpdateWorkspaceCommand,
    ) -> Result<WorkspaceRecord> {
        if command.workspace_id != command.actor.current_workspace_id {
            return Err(ControlPlaneError::PermissionDenied("workspace_access_denied").into());
        }
        if !command.actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(&command.actor)
                .await?;
            let operation_id = domain::ConsoleOperationId::try_from(WORKSPACE_UPDATE_OPERATION_ID)
                .expect("compiled workspace update operation id must be valid");
            let group = domain::ConsolePolicyGroup::other("other.workspace")
                .expect("compiled workspace policy group must be valid");
            if !domain::effective_console_simple_operation(&policies, &group, &operation_id) {
                return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
            }
        }

        self.repository
            .update_workspace(
                command.actor.user_id,
                command.workspace_id,
                &command.name,
                command.logo_url.as_deref(),
                &command.introduction,
            )
            .await
    }
}
