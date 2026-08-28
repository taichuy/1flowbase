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
    plugin_management::ready_current_node_plugin_installation,
    ports::{
        DataSourceCrudRuntimePort, DataSourceRepository, DataSourceRuntimePort,
        NetworkEgressRuntimePort, NetworkEgressSecretMaterial, PluginRepository,
        ProviderLiveEventSenders, ProviderRuntimeExecutionContext, ProviderRuntimeInvocationOutput,
        ProviderRuntimePort,
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
        ProviderStreamEvent, ProviderUsageWindowsResult,
    },
    ForwardProxyLease,
};
#[cfg(test)]
use runtime_core::runtime_backend::RuntimeBackendSlot;
use runtime_core::{
    runtime_backend::{
        RuntimeArtifactReference, RuntimeBackend, RuntimeBackendError, RuntimeExecutionPort,
        RuntimeExecutionRequest, RuntimeLegacyManifestEligibility, RuntimeNetworkEgressActivation,
        RuntimePackageActivation, RuntimeRequestId, RuntimeStreamEventSink, RuntimeStreamSinks,
        RuntimeTargetId,
    },
    runtime_engine::DataSourceRuntimeRecordBackend,
};
#[cfg(test)]
use runtime_extension_host::RuntimeExtensionHost;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use storage_durable_postgres::MainDurableStore;
use tracing::info;
use uuid::Uuid;

use crate::network_egress_client::NetworkEgressHttpClientResolver;
use crate::runtime_activity::{
    current_application_id, ApplicationActivityFinish, ApplicationActivityGuard,
    ApplicationActivityKind, ApplicationRuntimeActivityTracker,
};

mod model_provider_slot;

struct RuntimeEventChannelSink(tokio::sync::mpsc::Sender<ProviderStreamEvent>);

#[async_trait]
impl RuntimeStreamEventSink for RuntimeEventChannelSink {
    async fn emit(&self, event: ProviderStreamEvent) -> Result<(), RuntimeBackendError> {
        self.0
            .send(event)
            .await
            .map_err(|_| RuntimeBackendError::Execution {
                target_id: "provider_event_stream".to_string(),
                message: "provider live event receiver is closed".to_string(),
            })
    }
}

pub use model_provider_slot::{
    ModelProviderBindingProvenance, ModelProviderSlotBinding, ModelProviderSlotResolver,
};

#[derive(Clone)]
pub struct ApiRuntimeServices {
    runtime_backend: Arc<dyn RuntimeBackend>,
    orchestration_backend: orchestration_runtime::runtime_backend::OrchestrationRuntimeBackend,
    data_model_template_catalog:
        runtime_core::data_model_template_registry::DataModelTemplateCatalog,
    model_provider_slot_resolver: Option<ModelProviderSlotResolver>,
    provider_input_pipeline:
        Option<Arc<orchestration_runtime::provider_input_pipeline::ProviderInputPipeline>>,
}

#[derive(Clone)]
pub(crate) struct ApiRuntimeArtifactResolver {
    store: MainDurableStore,
    api_node_id: String,
    provider_install_root: PathBuf,
}

impl ApiRuntimeArtifactResolver {
    pub(crate) fn new(
        store: MainDurableStore,
        api_node_id: impl Into<String>,
        provider_install_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            store,
            api_node_id: api_node_id.into(),
            provider_install_root: provider_install_root.into(),
        }
    }
}

#[async_trait]
impl runtime_extension_host::RuntimeArtifactResolver for ApiRuntimeArtifactResolver {
    async fn resolve(
        &self,
        artifact: &runtime_core::runtime_backend::RuntimeArtifactReference,
    ) -> Result<PathBuf, RuntimeBackendError> {
        let installation_id = Uuid::parse_str(artifact.as_str()).map_err(|error| {
            RuntimeBackendError::InvalidRequest(format!(
                "runtime artifact reference is not an installation id: {error}"
            ))
        })?;
        let installation = ready_current_node_plugin_installation(
            &self.store,
            &self.api_node_id,
            &self.provider_install_root,
            installation_id,
        )
        .await
        .map_err(|error| RuntimeBackendError::Execution {
            target_id: artifact.as_str().to_string(),
            message: error.to_string(),
        })?;
        installation
            .local_path()
            .map(PathBuf::from)
            .ok_or_else(|| RuntimeBackendError::Execution {
                target_id: artifact.as_str().to_string(),
                message: "runtime artifact has no local materialization".to_string(),
            })
    }
}

#[cfg(test)]
struct TestRuntimeArtifactResolver;

#[cfg(test)]
#[async_trait]
impl runtime_extension_host::RuntimeArtifactResolver for TestRuntimeArtifactResolver {
    async fn resolve(
        &self,
        artifact: &runtime_core::runtime_backend::RuntimeArtifactReference,
    ) -> Result<PathBuf, RuntimeBackendError> {
        Ok(PathBuf::from(artifact.as_str()))
    }
}

impl ApiRuntimeServices {
    #[cfg(test)]
    pub fn new(
        extension_graph: Arc<plugin_framework::extension_bus::EffectiveExtensionGraph>,
    ) -> anyhow::Result<Self> {
        let runtime_host = Arc::new(RuntimeExtensionHost::new_with_artifact_resolver(
            time::OffsetDateTime::now_utc(),
            Arc::new(TestRuntimeArtifactResolver),
        )?);
        runtime_host.mark_ready()?;
        let mut slot = RuntimeBackendSlot::default();
        slot.bind(runtime_host)?;
        let runtime_backend = slot.backend()?;
        Self::new_with_runtime_backend(runtime_backend, extension_graph)
    }

    pub fn new_with_runtime_backend(
        runtime_backend: Arc<dyn RuntimeBackend>,
        extension_graph: Arc<plugin_framework::extension_bus::EffectiveExtensionGraph>,
    ) -> anyhow::Result<Self> {
        let runtime_execution: Arc<dyn RuntimeExecutionPort> = runtime_backend.clone();
        let provider_input_pipeline = Arc::new(
            orchestration_runtime::provider_input_pipeline::ProviderInputPipeline::from_graph(
                Arc::clone(&extension_graph),
                Vec::new(),
            )?,
        );
        let orchestration_backend =
            orchestration_runtime::runtime_backend::OrchestrationRuntimeBackend::new(
                runtime_execution,
            );
        Ok(Self {
            runtime_backend,
            orchestration_backend,
            data_model_template_catalog:
                runtime_core::data_model_template_registry::DataModelTemplateCatalog::core(),
            model_provider_slot_resolver: Some(ModelProviderSlotResolver::new(extension_graph)),
            provider_input_pipeline: Some(provider_input_pipeline),
        })
    }

    /// Explicit escape hatch for lightweight and legacy test states.
    /// Production boot must use [`Self::new_with_runtime_backend`] with the published graph.
    #[doc(hidden)]
    #[cfg(test)]
    pub fn new_without_model_provider_extension_graph_for_tests() -> Self {
        let runtime_host = Arc::new(
            RuntimeExtensionHost::new_with_artifact_resolver(
                time::OffsetDateTime::now_utc(),
                Arc::new(TestRuntimeArtifactResolver),
            )
            .expect("test runtime host must initialize"),
        );
        runtime_host
            .mark_ready()
            .expect("test runtime host must become ready");
        let mut slot = RuntimeBackendSlot::default();
        slot.bind(runtime_host)
            .expect("test runtime backend must bind exactly once");
        let runtime_backend = slot.backend().expect("test runtime backend must exist");
        let runtime_execution: Arc<dyn RuntimeExecutionPort> = runtime_backend.clone();
        let orchestration_backend =
            orchestration_runtime::runtime_backend::OrchestrationRuntimeBackend::new(
                runtime_execution,
            );
        Self {
            runtime_backend,
            orchestration_backend,
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

    pub fn runtime_backend(&self) -> &Arc<dyn RuntimeBackend> {
        &self.runtime_backend
    }
}

#[derive(Clone)]
pub struct ApiProviderRuntime {
    services: Arc<ApiRuntimeServices>,
    runtime_activity: Option<Arc<ApplicationRuntimeActivityTracker>>,
    network_egress: Option<Arc<NetworkEgressHttpClientResolver>>,
}

impl ApiProviderRuntime {
    pub fn new(services: Arc<ApiRuntimeServices>) -> Self {
        Self {
            services,
            runtime_activity: None,
            network_egress: None,
        }
    }

    pub fn new_with_activity(
        services: Arc<ApiRuntimeServices>,
        runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    ) -> Self {
        Self {
            services,
            runtime_activity: Some(runtime_activity),
            network_egress: None,
        }
    }

    pub fn with_network_egress(
        mut self,
        network_egress: Arc<NetworkEgressHttpClientResolver>,
    ) -> Self {
        self.network_egress = Some(network_egress);
        self
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

    pub async fn acquire_network_egress_http_forward_proxy(
        &self,
        provider_id: Uuid,
        installation: &domain::LocalPluginInstallationRecord,
        secret: NetworkEgressSecretMaterial,
        provider_egress_key: &str,
    ) -> anyhow::Result<ForwardProxyLease> {
        if installation.contract_version != plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT {
            return Err(ControlPlaneError::InvalidInput("plugin_installation").into());
        }
        self.ensure_network_egress_loaded(provider_id, installation, secret)
            .await?;
        let lease = self
            .services
            .runtime_backend
            .network_egress_resolve_http_forward_proxy(
                &provider_id.to_string(),
                provider_egress_key,
            )
            .await
            .map_err(map_runtime_backend_error)?;
        info!(
            provider_id = %provider_id,
            installation_id = %installation.id,
            plugin_id = %installation.plugin_id,
            plugin_version = %installation.plugin_version,
            "network egress provider resolved active artifact"
        );
        Ok(lease)
    }

    async fn validate_network_egress_provider_artifact(
        &self,
        provider_id: Uuid,
        installation: &domain::LocalPluginInstallationRecord,
        secret: NetworkEgressSecretMaterial,
    ) -> anyhow::Result<()> {
        if installation.contract_version != plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT {
            return Err(ControlPlaneError::InvalidInput("plugin_installation").into());
        }
        self.services
            .runtime_backend
            .network_egress_preflight(runtime_network_egress_activation(
                provider_id,
                installation,
                secret,
            )?)
            .await
            .map_err(map_runtime_backend_error)
    }

    pub async fn release_network_egress_http_forward_proxy(
        &self,
        provider_id: Uuid,
        lease_id: &str,
    ) -> anyhow::Result<()> {
        self.services
            .runtime_backend
            .network_egress_release_http_forward_proxy(&provider_id.to_string(), lease_id)
            .await
            .map_err(map_runtime_backend_error)
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
    async fn select_provider_distribution(
        &self,
        plugin_id: &str,
        invocation: extension_contracts::ProviderDistributionInvocation,
        context: ProviderRuntimeExecutionContext,
    ) -> anyhow::Result<extension_contracts::ProviderDistributionSelectionReceipt> {
        self.services
            .orchestration_backend
            .select_provider_distribution(
                runtime_core::runtime_backend::RuntimeProviderDistributionRequest {
                    request_id: RuntimeRequestId::new(Uuid::now_v7().to_string())?,
                    target: RuntimeTargetId::new(plugin_id.to_string())?,
                    invocation,
                    principal: runtime_core::runtime_backend::RuntimeExecutionPrincipal {
                        workspace_id: context.workspace_id.to_string(),
                        actor_id: context.actor_id.map(|id| id.to_string()),
                        deadline_unix_ms: context.deadline_unix_ms,
                    },
                },
            )
            .await
            .map_err(map_runtime_backend_error)
    }

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
            // Network egress workers receive their private configuration only through `sync`.
            // Installation activation is intentionally deferred so no worker can start without it.
            plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT => Ok(()),
            extension_contracts::PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1 => self
                .services
                .runtime_backend
                .activate_provider_distribution_rule(runtime_package_activation(
                    installation,
                    None,
                )?)
                .await
                .map_err(map_runtime_backend_error),
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
                .runtime_backend
                .deactivate_provider(&installation.plugin_id)
                .await
                .map_err(map_runtime_backend_error),
            "1flowbase.data_source/v1" => {
                self.services
                    .runtime_backend
                    .deactivate_data_source(&installation.plugin_id)
                    .await
                    .map_err(map_runtime_backend_error)?;
                Ok(())
            }
            "1flowbase.capability/v1" => {
                self.services
                    .runtime_backend
                    .deactivate_capability(&installation.plugin_id)
                    .await
                    .map_err(map_runtime_backend_error)?;
                Ok(())
            }
            // Network egress workers are provider-instance scoped. Artifact deactivation does not
            // identify an instance and therefore cannot own its worker lifecycle.
            plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT => Ok(()),
            extension_contracts::PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1 => self
                .services
                .runtime_backend
                .deactivate_provider_distribution_rule(&installation.plugin_id)
                .await
                .map_err(map_runtime_backend_error),
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
        self.services
            .runtime_backend
            .provider_validate(&binding.plugin_id, provider_config)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn authenticate_provider(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
        operation: ProviderAuthOperation,
    ) -> anyhow::Result<ProviderAuthResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_authenticate(&binding.plugin_id, provider_config, operation)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn list_models(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<Vec<ProviderModelDescriptor>> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_list_models(&binding.plugin_id, provider_config)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn get_balance(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderBalanceResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_get_balance(&binding.plugin_id, provider_config)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn get_usage_windows(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
    ) -> anyhow::Result<ProviderUsageWindowsResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_get_usage_windows(&binding.plugin_id, provider_config)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn reset_credit(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: Value,
        reset_credit_operation: ProviderResetCreditOperation,
    ) -> anyhow::Result<ProviderResetCreditResult> {
        let binding = self.resolve_model_provider_binding(installation)?;
        self.ensure_provider_loaded(&binding).await?;
        self.services
            .runtime_backend
            .provider_reset_credit(&binding.plugin_id, provider_config, reset_credit_operation)
            .await
            .map_err(map_runtime_backend_error)
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
            self.services
                .runtime_backend
                .provider_count_tokens(&binding.plugin_id, input)
                .await
                .map_err(map_runtime_backend_error)
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
        let result = self
            .services
            .runtime_backend
            .provider_compact(&binding.plugin_id, input)
            .await
            .map_err(map_runtime_backend_error);
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
            self.services
                .orchestration_backend
                .execute(runtime_execution_request(&binding, input, None)?)
                .await
                .map(|output| ProviderRuntimeInvocationOutput {
                    events: output.events,
                    result: output.result,
                })
                .map_err(map_runtime_backend_error)
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
            let sinks = live_events
                .map(|senders| RuntimeStreamSinks {
                    required: Some(Arc::new(RuntimeEventChannelSink(senders.required))),
                    diagnostic: Some(Arc::new(RuntimeEventChannelSink(senders.diagnostic))),
                })
                .unwrap_or_default();
            self.services
                .orchestration_backend
                .execute_stream(runtime_execution_request(&binding, input, None)?, sinks)
                .await
                .map(|output| ProviderRuntimeInvocationOutput {
                    events: output.events,
                    result: output.result,
                })
                .map_err(map_runtime_backend_error)
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

    async fn invoke_stream_with_network_egress(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        mut input: ProviderInvocationInput,
        live_events: Option<ProviderLiveEventSenders>,
        workspace_id: Uuid,
        selector: domain::NetworkEgressConsumerSelector,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let Some(network_egress) = self.network_egress.as_ref() else {
            return self
                .invoke_stream_with_live_events(installation, input, live_events)
                .await;
        };
        let Some(scope) = network_egress.acquire(workspace_id, selector).await? else {
            return self
                .invoke_stream_with_live_events(installation, input, live_events)
                .await;
        };

        input.set_network_egress_context(scope.provider_invocation_context());
        let invocation = self
            .invoke_stream_with_live_events(installation, input, live_events)
            .await;
        let release = scope.release().await;
        match (invocation, release) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(output), Ok(())) => Ok(output),
        }
    }

    async fn invoke_stream_with_execution_context(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        mut input: ProviderInvocationInput,
        live_events: Option<ProviderLiveEventSenders>,
        context: control_plane::ports::ProviderRuntimeExecutionContext,
        selector: domain::NetworkEgressConsumerSelector,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let binding = self.resolve_model_provider_binding(installation)?;
        binding.require_provider_code(&input.provider_code)?;
        let activity = self.start_runtime_activity(ApplicationActivityKind::ModelRequest);
        self.ensure_provider_loaded(&binding).await?;
        let scope = match self.network_egress.as_ref() {
            Some(network_egress) => {
                network_egress
                    .acquire(context.workspace_id, selector)
                    .await?
            }
            None => None,
        };
        if let Some(scope) = &scope {
            input.set_network_egress_context(scope.provider_invocation_context());
        }
        let sinks = live_events
            .map(|senders| RuntimeStreamSinks {
                required: Some(Arc::new(RuntimeEventChannelSink(senders.required))),
                diagnostic: Some(Arc::new(RuntimeEventChannelSink(senders.diagnostic))),
            })
            .unwrap_or_default();
        let principal = runtime_core::runtime_backend::RuntimeExecutionPrincipal {
            workspace_id: context.workspace_id.to_string(),
            actor_id: context.actor_id.map(|value| value.to_string()),
            deadline_unix_ms: context.deadline_unix_ms,
        };
        let invocation = self
            .services
            .orchestration_backend
            .execute_stream(
                runtime_execution_request(&binding, input, Some(principal))?,
                sinks,
            )
            .await
            .map(|output| ProviderRuntimeInvocationOutput {
                events: output.events,
                result: output.result,
            })
            .map_err(map_runtime_backend_error);
        let release = match scope {
            Some(scope) => scope.release().await,
            None => Ok(()),
        };
        let result = match (invocation, release) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(output), Ok(())) => Ok(output),
        };
        finish_runtime_activity(activity, &result);
        result
    }

    async fn acquire_http_node_client(
        &self,
        workspace_id: Uuid,
        timeout: std::time::Duration,
        verify_ssl: bool,
    ) -> anyhow::Result<Option<orchestration_runtime::execution_engine::HttpRequestClientLease>>
    {
        let Some(network_egress) = self.network_egress.as_ref() else {
            return Ok(None);
        };
        let Some(scope) = network_egress
            .acquire(
                workspace_id,
                domain::NetworkEgressConsumerSelector::HttpNodeDefault,
            )
            .await?
        else {
            return Ok(None);
        };
        scope
            .into_http_request_client_lease(timeout, verify_ssl)
            .await
            .map(Some)
    }
}

#[async_trait]
impl NetworkEgressRuntimePort for ApiProviderRuntime {
    async fn unload_network_egress_provider(&self, provider_id: Uuid) -> anyhow::Result<()> {
        self.services
            .runtime_backend
            .network_egress_deactivate(&provider_id.to_string())
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn preflight_network_egresses(
        &self,
        provider_id: Uuid,
        installation: &domain::LocalPluginInstallationRecord,
        secret: NetworkEgressSecretMaterial,
    ) -> anyhow::Result<()> {
        self.validate_network_egress_provider_artifact(provider_id, installation, secret)
            .await
    }

    async fn sync_network_egresses(
        &self,
        provider_id: Uuid,
        installation: &domain::LocalPluginInstallationRecord,
        secret: NetworkEgressSecretMaterial,
    ) -> anyhow::Result<Vec<plugin_framework::EgressDescriptor>> {
        if installation.contract_version != plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT {
            return Err(ControlPlaneError::InvalidInput("plugin_installation").into());
        }
        self.ensure_network_egress_loaded(provider_id, installation, secret)
            .await?;
        self.services
            .runtime_backend
            .network_egress_sync(&provider_id.to_string())
            .await
            .map_err(map_runtime_backend_error)
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

#[async_trait]
impl CapabilityPluginRuntimePort for ApiProviderRuntime {
    async fn validate_config(&self, input: ValidateCapabilityConfigInput) -> anyhow::Result<Value> {
        self.ensure_capability_loaded(&input.installation).await?;
        self.services
            .runtime_backend
            .capability_validate(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
            )
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn resolve_dynamic_options(
        &self,
        input: ResolveCapabilityOptionsInput,
    ) -> anyhow::Result<Value> {
        self.ensure_capability_loaded(&input.installation).await?;
        self.services
            .runtime_backend
            .capability_resolve_dynamic_options(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
            )
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn resolve_output_schema(
        &self,
        input: ResolveCapabilityOutputSchemaInput,
    ) -> anyhow::Result<Value> {
        self.ensure_capability_loaded(&input.installation).await?;
        self.services
            .runtime_backend
            .capability_resolve_output_schema(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
            )
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn execute_node(
        &self,
        input: ExecuteCapabilityNodeInput,
    ) -> anyhow::Result<CapabilityExecutionOutput> {
        let activity = self.start_runtime_activity(ApplicationActivityKind::ToolCall);
        self.ensure_capability_loaded(&input.installation).await?;
        let result = self
            .services
            .runtime_backend
            .capability_execute(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
                input.input_payload,
            )
            .await
            .map(|output| CapabilityExecutionOutput {
                output_payload: output.output_payload,
                granted_credit_permissions: output.granted_credit_permissions,
            })
            .map_err(map_runtime_backend_error);
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
        let result = self
            .services
            .runtime_backend
            .activate_provider(RuntimePackageActivation {
                plugin_id: binding.plugin_id.clone(),
                artifact: RuntimeArtifactReference::new(binding.artifact_reference())?,
                source_identity: Some(binding.source_identity.clone()),
                legacy_eligibility: binding
                    .legacy_manifest_eligibility()
                    .map(runtime_legacy_eligibility),
            })
            .await
            .map_err(map_runtime_backend_error);
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
        self.services
            .runtime_backend
            .activate_capability(runtime_package_activation(installation, None)?)
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn ensure_data_source_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()> {
        let templates = self
            .services
            .runtime_backend
            .activate_data_source(runtime_package_activation(installation, None)?)
            .await
            .map_err(map_runtime_backend_error)?;
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

    async fn ensure_network_egress_loaded(
        &self,
        provider_id: Uuid,
        installation: &domain::LocalPluginInstallationRecord,
        secret: NetworkEgressSecretMaterial,
    ) -> anyhow::Result<()> {
        let source_identity = format!(
            "installation_id={};checksum={};manifest_fingerprint={};updated_at={}",
            installation.id,
            installation.expected_checksum.as_deref().unwrap_or(""),
            installation
                .artifact
                .manifest_fingerprint
                .as_deref()
                .unwrap_or(""),
            installation.updated_at.unix_timestamp_nanos()
        );
        self.services
            .runtime_backend
            .network_egress_activate(RuntimeNetworkEgressActivation {
                runtime_id: provider_id.to_string(),
                plugin_id: installation.plugin_id.clone(),
                artifact: runtime_artifact_reference(installation)?,
                source_identity,
                secret_json: secret.secret_json,
            })
            .await
            .map_err(map_runtime_backend_error)
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

fn runtime_legacy_eligibility(
    eligibility: &plugin_framework::LegacyInstalledManifestEligibility,
) -> RuntimeLegacyManifestEligibility {
    RuntimeLegacyManifestEligibility {
        expected_publisher_namespace: eligibility.expected_publisher_namespace.clone(),
        expected_versioned_plugin_id: eligibility.expected_versioned_plugin_id.clone(),
        expected_raw_manifest_fingerprint: eligibility.expected_raw_manifest_fingerprint.clone(),
    }
}

fn runtime_artifact_reference(
    installation: &domain::LocalPluginInstallationRecord,
) -> anyhow::Result<RuntimeArtifactReference> {
    #[cfg(test)]
    {
        return RuntimeArtifactReference::new(
            installation
                .local_path()
                .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?,
        )
        .map_err(Into::into);
    }
    #[cfg(not(test))]
    {
        RuntimeArtifactReference::new(installation.id.to_string()).map_err(Into::into)
    }
}

fn runtime_package_activation(
    installation: &domain::LocalPluginInstallationRecord,
    source_identity: Option<String>,
) -> anyhow::Result<RuntimePackageActivation> {
    Ok(RuntimePackageActivation {
        plugin_id: installation.plugin_id.clone(),
        artifact: runtime_artifact_reference(installation)?,
        source_identity,
        legacy_eligibility: legacy_manifest_eligibility(installation)?
            .as_ref()
            .map(runtime_legacy_eligibility),
    })
}

fn runtime_network_egress_activation(
    provider_id: Uuid,
    installation: &domain::LocalPluginInstallationRecord,
    secret: NetworkEgressSecretMaterial,
) -> anyhow::Result<RuntimeNetworkEgressActivation> {
    Ok(RuntimeNetworkEgressActivation {
        runtime_id: provider_id.to_string(),
        plugin_id: installation.plugin_id.clone(),
        artifact: runtime_artifact_reference(installation)?,
        source_identity: format!(
            "installation_id={};checksum={};manifest_fingerprint={};updated_at={}",
            installation.id,
            installation.expected_checksum.as_deref().unwrap_or(""),
            installation
                .artifact
                .manifest_fingerprint
                .as_deref()
                .unwrap_or(""),
            installation.updated_at.unix_timestamp_nanos()
        ),
        secret_json: secret.secret_json,
    })
}

fn runtime_execution_request(
    binding: &ModelProviderSlotBinding,
    input: ProviderInvocationInput,
    principal: Option<runtime_core::runtime_backend::RuntimeExecutionPrincipal>,
) -> anyhow::Result<RuntimeExecutionRequest> {
    Ok(RuntimeExecutionRequest {
        request_id: RuntimeRequestId::new(Uuid::now_v7().to_string())?,
        target: RuntimeTargetId::new(binding.plugin_id.clone())?,
        input,
        principal,
    })
}

fn map_runtime_backend_error(error: RuntimeBackendError) -> anyhow::Error {
    match error {
        RuntimeBackendError::Contract(error) => map_provider_framework_error(error),
        RuntimeBackendError::CountTokens(error) => anyhow::Error::new(error),
        RuntimeBackendError::Compact(error) => anyhow::Error::new(error),
        RuntimeBackendError::InvalidRequest(_) => {
            ControlPlaneError::InvalidInput("provider_runtime").into()
        }
        RuntimeBackendError::DuplicateRequest(_)
        | RuntimeBackendError::DuplicateBackend
        | RuntimeBackendError::MissingBackend
        | RuntimeBackendError::Unavailable(_)
        | RuntimeBackendError::Cancelled(_)
        | RuntimeBackendError::UnsupportedOperation(_)
        | RuntimeBackendError::Execution { .. } => {
            ControlPlaneError::UpstreamUnavailable("provider_runtime").into()
        }
    }
}

fn map_data_source_runtime_error(error: RuntimeBackendError) -> anyhow::Error {
    match error {
        RuntimeBackendError::Contract(error) => map_framework_error(error, "data_source_runtime"),
        RuntimeBackendError::InvalidRequest(_) => {
            ControlPlaneError::InvalidInput("data_source_runtime").into()
        }
        RuntimeBackendError::CountTokens(_)
        | RuntimeBackendError::Compact(_)
        | RuntimeBackendError::DuplicateRequest(_)
        | RuntimeBackendError::DuplicateBackend
        | RuntimeBackendError::MissingBackend
        | RuntimeBackendError::Unavailable(_)
        | RuntimeBackendError::Cancelled(_)
        | RuntimeBackendError::UnsupportedOperation(_)
        | RuntimeBackendError::Execution { .. } => {
            ControlPlaneError::UpstreamUnavailable("data_source_runtime").into()
        }
    }
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
