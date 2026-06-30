use domain::AuthenticatorRecord;

#[derive(Debug, Clone)]
pub struct StoredAuthenticatorRow {
    pub id: uuid::Uuid,
    pub auth_type: String,
    pub title: String,
    pub enabled: bool,
    pub is_builtin: bool,
    pub sort_order: i32,
    pub options: serde_json::Value,
}

pub struct PgAuthMapper;

impl PgAuthMapper {
    pub fn to_authenticator_record(row: StoredAuthenticatorRow) -> AuthenticatorRecord {
        AuthenticatorRecord {
            id: row.id,
            auth_type: row.auth_type,
            title: row.title,
            enabled: row.enabled,
            is_builtin: row.is_builtin,
            sort_order: row.sort_order,
            options: row.options,
        }
    }
}
