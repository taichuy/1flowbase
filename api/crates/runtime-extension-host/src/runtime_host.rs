use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock as StdRwLock},
};

use async_trait::async_trait;
use runtime_core::runtime_backend::{
    CapabilityRuntimePort, DataSourceRuntimePort, NetworkEgressRuntimePort, ProviderRuntimePort,
    RuntimeArtifactReference, RuntimeBackendError, RuntimeBackendLifecycle, RuntimeBackendSnapshot,
    RuntimeCancelOutcome, RuntimeCapabilityExecutionOutcome, RuntimeExecutionOutcome,
    RuntimeExecutionPort, RuntimeExecutionRequest, RuntimeLegacyManifestEligibility,
    RuntimeNetworkEgressActivation, RuntimeObservationPort, RuntimePackageActivation,
    RuntimeRegistrySnapshot, RuntimeRequestId, RuntimeStreamEventSink, RuntimeStreamSinks,
};

#[async_trait]
pub trait RuntimeArtifactResolver: Send + Sync {
    async fn resolve(
        &self,
        artifact: &RuntimeArtifactReference,
    ) -> Result<PathBuf, RuntimeBackendError>;
}

#[derive(Debug)]
struct MissingRuntimeArtifactResolver;

#[async_trait]
impl RuntimeArtifactResolver for MissingRuntimeArtifactResolver {
    async fn resolve(
        &self,
        _artifact: &RuntimeArtifactReference,
    ) -> Result<PathBuf, RuntimeBackendError> {
        Err(RuntimeBackendError::Execution {
            target_id: "runtime-artifact".to_string(),
            message: "runtime artifact resolver is not configured".to_string(),
        })
    }
}
use runtime_profile::{RuntimeProfile, RuntimeProfileCollector};
use time::OffsetDateTime;
use tokio::{
    sync::{mpsc, Mutex, RwLock},
    task::AbortHandle,
};

use crate::{
    capability_host::CapabilityHost, data_source_host::DataSourceHost,
    network_egress_host::NetworkEgressHost, provider_host::ProviderHost,
};

#[derive(Clone)]
pub struct RuntimeExtensionHost {
    provider_host: Arc<RwLock<ProviderHost>>,
    capability_host: Arc<RwLock<CapabilityHost>>,
    data_source_host: Arc<RwLock<DataSourceHost>>,
    network_egress_host: Arc<RwLock<NetworkEgressHost>>,
    profile: Arc<RuntimeProfileCollector>,
    lifecycle: Arc<StdRwLock<RuntimeBackendLifecycle>>,
    active_requests: Arc<Mutex<HashMap<RuntimeRequestId, AbortHandle>>>,
    artifact_resolver: Arc<dyn RuntimeArtifactResolver>,
}

impl std::fmt::Debug for RuntimeExtensionHost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeExtensionHost")
            .field("lifecycle", &self.lifecycle())
            .finish_non_exhaustive()
    }
}

impl RuntimeExtensionHost {
    pub fn new(process_started_at: OffsetDateTime) -> Result<Self, RuntimeBackendError> {
        Self::new_with_artifact_resolver(
            process_started_at,
            Arc::new(MissingRuntimeArtifactResolver),
        )
    }

    pub fn new_with_artifact_resolver(
        process_started_at: OffsetDateTime,
        artifact_resolver: Arc<dyn RuntimeArtifactResolver>,
    ) -> Result<Self, RuntimeBackendError> {
        Self::from_shared_registries_with_artifact_resolver(
            process_started_at,
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
            artifact_resolver,
        )
    }

    pub fn from_registries(
        process_started_at: OffsetDateTime,
        provider_host: ProviderHost,
        capability_host: CapabilityHost,
        data_source_host: DataSourceHost,
    ) -> Result<Self, RuntimeBackendError> {
        Self::from_shared_registries(
            process_started_at,
            Arc::new(RwLock::new(provider_host)),
            Arc::new(RwLock::new(capability_host)),
            Arc::new(RwLock::new(data_source_host)),
        )
    }

    pub fn from_shared_registries(
        process_started_at: OffsetDateTime,
        provider_host: Arc<RwLock<ProviderHost>>,
        capability_host: Arc<RwLock<CapabilityHost>>,
        data_source_host: Arc<RwLock<DataSourceHost>>,
    ) -> Result<Self, RuntimeBackendError> {
        Self::from_shared_registries_with_artifact_resolver(
            process_started_at,
            provider_host,
            capability_host,
            data_source_host,
            Arc::new(MissingRuntimeArtifactResolver),
        )
    }

    pub fn from_shared_registries_with_artifact_resolver(
        process_started_at: OffsetDateTime,
        provider_host: Arc<RwLock<ProviderHost>>,
        capability_host: Arc<RwLock<CapabilityHost>>,
        data_source_host: Arc<RwLock<DataSourceHost>>,
        artifact_resolver: Arc<dyn RuntimeArtifactResolver>,
    ) -> Result<Self, RuntimeBackendError> {
        let profile = RuntimeProfileCollector::new(
            "runtime-extension-host",
            env!("CARGO_PKG_VERSION"),
            process_started_at,
            "ok",
        )
        .map_err(|error| RuntimeBackendError::Execution {
            target_id: "runtime-extension-host".to_string(),
            message: error.to_string(),
        })?;
        Ok(Self {
            provider_host,
            capability_host,
            data_source_host,
            network_egress_host: Arc::new(RwLock::new(NetworkEgressHost::default())),
            profile: Arc::new(profile),
            lifecycle: Arc::new(StdRwLock::new(RuntimeBackendLifecycle::Starting)),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
            artifact_resolver,
        })
    }

    pub fn mark_ready(&self) -> Result<(), RuntimeBackendError> {
        let mut lifecycle = self.lifecycle.write().map_err(|_| {
            RuntimeBackendError::InvalidRequest("runtime lifecycle lock is poisoned".to_string())
        })?;
        match *lifecycle {
            RuntimeBackendLifecycle::Starting | RuntimeBackendLifecycle::Ready => {
                *lifecycle = RuntimeBackendLifecycle::Ready;
                Ok(())
            }
            state => Err(RuntimeBackendError::Unavailable(state)),
        }
    }

    pub fn lifecycle(&self) -> RuntimeBackendLifecycle {
        self.lifecycle
            .read()
            .map(|state| *state)
            .unwrap_or(RuntimeBackendLifecycle::Failed)
    }

    pub fn collect_runtime_profile(&self) -> Result<RuntimeProfile, RuntimeBackendError> {
        self.profile
            .collect()
            .map_err(|error| RuntimeBackendError::Execution {
                target_id: "runtime-extension-host".to_string(),
                message: error.to_string(),
            })
    }

    pub async fn drain(&self) -> Result<(), RuntimeBackendError> {
        {
            let mut lifecycle = self.lifecycle.write().map_err(|_| {
                RuntimeBackendError::InvalidRequest(
                    "runtime lifecycle lock is poisoned".to_string(),
                )
            })?;
            match *lifecycle {
                RuntimeBackendLifecycle::Starting | RuntimeBackendLifecycle::Ready => {
                    *lifecycle = RuntimeBackendLifecycle::Draining;
                }
                RuntimeBackendLifecycle::Draining | RuntimeBackendLifecycle::Stopped => {}
                state => return Err(RuntimeBackendError::Unavailable(state)),
            }
        }

        let handles = self
            .active_requests
            .lock()
            .await
            .drain()
            .map(|(_, handle)| handle)
            .collect::<Vec<_>>();
        for handle in handles {
            handle.abort();
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), RuntimeBackendError> {
        self.drain().await?;
        let mut first_error = None;
        if let Err(error) = self.provider_host.write().await.stop_all().await {
            first_error = Some(RuntimeBackendError::from(error));
        }
        if let Err(error) = self.data_source_host.write().await.stop_all().await {
            first_error.get_or_insert_with(|| RuntimeBackendError::from(error));
        }
        if let Err(error) = self.capability_host.write().await.stop_all().await {
            first_error.get_or_insert_with(|| RuntimeBackendError::from(error));
        }
        if let Err(error) = self.network_egress_host.write().await.stop_all().await {
            first_error.get_or_insert_with(|| RuntimeBackendError::from(error));
        }
        let mut lifecycle = self.lifecycle.write().map_err(|_| {
            RuntimeBackendError::InvalidRequest("runtime lifecycle lock is poisoned".to_string())
        })?;
        if let Some(error) = first_error {
            *lifecycle = RuntimeBackendLifecycle::Failed;
            Err(error)
        } else {
            *lifecycle = RuntimeBackendLifecycle::Stopped;
            Ok(())
        }
    }

    fn ensure_accepting(&self) -> Result<(), RuntimeBackendError> {
        match self.lifecycle() {
            RuntimeBackendLifecycle::Ready => Ok(()),
            state => Err(RuntimeBackendError::Unavailable(state)),
        }
    }

    fn ensure_activating(&self) -> Result<(), RuntimeBackendError> {
        match self.lifecycle() {
            RuntimeBackendLifecycle::Starting | RuntimeBackendLifecycle::Ready => Ok(()),
            state => Err(RuntimeBackendError::Unavailable(state)),
        }
    }
}

fn legacy_eligibility(
    value: RuntimeLegacyManifestEligibility,
) -> extension_package_runtime::LegacyInstalledManifestEligibility {
    extension_package_runtime::LegacyInstalledManifestEligibility {
        expected_publisher_namespace: value.expected_publisher_namespace,
        expected_versioned_plugin_id: value.expected_versioned_plugin_id,
        expected_raw_manifest_fingerprint: value.expected_raw_manifest_fingerprint,
    }
}

#[async_trait]
impl ProviderRuntimePort for RuntimeExtensionHost {
    async fn activate_provider(
        &self,
        request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError> {
        self.ensure_activating()?;
        let package_root = self.artifact_resolver.resolve(&request.artifact).await?;
        let package_root = package_root.to_string_lossy();
        let mut host = self.provider_host.write().await;
        match request.legacy_eligibility {
            Some(eligibility) => host.load_legacy_installed_if_needed(
                &request.plugin_id,
                &package_root,
                request.source_identity.as_deref(),
                &legacy_eligibility(eligibility),
            ),
            None => host.load_if_needed(
                &request.plugin_id,
                &package_root,
                request.source_identity.as_deref(),
            ),
        }
        .map_err(RuntimeBackendError::from)
    }

    async fn deactivate_provider(&self, plugin_id: &str) -> Result<(), RuntimeBackendError> {
        self.provider_host
            .write()
            .await
            .unload(plugin_id)
            .await
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_validate(
        &self,
        target_id: &str,
        provider_config: serde_json::Value,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .provider_host
            .read()
            .await
            .validate_operation(target_id, provider_config)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|output| output.output)
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_authenticate(
        &self,
        target_id: &str,
        provider_config: serde_json::Value,
        operation: extension_contracts::ProviderAuthOperation,
    ) -> Result<extension_contracts::ProviderAuthResult, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .provider_host
            .read()
            .await
            .authenticate_operation(target_id, provider_config, operation)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|output| output.result)
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_list_models(
        &self,
        target_id: &str,
        provider_config: serde_json::Value,
    ) -> Result<Vec<extension_contracts::ProviderModelDescriptor>, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .provider_host
            .read()
            .await
            .list_models_operation(target_id, provider_config)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|output| output.models)
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_get_balance(
        &self,
        target_id: &str,
        provider_config: serde_json::Value,
    ) -> Result<extension_contracts::ProviderBalanceResult, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .provider_host
            .read()
            .await
            .get_balance_operation(target_id, provider_config)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|output| output.balance)
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_get_usage_windows(
        &self,
        target_id: &str,
        provider_config: serde_json::Value,
    ) -> Result<extension_contracts::ProviderUsageWindowsResult, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .provider_host
            .read()
            .await
            .get_usage_windows_operation(target_id, provider_config)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|output| output.usage)
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_reset_credit(
        &self,
        target_id: &str,
        provider_config: serde_json::Value,
        operation: extension_contracts::ProviderResetCreditOperation,
    ) -> Result<extension_contracts::ProviderResetCreditResult, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .provider_host
            .read()
            .await
            .reset_credit_operation(target_id, provider_config, operation)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|output| output.result)
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_count_tokens(
        &self,
        target_id: &str,
        input: extension_contracts::ProviderCountTokensInput,
    ) -> Result<extension_contracts::ProviderCountTokensResult, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .provider_host
            .read()
            .await
            .count_tokens_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|output| output.result)
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_compact(
        &self,
        target_id: &str,
        input: extension_contracts::ProviderInvocationInput,
    ) -> Result<extension_contracts::ProviderCompactResult, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .provider_host
            .read()
            .await
            .compact_operation(target_id, input)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|output| output.result)
            .map_err(RuntimeBackendError::from)
    }
}

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

#[async_trait]
impl CapabilityRuntimePort for RuntimeExtensionHost {
    async fn activate_capability(
        &self,
        request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError> {
        self.ensure_activating()?;
        let package_root = self.artifact_resolver.resolve(&request.artifact).await?;
        let mut host = self.capability_host.write().await;
        if host.is_loaded(&request.plugin_id) {
            return Ok(());
        }
        match request.legacy_eligibility {
            Some(eligibility) => {
                host.load_legacy_installed(&package_root, &legacy_eligibility(eligibility))
                    .await
            }
            None => host.load(&package_root).await,
        }
        .map(|_| ())
        .map_err(RuntimeBackendError::from)
    }

    async fn deactivate_capability(&self, plugin_id: &str) -> Result<(), RuntimeBackendError> {
        self.capability_host
            .write()
            .await
            .unload(plugin_id)
            .await
            .map_err(RuntimeBackendError::from)
    }

    async fn capability_validate(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: serde_json::Value,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .capability_host
            .read()
            .await
            .validate_config_operation(target_id, contribution_code, config_payload)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.output)
            .map_err(Into::into)
    }

    async fn capability_resolve_dynamic_options(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: serde_json::Value,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .capability_host
            .read()
            .await
            .resolve_dynamic_options_operation(target_id, contribution_code, config_payload)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.output)
            .map_err(Into::into)
    }

    async fn capability_resolve_output_schema(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: serde_json::Value,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .capability_host
            .read()
            .await
            .resolve_output_schema_operation(target_id, contribution_code, config_payload)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.output)
            .map_err(Into::into)
    }

    async fn capability_execute(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: serde_json::Value,
        input_payload: serde_json::Value,
    ) -> Result<RuntimeCapabilityExecutionOutcome, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .capability_host
            .read()
            .await
            .execute_operation(target_id, contribution_code, config_payload, input_payload)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| RuntimeCapabilityExecutionOutcome {
                output_payload: value.output_payload,
                granted_credit_permissions: value.granted_credit_permissions,
            })
            .map_err(Into::into)
    }
}

#[async_trait]
impl NetworkEgressRuntimePort for RuntimeExtensionHost {
    async fn network_egress_preflight(
        &self,
        request: RuntimeNetworkEgressActivation,
    ) -> Result<(), RuntimeBackendError> {
        self.ensure_activating()?;
        let package_root = self.artifact_resolver.resolve(&request.artifact).await?;
        let package_root = package_root.to_string_lossy();
        crate::network_egress_host::NetworkEgressHost::preflight(
            &request.runtime_id,
            &request.plugin_id,
            &package_root,
            crate::network_egress_host::NetworkEgressWorkerConfig::from_secret_json(
                request.secret_json,
            ),
        )
        .await
        .map(|_| ())
        .map_err(Into::into)
    }

    async fn network_egress_activate(
        &self,
        request: RuntimeNetworkEgressActivation,
    ) -> Result<(), RuntimeBackendError> {
        self.ensure_activating()?;
        let package_root = self.artifact_resolver.resolve(&request.artifact).await?;
        let package_root = package_root.to_string_lossy();
        self.network_egress_host
            .write()
            .await
            .load_if_needed(
                &request.runtime_id,
                &request.plugin_id,
                &package_root,
                &request.source_identity,
                crate::network_egress_host::NetworkEgressWorkerConfig::from_secret_json(
                    request.secret_json,
                ),
            )
            .await
            .map_err(Into::into)
    }

    async fn network_egress_sync(
        &self,
        runtime_id: &str,
    ) -> Result<Vec<extension_contracts::EgressDescriptor>, RuntimeBackendError> {
        self.ensure_accepting()?;
        self.network_egress_host
            .write()
            .await
            .sync_egresses(runtime_id)
            .await
            .map_err(Into::into)
    }

    async fn network_egress_resolve_http_forward_proxy(
        &self,
        runtime_id: &str,
        egress_key: &str,
    ) -> Result<extension_contracts::ForwardProxyLease, RuntimeBackendError> {
        self.ensure_accepting()?;
        self.network_egress_host
            .write()
            .await
            .resolve_http_forward_proxy(runtime_id, egress_key)
            .await
            .map_err(Into::into)
    }

    async fn network_egress_release_http_forward_proxy(
        &self,
        runtime_id: &str,
        lease_id: &str,
    ) -> Result<(), RuntimeBackendError> {
        self.ensure_accepting()?;
        self.network_egress_host
            .write()
            .await
            .release_http_forward_proxy(runtime_id, lease_id)
            .await
            .map_err(Into::into)
    }

    async fn network_egress_deactivate(&self, runtime_id: &str) -> Result<(), RuntimeBackendError> {
        self.network_egress_host
            .write()
            .await
            .unload(runtime_id)
            .await
            .map_err(Into::into)
    }
}

async fn forward_events(
    mut receiver: mpsc::Receiver<extension_contracts::provider_contract::ProviderStreamEvent>,
    sink: Arc<dyn RuntimeStreamEventSink>,
) -> Result<(), RuntimeBackendError> {
    while let Some(event) = receiver.recv().await {
        sink.emit(event).await?;
    }
    Ok(())
}

#[async_trait]
impl RuntimeExecutionPort for RuntimeExtensionHost {
    async fn execute(
        &self,
        request: RuntimeExecutionRequest,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
        self.execute_stream(request, RuntimeStreamSinks::default())
            .await
    }

    async fn execute_stream(
        &self,
        request: RuntimeExecutionRequest,
        sinks: RuntimeStreamSinks,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
        let request_id = request.request_id.clone();
        let target_id = request.target.as_str().to_string();
        let provider_host = Arc::clone(&self.provider_host);

        let mut active = self.active_requests.lock().await;
        // Admission and registration share the same critical section that drain() acquires after
        // transitioning the lifecycle. A request is therefore either registered for drain to
        // cancel, or observes Draining and is rejected; it cannot slip between those outcomes.
        self.ensure_accepting()?;
        if active.contains_key(&request_id) {
            return Err(RuntimeBackendError::DuplicateRequest(request_id));
        }

        let task = tokio::spawn(async move {
            let (required_sender, required_forwarder) = match sinks.required {
                Some(sink) => {
                    let (sender, receiver) = mpsc::channel(64);
                    (
                        Some(sender),
                        Some(tokio::spawn(forward_events(receiver, sink))),
                    )
                }
                None => (None, None),
            };
            let (diagnostic_sender, diagnostic_forwarder) = match sinks.diagnostic {
                Some(sink) => {
                    let (sender, receiver) = mpsc::channel(64);
                    (
                        Some(sender),
                        Some(tokio::spawn(forward_events(receiver, sink))),
                    )
                }
                None => (None, None),
            };
            let operation = {
                let host = provider_host.read().await;
                host.invoke_stream_with_live_events_operation(
                    &target_id,
                    request.input,
                    required_sender,
                    diagnostic_sender,
                )
                .map_err(RuntimeBackendError::from)?
            };
            let output = operation.await.map_err(RuntimeBackendError::from)?;
            if let Some(forwarder) = required_forwarder {
                forwarder
                    .await
                    .map_err(|error| RuntimeBackendError::Execution {
                        target_id: target_id.clone(),
                        message: error.to_string(),
                    })??;
            }
            if let Some(forwarder) = diagnostic_forwarder {
                forwarder
                    .await
                    .map_err(|error| RuntimeBackendError::Execution {
                        target_id: target_id.clone(),
                        message: error.to_string(),
                    })??;
            }
            Ok(RuntimeExecutionOutcome {
                events: output.events,
                result: output.result,
            })
        });

        active.insert(request_id.clone(), task.abort_handle());
        drop(active);
        let result = task.await;
        self.active_requests.lock().await.remove(&request_id);
        match result {
            Ok(result) => result,
            Err(error) if error.is_cancelled() => Err(RuntimeBackendError::Cancelled(request_id)),
            Err(error) => Err(RuntimeBackendError::Execution {
                target_id: request.target.as_str().to_string(),
                message: error.to_string(),
            }),
        }
    }

    async fn cancel(
        &self,
        request_id: &RuntimeRequestId,
    ) -> Result<RuntimeCancelOutcome, RuntimeBackendError> {
        let Some(handle) = self.active_requests.lock().await.remove(request_id) else {
            return Ok(RuntimeCancelOutcome::NotFound);
        };
        handle.abort();
        Ok(RuntimeCancelOutcome::Cancelled)
    }
}

#[async_trait]
impl RuntimeObservationPort for RuntimeExtensionHost {
    async fn snapshot(&self) -> Result<RuntimeBackendSnapshot, RuntimeBackendError> {
        let providers = self.provider_host.read().await.loaded_count();
        let capabilities = self.capability_host.read().await.loaded_count();
        let data_sources = self.data_source_host.read().await.loaded_count();
        let network_egress_providers = self.network_egress_host.read().await.loaded_count();
        let mut active_request_ids = self
            .active_requests
            .lock()
            .await
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        active_request_ids.sort();
        Ok(RuntimeBackendSnapshot {
            backend_kind: "in_process".to_string(),
            lifecycle: self.lifecycle(),
            registries: RuntimeRegistrySnapshot {
                providers,
                data_sources,
                capabilities,
                network_egress_providers,
            },
            active_request_ids,
        })
    }
}
