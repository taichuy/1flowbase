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
        let catalog = self.read_catalog_for_actor(&actor).await?;
        let tool = catalog
            .tools
            .iter()
            .find(|tool| tool.tool_id == tool_id)
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
        let valid =
            domain::mcp_management::validate_mcp_call_parameters(&tool.input_mapping).is_ok();
        let description =
            match domain::mcp_management::mcp_description_tool_id(&tool.input_mapping) {
                None => Some(tool),
                Some(id) => catalog.tools.iter().find(|target| {
                    target.tool_id == id
                        && target.status == domain::McpToolStatus::Enabled
                        && catalog.bindings.iter().any(|binding| {
                            binding.visible
                                && binding.tool_id == target.tool_id
                                && binding.tool_record_id == target.id
                                && catalog.instances.iter().any(|instance| {
                                    instance.id == binding.instance_record_id
                                        && instance.status == domain::McpInstanceStatus::Enabled
                                })
                        })
                }),
            }
            .filter(|_| valid);
        Ok(domain::McpDescriptionCheckResult {
            accepted: description.is_some_and(|target| des_id == Some(target.des_id.as_str())),
            current_des_id: description.map(|target| target.des_id.clone()),
        })
    }
}
