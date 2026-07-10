use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpInstanceStatus {
    Draft,
    Enabled,
    Disabled,
    Archived,
}

impl McpInstanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpToolStatus {
    Draft,
    Enabled,
    Disabled,
    Archived,
}

impl McpToolStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl McpRiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpListItemKind {
    Group,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpParameterType {
    Url,
    Form,
    JsonBody,
}

impl McpParameterType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Form => "form",
            Self::JsonBody => "json_body",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpParameterDescriptor {
    pub name: String,
    pub field_type: String,
    pub parameter_type: McpParameterType,
    pub description: Option<String>,
    pub required: bool,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpInstanceRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: McpInstanceStatus,
    pub default_entry_path: String,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpGroupRecord {
    pub id: Uuid,
    pub instance_record_id: Uuid,
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolRecord {
    pub id: Uuid,
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
    pub risk_level: McpRiskLevel,
    pub des_id: String,
    pub des_id_required: bool,
    pub status: McpToolStatus,
    pub revision: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolBindingRecord {
    pub id: Uuid,
    pub instance_record_id: Uuid,
    pub tool_record_id: Uuid,
    pub group_path: String,
    pub tool_id: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpInstanceDiscoveryPolicyRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub instance_record_id: Uuid,
    pub list_default_limit: i32,
    pub list_max_depth: i32,
    pub list_regex_enabled: bool,
    pub list_regex_max_length: i32,
    pub list_return_fields: serde_json::Value,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpInterfaceCatalogEntry {
    pub interface_id: String,
    pub method: String,
    pub path: String,
    pub name: String,
    pub short_description: String,
    pub parameter_descriptors: Vec<McpParameterDescriptor>,
    pub parameter_schema: serde_json::Value,
    pub result_schema: serde_json::Value,
    pub permission_code: Option<String>,
    pub security: serde_json::Value,
    pub risk_level: McpRiskLevel,
    pub bindable: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpCatalogSnapshot {
    pub instances: Vec<McpInstanceRecord>,
    pub groups: Vec<McpGroupRecord>,
    pub tools: Vec<McpToolRecord>,
    pub bindings: Vec<McpToolBindingRecord>,
    pub discovery_policies: Vec<McpInstanceDiscoveryPolicyRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpExportPackage {
    pub instances: Vec<McpInstanceRecord>,
    pub groups: Vec<McpGroupRecord>,
    pub tools: Vec<McpToolRecord>,
    pub bindings: Vec<McpToolBindingRecord>,
    pub discovery_policies: Vec<McpInstanceDiscoveryPolicyRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpInstanceDirectoryExportPackage {
    pub instances: Vec<McpInstanceRecord>,
    pub groups: Vec<McpGroupRecord>,
    pub bindings: Vec<McpToolBindingRecord>,
    pub discovery_policies: Vec<McpInstanceDiscoveryPolicyRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpDescriptionCheckResult {
    pub accepted: bool,
    pub current_des_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpListItemSummary {
    pub id: String,
    pub item_kind: McpListItemKind,
    pub path: String,
    pub name: String,
    pub description_short: Option<String>,
    pub children_count: i64,
    pub risk_level: Option<McpRiskLevel>,
}
