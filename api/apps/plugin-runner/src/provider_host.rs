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
        ModelDiscoveryMode, ProviderBalanceResult, ProviderInvocationInput,
        ProviderInvocationResult, ProviderModelDescriptor, ProviderStdioMethod,
        ProviderStdioRequest, ProviderStreamEvent, PROVIDER_CONTRACT_V2,
    },
    PluginRuntimeLimits,
};
use serde::Serialize;
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use crate::package_loader::{LoadedProviderPackage, PackageLoader};
use crate::stdio_runtime::{
    call_executable, call_executable_streaming, ProviderWorker,
    DEFAULT_PROVIDER_INVOCATION_TIMEOUT_MS,
};

type ProviderWorkerHandle = Arc<Mutex<ProviderWorker>>;
type ProviderWorkerRegistry = Arc<StdMutex<HashMap<String, ProviderWorkerHandle>>>;
type ProviderLiveEvents = Option<tokio::sync::mpsc::UnboundedSender<ProviderStreamEvent>>;

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
    live_events: ProviderLiveEvents,
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
        self.invoke_stream_with_live_events_operation(plugin_id, input, None)
    }

    pub async fn invoke_stream_with_live_events(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
        live_events: ProviderLiveEvents,
    ) -> FrameworkResult<ProviderInvokeStreamOutput> {
        self.invoke_stream_with_live_events_operation(plugin_id, input, live_events)?
            .await
    }

    pub fn invoke_stream_with_live_events_operation(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
        live_events: ProviderLiveEvents,
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
                live_events,
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
            live_events,
        } = invocation;

        let wire_input = current_provider_wire_input(&loaded, &input)?;
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
            input: wire_input,
        };
        let invocation_limits = provider_invocation_limits(&loaded.package.manifest.runtime.limits);
        let output = match loaded.package.manifest.execution_mode {
            PluginExecutionMode::ProcessPerCall => {
                call_executable_streaming(
                    &loaded.runtime_executable,
                    &request,
                    &invocation_limits,
                    live_events,
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
                        live_events,
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
        Ok(ProviderInvokeStreamOutput {
            events: output.events,
            result: output.result,
        })
    }
}

fn current_provider_wire_input(
    loaded: &LoadedProviderPackage,
    input: &ProviderInvocationInput,
) -> FrameworkResult<Value> {
    if loaded.package.manifest.contract_version != PROVIDER_CONTRACT_V2 {
        return Err(PluginFrameworkError::invalid_provider_contract(format!(
            "unsupported provider package contract: expected {PROVIDER_CONTRACT_V2}, found {}",
            loaded.package.manifest.contract_version
        )));
    }

    serde_json::to_value(input)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

mod operations;

use operations::{
    elapsed_milliseconds, format_timestamp, lock_provider_worker_registry, merge_models,
    normalize_balance, normalize_models, provider_invocation_limits, provider_pool_key,
    provider_stream_transport, provider_worker_handle,
};

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_framework::provider_contract::NativePromptBlock;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::Arc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;
    use tokio::time::sleep;

    struct TempProviderPackage {
        root: PathBuf,
    }

    impl TempProviderPackage {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!("provider-host-test-{nonce}"));
            fs::create_dir_all(&root).unwrap();
            let package = Self { root };
            package.write_provider_package("Fixture Provider");
            package
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write(&self, relative_path: &str, content: &str) {
            let path = self.root.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }

        fn write_provider_package(&self, display_name: &str) {
            self.write_provider_package_with_runtime_timeout(display_name, 30_000);
        }

        fn write_provider_package_with_runtime_timeout(&self, display_name: &str, timeout_ms: u64) {
            self.write_provider_package_with_contract(
                display_name,
                timeout_ms,
                PROVIDER_CONTRACT_V2,
            );
        }

        fn write_provider_package_with_contract(
            &self,
            display_name: &str,
            timeout_ms: u64,
            contract_version: &str,
        ) {
            self.write(
                "manifest.yaml",
                &format!(
                    r#"manifest_version: 1
plugin_id: fixture_provider
version: 0.1.0
vendor: 1flowbase
display_name: {display_name}
description: Fixture provider
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: {contract_version}
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture_provider
  limits:
    timeout_ms: {timeout_ms}
node_contributions: []
"#
                ),
            );
            self.write(
                "provider/fixture_provider.yaml",
                r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
model_discovery: static
config_schema: []
"#,
            );
            self.write(
                "i18n/en_US.json",
                r#"{ "plugin": { "label": "Fixture Provider" } }"#,
            );
            self.write("bin/fixture_provider", "#!/usr/bin/env bash\n");
        }

        fn write_slow_invoke_runtime(&self) {
            self.write(
                "bin/fixture_provider",
                r#"#!/usr/bin/env bash
set -euo pipefail
payload="$(cat)"
case "${payload}" in
  *'"method":"invoke"'*)
    printf '%s\n' '{"type":"text_delta","delta":"started"}'
    sleep 0.08
    printf '%s\n' '{"type":"result","result":{"final_content":"done","finish_reason":"stop"}}'
    ;;
  *)
    printf '%s' '{"ok":true,"result":{}}'
    ;;
esac
"#,
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                let path = self.path().join("bin/fixture_provider");
                let mut permissions = fs::metadata(&path).unwrap().permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(path, permissions).unwrap();
            }
        }

        fn write_stateful_provider_package(
            &self,
            plugin_id: &str,
            provider_code: &str,
            display_name: &str,
        ) {
            self.write(
                "manifest.yaml",
                &format!(
                    r#"manifest_version: 1
plugin_id: {plugin_id}
version: 0.1.0
vendor: 1flowbase
display_name: {display_name}
description: Fixture provider
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: stateful_provider_worker
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/fixture_provider
  limits:
    timeout_ms: 30000
node_contributions: []
"#
                ),
            );
            self.write(
                &format!("provider/{provider_code}.yaml"),
                &format!(
                    r#"provider_code: {provider_code}
display_name: {display_name}
protocol: openai_compatible
model_discovery: static
config_schema: []
"#
                ),
            );
        }

        fn write_slow_worker_runtime(&self, response_label: &str) {
            self.write(
                "bin/fixture_provider",
                &format!(
                    r#"#!/usr/bin/env bash
set -euo pipefail
while IFS= read -r payload; do
  case "${{payload}}" in
    *'"method":"invoke"'*)
      printf '%s\n' '{{"type":"text_delta","delta":"started"}}'
      sleep 0.20
      printf '%s\n' '{{"type":"result","result":{{"final_content":"{response_label}","finish_reason":"stop"}}}}'
      ;;
    *)
      printf '%s\n' '{{"ok":true,"result":{{}}}}'
      ;;
  esac
done
"#
                ),
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                let path = self.path().join("bin/fixture_provider");
                let mut permissions = fs::metadata(&path).unwrap().permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(path, permissions).unwrap();
            }
        }
    }

    impl Drop for TempProviderPackage {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn normalize_models_accepts_current_provider_descriptor_shape() {
        let models = normalize_models(json!([{
            "model_id": "gpt-4o-mini",
            "display_name": "GPT-4o mini",
            "source": "dynamic",
            "supports_streaming": true,
            "supports_tool_call": false,
            "supports_multimodal": false,
            "context_window": null,
            "max_output_tokens": null,
            "parameter_form": null,
            "provider_metadata": {}
        }]))
        .expect("current provider descriptor shape should stay supported");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model_id, "gpt-4o-mini");
    }

    #[test]
    fn normalize_models_rejects_legacy_provider_descriptor_shape() {
        assert!(
            normalize_models(json!([{
                "code": "gpt-4o-mini",
                "label": "GPT-4o mini",
                "family": "llm",
                "mode": "chat"
            }]))
            .is_err(),
            "legacy code/label model descriptors should be rejected once current contract is the only supported shape"
        );
    }

    #[test]
    fn load_if_needed_skips_reloading_matching_loaded_provider_source() {
        let package = TempProviderPackage::new();
        let mut host = ProviderHost::default();
        let summary = host
            .load_with_source_identity(package.path().to_str().unwrap(), Some("gen-1"))
            .unwrap();
        assert!(host.is_loaded(&summary.plugin_id));

        package.write_provider_package("Mutated Provider");
        host.load_if_needed(
            &summary.plugin_id,
            package.path().to_str().unwrap(),
            Some("gen-1"),
        )
        .unwrap();

        let loaded = host.loaded_packages.get(&summary.plugin_id).unwrap();
        assert_eq!(loaded.package.manifest.display_name, "Fixture Provider");
    }

    #[test]
    fn load_if_needed_reloads_when_provider_source_identity_changes() {
        let package = TempProviderPackage::new();
        let mut host = ProviderHost::default();
        let summary = host
            .load_with_source_identity(package.path().to_str().unwrap(), Some("gen-1"))
            .unwrap();
        assert!(host.is_loaded(&summary.plugin_id));

        package.write_provider_package("Mutated Provider");
        host.load_if_needed(
            &summary.plugin_id,
            package.path().to_str().unwrap(),
            Some("gen-2"),
        )
        .unwrap();

        let loaded = host.loaded_packages.get(&summary.plugin_id).unwrap();
        assert_eq!(loaded.package.manifest.display_name, "Mutated Provider");
    }

    fn invocation_input(model: &str) -> ProviderInvocationInput {
        ProviderInvocationInput {
            provider_instance_id: "provider-1".to_string(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: model.to_string(),
            provider_config: json!({}),
            ..ProviderInvocationInput::default()
        }
    }

    async fn wait_for_active_streams(host: &ProviderHost, count: usize) {
        for _ in 0..20 {
            if host.active_stream_snapshot().await.streams.len() == count {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
        panic!("expected {count} active provider stream(s)");
    }

    #[test]
    fn d1_ac_011_current_provider_package_serializes_typed_input_without_projection() {
        let package = TempProviderPackage::new();
        let mut host = ProviderHost::default();
        let summary = host.load(package.path().to_str().unwrap()).unwrap();
        let mut input = invocation_input("fixture-model");
        input.system = vec![NativePromptBlock::text("current typed system")];
        input
            .model_parameters
            .insert("max_output_tokens".to_string(), json!(512));

        let loaded = host
            .loaded_package(&summary.plugin_id)
            .expect("current provider package should be loaded");
        let wire_input = current_provider_wire_input(loaded, &input)
            .expect("current provider package should receive direct typed input");

        assert_eq!(
            wire_input["contract_version"],
            json!("1flowbase.provider/v2")
        );
        assert_eq!(wire_input["system"][0]["type"], json!("text"));
        assert_eq!(
            wire_input["model_parameters"]["max_output_tokens"],
            json!(512)
        );
        assert!(wire_input["model_parameters"].get("max_tokens").is_none());
    }

    #[tokio::test]
    async fn d1_ac_011_non_current_provider_package_is_rejected_before_starting_provider_process() {
        let package = TempProviderPackage::new();
        package.write_provider_package_with_contract(
            "Legacy Provider",
            30_000,
            "1flowbase.provider/v1",
        );
        let mut host = ProviderHost::default();
        let summary = host.load(package.path().to_str().unwrap()).unwrap();

        let error = host
            .invoke_stream(&summary.plugin_id, invocation_input("fixture-model"))
            .await
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("unsupported provider package contract"));
        assert!(host.active_stream_snapshot().await.streams.is_empty());
        assert!(lock_provider_worker_registry(&host.provider_workers)
            .expect("provider worker registry should be available")
            .is_empty());
    }

    #[tokio::test]
    async fn active_invocation_lease_serializes_same_provider_pool() {
        let package = TempProviderPackage::new();
        package.write_slow_invoke_runtime();
        let mut host = ProviderHost::default();
        let plugin_id = host
            .load(package.path().to_str().unwrap())
            .unwrap()
            .plugin_id;
        let host = Arc::new(host);

        let first_host = Arc::clone(&host);
        let first_plugin_id = plugin_id.clone();
        let first = tokio::spawn(async move {
            first_host
                .invoke_stream(&first_plugin_id, invocation_input("fixture-model"))
                .await
                .unwrap()
        });
        wait_for_active_streams(&host, 1).await;

        let second_host = Arc::clone(&host);
        let second_plugin_id = plugin_id.clone();
        let second = tokio::spawn(async move {
            second_host
                .invoke_stream(&second_plugin_id, invocation_input("fixture-model"))
                .await
                .unwrap()
        });
        sleep(Duration::from_millis(20)).await;

        assert_eq!(host.active_stream_snapshot().await.streams.len(), 1);
        first.await.unwrap();
        second.await.unwrap();
        assert!(host.active_stream_snapshot().await.streams.is_empty());
    }

    #[tokio::test]
    async fn invoke_stream_uses_default_provider_invocation_budget() {
        let package = TempProviderPackage::new();
        package.write_provider_package_with_runtime_timeout("Fixture Provider", 1);
        package.write_slow_invoke_runtime();
        let mut host = ProviderHost::default();
        let plugin_id = host
            .load(package.path().to_str().unwrap())
            .unwrap()
            .plugin_id;

        let output = host
            .invoke_stream(&plugin_id, invocation_input("fixture-model"))
            .await
            .expect("provider invocation should not inherit the short runtime command timeout");

        assert_eq!(output.result.final_content.as_deref(), Some("done"));
    }

    #[tokio::test]
    async fn active_invocation_lease_allows_different_provider_pools() {
        let package = TempProviderPackage::new();
        package.write_slow_invoke_runtime();
        let mut host = ProviderHost::default();
        let plugin_id = host
            .load(package.path().to_str().unwrap())
            .unwrap()
            .plugin_id;
        let host = Arc::new(host);

        let first_host = Arc::clone(&host);
        let first_plugin_id = plugin_id.clone();
        let first = tokio::spawn(async move {
            first_host
                .invoke_stream(&first_plugin_id, invocation_input("fixture-model-a"))
                .await
                .unwrap()
        });
        let second_host = Arc::clone(&host);
        let second_plugin_id = plugin_id.clone();
        let second = tokio::spawn(async move {
            second_host
                .invoke_stream(&second_plugin_id, invocation_input("fixture-model-b"))
                .await
                .unwrap()
        });

        wait_for_active_streams(&host, 2).await;
        first.await.unwrap();
        second.await.unwrap();
    }

    #[tokio::test]
    async fn stateful_worker_registry_does_not_serialize_different_plugins() {
        let first_package = TempProviderPackage::new();
        first_package.write_stateful_provider_package(
            "fixture_provider_a",
            "fixture_provider_a",
            "Fixture Provider A",
        );
        first_package.write_slow_worker_runtime("first done");
        let second_package = TempProviderPackage::new();
        second_package.write_stateful_provider_package(
            "fixture_provider_b",
            "fixture_provider_b",
            "Fixture Provider B",
        );
        second_package.write_slow_worker_runtime("second done");
        let mut host = ProviderHost::default();
        let first_plugin_id = host
            .load(first_package.path().to_str().unwrap())
            .unwrap()
            .plugin_id;
        let second_plugin_id = host
            .load(second_package.path().to_str().unwrap())
            .unwrap()
            .plugin_id;
        let host = Arc::new(host);

        let first_host = Arc::clone(&host);
        let first = tokio::spawn(async move {
            first_host
                .invoke_stream(&first_plugin_id, invocation_input("fixture-model-a"))
                .await
                .unwrap()
        });
        wait_for_active_streams(&host, 1).await;

        let second_host = Arc::clone(&host);
        let second = tokio::spawn(async move {
            second_host
                .invoke_stream(&second_plugin_id, invocation_input("fixture-model-b"))
                .await
                .unwrap()
        });

        tokio::time::timeout(Duration::from_millis(320), async {
            let first = first.await.unwrap();
            let second = second.await.unwrap();
            assert_eq!(first.result.final_content.as_deref(), Some("first done"));
            assert_eq!(second.result.final_content.as_deref(), Some("second done"));
        })
        .await
        .expect(
            "different stateful provider workers should not be serialized by the registry lock",
        );
    }
}
