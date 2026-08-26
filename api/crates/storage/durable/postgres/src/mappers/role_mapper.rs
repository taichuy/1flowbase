use domain::{RoleScopeKind, RoleTemplate};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoredRoleRow {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub introduction: String,
    pub scope_kind: RoleScopeKind,
    pub is_builtin: bool,
    pub is_editable: bool,
    pub auto_grant_new_permissions: bool,
    pub is_default_member_role: bool,
}

pub struct PgRoleMapper;

impl PgRoleMapper {
    pub fn to_role_template(row: StoredRoleRow, permissions: Vec<String>) -> RoleTemplate {
        RoleTemplate {
            code: row.code,
            name: row.name,
            introduction: row.introduction,
            scope_kind: row.scope_kind,
            is_builtin: row.is_builtin,
            is_editable: row.is_editable,
            auto_grant_new_permissions: row.auto_grant_new_permissions,
            is_default_member_role: row.is_default_member_role,
            permissions,
        }
    }
}
