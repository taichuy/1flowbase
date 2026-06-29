use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::resource_descriptor::ResourceDescriptor;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelMetadata {
    pub model_id: Uuid,
    pub model_code: String,
    pub status: domain::DataModelStatus,
    pub scope_kind: domain::DataModelScopeKind,
    pub scope_id: Uuid,
    pub data_source_instance_id: Option<Uuid>,
    pub source_kind: domain::DataModelSourceKind,
    pub external_resource_key: Option<String>,
    pub physical_table_name: String,
    pub scope_column_name: String,
    pub fields: Vec<domain::ModelFieldRecord>,
    pub record_capabilities: domain::DataModelRecordCapabilities,
    pub resource: ResourceDescriptor,
}

impl ModelMetadata {
    pub fn field_by_code(&self, field_code: &str) -> Option<&domain::ModelFieldRecord> {
        self.fields.iter().find(|field| field.code == field_code)
    }
}
