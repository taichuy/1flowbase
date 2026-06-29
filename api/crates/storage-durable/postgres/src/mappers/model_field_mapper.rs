use domain::ModelFieldRecord;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoredModelFieldRow {
    pub id: Uuid,
    pub data_model_id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub physical_column_name: String,
    pub external_field_key: Option<String>,
    pub field_kind: String,
    pub is_system: bool,
    pub is_writable: bool,
    pub is_required: bool,
    pub is_unique: bool,
    pub default_value: Option<Value>,
    pub display_interface: Option<String>,
    pub display_options: Value,
    pub relation_target_model_id: Option<Uuid>,
    pub relation_options: Value,
    pub sort_order: i32,
    pub availability_status: String,
}

pub struct PgModelFieldMapper;

impl PgModelFieldMapper {
    pub fn to_model_field_record(row: StoredModelFieldRow) -> ModelFieldRecord {
        ModelFieldRecord {
            id: row.id,
            data_model_id: row.data_model_id,
            code: row.code,
            title: row.title,
            description: row.description,
            physical_column_name: row.physical_column_name,
            external_field_key: row.external_field_key,
            field_kind: domain::ModelFieldKind::from_db(&row.field_kind),
            is_system: row.is_system,
            is_writable: row.is_writable,
            is_required: row.is_required,
            is_unique: row.is_unique,
            default_value: row.default_value,
            display_interface: row.display_interface,
            display_options: row.display_options,
            relation_target_model_id: row.relation_target_model_id,
            relation_options: row.relation_options,
            sort_order: row.sort_order,
            availability_status: domain::MetadataAvailabilityStatus::from_db(
                &row.availability_status,
            ),
        }
    }
}
