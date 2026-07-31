use std::collections::HashMap;

use plugin_framework::{
    data_source_contract::{
        validate_native_sql_output, DataSourceCatalogEntry, DataSourceConfigInput,
        DataSourceCreateRecordInput, DataSourceCreateRecordOutput, DataSourceDeleteRecordInput,
        DataSourceDeleteRecordOutput, DataSourceDescribeResourceInput, DataSourceExecuteSqlInput,
        DataSourceGetRecordInput, DataSourceGetRecordOutput, DataSourceImportSnapshotInput,
        DataSourceImportSnapshotOutput, DataSourceListRecordsInput, DataSourceListRecordsOutput,
        DataSourcePreviewReadInput, DataSourcePreviewReadOutput, DataSourceResourceDescriptor,
        DataSourceStdioMethod, DataSourceStdioRequest, DataSourceUpdateRecordInput,
        DataSourceUpdateRecordOutput, NativeSqlExecutionOutput, DATA_SOURCE_NATIVE_SQL_CAPABILITY,
    },
    error::{FrameworkResult, PluginFrameworkError},
};
use serde::Serialize;
use serde_json::Value;

use crate::{
    data_source_stdio::call_executable,
    package_loader::{LoadedDataSourcePackage, PackageLoader},
};

#[derive(Debug, Clone, Serialize)]
pub struct LoadedDataSourceSummary {
    pub plugin_id: String,
    pub source_code: String,
    pub plugin_version: String,
    pub execution_mode: String,
}

impl LoadedDataSourceSummary {
    fn from_loaded(loaded: &LoadedDataSourcePackage) -> Self {
        Self {
            plugin_id: loaded.package.identifier(),
            source_code: loaded.package.definition.source_code.clone(),
            plugin_version: loaded.package.manifest.version.clone(),
            execution_mode: loaded.package.manifest.execution_mode.as_str().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DataSourceValueOutput {
    pub output: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataSourceCatalogOutput {
    pub entries: Vec<DataSourceCatalogEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataSourceDescriptorOutput {
    pub descriptor: DataSourceResourceDescriptor,
}

#[derive(Debug, Default)]
pub struct DataSourceHost {
    loaded_packages: HashMap<String, LoadedDataSourcePackage>,
}

impl DataSourceHost {
    pub fn load(
        &mut self,
        package_root: impl AsRef<std::path::Path>,
    ) -> FrameworkResult<LoadedDataSourceSummary> {
        let loaded = PackageLoader::load_data_source(package_root)?;
        let summary = LoadedDataSourceSummary::from_loaded(&loaded);
        self.loaded_packages
            .insert(summary.plugin_id.clone(), loaded);
        Ok(summary)
    }

    pub fn reload(&mut self, plugin_id: &str) -> FrameworkResult<LoadedDataSourceSummary> {
        let package_root = self.loaded_package(plugin_id)?.package_root.clone();
        let loaded = PackageLoader::load_data_source(&package_root)?;
        let summary = LoadedDataSourceSummary::from_loaded(&loaded);
        self.loaded_packages.remove(plugin_id);
        self.loaded_packages
            .insert(summary.plugin_id.clone(), loaded);
        Ok(summary)
    }

    pub async fn validate_config(
        &self,
        plugin_id: &str,
        input: DataSourceConfigInput,
    ) -> FrameworkResult<DataSourceValueOutput> {
        self.validate_config_operation(plugin_id, input)?.await
    }

    pub fn validate_config_operation(
        &self,
        plugin_id: &str,
        input: DataSourceConfigInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceValueOutput>> + Send + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::ValidateConfig,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move {
            Ok(DataSourceValueOutput {
                output: operation.await?,
            })
        })
    }

    pub async fn test_connection(
        &self,
        plugin_id: &str,
        input: DataSourceConfigInput,
    ) -> FrameworkResult<DataSourceValueOutput> {
        self.test_connection_operation(plugin_id, input)?.await
    }

    pub fn test_connection_operation(
        &self,
        plugin_id: &str,
        input: DataSourceConfigInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceValueOutput>> + Send + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::TestConnection,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move {
            Ok(DataSourceValueOutput {
                output: operation.await?,
            })
        })
    }

    pub async fn discover_catalog(
        &self,
        plugin_id: &str,
        input: DataSourceConfigInput,
    ) -> FrameworkResult<DataSourceCatalogOutput> {
        self.discover_catalog_operation(plugin_id, input)?.await
    }

    pub fn discover_catalog_operation(
        &self,
        plugin_id: &str,
        input: DataSourceConfigInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceCatalogOutput>> + Send + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::DiscoverCatalog,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move {
            Ok(DataSourceCatalogOutput {
                entries: normalize_catalog(operation.await?)?,
            })
        })
    }

    pub async fn describe_resource(
        &self,
        plugin_id: &str,
        connection: DataSourceConfigInput,
        resource_key: String,
    ) -> FrameworkResult<DataSourceDescriptorOutput> {
        self.describe_resource_operation(plugin_id, connection, resource_key)?
            .await
    }

    pub fn describe_resource_operation(
        &self,
        plugin_id: &str,
        connection: DataSourceConfigInput,
        resource_key: String,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceDescriptorOutput>> + Send + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::DescribeResource,
            serde_json::to_value(DataSourceDescribeResourceInput {
                connection,
                resource_key,
            })
            .unwrap(),
        )?;
        Ok(async move {
            Ok(DataSourceDescriptorOutput {
                descriptor: normalize_descriptor(operation.await?)?,
            })
        })
    }

    pub async fn preview_read(
        &self,
        plugin_id: &str,
        input: DataSourcePreviewReadInput,
    ) -> FrameworkResult<DataSourcePreviewReadOutput> {
        self.preview_read_operation(plugin_id, input)?.await
    }

    pub fn preview_read_operation(
        &self,
        plugin_id: &str,
        input: DataSourcePreviewReadInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourcePreviewReadOutput>> + Send + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::PreviewRead,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move { normalize_preview_read(operation.await?) })
    }

    pub async fn import_snapshot(
        &self,
        plugin_id: &str,
        input: DataSourceImportSnapshotInput,
    ) -> FrameworkResult<DataSourceImportSnapshotOutput> {
        self.import_snapshot_operation(plugin_id, input)?.await
    }

    pub fn import_snapshot_operation(
        &self,
        plugin_id: &str,
        input: DataSourceImportSnapshotInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceImportSnapshotOutput>>
            + Send
            + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::ImportSnapshot,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move { normalize_import_snapshot(operation.await?) })
    }

    pub async fn list_records(
        &self,
        plugin_id: &str,
        input: DataSourceListRecordsInput,
    ) -> FrameworkResult<DataSourceListRecordsOutput> {
        self.list_records_operation(plugin_id, input)?.await
    }

    pub fn list_records_operation(
        &self,
        plugin_id: &str,
        input: DataSourceListRecordsInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceListRecordsOutput>> + Send + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::ListRecords,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move { normalize_list_records(operation.await?) })
    }

    pub async fn get_record(
        &self,
        plugin_id: &str,
        input: DataSourceGetRecordInput,
    ) -> FrameworkResult<DataSourceGetRecordOutput> {
        self.get_record_operation(plugin_id, input)?.await
    }

    pub fn get_record_operation(
        &self,
        plugin_id: &str,
        input: DataSourceGetRecordInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceGetRecordOutput>> + Send + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::GetRecord,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move { normalize_get_record(operation.await?) })
    }

    pub async fn create_record(
        &self,
        plugin_id: &str,
        input: DataSourceCreateRecordInput,
    ) -> FrameworkResult<DataSourceCreateRecordOutput> {
        self.create_record_operation(plugin_id, input)?.await
    }

    pub fn create_record_operation(
        &self,
        plugin_id: &str,
        input: DataSourceCreateRecordInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceCreateRecordOutput>>
            + Send
            + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::CreateRecord,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move { normalize_create_record(operation.await?) })
    }

    pub async fn update_record(
        &self,
        plugin_id: &str,
        input: DataSourceUpdateRecordInput,
    ) -> FrameworkResult<DataSourceUpdateRecordOutput> {
        self.update_record_operation(plugin_id, input)?.await
    }

    pub fn update_record_operation(
        &self,
        plugin_id: &str,
        input: DataSourceUpdateRecordInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceUpdateRecordOutput>>
            + Send
            + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::UpdateRecord,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move { normalize_update_record(operation.await?) })
    }

    pub async fn delete_record(
        &self,
        plugin_id: &str,
        input: DataSourceDeleteRecordInput,
    ) -> FrameworkResult<DataSourceDeleteRecordOutput> {
        self.delete_record_operation(plugin_id, input)?.await
    }

    pub fn delete_record_operation(
        &self,
        plugin_id: &str,
        input: DataSourceDeleteRecordInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<DataSourceDeleteRecordOutput>>
            + Send
            + 'static,
    > {
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::DeleteRecord,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move { normalize_delete_record(operation.await?) })
    }

    pub async fn execute_sql(
        &self,
        plugin_id: &str,
        input: DataSourceExecuteSqlInput,
    ) -> FrameworkResult<NativeSqlExecutionOutput> {
        self.execute_sql_operation(plugin_id, input)?.await
    }

    pub fn execute_sql_operation(
        &self,
        plugin_id: &str,
        input: DataSourceExecuteSqlInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<NativeSqlExecutionOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?;
        if !loaded.package.supports_native_sql() {
            return Err(PluginFrameworkError::invalid_provider_contract(format!(
                "data_source_capability_not_supported: {DATA_SOURCE_NATIVE_SQL_CAPABILITY}"
            )));
        }
        let operation = self.call_runtime_operation(
            plugin_id,
            DataSourceStdioMethod::ExecuteSql,
            serde_json::to_value(input).unwrap(),
        )?;
        Ok(async move { normalize_native_sql_output(operation.await?) })
    }

    fn loaded_package(&self, plugin_id: &str) -> FrameworkResult<&LoadedDataSourcePackage> {
        self.loaded_packages.get(plugin_id).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "data source package is not loaded: {plugin_id}"
            ))
        })
    }

    fn call_runtime_operation(
        &self,
        plugin_id: &str,
        method: DataSourceStdioMethod,
        input: Value,
    ) -> FrameworkResult<impl std::future::Future<Output = FrameworkResult<Value>> + Send + 'static>
    {
        let loaded = self.loaded_package(plugin_id)?.clone();
        Ok(async move { Self::call_runtime_loaded(loaded, method, input).await })
    }

    async fn call_runtime_loaded(
        loaded: LoadedDataSourcePackage,
        method: DataSourceStdioMethod,
        input: Value,
    ) -> FrameworkResult<Value> {
        let request = DataSourceStdioRequest { method, input };
        call_executable(
            &loaded.runtime_executable,
            &request,
            &loaded.package.manifest.runtime.limits,
        )
        .await
    }
}

fn normalize_catalog(raw: Value) -> FrameworkResult<Vec<DataSourceCatalogEntry>> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

fn normalize_descriptor(raw: Value) -> FrameworkResult<DataSourceResourceDescriptor> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

fn normalize_preview_read(raw: Value) -> FrameworkResult<DataSourcePreviewReadOutput> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

fn normalize_import_snapshot(raw: Value) -> FrameworkResult<DataSourceImportSnapshotOutput> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

fn normalize_list_records(raw: Value) -> FrameworkResult<DataSourceListRecordsOutput> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

fn normalize_get_record(raw: Value) -> FrameworkResult<DataSourceGetRecordOutput> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

fn normalize_native_sql_output(raw: Value) -> FrameworkResult<NativeSqlExecutionOutput> {
    let output = serde_json::from_value(raw).map_err(|error| {
        PluginFrameworkError::invalid_provider_contract(format!(
            "invalid_native_sql_result_contract: {error}"
        ))
    })?;
    validate_native_sql_output(&output).map_err(PluginFrameworkError::invalid_provider_contract)?;
    Ok(output)
}

fn normalize_create_record(raw: Value) -> FrameworkResult<DataSourceCreateRecordOutput> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

fn normalize_update_record(raw: Value) -> FrameworkResult<DataSourceUpdateRecordOutput> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

fn normalize_delete_record(raw: Value) -> FrameworkResult<DataSourceDeleteRecordOutput> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}
