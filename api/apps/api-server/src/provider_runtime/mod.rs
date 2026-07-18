use std::sync::Arc;

use async_trait::async_trait;
use control_plane::{
    capability_plugin_runtime::{
        CapabilityExecutionOutput, CapabilityPluginRuntimePort, ExecuteCapabilityNodeInput,
        ResolveCapabilityOptionsInput, ResolveCapabilityOutputSchemaInput,
        ValidateCapabilityConfigInput,
    },
    data_source::{collect_secret_strings, redact_value},
    errors::ControlPlaneError,
    plugin_lifecycle::reconcile_installation_snapshot,
    ports::{
        DataSourceCrudRuntimePort, DataSourceRepository, DataSourceRuntimePort, PluginRepository,
        ProviderRuntimeInvocationOutput, ProviderRuntimePort,
    },
};
use plugin_framework::{
    data_source_contract::{
        DataSourceConfigInput, DataSourceCreateRecordInput, DataSourceCreateRecordOutput,
        DataSourceDeleteRecordInput, DataSourceDeleteRecordOutput, DataSourceDescribeResourceInput,
        DataSourceGetRecordInput, DataSourceGetRecordOutput, DataSourceListRecordsInput,
        DataSourceListRecordsOutput, DataSourcePreviewReadInput, DataSourcePreviewReadOutput,
        DataSourceResourceDescriptor, DataSourceUpdateRecordInput, DataSourceUpdateRecordOutput,
    },
    error::PluginFrameworkError,
    provider_contract::{
        ProviderBalanceResult, ProviderCountTokensInput, ProviderCountTokensResult,
        ProviderInvocationInput, ProviderModelDescriptor,
    },
};
use plugin_runner::{
    capability_host::CapabilityHost, data_source_host::DataSourceHost, provider_host::ProviderHost,
};
use runtime_core::runtime_engine::DataSourceRuntimeRecordBackend;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use storage_durable::MainDurableStore;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::runtime_activity::{
    current_application_id, ApplicationActivityFinish, ApplicationActivityGuard,
    ApplicationActivityKind, ApplicationRuntimeActivityTracker,
};

#[derive(Clone)]
pub struct ApiRuntimeServices {
    provider_host: Arc<RwLock<ProviderHost>>,
    capability_host: Arc<RwLock<CapabilityHost>>,
    data_source_host: Arc<RwLock<DataSourceHost>>,
}

impl ApiRuntimeServices {
    pub fn new(
        provider_host: Arc<RwLock<ProviderHost>>,
        capability_host: Arc<RwLock<CapabilityHost>>,
        data_source_host: Arc<RwLock<DataSourceHost>>,
    ) -> Self {
        Self {
            provider_host,
            capability_host,
            data_source_host,
        }
    }
}

#[derive(Clone)]
pub struct ApiProviderRuntime {
    services: Arc<ApiRuntimeServices>,
    runtime_activity: Option<Arc<ApplicationRuntimeActivityTracker>>,
}

impl ApiProviderRuntime {
    pub fn new(services: Arc<ApiRuntimeServices>) -> Self {
        Self {
            services,
            runtime_activity: None,
        }
    }

    pub fn new_with_activity(
        services: Arc<ApiRuntimeServices>,
        runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    ) -> Self {
        Self {
            services,
            runtime_activity: Some(runtime_activity),
        }
    }

    pub async fn get_balance(
        &self,
        installation: &domain::PluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderBalanceResult> {
        <Self as ProviderRuntimePort>::get_balance(self, installation, provider_config).await
    }
}

#[derive(Clone)]
pub struct ApiDataSourceRuntimeRecordBackend {
    repository: MainDurableStore,
    runtime: ApiProviderRuntime,
    secret_master_key: String,
}

impl ApiDataSourceRuntimeRecordBackend {
    pub fn new(
        repository: MainDurableStore,
        runtime: ApiProviderRuntime,
        secret_master_key: impl Into<String>,
    ) -> Self {
        Self {
            repository,
            runtime,
            secret_master_key: secret_master_key.into(),
        }
    }

    async fn load_target(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
    ) -> anyhow::Result<DataSourceRuntimeTarget> {
        let instance = DataSourceRepository::get_instance(
            &self.repository,
            workspace_id,
            data_source_instance_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;
        if instance.status != domain::DataSourceInstanceStatus::Ready {
            return Err(ControlPlaneError::Conflict("data_source_instance_not_ready").into());
        }

        let installation =
            reconcile_installation_snapshot(&self.repository, instance.installation_id).await?;
        let assigned = PluginRepository::list_assignments(&self.repository, workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned {
            return Err(ControlPlaneError::Conflict("plugin_assignment_required").into());
        }
        if installation.desired_state == domain::PluginDesiredState::Disabled
            || installation.availability_status != domain::PluginAvailabilityStatus::Available
        {
            return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
        }
        if installation.contract_version != "1flowbase.data_source/v1" {
            return Err(ControlPlaneError::InvalidInput("plugin_installation").into());
        }
        if installation.provider_code != instance.source_code {
            return Err(ControlPlaneError::InvalidInput("source_code").into());
        }
        let secret_json = DataSourceRepository::get_secret_json(
            &self.repository,
            instance.id,
            &self.secret_master_key,
        )
        .await?
        .unwrap_or_else(|| serde_json::json!({}));
        let secret_values = collect_secret_strings(&secret_json);

        Ok(DataSourceRuntimeTarget {
            installation,
            connection: DataSourceConfigInput {
                config_json: instance.config_json,
                secret_json,
            },
            secret_values,
        })
    }
}

struct DataSourceRuntimeTarget {
    installation: domain::PluginInstallationRecord,
    connection: DataSourceConfigInput,
    secret_values: HashSet<String>,
}

#[async_trait]
impl ProviderRuntimePort for ApiProviderRuntime {
    async fn ensure_loaded(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> anyhow::Result<()> {
        self.ensure_provider_loaded(installation).await
    }

    async fn validate_provider(
        &self,
        installation: &domain::PluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<Value> {
        self.ensure_provider_loaded(installation).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.validate_operation(&installation.plugin_id, provider_config)
                .map_err(map_provider_framework_error)?
        };
        operation
            .await
            .map(|output| output.output)
            .map_err(map_provider_framework_error)
    }

    async fn list_models(
        &self,
        installation: &domain::PluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<Vec<ProviderModelDescriptor>> {
        self.ensure_provider_loaded(installation).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.list_models_operation(&installation.plugin_id, provider_config)
                .map_err(map_provider_framework_error)?
        };
        operation
            .await
            .map(|output| output.models)
            .map_err(map_provider_framework_error)
    }

    async fn get_balance(
        &self,
        installation: &domain::PluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderBalanceResult> {
        self.ensure_provider_loaded(installation).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.get_balance_operation(&installation.plugin_id, provider_config)
                .map_err(map_provider_framework_error)?
        };
        operation
            .await
            .map(|output| output.balance)
            .map_err(map_provider_framework_error)
    }

    async fn count_tokens(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: ProviderCountTokensInput,
    ) -> anyhow::Result<ProviderCountTokensResult> {
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        self.ensure_provider_loaded(installation).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.count_tokens_operation(&installation.plugin_id, input)
                .map_err(anyhow::Error::new)
        };
        let result = match operation {
            Ok(operation) => operation
                .await
                .map(|output| output.result)
                .map_err(anyhow::Error::new),
            Err(error) => Err(error),
        };
        finish_runtime_activity(activity, &result);
        result
    }

    async fn invoke_stream(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        self.ensure_provider_loaded(installation).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.invoke_stream_operation(&installation.plugin_id, input)
                .map_err(map_provider_framework_error)
        };
        let result = match operation {
            Ok(operation) => operation
                .await
                .map(|output| ProviderRuntimeInvocationOutput {
                    events: output.events,
                    result: output.result,
                })
                .map_err(map_provider_framework_error),
            Err(error) => Err(error),
        };
        finish_runtime_activity(activity, &result);
        result
    }

    async fn invoke_stream_with_live_events(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: ProviderInvocationInput,
        live_events: Option<
            tokio::sync::mpsc::UnboundedSender<
                plugin_framework::provider_contract::ProviderStreamEvent,
            >,
        >,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        self.ensure_provider_loaded(installation).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.invoke_stream_with_live_events_operation(
                &installation.plugin_id,
                input,
                live_events,
            )
            .map_err(map_provider_framework_error)
        };
        let result = match operation {
            Ok(operation) => operation
                .await
                .map(|output| ProviderRuntimeInvocationOutput {
                    events: output.events,
                    result: output.result,
                })
                .map_err(map_provider_framework_error),
            Err(error) => Err(error),
        };
        finish_runtime_activity(activity, &result);
        result
    }
}

#[async_trait]
impl DataSourceRuntimePort for ApiProviderRuntime {
    async fn ensure_loaded(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> anyhow::Result<()> {
        let mut host = self.services.data_source_host.write().await;
        match host.reload(&installation.plugin_id) {
            Ok(_) => Ok(()),
            Err(_) => host
                .load(&installation.installed_path)
                .map(|_| ())
                .map_err(|error| map_framework_error(error, "data_source_runtime")),
        }
    }

    async fn validate_config(
        &self,
        installation: &domain::PluginInstallationRecord,
        config_json: Value,
        secret_json: Value,
    ) -> anyhow::Result<Value> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.validate_config_operation(
                &installation.plugin_id,
                DataSourceConfigInput {
                    config_json,
                    secret_json,
                },
            )
            .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        operation
            .await
            .map(|output| output.output)
            .map_err(|error| map_framework_error(error, "data_source_runtime"))
    }

    async fn test_connection(
        &self,
        installation: &domain::PluginInstallationRecord,
        config_json: Value,
        secret_json: Value,
    ) -> anyhow::Result<Value> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.test_connection_operation(
                &installation.plugin_id,
                DataSourceConfigInput {
                    config_json,
                    secret_json,
                },
            )
            .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        operation
            .await
            .map(|output| output.output)
            .map_err(|error| map_framework_error(error, "data_source_runtime"))
    }

    async fn discover_catalog(
        &self,
        installation: &domain::PluginInstallationRecord,
        config_json: Value,
        secret_json: Value,
    ) -> anyhow::Result<Value> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.discover_catalog_operation(
                &installation.plugin_id,
                DataSourceConfigInput {
                    config_json,
                    secret_json,
                },
            )
            .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        let output = operation
            .await
            .map_err(|error| map_framework_error(error, "data_source_runtime"))?;
        Ok(serde_json::to_value(output.entries)?)
    }

    async fn describe_resource(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: DataSourceDescribeResourceInput,
    ) -> anyhow::Result<DataSourceResourceDescriptor> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.describe_resource_operation(
                &installation.plugin_id,
                input.connection,
                input.resource_key,
            )
            .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        operation
            .await
            .map(|output| output.descriptor)
            .map_err(|error| map_framework_error(error, "data_source_runtime"))
    }

    async fn preview_read(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: DataSourcePreviewReadInput,
    ) -> anyhow::Result<DataSourcePreviewReadOutput> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.preview_read_operation(&installation.plugin_id, input)
                .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        operation
            .await
            .map_err(|error| map_framework_error(error, "data_source_runtime"))
    }
}

#[async_trait]
impl DataSourceCrudRuntimePort for ApiProviderRuntime {
    async fn list_records(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: DataSourceListRecordsInput,
    ) -> anyhow::Result<DataSourceListRecordsOutput> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.list_records_operation(&installation.plugin_id, input)
                .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        operation
            .await
            .map_err(|error| map_framework_error(error, "data_source_runtime"))
    }

    async fn get_record(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: DataSourceGetRecordInput,
    ) -> anyhow::Result<DataSourceGetRecordOutput> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.get_record_operation(&installation.plugin_id, input)
                .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        operation
            .await
            .map_err(|error| map_framework_error(error, "data_source_runtime"))
    }

    async fn create_record(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: DataSourceCreateRecordInput,
    ) -> anyhow::Result<DataSourceCreateRecordOutput> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.create_record_operation(&installation.plugin_id, input)
                .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        operation
            .await
            .map_err(|error| map_framework_error(error, "data_source_runtime"))
    }

    async fn update_record(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: DataSourceUpdateRecordInput,
    ) -> anyhow::Result<DataSourceUpdateRecordOutput> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.update_record_operation(&installation.plugin_id, input)
                .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        operation
            .await
            .map_err(|error| map_framework_error(error, "data_source_runtime"))
    }

    async fn delete_record(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: DataSourceDeleteRecordInput,
    ) -> anyhow::Result<DataSourceDeleteRecordOutput> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.delete_record_operation(&installation.plugin_id, input)
                .map_err(|error| map_framework_error(error, "data_source_runtime"))?
        };
        operation
            .await
            .map_err(|error| map_framework_error(error, "data_source_runtime"))
    }
}

#[async_trait]
impl DataSourceRuntimeRecordBackend for ApiDataSourceRuntimeRecordBackend {
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
}

#[async_trait]
impl CapabilityPluginRuntimePort for ApiProviderRuntime {
    async fn validate_config(&self, input: ValidateCapabilityConfigInput) -> anyhow::Result<Value> {
        self.ensure_capability_loaded(&input.installation).await?;
        let operation = {
            let host = self.services.capability_host.read().await;
            host.validate_config_operation(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
            )
            .map_err(|error| map_framework_error(error, "capability_runtime"))?
        };
        operation
            .await
            .map(|output| output.output)
            .map_err(|error| map_framework_error(error, "capability_runtime"))
    }

    async fn resolve_dynamic_options(
        &self,
        input: ResolveCapabilityOptionsInput,
    ) -> anyhow::Result<Value> {
        self.ensure_capability_loaded(&input.installation).await?;
        let operation = {
            let host = self.services.capability_host.read().await;
            host.resolve_dynamic_options_operation(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
            )
            .map_err(|error| map_framework_error(error, "capability_runtime"))?
        };
        operation
            .await
            .map(|output| output.output)
            .map_err(|error| map_framework_error(error, "capability_runtime"))
    }

    async fn resolve_output_schema(
        &self,
        input: ResolveCapabilityOutputSchemaInput,
    ) -> anyhow::Result<Value> {
        self.ensure_capability_loaded(&input.installation).await?;
        let operation = {
            let host = self.services.capability_host.read().await;
            host.resolve_output_schema_operation(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
            )
            .map_err(|error| map_framework_error(error, "capability_runtime"))?
        };
        operation
            .await
            .map(|output| output.output)
            .map_err(|error| map_framework_error(error, "capability_runtime"))
    }

    async fn execute_node(
        &self,
        input: ExecuteCapabilityNodeInput,
    ) -> anyhow::Result<CapabilityExecutionOutput> {
        let activity = self.start_runtime_activity(ApplicationActivityKind::ToolCall);
        self.ensure_capability_loaded(&input.installation).await?;
        let operation = {
            let host = self.services.capability_host.read().await;
            host.execute_operation(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
                input.input_payload,
            )
            .map_err(|error| map_framework_error(error, "capability_runtime"))
        };
        let result = match operation {
            Ok(operation) => operation
                .await
                .map(|output| CapabilityExecutionOutput {
                    output_payload: output.output_payload,
                })
                .map_err(|error| map_framework_error(error, "capability_runtime")),
            Err(error) => Err(error),
        };
        finish_runtime_activity(activity, &result);
        result
    }
}

fn finish_runtime_activity<T, E>(
    activity: Option<ApplicationActivityGuard>,
    result: &Result<T, E>,
) {
    if let Some(activity) = activity {
        let finish = if result.is_ok() {
            ApplicationActivityFinish::Completed
        } else {
            ApplicationActivityFinish::Failed
        };
        activity.finish(finish);
    }
}

fn redact_data_source_output<T>(output: T, secrets: &HashSet<String>) -> anyhow::Result<T>
where
    T: Serialize + DeserializeOwned,
{
    let value = serde_json::to_value(output)?;
    Ok(serde_json::from_value(redact_value(&value, secrets))?)
}

impl ApiProviderRuntime {
    fn start_runtime_activity(
        &self,
        kind: ApplicationActivityKind,
    ) -> Option<ApplicationActivityGuard> {
        let application_id = current_application_id()?;
        self.runtime_activity
            .as_ref()
            .map(|tracker| tracker.start(application_id, kind))
    }

    async fn ensure_provider_loaded(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> anyhow::Result<()> {
        let ensure_loaded_started = std::time::Instant::now();
        let mut host = self.services.provider_host.write().await;
        let source_identity = provider_source_identity(installation);
        let result = host
            .load_if_needed(
                &installation.plugin_id,
                &installation.installed_path,
                Some(source_identity.as_str()),
            )
            .map_err(|error| map_framework_error(error, "provider_runtime"));
        tracing::debug!(
            plugin_id = %installation.plugin_id,
            provider_ensure_loaded_ms = ensure_loaded_started.elapsed().as_millis() as u64,
            "provider ensure_loaded finished"
        );
        result
    }

    async fn ensure_capability_loaded(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> anyhow::Result<()> {
        let mut host = self.services.capability_host.write().await;
        host.load(&installation.installed_path)
            .map(|_| ())
            .map_err(|error| map_framework_error(error, "capability_runtime"))
    }

    async fn ensure_data_source_loaded(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> anyhow::Result<()> {
        let mut host = self.services.data_source_host.write().await;
        match host.reload(&installation.plugin_id) {
            Ok(_) => Ok(()),
            Err(_) => host
                .load(&installation.installed_path)
                .map(|_| ())
                .map_err(|error| map_framework_error(error, "data_source_runtime")),
        }
    }
}

fn provider_source_identity(installation: &domain::PluginInstallationRecord) -> String {
    format!(
        "installation_id={};checksum={};manifest_fingerprint={};updated_at={}",
        installation.id,
        installation.checksum.as_deref().unwrap_or(""),
        installation.manifest_fingerprint.as_deref().unwrap_or(""),
        installation.updated_at.unix_timestamp_nanos()
    )
}

fn map_provider_framework_error(error: PluginFrameworkError) -> anyhow::Error {
    match error {
        runtime_error @ PluginFrameworkError::RuntimeContract { .. } => runtime_error.into(),
        other => map_framework_error(other, "provider_runtime"),
    }
}

fn map_framework_error(error: PluginFrameworkError, service_name: &'static str) -> anyhow::Error {
    match error {
        PluginFrameworkError::InvalidAssignment { .. }
        | PluginFrameworkError::InvalidProviderPackage { .. }
        | PluginFrameworkError::InvalidProviderContract { .. }
        | PluginFrameworkError::Serialization { .. } => {
            ControlPlaneError::InvalidInput(service_name).into()
        }
        PluginFrameworkError::Io { .. } | PluginFrameworkError::RuntimeContract { .. } => {
            ControlPlaneError::UpstreamUnavailable(service_name).into()
        }
    }
}
