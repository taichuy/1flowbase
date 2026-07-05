use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataModelScopeKind {
    Workspace,
    System,
}

impl DataModelScopeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::System => "system",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "system" => Self::System,
            _ => Self::Workspace,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataModelStatus {
    Draft,
    Published,
    Disabled,
    Broken,
}

impl DataModelStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Disabled => "disabled",
            Self::Broken => "broken",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "draft" => Self::Draft,
            "disabled" => Self::Disabled,
            "broken" => Self::Broken,
            _ => Self::Published,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataModelSourceKind {
    MainSource,
    ExternalSource,
}

impl DataModelSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MainSource => "main_source",
            Self::ExternalSource => "external_source",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "external_source" => Self::ExternalSource,
            _ => Self::MainSource,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataModelOwnerKind {
    Core,
    HostExtension,
    RuntimeExtension,
}

impl DataModelOwnerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::HostExtension => "host_extension",
            Self::RuntimeExtension => "runtime_extension",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "host_extension" => Self::HostExtension,
            "runtime_extension" => Self::RuntimeExtension,
            _ => Self::Core,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataModelProtection {
    pub owner_kind: DataModelOwnerKind,
    pub owner_id: Option<String>,
    pub is_protected: bool,
}

impl Default for DataModelProtection {
    fn default() -> Self {
        Self {
            owner_kind: DataModelOwnerKind::Core,
            owner_id: None,
            is_protected: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataModelAdvisorSeverity {
    Blocking,
    High,
    Medium,
    Info,
}

impl DataModelAdvisorSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataModelAdvisorFinding {
    pub id: String,
    pub data_model_id: Uuid,
    pub severity: DataModelAdvisorSeverity,
    pub code: String,
    pub message: String,
    pub recommended_action: String,
    pub can_acknowledge: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeDataModelPermissionProfile {
    Owner,
    ScopeAll,
    SystemAll,
}

impl ScopeDataModelPermissionProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::ScopeAll => "scope_all",
            Self::SystemAll => "system_all",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "scope_all" => Some(Self::ScopeAll),
            "system_all" => Some(Self::SystemAll),
            _ => None,
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "scope_all" => Self::ScopeAll,
            "system_all" => Self::SystemAll,
            _ => Self::Owner,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScopeDataModelGrantRecord {
    pub id: Uuid,
    pub scope_kind: DataModelScopeKind,
    pub scope_id: Uuid,
    pub data_model_id: Uuid,
    pub enabled: bool,
    pub permission_profile: ScopeDataModelPermissionProfile,
    pub created_by: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RoleDataPolicyScope {
    Own,
    ScopeAll,
    SystemAll,
}

impl RoleDataPolicyScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::ScopeAll => "scope_all",
            Self::SystemAll => "system_all",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "own" => Some(Self::Own),
            "scope_all" => Some(Self::ScopeAll),
            "system_all" => Some(Self::SystemAll),
            _ => None,
        }
    }

    pub fn from_db(value: &str) -> Self {
        Self::parse(value).unwrap_or(Self::Own)
    }

    pub fn from_permission_profile(profile: ScopeDataModelPermissionProfile) -> Self {
        match profile {
            ScopeDataModelPermissionProfile::Owner => Self::Own,
            ScopeDataModelPermissionProfile::ScopeAll => Self::ScopeAll,
            ScopeDataModelPermissionProfile::SystemAll => Self::SystemAll,
        }
    }

    pub fn to_permission_profile(self) -> ScopeDataModelPermissionProfile {
        match self {
            Self::Own => ScopeDataModelPermissionProfile::Owner,
            Self::ScopeAll => ScopeDataModelPermissionProfile::ScopeAll,
            Self::SystemAll => ScopeDataModelPermissionProfile::SystemAll,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleDataPolicyRecord {
    pub id: Uuid,
    pub role_id: Uuid,
    pub role_code: String,
    pub can_view: bool,
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub default_view_scope: RoleDataPolicyScope,
    pub default_update_scope: RoleDataPolicyScope,
    pub default_delete_scope: RoleDataPolicyScope,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleDataModelPolicyRecord {
    pub id: Uuid,
    pub role_id: Uuid,
    pub data_model_id: Uuid,
    pub can_create_override: Option<bool>,
    pub view_scope_override: Option<RoleDataPolicyScope>,
    pub update_scope_override: Option<RoleDataPolicyScope>,
    pub delete_scope_override: Option<RoleDataPolicyScope>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataAvailabilityStatus {
    Available,
    Unavailable,
    Broken,
}

impl MetadataAvailabilityStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::Broken => "broken",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "broken" => Self::Broken,
            "unavailable" => Self::Unavailable,
            _ => Self::Available,
        }
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Available)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFieldKind {
    String,
    Number,
    Boolean,
    Datetime,
    Enum,
    Text,
    Json,
    ManyToOne,
    OneToMany,
    ManyToMany,
}

impl ModelFieldKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Datetime => "datetime",
            Self::Enum => "enum",
            Self::Text => "text",
            Self::Json => "json",
            Self::ManyToOne => "many_to_one",
            Self::OneToMany => "one_to_many",
            Self::ManyToMany => "many_to_many",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "number" => Self::Number,
            "boolean" => Self::Boolean,
            "datetime" => Self::Datetime,
            "enum" => Self::Enum,
            "text" => Self::Text,
            "json" => Self::Json,
            "many_to_one" => Self::ManyToOne,
            "one_to_many" => Self::OneToMany,
            "many_to_many" => Self::ManyToMany,
            _ => Self::String,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelFieldRecord {
    pub id: Uuid,
    pub data_model_id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub physical_column_name: String,
    pub external_field_key: Option<String>,
    pub field_kind: ModelFieldKind,
    pub is_system: bool,
    pub is_writable: bool,
    pub is_required: bool,
    pub api_required: bool,
    pub is_unique: bool,
    pub default_value: Option<Value>,
    pub display_interface: Option<String>,
    pub display_options: Value,
    pub relation_target_model_id: Option<Uuid>,
    pub relation_options: Value,
    pub sort_order: i32,
    pub availability_status: MetadataAvailabilityStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelDefinitionRecord {
    pub id: Uuid,
    pub scope_kind: DataModelScopeKind,
    pub scope_id: Uuid,
    pub data_source_instance_id: Option<Uuid>,
    pub source_kind: DataModelSourceKind,
    pub external_resource_key: Option<String>,
    pub external_table_id: Option<String>,
    pub external_capability_snapshot: Option<Value>,
    pub code: String,
    pub title: String,
    pub physical_table_name: String,
    pub acl_namespace: String,
    pub audit_namespace: String,
    pub fields: Vec<ModelFieldRecord>,
    pub availability_status: MetadataAvailabilityStatus,
    pub status: DataModelStatus,
    pub protection: DataModelProtection,
}
