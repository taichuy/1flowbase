use async_trait::async_trait;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DepartmentListInput {
    pub parent_id: Option<Uuid>,
    pub prefix: Option<String>,
    pub ids: Option<Vec<Uuid>>,
    pub limit: u32,
    pub cursor: Option<String>,
}

impl Default for DepartmentListInput {
    fn default() -> Self {
        Self {
            parent_id: None,
            prefix: None,
            ids: None,
            limit: 100,
            cursor: None,
        }
    }
}

pub struct SaveDepartmentInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub department_id: Option<Uuid>,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub role_codes: Vec<String>,
    pub can_assign_roles: bool,
}

#[async_trait]
pub trait OrganizationRepository: Send + Sync {
    async fn list_department_page(
        &self,
        workspace_id: Uuid,
        input: DepartmentListInput,
    ) -> anyhow::Result<domain::DepartmentPage>;
    async fn list_departments(&self, workspace_id: Uuid)
        -> anyhow::Result<Vec<domain::Department>>;
    async fn save_department(
        &self,
        input: &SaveDepartmentInput,
    ) -> anyhow::Result<domain::Department>;
    async fn delete_department(
        &self,
        workspace_id: Uuid,
        department_id: Uuid,
    ) -> anyhow::Result<()>;
    async fn member_departments(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<domain::MemberDepartments>;
    async fn members_departments(
        &self,
        workspace_id: Uuid,
        user_ids: &[Uuid],
    ) -> anyhow::Result<std::collections::BTreeMap<Uuid, domain::MemberDepartments>>;
    async fn department_member_ids(
        &self,
        workspace_id: Uuid,
        department_id: Uuid,
    ) -> anyhow::Result<Vec<Uuid>>;
    async fn replace_member_departments(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        input: &domain::MemberDepartments,
    ) -> anyhow::Result<()>;
}
