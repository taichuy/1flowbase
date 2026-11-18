use super::*;
use crate::ports::ReplaceMcpInstanceDirectoryInput;
use domain::mcp_management::{McpDirectoryBinding, McpDirectoryGroup, McpInstanceDirectory};

pub struct ReplaceMcpInstanceDirectoryCommand {
    pub instance_id: String,
    pub expected_snapshot: McpInstanceDirectory,
    pub groups: Vec<McpDirectoryGroup>,
    pub bindings: Vec<McpDirectoryBinding>,
}

impl<R: McpManagementRepository> McpManagementService<R> {
    pub async fn read_instance_directory_for_actor(
        &self,
        actor: &domain::ActorContext,
        instance_id: &str,
    ) -> Result<McpInstanceDirectory> {
        validate_identifier(instance_id, "instance_id")?;
        self.repository
            .get_mcp_instance_directory(actor.current_workspace_id, instance_id)
            .await
    }

    pub async fn replace_instance_directory_for_actor(
        &self,
        actor: &domain::ActorContext,
        mut command: ReplaceMcpInstanceDirectoryCommand,
    ) -> Result<McpInstanceDirectory> {
        validate_identifier(&command.instance_id, "instance_id")?;
        let mut paths = HashSet::new();
        let mut ids = HashSet::new();
        for group in &mut command.groups {
            validate_group_path(&group.path)?;
            group.display_name = normalize_group_display_name(&group.path, &group.display_name)?;
            if !paths.insert(group.path.clone()) || group.id.is_some_and(|id| !ids.insert(id)) {
                return Err(ControlPlaneError::Conflict("mcp_directory_group").into());
            }
        }
        for group in &command.groups {
            let parent = group
                .path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or_default();
            if !parent.is_empty() && !paths.contains(parent) {
                return Err(ControlPlaneError::InvalidInput("group_parent").into());
            }
        }
        ids.clear();
        let mut identities = HashSet::new();
        for binding in &command.bindings {
            validate_identifier(&binding.tool_id, "tool_id")?;
            if binding.group_path != "/" && !paths.contains(binding.group_path.as_str()) {
                return Err(ControlPlaneError::InvalidInput("group_path").into());
            }
            if !identities.insert((&binding.group_path, &binding.tool_id))
                || binding.id.is_some_and(|id| !ids.insert(id))
            {
                return Err(ControlPlaneError::Conflict("mcp_directory_binding").into());
            }
        }
        self.repository
            .replace_mcp_instance_directory_atomically(&ReplaceMcpInstanceDirectoryInput {
                actor_user_id: actor.user_id,
                workspace_id: actor.current_workspace_id,
                instance_id: command.instance_id,
                expected_snapshot: command.expected_snapshot,
                groups: command.groups,
                bindings: command.bindings,
            })
            .await
    }
}
