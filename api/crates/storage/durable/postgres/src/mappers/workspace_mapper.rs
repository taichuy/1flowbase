use domain::WorkspaceRecord;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoredWorkspaceRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub logo_url: Option<String>,
    pub introduction: String,
}

pub struct PgWorkspaceMapper;

impl PgWorkspaceMapper {
    pub fn to_workspace_record(row: StoredWorkspaceRow) -> WorkspaceRecord {
        WorkspaceRecord {
            id: row.id,
            tenant_id: row.tenant_id,
            name: row.name,
            logo_url: row.logo_url,
            introduction: row.introduction,
        }
    }
}
