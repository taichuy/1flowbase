//! Portable definition projections. No ownership, physical storage or credentials.
use crate::application_public_api::ApplicationApiMappingConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

pub const PORTABLE_TEMPLATE_SCHEMA_VERSION: &str = "1flowbase.portable-template/v1";
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableTemplateSelection {
    #[serde(default)]
    pub page_ids: Vec<Uuid>,
    #[serde(default)]
    pub application_ids: Vec<Uuid>,
    #[serde(default)]
    pub data_model_ids: Vec<Uuid>,
    #[serde(default)]
    pub mcp_instance_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableTemplatePackage {
    pub schema_version: String,
    pub pages: Vec<PortablePage>,
    pub applications: Vec<PortableApplication>,
    pub data_models: Vec<PortableDataModel>,
    #[serde(default)]
    pub mcp_bundle: Option<domain::McpBundlePackage>,
    #[serde(default)]
    pub plugins: Vec<PortablePluginDependency>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableDataModel {
    pub id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub scope_kind: domain::DataModelScopeKind,
    pub template_provider: String,
    pub template_code: String,
    pub template_version: String,
    pub status: domain::DataModelStatus,
    /// Protected definitions are references, never recreated or overwritten.
    pub builtin: bool,
    pub fields: Vec<PortableModelField>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableModelField {
    pub id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub field_kind: domain::ModelFieldKind,
    pub is_system: bool,
    pub is_required: bool,
    pub api_required: bool,
    pub is_unique: bool,
    pub default_value: Option<Value>,
    pub display_interface: Option<String>,
    pub display_options: Value,
    pub relation_target_model_id: Option<Uuid>,
    pub relation_options: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableApplication {
    pub id: Uuid,
    pub application_type: domain::ApplicationType,
    pub workflow_trigger_type: Option<domain::WorkflowTriggerType>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
    pub flow_document: Value,
    pub mapping: Option<ApplicationApiMappingConfig>,
    pub published: Option<PortablePublication>,
    pub schedule: Option<PortableSchedule>,
    #[serde(default)]
    pub dependency_issues: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortablePublication {
    pub flow_document: Value,
    pub mapping: ApplicationApiMappingConfig,
    pub api_enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableSchedule {
    #[serde(default = "portable_schedule_default_enabled")]
    pub enabled: bool,
    pub cron: String,
    pub timezone: String,
    pub input_payload: Value,
}
fn portable_schedule_default_enabled() -> bool {
    true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortablePage {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: domain::FrontstagePageKind,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub is_hidden: bool,
    pub placement: domain::frontstage::FrontstageNavigationPlacement,
    pub content_presentation: domain::frontstage::FrontstagePageContentPresentation,
    pub slug: Option<String>,
    pub rank: String,
    pub tabs: Vec<PortableTab>,
    #[serde(default)]
    pub visibility_rules: Vec<PortablePageVisibilityRule>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortablePageVisibilityRule {
    pub role_code: String,
    pub tab_id: Option<Uuid>,
    pub visibility: domain::frontstage::FrontstagePageVisibility,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableTab {
    pub id: Uuid,
    pub title: Option<String>,
    pub rank: String,
    pub is_default: bool,
    pub route_segment: Option<String>,
    pub document_root_uid: String,
    pub document_payload: Value,
    pub blocks: Vec<PortableBlock>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableBlock {
    pub block_id: String,
    pub parent_block_id: Option<String>,
    pub rank: String,
    pub presentation: domain::frontstage::FrontstageBlockPresentation,
    pub title: Option<String>,
    pub description: Option<String>,
    pub code_ref: String,
    pub schema_version: u32,
    pub input_mapping: BTreeMap<String, String>,
    pub output_mapping: BTreeMap<String, String>,
    pub runtime_descriptor: Value,
    pub source_code: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortablePluginDependency {
    pub plugin_id: String,
    pub plugin_version: String,
    pub checksum: Option<String>,
    pub contribution_code: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PortableTemplateCatalog {
    pub pages: Vec<PortableCatalogItem>,
    pub applications: Vec<PortableCatalogItem>,
    pub data_models: Vec<PortableCatalogItem>,
    pub mcp_instances: Vec<PortableMcpCatalogItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableMcpCatalogItem {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableCatalogItem {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub code: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PortableTemplateCounts {
    pub pages: usize,
    pub applications: usize,
    pub data_models: usize,
    pub mcp_instances: usize,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PortableTemplatePreview {
    pub valid: bool,
    pub counts: PortableTemplateCounts,
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
    pub dependencies: Vec<PortablePluginDependency>,
    #[serde(default)]
    pub effects: Vec<PortableTemplateEffect>,
    #[serde(default)]
    pub mcp_shared_tool_impacts: Vec<domain::McpBundleSharedToolImpact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortableTemplateEffect {
    pub kind: String,
    pub source_id: String,
    pub target_id: Option<String>,
    pub action: String,
}
