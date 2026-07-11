use super::*;

#[derive(Debug, Clone)]
pub struct CreateFrontstagePageInput {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: domain::FrontstagePageKind,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub placement: domain::frontstage::FrontstageNavigationPlacement,
    pub slug: Option<String>,
    pub rank: String,
    pub default_tab: Option<CreateFrontstagePageTabInput>,
}

#[derive(Debug, Clone)]
pub struct CreateFrontstagePageTabInput {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub page_id: Uuid,
    pub title: Option<String>,
    pub rank: String,
    pub is_default: bool,
    pub document_root_uid: String,
}

#[derive(Debug, Clone)]
pub struct UpdateFrontstagePageMetadataInput {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub page_id: Uuid,
    pub title: Option<Option<String>>,
    pub icon: Option<Option<String>>,
    pub tooltip: Option<Option<String>>,
    pub is_hidden: Option<bool>,
    pub placement: Option<domain::frontstage::FrontstageNavigationPlacement>,
    pub slug: Option<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct MoveFrontstagePageInput {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub page_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub rank: String,
}

#[derive(Debug, Clone)]
pub struct UpdateFrontstagePageTabInput {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub title: Option<Option<String>>,
    pub rank: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SaveFrontstageTabDocumentInput {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub schema_payload: serde_json::Value,
    pub root_payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct SaveFrontstageBlockCodeInput {
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub code_ref: String,
    pub code: String,
}

#[async_trait]
pub trait FrontstagePageRepository: Send + Sync {
    async fn load_actor_context_for_workspace(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> anyhow::Result<domain::ActorContext>;

    async fn list_frontstage_pages(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::FrontstagePageRecord>>;

    async fn list_frontstage_page_visibility_rules_for_actor_roles(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>>;

    async fn list_frontstage_page_visibility_rules_for_role(
        &self,
        workspace_id: Uuid,
        role_code: &str,
    ) -> anyhow::Result<Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>>;

    async fn replace_frontstage_page_visibility_rules_for_role(
        &self,
        workspace_id: Uuid,
        role_code: &str,
        page_ids: &[Uuid],
        tab_ids: &[Uuid],
        actor_user_id: Uuid,
    ) -> anyhow::Result<()>;

    async fn get_frontstage_page(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
    ) -> anyhow::Result<Option<domain::FrontstagePageRecord>>;

    async fn list_frontstage_page_tabs(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
    ) -> anyhow::Result<Vec<domain::frontstage::FrontstagePageTabRecord>>;

    async fn get_frontstage_page_tab_detail(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        tab_id: Uuid,
    ) -> anyhow::Result<Option<domain::frontstage::FrontstagePageDetail>>;

    async fn create_frontstage_page(
        &self,
        input: &CreateFrontstagePageInput,
    ) -> anyhow::Result<domain::frontstage::FrontstagePageCreation>;

    async fn create_frontstage_page_tab(
        &self,
        input: &CreateFrontstagePageTabInput,
    ) -> anyhow::Result<domain::frontstage::FrontstagePageTabRecord>;

    async fn update_frontstage_page_metadata(
        &self,
        input: &UpdateFrontstagePageMetadataInput,
    ) -> anyhow::Result<domain::FrontstagePageRecord>;

    async fn update_frontstage_page_tab(
        &self,
        input: &UpdateFrontstagePageTabInput,
    ) -> anyhow::Result<domain::frontstage::FrontstagePageTabRecord>;

    async fn move_frontstage_page(
        &self,
        input: &MoveFrontstagePageInput,
    ) -> anyhow::Result<domain::FrontstagePageRecord>;

    async fn delete_frontstage_page(&self, workspace_id: Uuid, page_id: Uuid)
        -> anyhow::Result<()>;

    async fn delete_frontstage_page_tab(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        tab_id: Uuid,
        actor_user_id: Uuid,
    ) -> anyhow::Result<()>;

    async fn save_frontstage_tab_document(
        &self,
        input: &SaveFrontstageTabDocumentInput,
    ) -> anyhow::Result<domain::frontstage::FrontstagePageDetail>;

    async fn get_frontstage_block_code(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        code_ref: &str,
    ) -> anyhow::Result<Option<domain::frontstage::FrontstageBlockCodeRecord>>;

    async fn save_frontstage_block_code(
        &self,
        input: &SaveFrontstageBlockCodeInput,
    ) -> anyhow::Result<domain::frontstage::FrontstageBlockCodeRecord>;

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> anyhow::Result<()>;
}
