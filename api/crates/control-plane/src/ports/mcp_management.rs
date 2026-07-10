use super::*;

#[derive(Debug, Clone)]
pub struct CreateMcpInstanceInput {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: domain::McpInstanceStatus,
    pub default_entry_path: String,
}

#[derive(Debug, Clone)]
pub struct UpdateMcpInstanceInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: domain::McpInstanceStatus,
    pub default_entry_path: String,
}

#[derive(Debug, Clone)]
pub struct UpsertMcpGroupInput {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub instance_record_id: Uuid,
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct CreateMcpToolInput {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub tool_id: String,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub interface_id: String,
    pub parameter_schema: serde_json::Value,
    pub result_schema: serde_json::Value,
    pub input_mapping: serde_json::Value,
    pub output_mapping: serde_json::Value,
    pub permission_code: Option<String>,
    pub risk_level: domain::McpRiskLevel,
    pub des_id: String,
    pub des_id_required: bool,
    pub status: domain::McpToolStatus,
}

#[derive(Debug, Clone)]
pub struct UpdateMcpToolInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub tool_id: String,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub interface_id: String,
    pub parameter_schema: serde_json::Value,
    pub result_schema: serde_json::Value,
    pub input_mapping: serde_json::Value,
    pub output_mapping: serde_json::Value,
    pub permission_code: Option<String>,
    pub risk_level: domain::McpRiskLevel,
    pub des_id: String,
    pub des_id_required: bool,
    pub status: domain::McpToolStatus,
}

#[derive(Debug, Clone)]
pub struct CreateMcpToolBindingInput {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub instance_record_id: Uuid,
    pub tool_record_id: Uuid,
    pub group_path: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct UpdateMcpToolBindingInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub binding_id: Uuid,
    pub group_path: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct UpdateMcpInstanceDiscoveryPolicyInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_record_id: Uuid,
    pub list_default_limit: i32,
    pub list_max_depth: i32,
    pub list_regex_enabled: bool,
    pub list_regex_max_length: i32,
    pub list_return_fields: serde_json::Value,
}

#[async_trait]
pub trait McpManagementRepository: Send + Sync {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> anyhow::Result<ActorContext>;

    async fn list_mcp_instances(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpInstanceRecord>>;
    async fn get_mcp_instance(
        &self,
        workspace_id: Uuid,
        instance_id: &str,
    ) -> anyhow::Result<Option<domain::McpInstanceRecord>>;
    async fn create_mcp_instance(
        &self,
        input: &CreateMcpInstanceInput,
    ) -> anyhow::Result<domain::McpInstanceRecord>;
    async fn update_mcp_instance(
        &self,
        input: &UpdateMcpInstanceInput,
    ) -> anyhow::Result<domain::McpInstanceRecord>;
    async fn delete_mcp_instance(
        &self,
        workspace_id: Uuid,
        instance_id: &str,
    ) -> anyhow::Result<()>;

    async fn list_mcp_groups(
        &self,
        instance_record_ids: &[Uuid],
    ) -> anyhow::Result<Vec<domain::McpGroupRecord>>;
    async fn upsert_mcp_group(
        &self,
        input: &UpsertMcpGroupInput,
    ) -> anyhow::Result<domain::McpGroupRecord>;
    async fn update_mcp_group(
        &self,
        input: &UpsertMcpGroupInput,
    ) -> anyhow::Result<domain::McpGroupRecord>;
    async fn delete_mcp_group(&self, group_id: Uuid) -> anyhow::Result<()>;

    async fn list_mcp_tools(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpToolRecord>>;
    async fn get_mcp_tool(
        &self,
        workspace_id: Uuid,
        tool_id: &str,
    ) -> anyhow::Result<Option<domain::McpToolRecord>>;
    async fn create_mcp_tool(
        &self,
        input: &CreateMcpToolInput,
    ) -> anyhow::Result<domain::McpToolRecord>;
    async fn update_mcp_tool(
        &self,
        input: &UpdateMcpToolInput,
    ) -> anyhow::Result<domain::McpToolRecord>;
    async fn refresh_mcp_tool_des_id(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        tool_id: &str,
        des_id: &str,
    ) -> anyhow::Result<domain::McpToolRecord>;
    async fn delete_mcp_tool(&self, workspace_id: Uuid, tool_id: &str) -> anyhow::Result<()>;

    async fn list_mcp_tool_bindings(
        &self,
        instance_record_ids: &[Uuid],
    ) -> anyhow::Result<Vec<domain::McpToolBindingRecord>>;
    async fn create_mcp_tool_binding(
        &self,
        input: &CreateMcpToolBindingInput,
    ) -> anyhow::Result<domain::McpToolBindingRecord>;
    async fn update_mcp_tool_binding(
        &self,
        input: &UpdateMcpToolBindingInput,
    ) -> anyhow::Result<domain::McpToolBindingRecord>;
    async fn delete_mcp_tool_binding(
        &self,
        workspace_id: Uuid,
        binding_id: Uuid,
    ) -> anyhow::Result<()>;

    async fn list_mcp_instance_discovery_policies(
        &self,
        instance_record_ids: &[Uuid],
    ) -> anyhow::Result<Vec<domain::McpInstanceDiscoveryPolicyRecord>>;
    async fn get_mcp_instance_discovery_policy(
        &self,
        instance_record_id: Uuid,
    ) -> anyhow::Result<Option<domain::McpInstanceDiscoveryPolicyRecord>>;
    async fn update_mcp_instance_discovery_policy(
        &self,
        input: &UpdateMcpInstanceDiscoveryPolicyInput,
    ) -> anyhow::Result<domain::McpInstanceDiscoveryPolicyRecord>;
}
