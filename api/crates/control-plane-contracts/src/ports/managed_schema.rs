use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedSchemaFieldType {
    String,
    Text,
    Number,
    Boolean,
    Datetime,
    Json,
    Uuid,
}

impl ManagedSchemaFieldType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Text => "text",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Datetime => "datetime",
            Self::Json => "json",
            Self::Uuid => "uuid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedSchemaOperation {
    EnsureOwnedCollection {
        logical_collection: String,
        physical_table: String,
    },
    EnsureOwnedField {
        logical_collection: String,
        logical_field: String,
        physical_table: String,
        physical_column: String,
        field_type: ManagedSchemaFieldType,
        nullable: bool,
    },
    EnsureExtensionField {
        target_table: String,
        logical_field: String,
        physical_column: String,
        field_type: ManagedSchemaFieldType,
    },
    RetainInactive {
        ownership_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSchemaPlan {
    pub owner_id: String,
    pub owner_version: String,
    pub fingerprint: String,
    pub max_target_table_bytes: u64,
    pub lock_timeout_ms: u32,
    pub operations: Vec<ManagedSchemaOperation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedSchemaPreviewAction {
    Create,
    AlreadyPresent,
    Retain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSchemaPreviewEntry {
    pub ownership_key: String,
    pub action: ManagedSchemaPreviewAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSchemaPreview {
    pub owner_id: String,
    pub fingerprint: String,
    pub entries: Vec<ManagedSchemaPreviewEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSchemaApplyReceipt {
    pub receipt_id: Uuid,
    pub owner_id: String,
    pub owner_version: String,
    pub fingerprint: String,
    pub created_objects: u32,
    pub existing_objects: u32,
    pub retained_objects: u32,
    pub applied_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedSchemaObjectKind {
    OwnedCollection,
    OwnedField,
    ExtensionField,
}

impl ManagedSchemaObjectKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OwnedCollection => "owned_collection",
            Self::OwnedField => "owned_field",
            Self::ExtensionField => "extension_field",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSchemaOwnershipRecord {
    pub ownership_key: String,
    pub owner_id: String,
    pub owner_version: String,
    pub object_kind: ManagedSchemaObjectKind,
    pub logical_name: String,
    pub physical_table: String,
    pub physical_column: Option<String>,
    pub field_type: Option<ManagedSchemaFieldType>,
    pub nullable: Option<bool>,
    pub active: bool,
    pub plan_fingerprint: String,
}

#[async_trait]
pub trait ManagedSchemaRepository: Send + Sync {
    async fn preview_managed_schema(
        &self,
        plan: &ManagedSchemaPlan,
    ) -> anyhow::Result<ManagedSchemaPreview>;

    async fn apply_managed_schema(
        &self,
        plan: &ManagedSchemaPlan,
    ) -> anyhow::Result<ManagedSchemaApplyReceipt>;

    async fn list_managed_schema_ownership(
        &self,
    ) -> anyhow::Result<Vec<ManagedSchemaOwnershipRecord>>;
}
