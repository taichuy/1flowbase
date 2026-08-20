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
        ProviderLiveEventSenders, ProviderRuntimeInvocationOutput, ProviderRuntimePort,
    },
};
use plugin_framework::{
    data_source_contract::{
        DataSourceConfigInput, DataSourceCreateRecordInput, DataSourceCreateRecordOutput,
        DataSourceDeleteRecordInput, DataSourceDeleteRecordOutput, DataSourceDescribeResourceInput,
        DataSourceExecuteSqlInput, DataSourceGetRecordInput, DataSourceGetRecordOutput,
        DataSourceListRecordsInput, DataSourceListRecordsOutput, DataSourcePreviewReadInput,
        DataSourcePreviewReadOutput, DataSourceResourceDescriptor, DataSourceUpdateRecordInput,
        DataSourceUpdateRecordOutput, NativeSqlExecutionOutput,
    },
    error::PluginFrameworkError,
    provider_contract::{
        ProviderAuthOperation, ProviderAuthResult, ProviderBalanceResult, ProviderCompactResult,
        ProviderCountTokensInput, ProviderCountTokensResult, ProviderInvocationInput,
        ProviderModelDescriptor, ProviderResetCreditOperation, ProviderResetCreditResult,
        ProviderUsageWindowsResult,
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
use tracing::info;
use uuid::Uuid;

use crate::runtime_activity::{
    current_application_id, ApplicationActivityFinish, ApplicationActivityGuard,
    ApplicationActivityKind, ApplicationRuntimeActivityTracker,
};

mod model_provider_slot;

pub use model_provider_slot::{
    ModelProviderBindingProvenance, ModelProviderSlotBinding, ModelProviderSlotResolver,
};

#[derive(Clone)]
pub struct ApiRuntimeServices {
    provider_host: Arc<RwLock<ProviderHost>>,
    capability_host: Arc<RwLock<CapabilityHost>>,
    data_source_host: Arc<RwLock<DataSourceHost>>,
    data_model_template_catalog:
        runtime_core::data_model_template_registry::DataModelTemplateCatalog,
    model_provider_slot_resolver: Option<ModelProviderSlotResolver>,
    provider_input_pipeline:
        Option<Arc<orchestration_runtime::provider_input_pipeline::ProviderInputPipeline>>,
}

impl ApiRuntimeServices {
    pub fn new(
        provider_host: Arc<RwLock<ProviderHost>>,
        capability_host: Arc<RwLock<CapabilityHost>>,
        data_source_host: Arc<RwLock<DataSourceHost>>,
        extension_graph: Arc<plugin_framework::extension_bus::EffectiveExtensionGraph>,
    ) -> anyhow::Result<Self> {
        let provider_input_pipeline = Arc::new(
            orchestration_runtime::provider_input_pipeline::ProviderInputPipeline::from_graph(
                Arc::clone(&extension_graph),
                Vec::new(),
            )?,
        );
        Ok(Self {
            provider_host,
            capability_host,
            data_source_host,
            data_model_template_catalog:
                runtime_core::data_model_template_registry::DataModelTemplateCatalog::core(),
            model_provider_slot_resolver: Some(ModelProviderSlotResolver::new(extension_graph)),
            provider_input_pipeline: Some(provider_input_pipeline),
        })
    }

    /// Explicit escape hatch for lightweight and legacy test states.
    /// Production boot must use [`Self::new`] with the published Extension Bus graph.
    #[doc(hidden)]
    pub fn new_without_model_provider_extension_graph_for_tests(
        provider_host: Arc<RwLock<ProviderHost>>,
        capability_host: Arc<RwLock<CapabilityHost>>,
        data_source_host: Arc<RwLock<DataSourceHost>>,
    ) -> Self {
        Self {
            provider_host,
            capability_host,
            data_source_host,
            data_model_template_catalog:
                runtime_core::data_model_template_registry::DataModelTemplateCatalog::core(),
            model_provider_slot_resolver: None,
            provider_input_pipeline: None,
        }
    }

    pub fn model_provider_extension_graph(
        &self,
    ) -> Option<&Arc<plugin_framework::extension_bus::EffectiveExtensionGraph>> {
        self.model_provider_slot_resolver
            .as_ref()
            .map(ModelProviderSlotResolver::graph_arc)
    }

    pub fn data_model_template_catalog(
        &self,
    ) -> runtime_core::data_model_template_registry::DataModelTemplateCatalog {
        self.data_model_template_catalog.clone()
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
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderBalanceResult> {
        <Self as ProviderRuntimePort>::get_balance(self, installation, provider_config).await
    }

    pub async fn get_usage_windows(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderUsageWindowsResult> {
        <Self as ProviderRuntimePort>::get_usage_windows(self, installation, provider_config).await
    }

    pub async fn reset_credit(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
        operation: ProviderResetCreditOperation,
    ) -> anyhow::Result<ProviderResetCreditResult> {
        <Self as ProviderRuntimePort>::reset_credit(self, installation, provider_config, operation)
            .await
    }
}

#[derive(Clone)]
pub struct ApiDataSourceRuntimeRecordBackend {
    repository: MainDurableStore,
    runtime: ApiProviderRuntime,
    secret_master_key: String,
    node_id: String,
}

impl ApiDataSourceRuntimeRecordBackend {
    pub fn new(
        repository: MainDurableStore,
        runtime: ApiProviderRuntime,
        secret_master_key: impl Into<String>,
        node_id: impl Into<String>,
    ) -> Self {
        Self {
            repository,
            runtime,
            secret_master_key: secret_master_key.into(),
            node_id: node_id.into(),
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

        let installation = reconcile_installation_snapshot(
            &self.repository,
            &self.node_id,
            instance.installation_id,
        )
        .await?;
        let assigned = PluginRepository::list_assignments(&self.repository, workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned {
            return Err(ControlPlaneError::Conflict("plugin_assignment_required").into());
        }
        if installation.desired_state == domain::PluginDesiredState::Disabled
            || installation.availability_status() != domain::PluginAvailabilityStatus::Available
        {
            return Err(ControlPlaneError::PluginUnavailable.into());
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
    installation: domain::LocalPluginInstallationRecord,
    connection: DataSourceConfigInput,
    secret_values: HashSet<String>,
}

#[async_trait]
impl ProviderRuntimePort for ApiProviderRuntime {
    async fn activate_plugin(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()> {
        match installation.contract_version.as_str() {
            plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT => {
                let binding = self.resolve_model_provider_binding(installation)?;
                self.ensure_provider_loaded(&binding).await
            }
            "1flowbase.data_source/v1" => self.ensure_data_source_loaded(installation).await,
            "1flowbase.capability/v1" => self.ensure_capability_loaded(installation).await,
            _ => Err(ControlPlaneError::InvalidInput("plugin_installation").into()),
        }
    }

    async fn deactivate_plugin(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> anyhow::Result<()> {
        match installation.contract_version.as_str() {
            plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT => self
                .services
                .provider_host
                .write()
                .await
                .unload(&installation.plugin_id)
                .await
                .map_err(map_provider_framework_error),
            "1flowbase.data_source/v1" => {
                self.services
                    .data_source_host
                    .write()
                    .await
                    .unload(&installation.plugin_id)
                    .await
                    .map_err(map_provider_framework_error)?;
                Ok(())
            }
            "1flowbase.capability/v1" => {
                self.services
                    .capability_host
                    .write()
                    .await
                    .unload(&installation.plugin_id)
                    .await
                    .map_err(map_provider_framework_error)?;
                Ok(())
            }
            _ => Err(ControlPlaneError::InvalidInput("plugin_installation").into()),
        }
    }

    async fn pipeline_provider_input(
        &self,
        input: ProviderInvocationInput,
    ) -> std::result::Result<
        orchestration_runtime::provider_input_pipeline::ProviderInputPipelineOutput,
        orchestration_runtime::provider_input_pipeline::ProviderInputPipelineError,
    > {
        match self.services.provider_input_pipeline.as_ref() {
            Some(pipeline) => pipeline.execute(input).await,
            None => Ok(
                orchestration_runtime::provider_input_pipeline::ProviderInputPipelineOutput::unchanged(
                    input,
                ),
            ),
        }
    }

    async fn ensure_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await
    }

    async fn validate_provider(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<Value> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.validate_operation(&binding.plugin_id, provider_config)
                .map_err(map_provider_framework_error)?
        };
        operation
            .await
            .map(|output| output.output)
            .map_err(map_provider_framework_error)
    }

    async fn authenticate_provider(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
        operation: ProviderAuthOperation,
    ) -> anyhow::Result<ProviderAuthResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        let auth_operation = {
            let host = self.services.provider_host.read().await;
            host.authenticate_operation(&binding.plugin_id, provider_config, operation)
                .map_err(map_provider_framework_error)?
        };
        auth_operation
            .await
            .map(|output| output.result)
            .map_err(map_provider_framework_error)
    }

    async fn list_models(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<Vec<ProviderModelDescriptor>> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.list_models_operation(&binding.plugin_id, provider_config)
                .map_err(map_provider_framework_error)?
        };
        operation
            .await
            .map(|output| output.models)
            .map_err(map_provider_framework_error)
    }

    async fn get_balance(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderBalanceResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.get_balance_operation(&binding.plugin_id, provider_config)
                .map_err(map_provider_framework_error)?
        };
        operation
            .await
            .map(|output| output.balance)
            .map_err(map_provider_framework_error)
    }

    async fn get_usage_windows(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderUsageWindowsResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.get_usage_windows_operation(&binding.plugin_id, provider_config)
                .map_err(map_provider_framework_error)?
        };
        operation
            .await
            .map(|output| output.usage)
            .map_err(map_provider_framework_error)
    }

    async fn reset_credit(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
        reset_credit_operation: ProviderResetCreditOperation,
    ) -> anyhow::Result<ProviderResetCreditResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.reset_credit_operation(&binding.plugin_id, provider_config, reset_credit_operation)
                .map_err(map_provider_framework_error)?
        };
        operation
            .await
            .map(|output| output.result)
            .map_err(map_provider_framework_error)
    }

    async fn count_tokens(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: ProviderCountTokensInput,
    ) -> anyhow::Result<ProviderCountTokensResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.as_invocation().provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        let operation_name = "count_tokens";
        trace_provider_operation_boundary(&binding, operation_name, "start", "started");
        let result = async {
            self.ensure_provider_loaded(&binding).await?;
            let operation = {
                let host = self.services.provider_host.read().await;
                host.count_tokens_operation(&binding.plugin_id, input)
                    .map_err(anyhow::Error::new)
            };
            match operation {
                Ok(operation) => operation
                    .await
                    .map(|output| output.result)
                    .map_err(anyhow::Error::new),
                Err(error) => Err(error),
            }
        }
        .await;
        trace_provider_operation_boundary(
            &binding,
            operation_name,
            "end",
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
        );
        finish_runtime_activity(activity, &result);
        result
    }

    async fn compact(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderCompactResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        self.ensure_provider_loaded(&binding).await?;
        let operation = {
            let host = self.services.provider_host.read().await;
            host.compact_operation(&binding.plugin_id, input)
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
        installation: &domain::LocalPluginInstallationRecord,
        input: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        let operation_name = "invoke_stream";
        trace_provider_operation_boundary(&binding, operation_name, "start", "started");
        let result = async {
            self.ensure_provider_loaded(&binding).await?;
            let operation = {
                let host = self.services.provider_host.read().await;
                host.invoke_stream_operation(&binding.plugin_id, input)
                    .map_err(map_provider_framework_error)
            };
            match operation {
                Ok(operation) => operation
                    .await
                    .map(|output| ProviderRuntimeInvocationOutput {
                        events: output.events,
                        result: output.result,
                    })
                    .map_err(map_provider_framework_error),
                Err(error) => Err(error),
            }
        }
        .await;
        trace_provider_operation_boundary(
            &binding,
            operation_name,
            "end",
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
        );
        finish_runtime_activity(activity, &result);
        result
    }

    async fn invoke_stream_with_live_events(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: ProviderInvocationInput,
        live_events: Option<ProviderLiveEventSenders>,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        let operation_name = "invoke_stream_with_live_events";
        trace_provider_operation_boundary(&binding, operation_name, "start", "started");
        let result = async {
            self.ensure_provider_loaded(&binding).await?;
            let operation = {
                let host = self.services.provider_host.read().await;
                let (required_live_events, diagnostic_live_events) = live_events
                    .map(|senders| (Some(senders.required), Some(senders.diagnostic)))
                    .unwrap_or((None, None));
                host.invoke_stream_with_live_events_operation(
                    &binding.plugin_id,
                    input,
                    required_live_events,
                    diagnostic_live_events,
                )
                .map_err(map_provider_framework_error)
            };
            match operation {
                Ok(operation) => operation
                    .await
                    .map(|output| ProviderRuntimeInvocationOutput {
                        events: output.events,
                        result: output.result,
                    })
                    .map_err(map_provider_framework_error),
                Err(error) => Err(error),
            }
        }
        .await;
        trace_provider_operation_boundary(
            &binding,
            operation_name,
            "end",
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
        );
        finish_runtime_activity(activity, &result);
        result
    }
}

fn trace_provider_operation_boundary(
    binding: &ModelProviderSlotBinding,
    operation: &'static str,
    phase: &'static str,
    status: &'static str,
) {
    let package_sha256 = binding
        .artifact_checksum
        .as_deref()
        .unwrap_or("")
        .trim_start_matches("sha256:");
    info!(
        operation = %operation,
        provider_code = %binding.provider_code,
        installation_id = %binding.installation_id,
        package_sha256 = %package_sha256,
        phase = %phase,
        status = %status,
        "provider runtime operation boundary"
    );
}

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
        installation: &domain::LocalPluginInstallationRecord,
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
        installation: &domain::LocalPluginInstallationRecord,
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
        installation: &domain::LocalPluginInstallationRecord,
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
        installation: &domain::LocalPluginInstallationRecord,
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

    async fn execute_sql(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: DataSourceExecuteSqlInput,
    ) -> anyhow::Result<NativeSqlExecutionOutput> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.execute_sql_operation(&installation.plugin_id, input)
                .map_err(map_provider_framework_error)?
        };
        operation.await.map_err(map_provider_framework_error)
    }

    async fn execute_model_operation(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: plugin_framework::DataSourceExecuteModelOperationInput,
    ) -> anyhow::Result<Value> {
        self.ensure_data_source_loaded(installation).await?;
        let operation = {
            let host = self.services.data_source_host.read().await;
            host.execute_model_operation_call(&installation.plugin_id, input)
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
        installation: &domain::LocalPluginInstallationRecord,
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
        installation: &domain::LocalPluginInstallationRecord,
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
        installation: &domain::LocalPluginInstallationRecord,
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
        installation: &domain::LocalPluginInstallationRecord,
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
        installation: &domain::LocalPluginInstallationRecord,
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
    async fn execute_sql(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: &str,
        sql: &str,
    ) -> anyhow::Result<NativeSqlExecutionOutput> {
        if data_source_instance_id == "main" {
            return storage_durable::execute_native_sql(self.repository.pool(), sql)
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
                    granted_credit_permissions: output.granted_credit_permissions,
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

    fn resolve_model_provider_binding(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<ModelProviderSlotBinding> {
        match self.services.model_provider_slot_resolver.as_ref() {
            Some(resolver) => resolver.resolve(installation),
            None => ModelProviderSlotBinding::legacy_for_tests(installation),
        }
    }

    async fn ensure_provider_loaded(
        &self,
        binding: &ModelProviderSlotBinding,
    ) -> anyhow::Result<()> {
        let ensure_loaded_started = std::time::Instant::now();
        let mut host = self.services.provider_host.write().await;
        let result = match binding.legacy_manifest_eligibility() {
            Some(eligibility) => host.load_legacy_installed_if_needed(
                &binding.plugin_id,
                binding.package_root(),
                Some(binding.source_identity.as_str()),
                eligibility,
            ),
            None => host.load_if_needed(
                &binding.plugin_id,
                binding.package_root(),
                Some(binding.source_identity.as_str()),
            ),
        }
        .map_err(map_provider_framework_error);
        tracing::debug!(
            plugin_id = %binding.plugin_id,
            provider_ensure_loaded_ms = ensure_loaded_started.elapsed().as_millis() as u64,
            "provider ensure_loaded finished"
        );
        result
    }

    async fn ensure_capability_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()> {
        let mut host = self.services.capability_host.write().await;
        if host.is_loaded(&installation.plugin_id) {
            return Ok(());
        }
        let eligibility = legacy_manifest_eligibility(installation)?;
        match eligibility.as_ref() {
            Some(eligibility) => {
                host.load_legacy_installed(required_local_path(installation)?, eligibility)
                    .await
            }
            None => host.load(required_local_path(installation)?).await,
        }
        .map(|_| ())
        .map_err(|error| map_framework_error(error, "capability_runtime"))
    }

    async fn ensure_data_source_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()> {
        let mut host = self.services.data_source_host.write().await;
        if !host.is_loaded(&installation.plugin_id) {
            let eligibility = legacy_manifest_eligibility(installation)?;
            match eligibility.as_ref() {
                Some(eligibility) => {
                    host.load_legacy_installed(required_local_path(installation)?, eligibility)
                        .await
                }
                None => host.load(required_local_path(installation)?).await,
            }
            .map(|_| ())
            .map_err(|error| map_framework_error(error, "data_source_runtime"))?;
        }
        let templates = host
            .data_model_templates(&installation.plugin_id)
            .map_err(|error| map_framework_error(error, "data_source_runtime"))?;
        drop(host);
        self.services
            .data_model_template_catalog
            .replace_provider(
                installation.plugin_id.clone(),
                &installation.provider_code,
                templates,
            )
            .map_err(|error| {
                ControlPlaneError::Conflict(match error {
                    runtime_core::data_model_template_registry::DataModelTemplateRegistryError::DuplicateIdentity(_) => "data_model_template_duplicate_identity",
                    _ => "data_model_template_unavailable",
                })
            })?;
        Ok(())
    }
}

fn data_source_capability_codes(
    capabilities: &plugin_framework::DataSourceCrudCapabilities,
) -> Vec<String> {
    let mut codes = Vec::new();
    if capabilities.supports_list {
        codes.push("list_records".to_owned());
    }
    if capabilities.supports_get {
        codes.push("get_record".to_owned());
    }
    if capabilities.supports_list && capabilities.supports_get {
        codes.push(
            runtime_core::general_data_model_template::GENERAL_RECORDS_READ_CAPABILITY.to_owned(),
        );
    }
    if capabilities.supports_create {
        codes.push("create_record".to_owned());
    }
    if capabilities.supports_update {
        codes.push("update_record".to_owned());
    }
    if capabilities.supports_delete {
        codes.push("delete_record".to_owned());
    }
    codes
}

fn legacy_manifest_eligibility(
    installation: &domain::LocalPluginInstallationRecord,
) -> anyhow::Result<Option<plugin_framework::LegacyInstalledManifestEligibility>> {
    let Some(compatibility) = installation.legacy_manifest_compatibility.as_deref() else {
        return Ok(None);
    };
    if compatibility != "missing_publisher_namespace_v1" {
        return Err(
            ControlPlaneError::Conflict("plugin_manifest_compatibility_unsupported").into(),
        );
    }
    let fingerprint =
        installation
            .artifact
            .manifest_fingerprint
            .clone()
            .ok_or(ControlPlaneError::Conflict(
                "plugin_manifest_fingerprint_missing",
            ))?;
    Ok(Some(plugin_framework::LegacyInstalledManifestEligibility {
        expected_publisher_namespace: installation.organization.clone(),
        expected_versioned_plugin_id: installation.plugin_id.clone(),
        expected_raw_manifest_fingerprint: fingerprint,
    }))
}

fn required_local_path(
    installation: &domain::LocalPluginInstallationRecord,
) -> anyhow::Result<&str> {
    installation
        .local_path()
        .ok_or_else(|| ControlPlaneError::Conflict("plugin_artifact_path_missing").into())
}

fn map_provider_framework_error(error: PluginFrameworkError) -> anyhow::Error {
    match error {
        preserved_error @ (PluginFrameworkError::RuntimeContract { .. }
        | PluginFrameworkError::InvalidProviderContract { .. }) => preserved_error.into(),
        other => map_framework_error(other, "provider_runtime"),
    }
}

fn map_framework_error(error: PluginFrameworkError, service_name: &'static str) -> anyhow::Error {
    match error {
        PluginFrameworkError::InvalidAssignment { .. }
        | PluginFrameworkError::InvalidProviderPackage { .. }
        | PluginFrameworkError::InvalidProviderContract { .. }
        | PluginFrameworkError::PackageRuntimeTargetMismatch { .. }
        | PluginFrameworkError::Serialization { .. } => {
            ControlPlaneError::InvalidInput(service_name).into()
        }
        PluginFrameworkError::Io { .. } | PluginFrameworkError::RuntimeContract { .. } => {
            ControlPlaneError::UpstreamUnavailable(service_name).into()
        }
    }
}
