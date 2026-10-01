use uuid::Uuid;

pub struct CreateMcpInstanceCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: domain::McpInstanceStatus,
    pub default_entry_path: String,
    pub webmcp_exposure: domain::WebMcpExposure,
}

pub struct CopyMcpInstanceCommand {
    pub actor_user_id: Uuid,
    pub source_instance_id: String,
    pub instance_id: String,
    pub name: String,
}

pub struct UpsertMcpGroupCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

pub struct MoveMcpGroupCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub source_path: String,
    pub target_parent_path: String,
    pub sort_order: i32,
}

pub struct CreateMcpToolCommand {
    pub actor_user_id: Uuid,
    pub tool_id: String,
    pub des_id: Option<String>,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub interface_entry: domain::McpInterfaceCatalogEntry,
    pub input_mapping: serde_json::Value,
    pub output_mapping: serde_json::Value,
    pub max_inline_chars: Option<i64>,
    pub response_fields: Option<Vec<String>>,
    pub status: domain::McpToolStatus,
}

pub struct UpdateMcpToolCommand {
    pub actor_user_id: Uuid,
    pub tool_id: String,
    pub des_id: Option<String>,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub interface_entry: domain::McpInterfaceCatalogEntry,
    pub input_mapping: serde_json::Value,
    pub output_mapping: serde_json::Value,
    pub max_inline_chars: Option<i64>,
    pub response_fields: Option<Vec<String>>,
    pub status: domain::McpToolStatus,
}

pub struct RefreshMcpToolDescriptionCommand {
    pub actor_user_id: Uuid,
    pub tool_id: String,
}

pub struct CreateMcpToolBindingCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub group_path: String,
    pub tool_id: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

pub struct UpdateMcpToolBindingCommand {
    pub actor_user_id: Uuid,
    pub binding_id: Uuid,
    pub group_path: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}
pub struct UpdateMcpInstanceDiscoveryPolicyCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub list_default_limit: i32,
    pub list_max_depth: i32,
    pub list_regex_enabled: bool,
    pub list_regex_max_length: i32,
    pub list_return_fields: serde_json::Value,
}

pub struct SaveMcpClientCredentialCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub api_key: String,
    pub master_key: String,
}
