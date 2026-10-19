use super::*;

#[async_trait]
impl DataSourceRuntimePort for RuntimeExtensionHost {
    async fn activate_data_source(
        &self,
        request: RuntimePackageActivation,
    ) -> Result<Vec<extension_contracts::DataModelTemplateDescriptor>, RuntimeBackendError> {
        self.ensure_activating()?;
        let package_root = self.artifact_resolver.resolve(&request.artifact).await?;
        let mut host = self.data_source_host.write().await;
        if !host.is_loaded(&request.plugin_id) {
            match request.legacy_eligibility {
                Some(eligibility) => {
                    host.load_legacy_installed(&package_root, &legacy_eligibility(eligibility))
                        .await
                }
                None => host.load(&package_root).await,
            }
            .map_err(RuntimeBackendError::from)?;
        }
        host.data_model_templates(&request.plugin_id)
            .map_err(RuntimeBackendError::from)
    }

    async fn deactivate_data_source(&self, plugin_id: &str) -> Result<(), RuntimeBackendError> {
        self.data_source_host
            .write()
            .await
            .unload(plugin_id)
            .await
            .map_err(RuntimeBackendError::from)
    }

    async fn data_source_validate(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceConfigInput,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .validate_config_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.output)
            .map_err(Into::into)
    }

    async fn data_source_test_connection(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceConfigInput,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .test_connection_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.output)
            .map_err(Into::into)
    }

    async fn data_source_discover_catalog(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceConfigInput,
    ) -> Result<Vec<extension_contracts::DataSourceCatalogEntry>, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .discover_catalog_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.entries)
            .map_err(Into::into)
    }

    async fn data_source_describe_resource(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceDescribeResourceInput,
    ) -> Result<extension_contracts::DataSourceResourceDescriptor, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .describe_resource_operation(target_id, input.connection, input.resource_key)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.descriptor)
            .map_err(Into::into)
    }

    async fn data_source_preview_read(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourcePreviewReadInput,
    ) -> Result<extension_contracts::DataSourcePreviewReadOutput, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .preview_read_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(Into::into)
    }

    async fn data_source_import_snapshot(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceImportSnapshotInput,
    ) -> Result<extension_contracts::DataSourceImportSnapshotOutput, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .import_snapshot_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(Into::into)
    }

    async fn data_source_list_records(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceListRecordsInput,
    ) -> Result<extension_contracts::DataSourceListRecordsOutput, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .list_records_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(Into::into)
    }

    async fn data_source_get_record(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceGetRecordInput,
    ) -> Result<extension_contracts::DataSourceGetRecordOutput, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .get_record_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(Into::into)
    }

    async fn data_source_create_record(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceCreateRecordInput,
    ) -> Result<extension_contracts::DataSourceCreateRecordOutput, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .create_record_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(Into::into)
    }

    async fn data_source_update_record(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceUpdateRecordInput,
    ) -> Result<extension_contracts::DataSourceUpdateRecordOutput, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .update_record_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(Into::into)
    }

    async fn data_source_delete_record(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceDeleteRecordInput,
    ) -> Result<extension_contracts::DataSourceDeleteRecordOutput, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .delete_record_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(Into::into)
    }

    async fn data_source_execute_sql(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceExecuteSqlInput,
    ) -> Result<extension_contracts::NativeSqlExecutionOutput, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .execute_sql_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(Into::into)
    }

    async fn data_source_execute_model_operation(
        &self,
        target_id: &str,
        input: extension_contracts::DataSourceExecuteModelOperationInput,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .data_source_host
            .read()
            .await
            .execute_model_operation_call(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(Into::into)
    }
}
