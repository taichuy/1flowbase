use super::*;

impl<R> McpManagementService<R>
where
    R: McpManagementRepository,
{
    pub async fn description_check(
        &self,
        actor_user_id: Uuid,
        tool_id: &str,
        des_id: Option<&str>,
    ) -> Result<domain::McpDescriptionCheckResult> {
        let actor = self.authorize_view(actor_user_id).await?;
        let workspace_id = actor.current_workspace_id;
        let tool = self
            .repository
            .get_mcp_tool(workspace_id, tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
        let has_reference = tool
            .input_mapping
            .get("mappings")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .any(|mapping| mapping.pointer("/source/tool_id").is_some());
        if has_reference
            && domain::mcp_management::validate_mcp_call_parameters(&tool.input_mapping).is_err()
        {
            return Ok(domain::McpDescriptionCheckResult {
                accepted: false,
                current_des_id: None,
            });
        }
        if !domain::mcp_management::mcp_des_id_required(&tool.input_mapping) {
            return Ok(domain::McpDescriptionCheckResult {
                accepted: true,
                current_des_id: Some(tool.des_id.clone()),
            });
        }
        let description =
            if domain::mcp_management::validate_mcp_call_parameters(&tool.input_mapping).is_err() {
                None
            } else if let Some(id) =
                domain::mcp_management::mcp_description_tool_id(&tool.input_mapping)
            {
                if let Some(target) = self
                    .repository
                    .get_mcp_tool(workspace_id, id)
                    .await?
                    .filter(|target| target.status == domain::McpToolStatus::Enabled)
                {
                    let enabled_instances = self
                        .repository
                        .list_mcp_instances(workspace_id)
                        .await?
                        .into_iter()
                        .filter(|instance| instance.status == domain::McpInstanceStatus::Enabled)
                        .map(|instance| instance.id)
                        .collect::<Vec<_>>();
                    self.repository
                        .list_mcp_tool_bindings(&enabled_instances)
                        .await?
                        .iter()
                        .any(|binding| {
                            binding.visible
                                && binding.tool_id == target.tool_id
                                && binding.tool_record_id == target.id
                        })
                        .then_some(target)
                } else {
                    None
                }
            } else {
                Some(tool)
            };
        Ok(domain::McpDescriptionCheckResult {
            accepted: description
                .as_ref()
                .is_some_and(|target| des_id == Some(target.des_id.as_str())),
            current_des_id: description.map(|target| target.des_id),
        })
    }
}
