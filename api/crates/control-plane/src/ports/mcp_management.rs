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
    pub execution_target: domain::McpToolExecutionTarget,
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
    pub execution_target: domain::McpToolExecutionTarget,
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
pub struct CreateMcpUpstreamConnectionInput {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub endpoint: String,
    pub transport: domain::McpUpstreamTransport,
    pub auth_type: domain::McpUpstreamAuthType,
    pub custom_header_name: Option<String>,
    pub status: domain::McpUpstreamConnectionStatus,
}

#[derive(Debug, Clone)]
pub struct UpdateMcpUpstreamConnectionInput {
    pub id: Uuid,
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub endpoint: String,
    pub transport: domain::McpUpstreamTransport,
    pub auth_type: domain::McpUpstreamAuthType,
    pub custom_header_name: Option<String>,
    pub status: domain::McpUpstreamConnectionStatus,
}

#[derive(Debug, Clone)]
pub struct UpsertMcpUpstreamSecretInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub upstream_connection_id: Uuid,
    pub plaintext_secret_json: serde_json::Value,
    pub master_key: String,
}

#[derive(Debug, Clone)]
pub struct UpsertMcpUpstreamToolSourceInput {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub upstream_connection_id: Uuid,
    pub remote_tool_name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub schema_hash: String,
    pub source_status: domain::McpUpstreamSourceStatus,
    pub discovered_at: time::OffsetDateTime,
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

#[derive(Debug, Clone)]
pub struct CreateMcpInstanceGraphInput {
    pub instance: CreateMcpInstanceInput,
    pub groups: Vec<UpsertMcpGroupInput>,
    pub bindings: Vec<CreateMcpToolBindingInput>,
    pub discovery_policy: UpdateMcpInstanceDiscoveryPolicyInput,
}

#[derive(Debug, Clone)]
pub struct UpsertMcpClientCredentialInput {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_record_id: Uuid,
    pub api_key: String,
    pub master_key: String,
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
    async fn create_mcp_instance_graph_atomically(
        &self,
        input: &CreateMcpInstanceGraphInput,
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

    async fn get_mcp_client_credential(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        instance_record_id: Uuid,
        master_key: &str,
    ) -> anyhow::Result<Option<String>>;
    async fn upsert_mcp_client_credential(
        &self,
        input: &UpsertMcpClientCredentialInput,
    ) -> anyhow::Result<()>;
    async fn delete_mcp_client_credential(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        instance_record_id: Uuid,
    ) -> anyhow::Result<()>;

    async fn list_mcp_upstream_connections(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpUpstreamConnectionRecord>>;
    async fn get_mcp_upstream_connection(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> anyhow::Result<Option<domain::McpUpstreamConnectionRecord>>;
    async fn create_mcp_upstream_connection(
        &self,
        input: &CreateMcpUpstreamConnectionInput,
    ) -> anyhow::Result<domain::McpUpstreamConnectionRecord>;
    async fn update_mcp_upstream_connection(
        &self,
        input: &UpdateMcpUpstreamConnectionInput,
    ) -> anyhow::Result<domain::McpUpstreamConnectionRecord>;
    async fn delete_mcp_upstream_connection(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> anyhow::Result<()>;
    async fn get_mcp_upstream_secret(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        master_key: &str,
    ) -> anyhow::Result<Option<serde_json::Value>>;
    async fn upsert_mcp_upstream_secret(
        &self,
        input: &UpsertMcpUpstreamSecretInput,
    ) -> anyhow::Result<()>;
    async fn delete_mcp_upstream_secret(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> anyhow::Result<()>;
    async fn record_mcp_upstream_connection_result(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        connected_at: Option<time::OffsetDateTime>,
        discovered_at: Option<time::OffsetDateTime>,
        last_error: Option<&str>,
    ) -> anyhow::Result<()>;
    async fn list_mcp_upstream_tool_sources(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpUpstreamToolSourceRecord>>;
    async fn upsert_mcp_upstream_tool_source(
        &self,
        input: &UpsertMcpUpstreamToolSourceInput,
    ) -> anyhow::Result<domain::McpUpstreamToolSourceRecord>;
    async fn mark_mcp_upstream_tool_sources_missing(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        discovered_remote_tool_names: &[String],
    ) -> anyhow::Result<()>;
    async fn link_mcp_upstream_tool_source(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        remote_tool_name: &str,
        tool_record_id: Uuid,
    ) -> anyhow::Result<domain::McpUpstreamToolSourceRecord>;

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
    async fn move_mcp_group(
        &self,
        actor_user_id: Uuid,
        instance_record_id: Uuid,
        source_path: &str,
        target_path: &str,
        sort_order: i32,
    ) -> anyhow::Result<domain::McpGroupRecord>;
    async fn delete_mcp_group_subtree(
        &self,
        instance_record_id: Uuid,
        path: &str,
    ) -> anyhow::Result<()>;

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
