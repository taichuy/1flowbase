use super::*;

#[derive(Debug, Clone)]
pub struct CreateFileStorageInput {
    pub storage_id: Uuid,
    pub actor_user_id: Uuid,
    pub code: String,
    pub title: String,
    pub driver_type: String,
    pub enabled: bool,
    pub is_default: bool,
    pub config_json: serde_json::Value,
    pub rule_json: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpdateFileStorageBindingInput {
    pub actor_user_id: Uuid,
    pub file_table_id: Uuid,
    pub bound_storage_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateFileStorageInput {
    pub actor_user_id: Uuid,
    pub file_storage_id: Uuid,
    pub title: String,
    pub enabled: bool,
    pub is_default: bool,
    pub config_json: serde_json::Value,
    pub rule_json: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct DeleteFileStorageInput {
    pub actor_user_id: Uuid,
    pub file_storage_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct DeleteFileTableInput {
    pub actor_user_id: Uuid,
    pub file_table_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateFileTableRegistrationInput {
    pub file_table_id: Uuid,
    pub actor_user_id: Uuid,
    pub code: String,
    pub title: String,
    pub scope_kind: domain::FileTableScopeKind,
    pub scope_id: Uuid,
    pub model_definition_id: Uuid,
    pub bound_storage_id: Uuid,
    pub is_builtin: bool,
    pub is_default: bool,
}

#[async_trait]
pub trait FileManagementRepository: Send + Sync {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> anyhow::Result<ActorContext>;
    async fn find_file_table_by_code(
        &self,
        code: &str,
    ) -> anyhow::Result<Option<domain::FileTableRecord>>;
    async fn get_file_table(
        &self,
        file_table_id: Uuid,
    ) -> anyhow::Result<Option<domain::FileTableRecord>>;
    async fn create_file_storage(
        &self,
        input: &CreateFileStorageInput,
    ) -> anyhow::Result<domain::FileStorageRecord>;
    async fn create_file_table_registration(
        &self,
        input: &CreateFileTableRegistrationInput,
    ) -> anyhow::Result<domain::FileTableRecord>;
    async fn list_file_storages(&self) -> anyhow::Result<Vec<domain::FileStorageRecord>>;
    async fn get_default_file_storage(&self) -> anyhow::Result<Option<domain::FileStorageRecord>>;
    async fn get_file_storage(
        &self,
        storage_id: Uuid,
    ) -> anyhow::Result<Option<domain::FileStorageRecord>>;
    async fn list_visible_file_tables(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::FileTableRecord>>;
    async fn update_file_table_binding(
        &self,
        input: &UpdateFileStorageBindingInput,
    ) -> anyhow::Result<domain::FileTableRecord>;
    async fn update_file_storage(
        &self,
        input: &UpdateFileStorageInput,
    ) -> anyhow::Result<domain::FileStorageRecord> {
        let _ = input;
        anyhow::bail!("update_file_storage not implemented")
    }
    async fn delete_file_storage(&self, input: &DeleteFileStorageInput) -> anyhow::Result<()> {
        let _ = input;
        anyhow::bail!("delete_file_storage not implemented")
    }
    async fn delete_file_table(&self, input: &DeleteFileTableInput) -> anyhow::Result<()> {
        let _ = input;
        anyhow::bail!("delete_file_table not implemented")
    }
}
