use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpToolExecutionTarget {
    InterfaceWrapper {
        interface_id: String,
    },
    McpProxy {
        upstream_connection_id: Uuid,
        remote_tool_name: String,
        source_schema_hash: String,
    },
}

impl McpToolExecutionTarget {
    pub fn interface_id(&self) -> Option<&str> {
        match self {
            Self::InterfaceWrapper { interface_id } => Some(interface_id),
            Self::McpProxy { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpFieldMapping {
    pub source_path: String,
    pub target_path: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpFieldMappingError {
    path: String,
}

impl McpFieldMappingError {
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl std::fmt::Display for McpFieldMappingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "required MCP field mapping source is missing: {}",
            self.path
        )
    }
}

impl std::error::Error for McpFieldMappingError {}

pub fn apply_mcp_field_mapping(
    source: &serde_json::Value,
    mappings: &[McpFieldMapping],
) -> Result<serde_json::Value, McpFieldMappingError> {
    for mapping in mappings {
        validated_path_segments(&mapping.source_path)
            .map_err(|path| McpFieldMappingError { path })?;
        validated_path_segments(&mapping.target_path)
            .map_err(|path| McpFieldMappingError { path })?;
    }
    let mut target = serde_json::Value::Object(serde_json::Map::new());
    for mapping in mappings {
        let value = read_object_path(source, &mapping.source_path);
        let Some(value) = value else {
            if mapping.required {
                return Err(McpFieldMappingError {
                    path: mapping.source_path.clone(),
                });
            }
            continue;
        };
        write_object_path(&mut target, &mapping.target_path, value.clone())
            .map_err(|path| McpFieldMappingError { path })?;
    }
    Ok(target)
}

pub fn identity_mcp_field_mapping(schema: &serde_json::Value) -> Vec<McpFieldMapping> {
    let required = schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<std::collections::BTreeSet<_>>()
        })
        .unwrap_or_default();
    let mut mappings = Vec::new();
    collect_identity_mappings(schema, "", &required, &mut mappings);
    mappings
}

fn collect_identity_mappings(
    schema: &serde_json::Value,
    prefix: &str,
    required: &std::collections::BTreeSet<&str>,
    mappings: &mut Vec<McpFieldMapping>,
) {
    let Some(properties) = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
    else {
        return;
    };
    for (name, property) in properties {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        if property
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .is_some()
        {
            let nested_required = property
                .get("required")
                .and_then(serde_json::Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .unwrap_or_default();
            collect_identity_mappings(property, &path, &nested_required, mappings);
        } else {
            mappings.push(McpFieldMapping {
                source_path: path.clone(),
                target_path: path,
                required: required.contains(name.as_str()),
            });
        }
    }
}

fn read_object_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    validated_path_segments(path)
        .ok()?
        .into_iter()
        .try_fold(value, |current, segment| current.as_object()?.get(segment))
}

fn write_object_path(
    value: &mut serde_json::Value,
    path: &str,
    mapped: serde_json::Value,
) -> Result<(), String> {
    let segments = validated_path_segments(path)?;
    let mut current = value;
    for segment in &segments[..segments.len() - 1] {
        let object = current.as_object_mut().ok_or_else(|| path.to_string())?;
        current = object
            .entry((*segment).to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    }
    current
        .as_object_mut()
        .ok_or_else(|| path.to_string())?
        .insert(segments[segments.len() - 1].to_string(), mapped);
    Ok(())
}

fn validated_path_segments(path: &str) -> Result<Vec<&str>, String> {
    let segments = path.split('.').collect::<Vec<_>>();
    if segments.is_empty()
        || segments.iter().any(|segment| {
            segment.is_empty()
                || !segment.bytes().enumerate().all(|(index, byte)| match byte {
                    b'a'..=b'z' | b'A'..=b'Z' | b'_' => true,
                    b'0'..=b'9' | b'-' => index > 0,
                    _ => false,
                })
        })
    {
        return Err(path.to_string());
    }
    Ok(segments)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpCallToolResult {
    pub content: serde_json::Value,
    #[serde(rename = "structuredContent", skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Value>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl McpCallToolResult {
    pub fn map_structured_content(
        &self,
        mappings: &[McpFieldMapping],
    ) -> Result<Self, McpFieldMappingError> {
        let structured_content = self
            .structured_content
            .as_ref()
            .map(|content| apply_mcp_field_mapping(content, mappings))
            .transpose()?;
        Ok(Self {
            content: self.content.clone(),
            structured_content,
            is_error: self.is_error,
        })
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpUpstreamTransport {
    StreamableHttp,
}

impl McpUpstreamTransport {
    pub fn as_str(self) -> &'static str {
        "streamable_http"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpUpstreamAuthType {
    None,
    Bearer,
    CustomHeader,
}

impl McpUpstreamAuthType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bearer => "bearer",
            Self::CustomHeader => "custom_header",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpUpstreamConnectionStatus {
    Enabled,
    Disabled,
}

impl McpUpstreamConnectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpUpstreamSourceStatus {
    NotImported,
    Imported,
    DefinitionChanged,
    RemoteMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpToolAvailabilityStatus {
    Available,
    InterfaceMissing,
    UpstreamDisabled,
    CredentialsMissing,
    UpstreamToolMissing,
    MappingInvalid,
}

impl McpToolAvailabilityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::InterfaceMissing => "interface_missing",
            Self::UpstreamDisabled => "upstream_disabled",
            Self::CredentialsMissing => "credentials_missing",
            Self::UpstreamToolMissing => "upstream_tool_missing",
            Self::MappingInvalid => "mapping_invalid",
        }
    }
}

impl McpUpstreamSourceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotImported => "not_imported",
            Self::Imported => "imported",
            Self::DefinitionChanged => "definition_changed",
            Self::RemoteMissing => "remote_missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpUpstreamConnectionRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub endpoint: String,
    pub transport: McpUpstreamTransport,
    pub auth_type: McpUpstreamAuthType,
    pub custom_header_name: Option<String>,
    pub status: McpUpstreamConnectionStatus,
    pub credentials_configured: bool,
    pub last_connected_at: Option<OffsetDateTime>,
    pub last_discovered_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpUpstreamToolSourceRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub upstream_connection_id: Uuid,
    pub remote_tool_name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub schema_hash: String,
    pub source_status: McpUpstreamSourceStatus,
    pub imported_tool_id: Option<String>,
    pub discovered_at: OffsetDateTime,
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
    pub execution_target: McpToolExecutionTarget,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpInterfaceCatalogSource {
    StaticApi,
    PublishedWorkflow,
    BuiltinDataModelCrud,
    WorkspaceDataModelCrud,
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
    pub source: McpInterfaceCatalogSource,
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
