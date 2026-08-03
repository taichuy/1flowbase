use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex as StdMutex,
    },
};

use plugin_framework::{
    error::{FrameworkResult, PluginFrameworkError},
    manifest_v1::PluginExecutionMode,
    provider_contract::{
        ModelDiscoveryMode, ProviderBalanceResult, ProviderCompactError, ProviderCompactResult,
        ProviderCountTokensError, ProviderCountTokensFallbackReason, ProviderCountTokensInput,
        ProviderCountTokensResult, ProviderGenerateTranslationReceipt, ProviderInvocationInput,
        ProviderInvocationResult, ProviderModelDescriptor, ProviderRuntimeError,
        ProviderRuntimeErrorKind, ProviderStdioMethod, ProviderStdioRequest, ProviderStreamEvent,
        ProviderWireOperation, CURRENT_PROVIDER_CONTRACT,
    },
    provider_count_tokens_estimator::estimate_provider_count_tokens,
    PluginRuntimeLimits,
};
use serde::Serialize;
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

#[cfg(test)]
use plugin_framework::provider_contract::ProviderCountTokensMethod;

use crate::package_loader::{LoadedProviderPackage, PackageLoader};
use crate::stdio_runtime::{
    call_executable, call_executable_streaming, ProviderWorker,
    DEFAULT_PROVIDER_INVOCATION_TIMEOUT_MS,
};

type ProviderWorkerHandle = Arc<Mutex<ProviderWorker>>;
type ProviderWorkerRegistry = Arc<StdMutex<HashMap<String, ProviderWorkerHandle>>>;
type ProviderLiveEvents = Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>;

#[derive(Debug, Clone, Serialize)]
pub struct LoadedProviderSummary {
    pub plugin_id: String,
    pub provider_code: String,
    pub plugin_version: String,
    pub protocol: String,
    pub model_discovery_mode: ModelDiscoveryMode,
}

impl LoadedProviderSummary {
    fn from_loaded(loaded: &LoadedProviderPackage) -> Self {
        Self {
            plugin_id: loaded.package.identifier(),
            provider_code: loaded.package.provider.provider_code.clone(),
            plugin_version: loaded.package.manifest.version.clone(),
            protocol: loaded.package.provider.protocol.clone(),
            model_discovery_mode: loaded.package.provider.model_discovery_mode,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedProviderSource {
    package_root: PathBuf,
    source_identity: Option<String>,
}

impl LoadedProviderSource {
    fn resolve(package_root: &str, source_identity: Option<&str>) -> FrameworkResult<Self> {
        let package_root = fs::canonicalize(package_root).map_err(|error| {
            PluginFrameworkError::invalid_provider_package(format!(
                "cannot resolve package root: {error}"
            ))
        })?;
        Ok(Self {
            package_root,
            source_identity: source_identity.map(ToOwned::to_owned),
        })
    }

    fn can_skip_reload(&self, requested: &Self) -> bool {
        self.source_identity.is_some() && self == requested
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderValidationOutput {
    pub output: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderModelsOutput {
    pub models: Vec<ProviderModelDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderBalanceOutput {
    pub balance: ProviderBalanceResult,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderCountTokensOutput {
    pub result: ProviderCountTokensResult,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProviderCompactOutput {
    pub result: ProviderCompactResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInvokeStreamOutput {
    pub events: Vec<ProviderStreamEvent>,
    pub result: ProviderInvocationResult,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderActiveStreamsOutput {
    pub streams: Vec<ProviderActiveStreamSnapshot>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderActiveStreamSnapshot {
    pub invocation_id: String,
    pub plugin_id: String,
    pub provider_instance_id: String,
    pub provider_code: String,
    pub protocol: String,
    pub model: String,
    pub transport: String,
    pub status: String,
    pub started_at: String,
    pub last_event_at: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
struct ActiveProviderStreamRecord {
    invocation_id: String,
    plugin_id: String,
    provider_instance_id: String,
    provider_code: String,
    protocol: String,
    model: String,
    transport: String,
    status: String,
    started_at: OffsetDateTime,
    last_event_at: OffsetDateTime,
}

impl ActiveProviderStreamRecord {
    fn new(invocation_id: String, plugin_id: &str, input: &ProviderInvocationInput) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            invocation_id,
            plugin_id: plugin_id.to_string(),
            provider_instance_id: input.provider_instance_id.clone(),
            provider_code: input.provider_code.clone(),
            protocol: input.protocol.clone(),
            model: input.model.clone(),
            transport: provider_stream_transport(input),
            status: "running".to_string(),
            started_at: now,
            last_event_at: now,
        }
    }

    fn snapshot(&self, now: OffsetDateTime) -> ProviderActiveStreamSnapshot {
        ProviderActiveStreamSnapshot {
            invocation_id: self.invocation_id.clone(),
            plugin_id: self.plugin_id.clone(),
            provider_instance_id: self.provider_instance_id.clone(),
            provider_code: self.provider_code.clone(),
            protocol: self.protocol.clone(),
            model: self.model.clone(),
            transport: self.transport.clone(),
            status: self.status.clone(),
            started_at: format_timestamp(self.started_at),
            last_event_at: format_timestamp(self.last_event_at),
            duration_ms: elapsed_milliseconds(self.started_at, now),
        }
    }
}

#[derive(Debug)]
struct ActiveProviderInvocationLease {
    provider_pool_key: String,
    _permit: OwnedSemaphorePermit,
}

struct PreparedProviderStreamInvocation {
    loaded: LoadedProviderPackage,
    provider_workers: ProviderWorkerRegistry,
    active_streams: Arc<Mutex<HashMap<String, ActiveProviderStreamRecord>>>,
    active_invocation_leases: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    invocation_id: String,
    plugin_id: String,
    input: ProviderInvocationInput,
    required_live_events: ProviderLiveEvents,
    diagnostic_live_events: ProviderLiveEvents,
}

impl Drop for ActiveProviderInvocationLease {
    fn drop(&mut self) {
        tracing::debug!(
            provider_pool_key = %self.provider_pool_key,
            "active provider invocation lease released"
        );
    }
}

#[derive(Debug)]
pub struct ProviderHost {
    loaded_packages: HashMap<String, LoadedProviderPackage>,
    loaded_sources: HashMap<String, LoadedProviderSource>,
    provider_workers: ProviderWorkerRegistry,
    active_streams: Arc<Mutex<HashMap<String, ActiveProviderStreamRecord>>>,
    active_invocation_leases: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    next_invocation_sequence: AtomicU64,
}

impl Default for ProviderHost {
    fn default() -> Self {
        Self {
            loaded_packages: HashMap::new(),
            loaded_sources: HashMap::new(),
            provider_workers: Arc::new(StdMutex::new(HashMap::new())),
            active_streams: Arc::new(Mutex::new(HashMap::new())),
            active_invocation_leases: Arc::new(Mutex::new(HashMap::new())),
            next_invocation_sequence: AtomicU64::new(1),
        }
    }
}

impl ProviderHost {
    pub fn load(&mut self, package_root: &str) -> FrameworkResult<LoadedProviderSummary> {
        self.load_with_source_identity(package_root, None)
    }

    fn load_with_source_identity(
        &mut self,
        package_root: &str,
        source_identity: Option<&str>,
    ) -> FrameworkResult<LoadedProviderSummary> {
        let source = LoadedProviderSource::resolve(package_root, source_identity)?;
        self.load_source(source, None)
    }

    fn load_source(
        &mut self,
        source: LoadedProviderSource,
        expected_plugin_id: Option<&str>,
    ) -> FrameworkResult<LoadedProviderSummary> {
        let loaded = PackageLoader::load(&source.package_root)?;
        let summary = LoadedProviderSummary::from_loaded(&loaded);
        if let Some(expected_plugin_id) = expected_plugin_id {
            if summary.plugin_id != expected_plugin_id {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "loaded provider package id {} does not match requested {expected_plugin_id}",
                    summary.plugin_id
                )));
            }
        }
        self.loaded_packages
            .insert(summary.plugin_id.clone(), loaded);
        self.loaded_sources
            .insert(summary.plugin_id.clone(), source);
        self.remove_provider_worker(&summary.plugin_id)?;
        Ok(summary)
    }

    pub fn is_loaded(&self, plugin_id: &str) -> bool {
        self.loaded_packages.contains_key(plugin_id)
    }

    pub fn load_if_needed(
        &mut self,
        plugin_id: &str,
        package_root: &str,
        source_identity: Option<&str>,
    ) -> FrameworkResult<()> {
        let requested_source = LoadedProviderSource::resolve(package_root, source_identity)?;
        if self
            .loaded_sources
            .get(plugin_id)
            .is_some_and(|loaded_source| loaded_source.can_skip_reload(&requested_source))
        {
            return Ok(());
        }
        self.load_source(requested_source, Some(plugin_id))
            .map(|_| ())
    }

    pub fn reload(&mut self, plugin_id: &str) -> FrameworkResult<LoadedProviderSummary> {
        let source = match self.loaded_sources.get(plugin_id).cloned() {
            Some(source) => source,
            None => {
                let package_root = self
                    .loaded_packages
                    .get(plugin_id)
                    .ok_or_else(|| {
                        PluginFrameworkError::invalid_provider_package(format!(
                            "provider package is not loaded: {plugin_id}"
                        ))
                    })?
                    .package_root
                    .clone();
                LoadedProviderSource {
                    package_root,
                    source_identity: None,
                }
            }
        };
        if !self.loaded_packages.contains_key(plugin_id) {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "provider package is not loaded: {plugin_id}"
            )));
        }
        self.load_source(source, Some(plugin_id))
    }

    pub async fn validate(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<ProviderValidationOutput> {
        self.validate_operation(plugin_id, provider_config)?.await
    }

    pub fn validate_operation(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderValidationOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?.clone();
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let output = Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Validate,
                provider_config,
            )
            .await?;
            Ok(ProviderValidationOutput { output })
        })
    }

    pub async fn list_models(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<ProviderModelsOutput> {
        self.list_models_operation(plugin_id, provider_config)?
            .await
    }

    pub fn list_models_operation(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderModelsOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?.clone();
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let models = match loaded.package.provider.model_discovery_mode {
                ModelDiscoveryMode::Static => loaded.package.predefined_models.clone(),
                ModelDiscoveryMode::Dynamic => {
                    let dynamic = Self::call_runtime_loaded(
                        loaded,
                        provider_workers,
                        ProviderStdioMethod::ListModels,
                        provider_config,
                    )
                    .await?;
                    normalize_models(dynamic)?
                }
                ModelDiscoveryMode::Hybrid => {
                    let predefined_models = loaded.package.predefined_models.clone();
                    let dynamic = Self::call_runtime_loaded(
                        loaded,
                        provider_workers,
                        ProviderStdioMethod::ListModels,
                        provider_config,
                    )
                    .await?;
                    merge_models(&predefined_models, normalize_models(dynamic)?)
                }
            };
            Ok(ProviderModelsOutput { models })
        })
    }

    pub async fn get_balance(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<ProviderBalanceOutput> {
        self.get_balance_operation(plugin_id, provider_config)?
            .await
    }

    pub fn get_balance_operation(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderBalanceOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?.clone();
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let raw_balance = Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Balance,
                provider_config,
            )
            .await?;
            Ok(ProviderBalanceOutput {
                balance: normalize_balance(raw_balance)?,
            })
        })
    }

    pub async fn count_tokens(
        &self,
        plugin_id: &str,
        input: ProviderCountTokensInput,
    ) -> Result<ProviderCountTokensOutput, ProviderCountTokensError> {
        self.count_tokens_operation(plugin_id, input)?.await
    }

    // Provider errors intentionally preserve complete typed upstream diagnostics.
    #[allow(clippy::result_large_err)]
    pub fn count_tokens_operation(
        &self,
        plugin_id: &str,
        input: ProviderCountTokensInput,
    ) -> Result<
        impl std::future::Future<
                Output = Result<ProviderCountTokensOutput, ProviderCountTokensError>,
            > + Send
            + 'static,
        ProviderCountTokensError,
    > {
        let loaded = self.loaded_package(plugin_id).ok().cloned();
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let Some(loaded) = loaded else {
                return Ok(ProviderCountTokensOutput {
                    result: generic_count_tokens_fallback(
                        &input,
                        ProviderCountTokensFallbackReason::PluginUnavailable,
                    ),
                });
            };
            if loaded.package.manifest.contract_version != CURRENT_PROVIDER_CONTRACT {
                return Ok(ProviderCountTokensOutput {
                    result: generic_count_tokens_fallback(
                        &input,
                        ProviderCountTokensFallbackReason::PluginUnavailable,
                    ),
                });
            }
            let wire_input = match current_provider_count_tokens_wire_input(&loaded, &input) {
                Ok(wire_input) => wire_input,
                Err(ProviderCountTokensError::Unsupported { .. }) => {
                    return Ok(ProviderCountTokensOutput {
                        result: generic_count_tokens_fallback(
                            &input,
                            ProviderCountTokensFallbackReason::CapabilityUnavailable,
                        ),
                    });
                }
                Err(error) => return Err(error),
            };
            tracing::info!(
                provider_code = %input.provider_code,
                model = %input.model,
                "provider count tokens wire prepared"
            );
            let output = match Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Invoke,
                wire_input,
            )
            .await
            {
                Ok(output) => output,
                Err(_) => {
                    return Ok(ProviderCountTokensOutput {
                        result: generic_count_tokens_fallback(
                            &input,
                            ProviderCountTokensFallbackReason::ProviderRuntimeFailure,
                        ),
                    });
                }
            };
            let result = match serde_json::from_value::<ProviderCountTokensResult>(output) {
                Ok(result) => result,
                Err(_) => {
                    return Ok(ProviderCountTokensOutput {
                        result: generic_count_tokens_fallback(
                            &input,
                            ProviderCountTokensFallbackReason::MalformedProviderResult,
                        ),
                    });
                }
            };
            if result.operation != ProviderWireOperation::CountTokens {
                return Ok(ProviderCountTokensOutput {
                    result: generic_count_tokens_fallback(
                        &input,
                        ProviderCountTokensFallbackReason::MalformedProviderResult,
                    ),
                });
            }
            Ok(ProviderCountTokensOutput { result })
        })
    }

    pub async fn compact(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
    ) -> Result<ProviderCompactOutput, ProviderCompactError> {
        self.compact_operation(plugin_id, input)?.await
    }

    // Provider errors intentionally preserve complete typed upstream diagnostics.
    #[allow(clippy::result_large_err)]
    pub fn compact_operation(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
    ) -> Result<
        impl std::future::Future<Output = Result<ProviderCompactOutput, ProviderCompactError>>
            + Send
            + 'static,
        ProviderCompactError,
    > {
        let expected_profile = input
            .compact_profile()
            .map_err(|message| ProviderCompactError::InvalidContract { message })?;
        let loaded = self
            .loaded_package(plugin_id)
            .map_err(compact_framework_error)?
            .clone();
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let wire_input = current_provider_compact_wire_input(&loaded, &input)?;
            tracing::info!(
                provider_code = %input.provider_code,
                model = %input.model,
                profile = %expected_profile.as_str(),
                "provider compact wire prepared"
            );
            let output = Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Invoke,
                wire_input,
            )
            .await
            .map_err(compact_framework_error)?;
            let result =
                serde_json::from_value::<ProviderCompactResult>(output).map_err(|error| {
                    ProviderCompactError::Runtime {
                        error: ProviderRuntimeError::new(
                            ProviderRuntimeErrorKind::ProviderInvalidResponse,
                            format!("provider Compact result is malformed: {error}"),
                        ),
                    }
                })?;
            if !result.satisfies_profile(expected_profile) {
                return Err(ProviderCompactError::Runtime {
                    error: ProviderRuntimeError::new(
                        ProviderRuntimeErrorKind::ProviderInvalidResponse,
                        format!(
                            "provider Compact result must declare operation=compact with profile={}",
                            expected_profile.as_str()
                        ),
                    ),
                });
            }
            Ok(ProviderCompactOutput { result })
        })
    }

    pub async fn invoke_stream(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
    ) -> FrameworkResult<ProviderInvokeStreamOutput> {
        self.invoke_stream_operation(plugin_id, input)?.await
    }

    pub fn invoke_stream_operation(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderInvokeStreamOutput>> + Send + 'static,
    > {
        self.invoke_stream_with_live_events_operation(plugin_id, input, None, None)
    }

    pub async fn invoke_stream_with_live_events(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
        required_live_events: ProviderLiveEvents,
        diagnostic_live_events: ProviderLiveEvents,
    ) -> FrameworkResult<ProviderInvokeStreamOutput> {
        self.invoke_stream_with_live_events_operation(
            plugin_id,
            input,
            required_live_events,
            diagnostic_live_events,
        )?
        .await
    }

    pub fn invoke_stream_with_live_events_operation(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
        required_live_events: ProviderLiveEvents,
        diagnostic_live_events: ProviderLiveEvents,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderInvokeStreamOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?.clone();
        let provider_workers = Arc::clone(&self.provider_workers);
        let active_streams = Arc::clone(&self.active_streams);
        let active_invocation_leases = Arc::clone(&self.active_invocation_leases);
        let sequence = self
            .next_invocation_sequence
            .fetch_add(1, Ordering::Relaxed);
        let invocation_id = format!("{plugin_id}:{sequence}");
        let plugin_id = plugin_id.to_string();
        Ok(async move {
            Self::invoke_stream_prepared(PreparedProviderStreamInvocation {
                loaded,
                provider_workers,
                active_streams,
                active_invocation_leases,
                invocation_id,
                plugin_id,
                input,
                required_live_events,
                diagnostic_live_events,
            })
            .await
        })
    }

    pub async fn active_stream_snapshot(&self) -> ProviderActiveStreamsOutput {
        let now = OffsetDateTime::now_utc();
        let mut streams = self
            .active_streams
            .lock()
            .await
            .values()
            .map(|record| record.snapshot(now))
            .collect::<Vec<_>>();
        streams.sort_by(|left, right| left.started_at.cmp(&right.started_at));
        ProviderActiveStreamsOutput { streams }
    }

    async fn register_active_stream(
        active_streams: &Arc<Mutex<HashMap<String, ActiveProviderStreamRecord>>>,
        invocation_id: String,
        plugin_id: &str,
        input: &ProviderInvocationInput,
    ) {
        let record = ActiveProviderStreamRecord::new(invocation_id.clone(), plugin_id, input);
        active_streams.lock().await.insert(invocation_id, record);
    }

    fn active_stream_event_observer(
        active_streams: Arc<Mutex<HashMap<String, ActiveProviderStreamRecord>>>,
        invocation_id: String,
    ) -> tokio::sync::mpsc::UnboundedSender<()> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while receiver.recv().await.is_some() {
                if let Some(record) = active_streams.lock().await.get_mut(&invocation_id) {
                    record.last_event_at = OffsetDateTime::now_utc();
                }
            }
        });
        sender
    }

    async fn remove_active_stream(
        active_streams: &Arc<Mutex<HashMap<String, ActiveProviderStreamRecord>>>,
        invocation_id: &str,
    ) {
        active_streams.lock().await.remove(invocation_id);
    }

    async fn acquire_active_invocation_lease(
        active_invocation_leases: &Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
        input: &ProviderInvocationInput,
    ) -> FrameworkResult<ActiveProviderInvocationLease> {
        let provider_pool_key = provider_pool_key(input);
        let semaphore = {
            let mut leases = active_invocation_leases.lock().await;
            leases
                .entry(provider_pool_key.clone())
                .or_insert_with(|| Arc::new(Semaphore::new(1)))
                .clone()
        };
        tracing::debug!(
            provider_pool_key = %provider_pool_key,
            "active provider invocation lease acquiring"
        );
        let permit = semaphore.acquire_owned().await.map_err(|_| {
            PluginFrameworkError::runtime(
                plugin_framework::provider_contract::ProviderRuntimeError::normalize(
                    "provider_invocation_lease",
                    "active provider invocation lease is closed",
                    None,
                ),
            )
        })?;
        tracing::debug!(
            provider_pool_key = %provider_pool_key,
            "active provider invocation lease acquired"
        );
        Ok(ActiveProviderInvocationLease {
            provider_pool_key,
            _permit: permit,
        })
    }

    fn remove_provider_worker(&mut self, plugin_id: &str) -> FrameworkResult<()> {
        let mut workers = lock_provider_worker_registry(&self.provider_workers)?;
        workers.remove(plugin_id);
        Ok(())
    }

    fn loaded_package(&self, plugin_id: &str) -> FrameworkResult<&LoadedProviderPackage> {
        self.loaded_packages.get(plugin_id).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "provider package is not loaded: {plugin_id}"
            ))
        })
    }

    async fn call_runtime_loaded(
        loaded: LoadedProviderPackage,
        provider_workers: ProviderWorkerRegistry,
        method: ProviderStdioMethod,
        input: Value,
    ) -> FrameworkResult<Value> {
        let request = ProviderStdioRequest { method, input };
        match loaded.package.manifest.execution_mode {
            PluginExecutionMode::ProcessPerCall => {
                call_executable(
                    &loaded.runtime_executable,
                    &request,
                    &loaded.package.manifest.runtime.limits,
                )
                .await
            }
            PluginExecutionMode::StatefulProviderWorker => {
                let plugin_id = loaded.package.identifier();
                let worker = provider_worker_handle(&provider_workers, plugin_id, &loaded)?;
                let mut worker = worker.lock().await;
                worker.call(&request).await
            }
            _ => Err(PluginFrameworkError::invalid_provider_package(
                "model provider package declares unsupported execution_mode",
            )),
        }
    }

    async fn invoke_stream_prepared(
        invocation: PreparedProviderStreamInvocation,
    ) -> FrameworkResult<ProviderInvokeStreamOutput> {
        let PreparedProviderStreamInvocation {
            loaded,
            provider_workers,
            active_streams,
            active_invocation_leases,
            invocation_id,
            plugin_id,
            input,
            required_live_events,
            diagnostic_live_events,
        } = invocation;

        let prepared_wire = current_provider_wire_input(&loaded, &input)?;
        tracing::info!(
            wire_audit = ?input.wire_audit(),
            "provider generate wire prepared"
        );
        let _lease =
            Self::acquire_active_invocation_lease(&active_invocation_leases, &input).await?;
        Self::register_active_stream(&active_streams, invocation_id.clone(), &plugin_id, &input)
            .await;
        let event_observer = Some(Self::active_stream_event_observer(
            Arc::clone(&active_streams),
            invocation_id.clone(),
        ));
        let request = ProviderStdioRequest {
            method: ProviderStdioMethod::Invoke,
            input: prepared_wire.wire_value,
        };
        let invocation_limits = provider_invocation_limits(&loaded.package.manifest.runtime.limits);
        let output = match loaded.package.manifest.execution_mode {
            PluginExecutionMode::ProcessPerCall => {
                call_executable_streaming(
                    &loaded.runtime_executable,
                    &request,
                    &invocation_limits,
                    required_live_events,
                    diagnostic_live_events,
                    event_observer,
                )
                .await
            }
            PluginExecutionMode::StatefulProviderWorker => {
                let worker = provider_worker_handle(&provider_workers, plugin_id, &loaded)?;
                let mut worker = worker.lock().await;
                worker
                    .call_streaming_with_limits(
                        &request,
                        &invocation_limits,
                        required_live_events,
                        diagnostic_live_events,
                        event_observer,
                    )
                    .await
            }
            _ => Err(PluginFrameworkError::invalid_provider_package(
                "model provider package declares unsupported execution_mode",
            )),
        };
        Self::remove_active_stream(&active_streams, &invocation_id).await;
        let output = output?;
        let mut result = output.result;
        prepared_wire
            .translation_receipt
            .attach_to_provider_metadata(&mut result.provider_metadata)?;
        Ok(ProviderInvokeStreamOutput {
            events: output.events,
            result,
        })
    }
}

struct PreparedProviderGenerateWire {
    wire_value: Value,
    translation_receipt: ProviderGenerateTranslationReceipt,
}

fn current_provider_wire_input(
    loaded: &LoadedProviderPackage,
    input: &ProviderInvocationInput,
) -> FrameworkResult<PreparedProviderGenerateWire> {
    if loaded.package.manifest.contract_version != CURRENT_PROVIDER_CONTRACT {
        return Err(PluginFrameworkError::invalid_provider_contract(format!(
            "unsupported provider package contract: expected {CURRENT_PROVIDER_CONTRACT}, found {}",
            loaded.package.manifest.contract_version
        )));
    }

    if input.operation != ProviderWireOperation::Generate {
        return Err(PluginFrameworkError::invalid_provider_contract(
            "provider stream invocation must declare operation=generate",
        ));
    }

    let (wire_value, translation_receipt) = input
        .to_current_provider_generate_wire_value(&loaded.package.manifest.runtime.capabilities)?;
    Ok(PreparedProviderGenerateWire {
        wire_value,
        translation_receipt,
    })
}

// Provider errors intentionally preserve complete typed upstream diagnostics.
#[allow(clippy::result_large_err)]
fn current_provider_compact_wire_input(
    loaded: &LoadedProviderPackage,
    input: &ProviderInvocationInput,
) -> Result<Value, ProviderCompactError> {
    if loaded.package.manifest.contract_version != CURRENT_PROVIDER_CONTRACT {
        return Err(ProviderCompactError::InvalidContract {
            message: format!(
                "unsupported provider package contract: expected {CURRENT_PROVIDER_CONTRACT}, found {}",
                loaded.package.manifest.contract_version
            ),
        });
    }

    input.to_current_provider_compact_wire_value(&loaded.package.manifest.runtime.capabilities)
}

// Provider errors intentionally preserve complete typed upstream diagnostics.
#[allow(clippy::result_large_err)]
fn current_provider_count_tokens_wire_input(
    loaded: &LoadedProviderPackage,
    input: &ProviderCountTokensInput,
) -> Result<Value, ProviderCountTokensError> {
    if loaded.package.manifest.contract_version != CURRENT_PROVIDER_CONTRACT {
        return Err(ProviderCountTokensError::InvalidContract {
            message: format!(
                "unsupported provider package contract: expected {CURRENT_PROVIDER_CONTRACT}, found {}",
                loaded.package.manifest.contract_version
            ),
        });
    }

    input.to_current_provider_wire_value(&loaded.package.manifest.runtime.capabilities)
}

fn generic_count_tokens_fallback(
    input: &ProviderCountTokensInput,
    reason: ProviderCountTokensFallbackReason,
) -> ProviderCountTokensResult {
    match estimate_provider_count_tokens(input.as_invocation()) {
        Ok(mut result) => {
            result.fallback_reason = Some(reason);
            result
        }
        Err(_) => ProviderCountTokensResult::fallback_zero(),
    }
}

fn compact_framework_error(error: PluginFrameworkError) -> ProviderCompactError {
    match error {
        PluginFrameworkError::RuntimeContract { error } => {
            ProviderCompactError::Runtime { error: *error }
        }
        PluginFrameworkError::Serialization { message, .. } => ProviderCompactError::Runtime {
            error: ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::ProviderInvalidResponse,
                format!("provider Compact response is malformed: {message}"),
            ),
        },
        PluginFrameworkError::Io { message, .. } => ProviderCompactError::Runtime {
            error: ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::EndpointUnreachable,
                format!("provider Compact runtime is unavailable: {message}"),
            ),
        },
        other => ProviderCompactError::InvalidContract {
            message: other.to_string(),
        },
    }
}

mod operations;

use operations::{
    elapsed_milliseconds, format_timestamp, lock_provider_worker_registry, merge_models,
    normalize_balance, normalize_models, provider_invocation_limits, provider_pool_key,
    provider_stream_transport, provider_worker_handle,
};

#[cfg(test)]
#[path = "_tests/provider_host.rs"]
mod tests;
