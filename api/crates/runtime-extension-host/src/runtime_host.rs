use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock as StdRwLock},
};

use async_trait::async_trait;
use extension_contracts::{
    PluginDataBinding, PluginDataError, PluginDataErrorKind, PluginDataPort, PluginDataRequest,
};
use runtime_core::runtime_backend::{
    CapabilityRuntimePort, DataSourceRuntimePort, NetworkEgressRuntimePort, ProviderRuntimePort,
    RuntimeArtifactReference, RuntimeBackendError, RuntimeBackendLifecycle, RuntimeBackendSnapshot,
    RuntimeCancelOutcome, RuntimeCapabilityExecutionOutcome, RuntimeExecutionOutcome,
    RuntimeExecutionPort, RuntimeExecutionRequest, RuntimeLegacyManifestEligibility,
    RuntimeManagedActivation, RuntimeManagedCapabilityRequest, RuntimeNetworkEgressActivation,
    RuntimeObservationPort, RuntimePackageActivation, RuntimeProviderDistributionRequest,
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

#[derive(Debug)]
struct MissingPluginDataPort;

impl PluginDataPort for MissingPluginDataPort {
    fn execute<'a>(
        &'a self,
        _binding: &'a PluginDataBinding,
        _request: &'a PluginDataRequest,
    ) -> extension_contracts::PluginDataFuture<'a> {
        Box::pin(async {
            Err(PluginDataError {
                kind: PluginDataErrorKind::StorageUnavailable,
                code: "plugin_data_port_not_configured".to_string(),
                retryable: false,
            })
        })
    }
}
use runtime_profile::{RuntimeProfile, RuntimeProfileCollector, RuntimeSampleSource};
use time::OffsetDateTime;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::{
    capability_host::CapabilityHost, data_source_host::DataSourceHost,
    network_egress_host::NetworkEgressHost, provider_distribution_host::ProviderDistributionHost,
    provider_host::ProviderHost,
};

#[derive(Clone)]
pub struct RuntimeExtensionHost {
    provider_host: Arc<RwLock<ProviderHost>>,
    capability_host: Arc<RwLock<CapabilityHost>>,
    managed_workers: Arc<RwLock<crate::managed_worker::ManagedWorkers>>,
    data_source_host: Arc<RwLock<DataSourceHost>>,
    network_egress_host: Arc<RwLock<NetworkEgressHost>>,
    provider_distribution_host: Arc<RwLock<ProviderDistributionHost>>,
    profile: Arc<RuntimeProfileCollector>,
    lifecycle: Arc<StdRwLock<RuntimeBackendLifecycle>>,
    active_requests: Arc<Mutex<HashMap<RuntimeRequestId, Arc<execution::ActiveRuntimeRequest>>>>,
    artifact_resolver: Arc<dyn RuntimeArtifactResolver>,
    plugin_data: Arc<dyn PluginDataPort>,
    pub(crate) file_read: Option<Arc<dyn runtime_core::runtime_backend::FileReadPort>>,
    pub(crate) application_logs:
        Option<Arc<dyn runtime_core::runtime_backend::ApplicationLogsPort>>,
    pub(crate) http_egress: Option<Arc<dyn runtime_core::runtime_backend::HttpEgressPort>>,
    pub(crate) file_import: Option<Arc<dyn runtime_core::runtime_backend::FileImportPort>>,
    pub(crate) plugin_credentials: Option<Arc<dyn extension_contracts::PluginCredentialPort>>,
    managed_hash_budget: Arc<tokio::sync::Semaphore>,
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
            Arc::new(MissingPluginDataPort),
            Arc::new(RuntimeSampleSource::default()),
        )
    }

    pub fn new_with_artifact_resolver_and_plugin_data(
        process_started_at: OffsetDateTime,
        artifact_resolver: Arc<dyn RuntimeArtifactResolver>,
        plugin_data: Arc<dyn PluginDataPort>,
    ) -> Result<Self, RuntimeBackendError> {
        Self::new_with_artifact_resolver_plugin_data_and_profile_source(
            process_started_at,
            artifact_resolver,
            plugin_data,
            Arc::new(RuntimeSampleSource::default()),
        )
    }

    pub fn new_with_artifact_resolver_plugin_data_and_profile_source(
        process_started_at: OffsetDateTime,
        artifact_resolver: Arc<dyn RuntimeArtifactResolver>,
        plugin_data: Arc<dyn PluginDataPort>,
        sample_source: Arc<RuntimeSampleSource>,
    ) -> Result<Self, RuntimeBackendError> {
        Self::from_shared_registries_with_artifact_resolver(
            process_started_at,
            Arc::new(RwLock::new(ProviderHost::default())),
            Arc::new(RwLock::new(CapabilityHost::default())),
            Arc::new(RwLock::new(DataSourceHost::default())),
            artifact_resolver,
            plugin_data,
            sample_source,
        )
    }

    fn from_shared_registries_with_artifact_resolver(
        process_started_at: OffsetDateTime,
        provider_host: Arc<RwLock<ProviderHost>>,
        capability_host: Arc<RwLock<CapabilityHost>>,
        data_source_host: Arc<RwLock<DataSourceHost>>,
        artifact_resolver: Arc<dyn RuntimeArtifactResolver>,
        plugin_data: Arc<dyn PluginDataPort>,
        sample_source: Arc<RuntimeSampleSource>,
    ) -> Result<Self, RuntimeBackendError> {
        let profile = RuntimeProfileCollector::new_with_sample_source(
            "runtime-extension-host",
            env!("CARGO_PKG_VERSION"),
            process_started_at,
            "ok",
            sample_source,
        )
        .map_err(|error| RuntimeBackendError::Execution {
            target_id: "runtime-extension-host".to_string(),
            message: error.to_string(),
        })?;
        Ok(Self {
            provider_host,
            capability_host,
            managed_workers: Arc::new(
                RwLock::new(crate::managed_worker::ManagedWorkers::default()),
            ),
            data_source_host,
            network_egress_host: Arc::new(RwLock::new(NetworkEgressHost::default())),
            provider_distribution_host: Arc::new(RwLock::new(ProviderDistributionHost::default())),
            profile: Arc::new(profile),
            lifecycle: Arc::new(StdRwLock::new(RuntimeBackendLifecycle::Starting)),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
            artifact_resolver,
            plugin_data,
            file_read: None,
            application_logs: None,
            http_egress: None,
            file_import: None,
            plugin_credentials: None,
            managed_hash_budget: Arc::new(tokio::sync::Semaphore::new(4)),
        })
    }

    /// Composition-root injection before activation. The port must enforce live authority.
    /// Inject the trusted resource replay/file upload capability at composition time.
    pub fn with_file_read(
        mut self,
        port: Arc<dyn runtime_core::runtime_backend::FileReadPort>,
    ) -> Self {
        self.file_read = Some(port);
        self
    }

    pub fn with_application_logs(
        mut self,
        port: Arc<dyn runtime_core::runtime_backend::ApplicationLogsPort>,
    ) -> Self {
        self.application_logs = Some(port);
        self
    }
    pub fn with_http_egress(
        mut self,
        port: Arc<dyn runtime_core::runtime_backend::HttpEgressPort>,
    ) -> Self {
        self.http_egress = Some(port);
        self
    }
    pub fn with_file_import(
        mut self,
        port: Arc<dyn runtime_core::runtime_backend::FileImportPort>,
    ) -> Self {
        self.file_import = Some(port);
        self
    }

    pub fn with_plugin_credentials(
        mut self,
        port: Arc<dyn extension_contracts::PluginCredentialPort>,
    ) -> Self {
        self.plugin_credentials = Some(port);
        self
    }

    /// Composition-root policy; configure before activation or admission.
    pub async fn configure_provider_worker_idle_grace(
        &self,
        grace: std::time::Duration,
    ) -> Result<(), RuntimeBackendError> {
        if self.lifecycle() != RuntimeBackendLifecycle::Starting {
            return Err(RuntimeBackendError::InvalidRequest(
                "worker policy must be configured before runtime readiness".into(),
            ));
        }
        let mut host = self.provider_host.write().await;
        if host.loaded_count() != 0 {
            return Err(RuntimeBackendError::InvalidRequest(
                "worker policy must be configured before package activation".into(),
            ));
        }
        *host = ProviderHost::with_worker_idle_grace(grace);
        Ok(())
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

        self.managed_workers
            .read()
            .await
            .close_admission()
            .map_err(RuntimeBackendError::from)?;
        let handles = self
            .active_requests
            .lock()
            .await
            .iter()
            .map(|(id, owner)| (id.clone(), owner.clone()))
            .collect::<Vec<_>>();
        for (_, owner) in &handles {
            owner.cancel();
        }
        for (id, owner) in handles {
            owner.join(&id).await?;
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), RuntimeBackendError> {
        // Per-call cancellation must preserve shared peers. Global stop owns all peers
        // and must still reach their quiesce owners when graceful draining cannot join.
        if let Err(error) = self.drain().await {
            tracing::warn!(%error, "runtime graceful drain incomplete; continuing global cleanup");
        }
        let budget = std::time::Duration::from_secs(5);
        let _hashes = tokio::time::timeout(
            budget,
            self.managed_hash_budget.clone().acquire_many_owned(4),
        )
        .await
        .map_err(|_| {
            RuntimeBackendError::InvalidRequest("managed executable hash tasks unfinished".into())
        })?
        .map_err(|_| {
            RuntimeBackendError::InvalidRequest("managed executable hash gate unavailable".into())
        })?;
        let managed_scopes = self.managed_workers.read().await.scopes();
        let service_pools = self.managed_workers.read().await.service_pools();
        tokio::time::timeout(budget, async {
            for pool in &service_pools {
                pool.retire_all().await?;
            }
            for scope in &managed_scopes {
                scope.dispose().await?;
            }
            Ok::<_, extension_package_runtime::PluginFrameworkError>(())
        })
        .await
        .map_err(|_| {
            RuntimeBackendError::InvalidRequest("managed worker scopes unfinished".into())
        })??;
        self.managed_workers.write().await.clear_disposed();
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
        // Registry shutdown releases busy carriers. Confirm their request/forwarder
        // owners too; a successful global stop must not leave a detached producer.
        let requests = self
            .active_requests
            .lock()
            .await
            .iter()
            .map(|(id, owner)| (id.clone(), owner.clone()))
            .collect::<Vec<_>>();
        let mut joins = tokio::task::JoinSet::new();
        for (id, owner) in requests {
            joins.spawn(async move { owner.join(&id).await });
        }
        while let Some(joined) = joins.join_next().await {
            match joined {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    first_error.get_or_insert(error);
                }
                Err(error) => {
                    first_error.get_or_insert_with(|| {
                        RuntimeBackendError::InvalidRequest(format!(
                            "runtime stop join owner failed: {error}"
                        ))
                    });
                }
            }
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

    async fn reconcile_provider_worker_demand(
        &self,
        plugin_id: &str,
        revision: u64,
        selectable: Option<bool>,
    ) -> Result<(), RuntimeBackendError> {
        self.ensure_accepting()?;
        self.provider_host
            .read()
            .await
            .reconcile_worker_demand(plugin_id, revision, selectable)
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_transport_worker_exit_evidence(
        &self,
        target_id: &str,
        logical_session_id: &str,
        generation: u64,
    ) -> Result<Option<extension_contracts::ProviderTransportClosureEvidence>, RuntimeBackendError>
    {
        self.ensure_accepting()?;
        self.provider_host
            .read()
            .await
            .transport_worker_exit_evidence(target_id, logical_session_id, generation)
            .await
            .map_err(RuntimeBackendError::from)
    }

    async fn provider_transport_session(
        &self,
        target_id: &str,
        command: extension_contracts::ProviderTransportSessionCommand,
    ) -> Result<extension_contracts::ProviderTransportSessionReceipt, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .provider_host
            .read()
            .await
            .transport_session_operation(target_id, command)
            .map_err(RuntimeBackendError::from)?;
        operation.await.map_err(RuntimeBackendError::from)
    }
}

impl RuntimeExtensionHost {
    fn admit_managed_hash(&self) -> Result<tokio::sync::OwnedSemaphorePermit, RuntimeBackendError> {
        let lifecycle = self.lifecycle.read().map_err(|_| {
            RuntimeBackendError::InvalidRequest("runtime lifecycle lock is poisoned".into())
        })?;
        if !matches!(
            *lifecycle,
            RuntimeBackendLifecycle::Starting | RuntimeBackendLifecycle::Ready
        ) {
            return Err(RuntimeBackendError::Unavailable(*lifecycle));
        }
        self.managed_hash_budget
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                RuntimeBackendError::InvalidRequest(
                    "managed executable hash capacity exhausted".into(),
                )
            })
    }
    async fn verify_managed_executable(
        &self,
        handle: &extension_contracts::ManagedExecutionHandle,
    ) -> Result<(), RuntimeBackendError> {
        let hash_permit = self.admit_managed_hash()?;
        let binding = self
            .managed_workers
            .read()
            .await
            .executable_for_handle(handle)
            .map_err(RuntimeBackendError::from)?;
        tokio::task::spawn_blocking(move || {
            let _permit = hash_permit;
            binding.verify_executable()
        })
        .await
        .map_err(|_| {
            RuntimeBackendError::from(
                extension_package_runtime::PluginFrameworkError::invalid_provider_package(
                    "managed executable validation unavailable",
                ),
            )
        })?
        .map_err(RuntimeBackendError::from)
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
    async fn activate_provider_distribution_rule(
        &self,
        request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError> {
        self.ensure_activating()?;
        let package_root = self.artifact_resolver.resolve(&request.artifact).await?;
        self.provider_distribution_host
            .write()
            .await
            .load(&package_root.to_string_lossy(), &request.plugin_id)
            .map_err(RuntimeBackendError::from)
    }

    async fn deactivate_provider_distribution_rule(
        &self,
        plugin_id: &str,
    ) -> Result<(), RuntimeBackendError> {
        self.provider_distribution_host
            .write()
            .await
            .unload(plugin_id)
            .await
            .map_err(RuntimeBackendError::from)
    }

    async fn select_provider_distribution(
        &self,
        request: RuntimeProviderDistributionRequest,
    ) -> Result<extension_contracts::ProviderDistributionSelectionReceipt, RuntimeBackendError>
    {
        self.ensure_accepting()?;
        self.provider_distribution_host
            .read()
            .await
            .select(
                request.target.as_str(),
                request.invocation,
                request.principal,
                Arc::clone(&self.plugin_data),
            )
            .await
            .map_err(RuntimeBackendError::from)
    }

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
        self.execute_owned(request, sinks).await
    }

    async fn cancel(
        &self,
        request_id: &RuntimeRequestId,
    ) -> Result<RuntimeCancelOutcome, RuntimeBackendError> {
        let Some(owner) = self.active_requests.lock().await.get(request_id).cloned() else {
            return Ok(RuntimeCancelOutcome::NotFound);
        };
        owner.cancel();
        owner.join(request_id).await?;
        Ok(RuntimeCancelOutcome::Cancelled)
    }
}

#[async_trait]
impl RuntimeObservationPort for RuntimeExtensionHost {
    async fn snapshot(&self) -> Result<RuntimeBackendSnapshot, RuntimeBackendError> {
        let providers = self.provider_host.read().await.loaded_count();
        let capabilities = self.capability_host.read().await.loaded_count();
        let capabilities = capabilities + self.managed_workers.read().await.loaded_count();
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

#[cfg(test)]
#[path = "_tests/managed_hash_budget.rs"]
mod managed_hash_budget_tests;

#[cfg(test)]
#[path = "_tests/shared_profile_source.rs"]
mod shared_profile_source_tests;

mod data_source;
mod execution;

#[path = "runtime_host/capability_port.rs"]
mod capability_port;
