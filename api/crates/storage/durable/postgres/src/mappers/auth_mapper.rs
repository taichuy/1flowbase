use domain::LoginEntryRecord;

#[derive(Debug, Clone)]
pub struct StoredLoginEntryRow {
    pub id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub auth_type: String,
    pub title: String,
    pub enabled: bool,
    pub is_builtin: bool,
    pub sort_order: i32,
    pub public_ui_block: String,
    pub options: serde_json::Value,
}

pub struct PgAuthMapper;

impl PgAuthMapper {
    pub fn to_login_entry_record(row: StoredLoginEntryRow) -> LoginEntryRecord {
        LoginEntryRecord {
            id: row.id,
            connection_id: row.connection_id,
            auth_type: row.auth_type,
            title: row.title,
            enabled: row.enabled,
            is_builtin: row.is_builtin,
            sort_order: row.sort_order,
            public_ui_block: row.public_ui_block,
            options: row.options,
        }
    }
}
