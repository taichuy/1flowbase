use super::*;

#[async_trait]
impl DataSourceRuntimePort for ApiProviderRuntime {
    async fn ensure_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()> {
        self.ensure_data_source_loaded(installation).await
    }

    async fn compatible_data_model_templates(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        source: &plugin_framework::DataModelTemplateSource,
        capabilities: &plugin_framework::DataSourceCrudCapabilities,
    ) -> anyhow::Result<Vec<plugin_framework::DataModelTemplateDescriptor>> {
        self.ensure_data_source_loaded(installation).await?;
        let capability_codes = data_source_capability_codes(capabilities);
        Ok(self
            .services
            .data_model_template_catalog
            .compatible_templates(source, capability_codes.iter().map(String::as_str))
            .into_iter()
            .map(|template| template.descriptor().clone())
            .collect())
    }

    async fn validate_config(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        config_json: Value,
        secret_json: Value,
    ) -> anyhow::Result<Value> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_validate(
                &installation.plugin_id,
                DataSourceConfigInput {
                    config_json,
                    secret_json,
                },
            )
            .await
            .map_err(map_data_source_runtime_error)
    }

    async fn test_connection(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        config_json: Value,
        secret_json: Value,
    ) -> anyhow::Result<Value> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_test_connection(
                &installation.plugin_id,
                DataSourceConfigInput {
                    config_json,
                    secret_json,
                },
            )
            .await
            .map_err(map_data_source_runtime_error)
    }

    async fn discover_catalog(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        config_json: Value,
        secret_json: Value,
    ) -> anyhow::Result<Value> {
        self.ensure_data_source_loaded(installation).await?;
        let entries = self
            .services
            .runtime_backend
            .data_source_discover_catalog(
                &installation.plugin_id,
                DataSourceConfigInput {
                    config_json,
                    secret_json,
                },
            )
            .await
            .map_err(map_data_source_runtime_error)?;
        Ok(serde_json::to_value(entries)?)
    }

    async fn describe_resource(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceDescribeResourceInput,
    ) -> anyhow::Result<DataSourceResourceDescriptor> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_describe_resource(&installation.plugin_id, input)
            .await
            .map_err(map_data_source_runtime_error)
    }

    async fn preview_read(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourcePreviewReadInput,
    ) -> anyhow::Result<DataSourcePreviewReadOutput> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_preview_read(&installation.plugin_id, input)
            .await
            .map_err(map_data_source_runtime_error)
    }

    async fn execute_sql(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceExecuteSqlInput,
    ) -> anyhow::Result<NativeSqlExecutionOutput> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_execute_sql(&installation.plugin_id, input)
            .await
            .map_err(map_data_source_runtime_error)
    }

    async fn execute_model_operation(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: plugin_framework::DataSourceExecuteModelOperationInput,
    ) -> anyhow::Result<Value> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_execute_model_operation(&installation.plugin_id, input)
            .await
            .map_err(map_data_source_runtime_error)
    }
}

#[async_trait]
impl DataSourceCrudRuntimePort for ApiProviderRuntime {
    async fn list_records(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceListRecordsInput,
    ) -> anyhow::Result<DataSourceListRecordsOutput> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_list_records(&installation.plugin_id, input)
            .await
            .map_err(map_data_source_runtime_error)
    }

    async fn get_record(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceGetRecordInput,
    ) -> anyhow::Result<DataSourceGetRecordOutput> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_get_record(&installation.plugin_id, input)
            .await
            .map_err(map_data_source_runtime_error)
    }

    async fn create_record(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceCreateRecordInput,
    ) -> anyhow::Result<DataSourceCreateRecordOutput> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_create_record(&installation.plugin_id, input)
            .await
            .map_err(map_data_source_runtime_error)
    }

    async fn update_record(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceUpdateRecordInput,
    ) -> anyhow::Result<DataSourceUpdateRecordOutput> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_update_record(&installation.plugin_id, input)
            .await
            .map_err(map_data_source_runtime_error)
    }

    async fn delete_record(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceDeleteRecordInput,
    ) -> anyhow::Result<DataSourceDeleteRecordOutput> {
        self.ensure_data_source_loaded(installation).await?;
        self.services
            .runtime_backend
            .data_source_delete_record(&installation.plugin_id, input)
            .await
            .map_err(map_data_source_runtime_error)
    }
}

#[async_trait]
impl DataSourceRuntimeRecordBackend for ApiDataSourceRuntimeRecordBackend {
    async fn execute_sql(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: &str,
        sql: &str,
    ) -> anyhow::Result<NativeSqlExecutionOutput> {
        if data_source_instance_id == "main" {
            return storage_durable_postgres::execute_native_sql(self.repository.pool(), sql)
                .await
                .map_err(|error| anyhow::Error::new(PluginFrameworkError::runtime(error)));
        }

        let instance_id = Uuid::parse_str(data_source_instance_id)
            .map_err(|_| ControlPlaneError::InvalidInput("data_source_instance_id"))?;
        let target = self.load_target(workspace_id, instance_id).await?;
        let output = DataSourceRuntimePort::execute_sql(
            &self.runtime,
            &target.installation,
            DataSourceExecuteSqlInput {
                connection: target.connection,
                sql: sql.to_string(),
            },
        )
        .await?;
        redact_data_source_output(output, &target.secret_values)
    }

    async fn list_records(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        mut input: DataSourceListRecordsInput,
    ) -> anyhow::Result<DataSourceListRecordsOutput> {
        let target = self
            .load_target(workspace_id, data_source_instance_id)
            .await?;
        input.connection = target.connection;
        let output =
            DataSourceCrudRuntimePort::list_records(&self.runtime, &target.installation, input)
                .await?;
        redact_data_source_output(output, &target.secret_values)
    }

    async fn get_record(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        mut input: DataSourceGetRecordInput,
    ) -> anyhow::Result<DataSourceGetRecordOutput> {
        let target = self
            .load_target(workspace_id, data_source_instance_id)
            .await?;
        input.connection = target.connection;
        let output =
            DataSourceCrudRuntimePort::get_record(&self.runtime, &target.installation, input)
                .await?;
        redact_data_source_output(output, &target.secret_values)
    }

    async fn create_record(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        mut input: DataSourceCreateRecordInput,
    ) -> anyhow::Result<DataSourceCreateRecordOutput> {
        let target = self
            .load_target(workspace_id, data_source_instance_id)
            .await?;
        input.connection = target.connection;
        input.transaction_id = None;
        let output =
            DataSourceCrudRuntimePort::create_record(&self.runtime, &target.installation, input)
                .await?;
        redact_data_source_output(output, &target.secret_values)
    }

    async fn update_record(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        mut input: DataSourceUpdateRecordInput,
    ) -> anyhow::Result<DataSourceUpdateRecordOutput> {
        let target = self
            .load_target(workspace_id, data_source_instance_id)
            .await?;
        input.connection = target.connection;
        input.transaction_id = None;
        let output =
            DataSourceCrudRuntimePort::update_record(&self.runtime, &target.installation, input)
                .await?;
        redact_data_source_output(output, &target.secret_values)
    }

    async fn delete_record(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        mut input: DataSourceDeleteRecordInput,
    ) -> anyhow::Result<DataSourceDeleteRecordOutput> {
        let target = self
            .load_target(workspace_id, data_source_instance_id)
            .await?;
        input.connection = target.connection;
        input.transaction_id = None;
        let output =
            DataSourceCrudRuntimePort::delete_record(&self.runtime, &target.installation, input)
                .await?;
        redact_data_source_output(output, &target.secret_values)
    }

    async fn execute_model_operation(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        expected_capabilities: plugin_framework::DataSourceCrudCapabilities,
        mut input: plugin_framework::DataSourceExecuteModelOperationInput,
    ) -> anyhow::Result<Value> {
        let target = self
            .load_target(workspace_id, data_source_instance_id)
            .await?;
        let descriptor = DataSourceRuntimePort::describe_resource(
            &self.runtime,
            &target.installation,
            DataSourceDescribeResourceInput {
                connection: target.connection.clone(),
                resource_key: input.resource_key.clone(),
            },
        )
        .await?;
        if descriptor.resource_key != input.resource_key
            || descriptor.capabilities != expected_capabilities
        {
            return Err(ControlPlaneError::Conflict("data_source_capability_drift").into());
        }
        let source = plugin_framework::DataModelTemplateSource {
            kind: plugin_framework::DataModelSourceKind::ExternalSource,
            provider: Some(target.installation.provider_code.clone()),
        };
        let compatible = DataSourceRuntimePort::compatible_data_model_templates(
            &self.runtime,
            &target.installation,
            &source,
            &descriptor.capabilities,
        )
        .await?;
        let operation_available = compatible.iter().any(|template| {
            template.identity == input.template_identity
                && template.operations.iter().any(|operation| {
                    operation.code == input.operation_code
                        && operation.handler_ref == input.handler_ref
                })
        });
        if !operation_available {
            return Err(ControlPlaneError::Conflict("data_model_operation_unavailable").into());
        }
        input.connection = target.connection;
        let output = DataSourceRuntimePort::execute_model_operation(
            &self.runtime,
            &target.installation,
            input,
        )
        .await?;
        redact_data_source_output(output, &target.secret_values)
    }
}
