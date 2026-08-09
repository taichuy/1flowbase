use super::*;

#[async_trait]
pub trait DataSourceRuntimePort: Send + Sync {
    async fn ensure_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()>;
    async fn compatible_data_model_templates(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        source: &plugin_framework::DataModelTemplateSource,
        capabilities: &plugin_framework::DataSourceCrudCapabilities,
    ) -> anyhow::Result<Vec<plugin_framework::DataModelTemplateDescriptor>>;
    async fn validate_config(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        config_json: serde_json::Value,
        secret_json: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value>;
    async fn test_connection(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        config_json: serde_json::Value,
        secret_json: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value>;
    async fn discover_catalog(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        config_json: serde_json::Value,
        secret_json: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value>;
    async fn describe_resource(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceDescribeResourceInput,
    ) -> anyhow::Result<DataSourceResourceDescriptor>;
    async fn preview_read(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourcePreviewReadInput,
    ) -> anyhow::Result<DataSourcePreviewReadOutput>;
    async fn execute_sql(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceExecuteSqlInput,
    ) -> anyhow::Result<NativeSqlExecutionOutput> {
        let _ = (installation, input);
        anyhow::bail!("native SQL is not implemented by this data source runtime")
    }
    async fn execute_model_operation(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: plugin_framework::DataSourceExecuteModelOperationInput,
    ) -> anyhow::Result<serde_json::Value>;
}
