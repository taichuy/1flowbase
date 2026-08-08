use super::*;

#[derive(Debug, Serialize, ToSchema)]
pub struct McpInstanceResponse {
    pub id: String,
    pub workspace_id: String,
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: String,
    pub default_entry_path: String,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub llm_tool_registration: McpLlmToolRegistrationResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpLlmToolRegistrationResponse {
    pub prefix: String,
    pub tools: Vec<McpLlmToolNameResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpLlmToolNameResponse {
    pub operation: String,
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveMcpClientCredentialBody {
    pub api_key: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpClientCredentialResponse {
    pub saved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpGroupResponse {
    pub id: String,
    pub instance_record_id: String,
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpToolResponse {
    pub id: String,
    pub workspace_id: String,
    pub tool_id: String,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub execution_target: McpToolExecutionTargetDto,
    pub operation: String,
    #[schema(value_type = Object)]
    pub parameter_schema: serde_json::Value,
    pub result_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub input_mapping: serde_json::Value,
    #[schema(value_type = Object)]
    pub output_mapping: serde_json::Value,
    pub permission_code: Option<String>,
    pub risk_level: String,
    pub des_id: String,
    pub des_id_required: bool,
    pub status: String,
    pub availability_status: McpToolAvailabilityStatusDto,
    pub availability_reason: Option<String>,
    pub revision: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpToolAvailabilityStatusDto {
    Available,
    InterfaceMissing,
    UpstreamDisabled,
    CredentialsMissing,
    UpstreamToolMissing,
    MappingInvalid,
}

impl From<domain::McpToolAvailabilityStatus> for McpToolAvailabilityStatusDto {
    fn from(status: domain::McpToolAvailabilityStatus) -> Self {
        match status {
            domain::McpToolAvailabilityStatus::Available => Self::Available,
            domain::McpToolAvailabilityStatus::InterfaceMissing => Self::InterfaceMissing,
            domain::McpToolAvailabilityStatus::UpstreamDisabled => Self::UpstreamDisabled,
            domain::McpToolAvailabilityStatus::CredentialsMissing => Self::CredentialsMissing,
            domain::McpToolAvailabilityStatus::UpstreamToolMissing => Self::UpstreamToolMissing,
            domain::McpToolAvailabilityStatus::MappingInvalid => Self::MappingInvalid,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpToolExecutionTargetDto {
    InterfaceWrapper {
        interface_id: String,
    },
    McpProxy {
        upstream_connection_id: String,
        remote_tool_name: String,
        source_schema_hash: String,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpToolBindingResponse {
    pub id: String,
    pub instance_record_id: String,
    pub tool_record_id: String,
    pub group_path: String,
    pub tool_id: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpInstanceDiscoveryPolicyResponse {
    pub id: String,
    pub workspace_id: String,
    pub instance_record_id: String,
    pub instance_id: String,
    pub list_default_limit: i32,
    pub list_max_depth: i32,
    pub list_regex_enabled: bool,
    pub list_regex_max_length: i32,
    #[schema(value_type = Vec<String>)]
    pub list_return_fields: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpCatalogResponse {
    pub instances: Vec<McpInstanceResponse>,
    pub groups: Vec<McpGroupResponse>,
    pub tools: Vec<McpToolResponse>,
    pub bindings: Vec<McpToolBindingResponse>,
    pub discovery_policies: Vec<McpInstanceDiscoveryPolicyResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpParameterDescriptorResponse {
    pub name: String,
    pub field_type: String,
    pub parameter_type: String,
    pub description: Option<String>,
    pub required: bool,
    pub schema: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpInterfaceCatalogEntryResponse {
    pub interface_id: String,
    pub method: String,
    pub path: String,
    pub name: String,
    pub short_description: String,
    pub parameter_descriptors: Vec<McpParameterDescriptorResponse>,
    pub parameter_schema: serde_json::Value,
    pub result_schema: serde_json::Value,
    pub permission_code: Option<String>,
    #[schema(value_type = [Object])]
    pub security: serde_json::Value,
    pub risk_level: String,
    pub bindable: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpDescriptionCheckResponse {
    pub accepted: bool,
    pub current_des_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpListItemSummaryResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpExportPackageResponse {
    pub instances: Vec<McpInstanceResponse>,
    pub groups: Vec<McpGroupResponse>,
    pub tools: Vec<McpToolResponse>,
    pub bindings: Vec<McpToolBindingResponse>,
    pub discovery_policies: Vec<McpInstanceDiscoveryPolicyResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMcpInstanceBody {
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: String,
    pub default_entry_path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CopyMcpInstanceBody {
    pub instance_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertMcpGroupBody {
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveMcpGroupBody {
    pub source_path: String,
    pub target_parent_path: String,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DeleteMcpGroupQuery {
    pub path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMcpToolBody {
    pub tool_id: String,
    pub des_id: Option<String>,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub execution_target: McpToolExecutionTargetDto,
    #[schema(value_type = Object)]
    pub parameter_schema: serde_json::Value,
    pub result_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub input_mapping: serde_json::Value,
    #[schema(value_type = Object)]
    pub output_mapping: serde_json::Value,
    pub permission_code: Option<String>,
    pub risk_level: String,
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMcpToolBody {
    pub name: String,
    pub des_id: Option<String>,
    pub short_description: String,
    pub full_description: String,
    pub execution_target: McpToolExecutionTargetDto,
    #[schema(value_type = Object)]
    pub parameter_schema: serde_json::Value,
    pub result_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub input_mapping: serde_json::Value,
    #[schema(value_type = Object)]
    pub output_mapping: serde_json::Value,
    pub permission_code: Option<String>,
    pub risk_level: String,
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMcpToolBindingBody {
    pub group_path: String,
    pub tool_id: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMcpToolBindingBody {
    pub group_path: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMcpInstanceDiscoveryPolicyBody {
    pub list_default_limit: i32,
    pub list_max_depth: i32,
    pub list_regex_enabled: bool,
    pub list_regex_max_length: i32,
    #[schema(value_type = Vec<String>)]
    pub list_return_fields: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct McpDescriptionCheckBody {
    pub des_id: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct McpInterfaceCatalogQuery {
    pub bindable_only: Option<bool>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct McpListQuery {
    pub instance_id: Option<String>,
    pub path: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub depth: Option<i32>,
    pub path_regex: Option<String>,
    pub limit: Option<usize>,
}
