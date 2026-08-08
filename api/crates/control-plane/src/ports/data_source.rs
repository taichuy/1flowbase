use super::*;
use plugin_framework::data_source_contract::{
    DataSourceCreateRecordInput, DataSourceCreateRecordOutput, DataSourceDeleteRecordInput,
    DataSourceDeleteRecordOutput, DataSourceGetRecordInput, DataSourceGetRecordOutput,
    DataSourceListRecordsInput, DataSourceListRecordsOutput, DataSourceUpdateRecordInput,
    DataSourceUpdateRecordOutput,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSourceInstanceVisibility {
    Own,
    ScopeAll,
}

#[derive(Debug, Clone)]
pub struct CreateDataSourceInstanceInput {
    pub instance_id: Uuid,
    pub workspace_id: Uuid,
    pub installation_id: Uuid,
    pub source_code: String,
    pub display_name: String,
    pub status: domain::DataSourceInstanceStatus,
    pub config_json: serde_json::Value,
    pub metadata_json: serde_json::Value,
    pub defaults: domain::DataSourceDefaults,
    pub created_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateDataSourceInstanceStatusInput {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub status: domain::DataSourceInstanceStatus,
    pub metadata_json: serde_json::Value,
    pub updated_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateDataSourceDefaultsInput {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub defaults: domain::DataSourceDefaults,
    pub updated_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateMainSourceDefaultsInput {
    pub workspace_id: Uuid,
    pub defaults: domain::DataSourceDefaults,
    pub updated_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateDataSourceInstanceConfigInput {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub config_json: serde_json::Value,
    pub updated_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpsertDataSourceSecretInput {
    pub data_source_instance_id: Uuid,
    pub secret_ref: String,
    pub plaintext_secret_json: serde_json::Value,
    pub master_key: String,
    pub secret_version: i32,
}

#[derive(Debug, Clone)]
pub struct RotateDataSourceSecretInput {
    pub workspace_id: Uuid,
    pub data_source_instance_id: Uuid,
    pub secret_ref: String,
    pub plaintext_secret_json: serde_json::Value,
    pub master_key: String,
    pub updated_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct RotateDataSourceSecretOutput {
    pub secret: domain::DataSourceSecretRecord,
    pub instance: domain::DataSourceInstanceRecord,
}

#[derive(Debug, Clone)]
pub struct UpsertDataSourceCatalogCacheInput {
    pub data_source_instance_id: Uuid,
    pub refresh_status: domain::DataSourceCatalogRefreshStatus,
    pub catalog_json: serde_json::Value,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct CreateDataSourcePreviewSessionInput {
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub data_source_instance_id: Option<Uuid>,
    pub config_fingerprint: String,
    pub preview_json: serde_json::Value,
    pub expires_at: OffsetDateTime,
}

#[async_trait]
pub trait DataSourceRepository: Send + Sync {
    async fn list_instances(
        &self,
        _workspace_id: Uuid,
        _actor_user_id: Uuid,
        _visibility: DataSourceInstanceVisibility,
    ) -> anyhow::Result<Vec<domain::DataSourceInstanceRecord>> {
        anyhow::bail!("list_data_source_instances is not implemented")
    }
    async fn load_role_console_policies_for_user(
        &self,
        actor: &domain::ActorContext,
    ) -> anyhow::Result<Vec<domain::RoleConsolePolicy>>;
    async fn create_instance(
        &self,
        input: &CreateDataSourceInstanceInput,
    ) -> anyhow::Result<domain::DataSourceInstanceRecord>;
    async fn update_instance_status(
        &self,
        input: &UpdateDataSourceInstanceStatusInput,
    ) -> anyhow::Result<domain::DataSourceInstanceRecord>;
    async fn update_instance_defaults(
        &self,
        input: &UpdateDataSourceDefaultsInput,
    ) -> anyhow::Result<domain::DataSourceInstanceRecord>;
    async fn get_main_source_defaults(
        &self,
        _workspace_id: Uuid,
    ) -> anyhow::Result<domain::DataSourceDefaults> {
        Ok(domain::DataSourceDefaults::default())
    }
    async fn update_main_source_defaults(
        &self,
        _input: &UpdateMainSourceDefaultsInput,
    ) -> anyhow::Result<domain::DataSourceDefaults> {
        anyhow::bail!("update_main_source_defaults is not implemented")
    }
    async fn update_instance_config(
        &self,
        input: &UpdateDataSourceInstanceConfigInput,
    ) -> anyhow::Result<domain::DataSourceInstanceRecord>;
    async fn get_instance(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> anyhow::Result<Option<domain::DataSourceInstanceRecord>>;
    async fn get_instance_for_visibility(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        actor_user_id: Uuid,
        visibility: DataSourceInstanceVisibility,
    ) -> anyhow::Result<Option<domain::DataSourceInstanceRecord>>;
    async fn upsert_secret(
        &self,
        input: &UpsertDataSourceSecretInput,
    ) -> anyhow::Result<domain::DataSourceSecretRecord>;
    async fn rotate_secret(
        &self,
        input: &RotateDataSourceSecretInput,
    ) -> anyhow::Result<RotateDataSourceSecretOutput>;
    async fn get_secret_record(
        &self,
        instance_id: Uuid,
    ) -> anyhow::Result<Option<domain::DataSourceSecretRecord>>;
    async fn get_secret_json(
        &self,
        instance_id: Uuid,
        master_key: &str,
    ) -> anyhow::Result<Option<serde_json::Value>>;
    async fn upsert_catalog_cache(
        &self,
        input: &UpsertDataSourceCatalogCacheInput,
    ) -> anyhow::Result<domain::DataSourceCatalogCacheRecord>;
    async fn get_catalog_cache(
        &self,
        _workspace_id: Uuid,
        _instance_id: Uuid,
    ) -> anyhow::Result<Option<domain::DataSourceCatalogCacheRecord>> {
        Ok(None)
    }
    async fn create_preview_session(
        &self,
        input: &CreateDataSourcePreviewSessionInput,
    ) -> anyhow::Result<domain::DataSourcePreviewSessionRecord>;
}

#[async_trait]
pub trait DataSourceCrudRuntimePort: Send + Sync {
    async fn list_records(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceListRecordsInput,
    ) -> anyhow::Result<DataSourceListRecordsOutput>;

    async fn get_record(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceGetRecordInput,
    ) -> anyhow::Result<DataSourceGetRecordOutput>;

    async fn create_record(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceCreateRecordInput,
    ) -> anyhow::Result<DataSourceCreateRecordOutput>;

    async fn update_record(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceUpdateRecordInput,
    ) -> anyhow::Result<DataSourceUpdateRecordOutput>;

    async fn delete_record(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceDeleteRecordInput,
    ) -> anyhow::Result<DataSourceDeleteRecordOutput>;
}
