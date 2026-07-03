use domain::{ModelDefinitionRecord, ModelFieldRecord};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoredModelDefinitionRow {
    pub id: Uuid,
    pub scope_kind: String,
    pub scope_id: Uuid,
    pub data_source_instance_id: Option<Uuid>,
    pub source_kind: String,
    pub external_resource_key: Option<String>,
    pub external_table_id: Option<String>,
    pub external_capability_snapshot: Option<serde_json::Value>,
    pub code: String,
    pub title: String,
    pub physical_table_name: String,
    pub acl_namespace: String,
    pub audit_namespace: String,
    pub availability_status: String,
    pub status: String,
    pub owner_kind: String,
    pub owner_id: Option<String>,
    pub is_protected: bool,
    pub fields: Vec<ModelFieldRecord>,
}

pub struct PgModelDefinitionMapper;

impl PgModelDefinitionMapper {
    pub fn to_model_definition_record(row: StoredModelDefinitionRow) -> ModelDefinitionRecord {
        ModelDefinitionRecord {
            id: row.id,
            scope_kind: domain::DataModelScopeKind::from_db(&row.scope_kind),
            scope_id: row.scope_id,
            data_source_instance_id: row.data_source_instance_id,
            source_kind: domain::DataModelSourceKind::from_db(&row.source_kind),
            external_resource_key: row.external_resource_key,
            external_table_id: row.external_table_id,
            external_capability_snapshot: row.external_capability_snapshot,
            code: row.code,
            title: row.title,
            physical_table_name: row.physical_table_name,
            acl_namespace: row.acl_namespace,
            audit_namespace: row.audit_namespace,
            fields: row.fields,
            availability_status: domain::MetadataAvailabilityStatus::from_db(
                &row.availability_status,
            ),
            status: domain::DataModelStatus::from_db(&row.status),
            protection: domain::DataModelProtection {
                owner_kind: domain::DataModelOwnerKind::from_db(&row.owner_kind),
                owner_id: row.owner_id,
                is_protected: row.is_protected,
            },
        }
    }
}
