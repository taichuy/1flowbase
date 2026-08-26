use std::{
    collections::{HashMap, HashSet},
    fs::{self, DirBuilder, File, OpenOptions},
    io::Write,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

use extension_package_runtime::{
    error::{FrameworkResult, PluginFrameworkError},
    AcquireHttpForwardProxyInput, EgressAvailability, EgressDescriptor, ForwardProxyLease,
    NetworkEgressProviderPackage, NetworkEgressProviderStdioRequest,
    NetworkEgressProviderStdioResponse, PluginRuntimeLimits, ReleaseHttpForwardProxyInput,
    SyncEgressesInput, SyncEgressesResult,
};
use http::Uri;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::TcpStream,
    process::{Child, ChildStdin, ChildStdout, Command},
};

const LEASE_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(1);
const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
static NEXT_CONFIG_ID: AtomicU64 = AtomicU64::new(0);
static NEXT_HOST_LEASE_ID: AtomicU64 = AtomicU64::new(0);
static NEXT_WORKER_GENERATION_ID: AtomicU64 = AtomicU64::new(0);

/// Private startup-only material. It is deliberately non-serializable and redacts its payload
/// so configuration can reach the worker without becoming part of a runtime protocol.
#[derive(Clone, PartialEq)]
pub struct NetworkEgressWorkerConfig {
    secret_json: serde_json::Value,
}

impl NetworkEgressWorkerConfig {
    pub fn from_secret_json(secret_json: serde_json::Value) -> Self {
        Self { secret_json }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEgressWorkerState {
    Active,
    Inactive,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEgressCleanupReason {
    Stopped,
    Reloaded,
    LeaseExpired,
    RuntimeFailure,
}

/// Audit evidence records process-tree and release outcomes without retaining implementation
/// configuration, proxy credentials, or the opaque cleanup capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkEgressCleanupReceipt {
    pub runtime_id: String,
    pub prior_pid: Option<u32>,
    pub process_group_id: Option<u32>,
    pub termination_signal_sent: bool,
    pub process_tree_exited: bool,
    pub lease_revoked: bool,
    pub final_state: NetworkEgressWorkerState,
    pub reason: NetworkEgressCleanupReason,
    pub cleanup_error: Option<String>,
}

#[derive(Default)]
pub struct NetworkEgressHost {
    /// Workers are generation-scoped so an old artifact can drain existing leases after a
    /// provider instance atomically moves to a newer current artifact.
    workers: HashMap<String, NetworkEgressWorker>,
    sources: HashMap<String, NetworkEgressSource>,
    active_generations: HashMap<String, String>,
    generation_runtime_ids: HashMap<String, String>,
    retired_generations: HashSet<String>,
    lease_owners: HashMap<String, NetworkEgressLeaseOwner>,
    cleanup_receipts: HashMap<String, NetworkEgressCleanupReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkEgressLeaseOwner {
    runtime_id: String,
    generation_id: String,
    worker_lease_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkEgressSource {
    package_root: PathBuf,
    source_identity: String,
}

impl NetworkEgressSource {
    fn resolve(package_root: &str, source_identity: &str) -> FrameworkResult<Self> {
        let package_root = std::fs::canonicalize(package_root).map_err(|error| {
            PluginFrameworkError::invalid_provider_package(format!(
                "cannot resolve network egress package root: {error}"
            ))
        })?;
        Ok(Self {
            package_root,
            source_identity: source_identity.to_string(),
        })
    }
}

impl NetworkEgressHost {
    pub fn loaded_count(&self) -> usize {
        self.active_generations.len()
    }

    pub async fn stop_all(&mut self) -> FrameworkResult<()> {
        let runtime_ids = self.active_generations.keys().cloned().collect::<Vec<_>>();
        for runtime_id in runtime_ids {
            self.unload(&runtime_id).await?;
        }
        Ok(())
    }

    /// Validates one provider configuration against a candidate artifact without publishing that
    /// artifact as the provider instance's active generation.
    pub async fn preflight(
        runtime_id: &str,
        plugin_id: &str,
        package_root: &str,
        config: NetworkEgressWorkerConfig,
    ) -> FrameworkResult<Vec<EgressDescriptor>> {
        let package = NetworkEgressProviderPackage::load_from_dir(package_root)?;
        if package.identifier() != plugin_id {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "network egress package id {} does not match requested {plugin_id}",
                package.identifier()
            )));
        }
        let mut worker = NetworkEgressWorker::start(
            package.runtime_entry(),
            package.manifest.runtime.limits,
            config,
        )?;
        let result = worker.sync_egresses().await.map(|result| result.egresses);
        let receipt = worker
            .stop(runtime_id, NetworkEgressCleanupReason::Stopped)
            .await;
        match (result, receipt.cleanup_error) {
            (Err(error), _) => Err(error),
            (Ok(_), Some(_)) => Err(network_runtime_error(
                "network egress preflight worker cleanup failed",
            )),
            (Ok(egresses), None) => Ok(egresses),
        }
    }

    /// Registers the stateful runtime worker only. A lease is intentionally not acquired until
    /// the resolver supplies the provider-owned egress key selected by the caller.
    pub async fn load_if_needed(
        &mut self,
        runtime_id: &str,
        plugin_id: &str,
        package_root: &str,
        source_identity: &str,
        config: NetworkEgressWorkerConfig,
    ) -> FrameworkResult<()> {
        let requested = NetworkEgressSource::resolve(package_root, source_identity)?;
        if self
            .active_generations
            .get(runtime_id)
            .and_then(|generation_id| self.sources.get(generation_id))
            == Some(&requested)
        {
            return self.ensure_worker_is_live(runtime_id).await;
        }

        let package = NetworkEgressProviderPackage::load_from_dir(&requested.package_root)?;
        if package.identifier() != plugin_id {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "network egress package id {} does not match requested {plugin_id}",
                package.identifier()
            )));
        }
        let worker = NetworkEgressWorker::start(
            package.runtime_entry(),
            package.manifest.runtime.limits,
            config,
        )
        .inspect_err(|_| {
            self.cleanup_receipts
                .insert(runtime_id.to_string(), startup_receipt(runtime_id));
        })?;
        tracing::info!(
            provider_id = runtime_id,
            plugin_id,
            worker_pid = worker.process_group_id,
            source_identity,
            "network egress worker generation started"
        );

        let generation_id = format!(
            "{runtime_id}:{}:{}:{}",
            requested.package_root.display(),
            requested.source_identity,
            NEXT_WORKER_GENERATION_ID.fetch_add(1, Ordering::Relaxed)
        );
        self.sources.insert(generation_id.clone(), requested);
        self.workers.insert(generation_id.clone(), worker);
        self.generation_runtime_ids
            .insert(generation_id.clone(), runtime_id.to_string());
        let prior_generation = self
            .active_generations
            .insert(runtime_id.to_string(), generation_id);
        if let Some(prior_generation) = prior_generation {
            self.retired_generations.insert(prior_generation.clone());
            if self
                .workers
                .get(&prior_generation)
                .is_some_and(|worker| worker.active_lease_count() == 0)
            {
                self.stop_generation(&prior_generation, NetworkEgressCleanupReason::Reloaded)
                    .await?;
            }
        }
        Ok(())
    }

    /// Resolves and validates a fresh HTTP forward-proxy lease for a caller-selected egress.
    /// The provider configuration never crosses this boundary.
    pub async fn resolve_http_forward_proxy(
        &mut self,
        runtime_id: &str,
        provider_egress_key: &str,
    ) -> FrameworkResult<ForwardProxyLease> {
        let generation_id = self.active_generation(runtime_id)?;
        let worker = self.workers.get_mut(&generation_id).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "network egress runtime is not loaded: {runtime_id}"
            ))
        })?;
        let result = worker.resolve_http_forward_proxy(provider_egress_key).await;
        if result.is_err() && !worker.is_live()? {
            self.retire_failed_generation(&generation_id).await;
        }
        let mut lease = result?;
        let worker_lease_id = lease.lease_id.clone();
        let lease_id = next_host_lease_id(runtime_id);
        self.lease_owners.insert(
            lease_id.clone(),
            NetworkEgressLeaseOwner {
                runtime_id: runtime_id.to_string(),
                generation_id,
                worker_lease_id,
            },
        );
        lease.lease_id = lease_id;
        Ok(lease)
    }

    /// Returns the provider-owned egress catalog through the validated v1 worker operation.
    /// Descriptors deliberately contain only display and availability data, never provider
    /// configuration, proxy capabilities, or secrets.
    pub async fn sync_egresses(
        &mut self,
        runtime_id: &str,
    ) -> FrameworkResult<Vec<EgressDescriptor>> {
        let generation_id = self.active_generation(runtime_id)?;
        let worker = self.workers.get_mut(&generation_id).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "network egress runtime is not loaded: {runtime_id}"
            ))
        })?;
        let result = worker.sync_egresses().await.map(|result| result.egresses);
        if result.is_err() && !worker.is_live()? {
            self.retire_failed_generation(&generation_id).await;
        }
        result
    }

    /// Releases the exact lease acquired for one Core consumer operation. The opaque cleanup
    /// capability remains inside the worker so callers cannot retain or replay it.
    pub async fn release_http_forward_proxy(
        &mut self,
        runtime_id: &str,
        lease_id: &str,
    ) -> FrameworkResult<()> {
        let owner = self.lease_owners.get(lease_id).cloned().ok_or_else(|| {
            network_runtime_error("network egress lease has no owning worker generation")
        })?;
        if owner.runtime_id != runtime_id {
            return Err(network_runtime_error(
                "network egress lease belongs to a different provider instance",
            ));
        }
        let worker = self.workers.get_mut(&owner.generation_id).ok_or_else(|| {
            network_runtime_error("network egress lease worker generation is unavailable")
        })?;
        if !worker.release_lease(&owner.worker_lease_id).await {
            return Err(network_runtime_error(
                "network egress lease release did not return a matching receipt",
            ));
        }
        self.lease_owners.remove(lease_id);
        if self.retired_generations.contains(&owner.generation_id)
            && self
                .workers
                .get(&owner.generation_id)
                .is_some_and(|worker| worker.active_lease_count() == 0)
        {
            self.stop_generation(&owner.generation_id, NetworkEgressCleanupReason::Reloaded)
                .await?;
        }
        Ok(())
    }

    pub async fn unload(&mut self, runtime_id: &str) -> FrameworkResult<()> {
        let generations = self
            .generation_runtime_ids
            .iter()
            .filter(|(_, owner_runtime_id)| owner_runtime_id.as_str() == runtime_id)
            .map(|(generation_id, _)| generation_id.clone())
            .collect::<Vec<_>>();
        self.active_generations.remove(runtime_id);
        for generation_id in generations {
            self.stop_generation(&generation_id, NetworkEgressCleanupReason::Stopped)
                .await?;
        }
        Ok(())
    }

    pub fn cleanup_receipt(&self, runtime_id: &str) -> Option<&NetworkEgressCleanupReceipt> {
        self.cleanup_receipts.get(runtime_id)
    }

    fn active_generation(&self, runtime_id: &str) -> FrameworkResult<String> {
        self.active_generations
            .get(runtime_id)
            .cloned()
            .ok_or_else(|| {
                PluginFrameworkError::invalid_provider_package(format!(
                    "network egress runtime is not loaded: {runtime_id}"
                ))
            })
    }

    async fn ensure_worker_is_live(&mut self, runtime_id: &str) -> FrameworkResult<()> {
        let generation_id = self.active_generation(runtime_id)?;
        let is_live = self
            .workers
            .get_mut(&generation_id)
            .ok_or_else(|| {
                PluginFrameworkError::invalid_provider_package(format!(
                    "network egress runtime is not loaded: {runtime_id}"
                ))
            })?
            .is_live()?;
        if is_live {
            return Ok(());
        }
        self.retire_failed_generation(&generation_id).await;
        Err(network_runtime_error("network egress worker exited"))
    }

    async fn retire_failed_generation(&mut self, generation_id: &str) {
        let runtime_id = self.generation_runtime_ids.get(generation_id).cloned();
        if let Some(worker) = self.workers.remove(generation_id) {
            let receipt = worker
                .stop(
                    runtime_id.as_deref().unwrap_or(generation_id),
                    NetworkEgressCleanupReason::RuntimeFailure,
                )
                .await;
            if let Some(runtime_id) = runtime_id.as_ref() {
                self.cleanup_receipts.insert(runtime_id.clone(), receipt);
            }
        }
        if runtime_id
            .as_ref()
            .and_then(|runtime_id| self.active_generations.get(runtime_id))
            .is_some_and(|active| active == generation_id)
        {
            self.active_generations
                .remove(runtime_id.as_deref().unwrap_or_default());
        }
        self.lease_owners
            .retain(|_, owner| owner.generation_id != generation_id);
        self.retired_generations.remove(generation_id);
        self.generation_runtime_ids.remove(generation_id);
        self.sources.remove(generation_id);
    }

    async fn stop_generation(
        &mut self,
        generation_id: &str,
        reason: NetworkEgressCleanupReason,
    ) -> FrameworkResult<()> {
        let runtime_id = self
            .generation_runtime_ids
            .get(generation_id)
            .cloned()
            .unwrap_or_else(|| generation_id.to_string());
        let mut release_failed = false;
        if let Some(worker) = self.workers.remove(generation_id) {
            let receipt = worker.stop(&runtime_id, reason).await;
            release_failed = receipt.cleanup_error.is_some();
            self.cleanup_receipts.insert(runtime_id.clone(), receipt);
        }
        if self
            .active_generations
            .get(&runtime_id)
            .is_some_and(|active| active == generation_id)
        {
            self.active_generations.remove(&runtime_id);
        }
        self.lease_owners
            .retain(|_, owner| owner.generation_id != generation_id);
        self.retired_generations.remove(generation_id);
        self.generation_runtime_ids.remove(generation_id);
        self.sources.remove(generation_id);
        if release_failed {
            return Err(network_runtime_error(
                "network egress worker cleanup did not receive a lease release receipt",
            ));
        }
        tracing::info!(
            provider_id = runtime_id,
            generation_id,
            reason = ?reason,
            "network egress worker generation stopped"
        );
        Ok(())
    }
}

struct NetworkEgressWorker {
    executable_path: PathBuf,
    limits: PluginRuntimeLimits,
    process_group_id: u32,
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    /// Every consumer scope owns one opaque lease identity. The worker retains the complete
    /// verified lease so that neither callers nor consumers ever see its cleanup capability.
    leases_by_id: HashMap<String, VerifiedForwardProxyLease>,
    config_file: NetworkEgressConfigFile,
}

#[derive(Debug, Clone)]
struct VerifiedForwardProxyLease {
    lease: ForwardProxyLease,
}

struct NetworkEgressConfigFile {
    directory: PathBuf,
    path: PathBuf,
    cleaned: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkEgressWorkerErrorResponse {
    operation: String,
    error: NetworkEgressWorkerError,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkEgressWorkerError {
    code: String,
    message: String,
}

impl NetworkEgressWorkerErrorResponse {
    fn validate(&self) -> FrameworkResult<()> {
        if !matches!(
            self.operation.as_str(),
            "sync_egresses" | "acquire_http_forward_proxy" | "release_http_forward_proxy"
        ) {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "network egress worker error operation is invalid",
            ));
        }
        if !self.error.code.starts_with("network_egress_")
            || self.error.code.len() > 128
            || !self
                .error
                .code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || self.error.message.trim().is_empty()
            || self.error.message.len() > 256
            || self.error.message.contains(['\n', '\r'])
        {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "network egress worker error is invalid",
            ));
        }
        Ok(())
    }

    fn into_framework_error(self) -> PluginFrameworkError {
        PluginFrameworkError::runtime(
            extension_package_runtime::provider_contract::ProviderRuntimeError::new(
                extension_package_runtime::provider_contract::ProviderRuntimeErrorKind::ProviderInvalidResponse,
                "network egress provider rejected the requested operation",
            )
            .with_provider_summary(self.error.message)
            .with_provider_details(serde_json::json!({ "code": self.error.code })),
        )
    }
}

impl NetworkEgressConfigFile {
    fn create(config: NetworkEgressWorkerConfig) -> FrameworkResult<Self> {
        let directory = private_config_directory()?;
        let path = directory.join("config.json");
        let result = (|| -> std::io::Result<()> {
            let payload = serde_json::to_vec(&config.secret_json).map_err(std::io::Error::other)?;
            let mut file = private_config_file(&path)?;
            file.write_all(&payload)?;
            file.sync_all()
        })();
        if result.is_err() {
            wipe_and_remove(&path, &directory);
            return Err(network_runtime_error(
                "cannot provision private network egress configuration",
            ));
        }
        Ok(Self {
            directory,
            path,
            cleaned: false,
        })
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn cleanup(&mut self) -> bool {
        if self.cleaned {
            return true;
        }
        self.cleaned = true;
        wipe_and_remove(&self.path, &self.directory)
    }
}

impl Drop for NetworkEgressConfigFile {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn private_config_directory() -> FrameworkResult<PathBuf> {
    let base = std::env::temp_dir();
    for _ in 0..16 {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                network_runtime_error("cannot provision private network egress configuration")
            })?
            .as_nanos();
        let sequence = NEXT_CONFIG_ID.fetch_add(1, Ordering::Relaxed);
        let directory = base.join(format!(
            "1flowbase-network-egress-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        match private_directory(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => break,
        }
    }
    Err(network_runtime_error(
        "cannot provision private network egress configuration",
    ))
}

#[cfg(unix)]
fn private_directory(path: &std::path::Path) -> std::io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.mode(0o700);
    builder.create(path)
}

#[cfg(not(unix))]
fn private_directory(path: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir(path)
}

#[cfg(unix)]
fn private_config_file(path: &std::path::Path) -> std::io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn private_config_file(path: &std::path::Path) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn wipe_and_remove(path: &std::path::Path, directory: &std::path::Path) -> bool {
    let wiped = wipe_file(path);
    let removed_file = fs::remove_file(path).is_ok() || !path.exists();
    let removed_directory = fs::remove_dir(directory).is_ok() || !directory.exists();
    wiped && removed_file && removed_directory
}

fn wipe_file(path: &std::path::Path) -> bool {
    let Ok(mut file) = OpenOptions::new().write(true).open(path) else {
        return !path.exists();
    };
    let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
        return false;
    };
    let zeros = [0_u8; 8192];
    let mut remaining = length;
    while remaining > 0 {
        let chunk = usize::try_from(remaining.min(zeros.len() as u64)).unwrap_or(zeros.len());
        if file.write_all(&zeros[..chunk]).is_err() {
            return false;
        }
        remaining -= chunk as u64;
    }
    file.sync_all().is_ok()
}

impl NetworkEgressWorker {
    fn active_lease_count(&self) -> usize {
        self.leases_by_id.len()
    }

    fn start(
        executable_path: PathBuf,
        limits: PluginRuntimeLimits,
        config: NetworkEgressWorkerConfig,
    ) -> FrameworkResult<Self> {
        let config_file = NetworkEgressConfigFile::create(config)?;
        let mut command = Command::new(&executable_path);
        command
            .arg("--network-egress-config-file")
            .arg(config_file.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(false);
        configure_worker_process_group(&mut command)?;
        apply_memory_limit(&mut command, limits.memory_bytes)?;
        let mut child = command
            .spawn()
            .map_err(|error| PluginFrameworkError::io(Some(&executable_path), error.to_string()))?;
        let process_group_id = child.id().ok_or_else(|| {
            network_runtime_error("network egress worker process identity is unavailable")
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| network_runtime_error("network egress worker stdin was not captured"))?;
        let stdout = child.stdout.take().ok_or_else(|| {
            network_runtime_error("network egress worker stdout was not captured")
        })?;
        Ok(Self {
            executable_path,
            limits,
            process_group_id,
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            leases_by_id: HashMap::new(),
            config_file,
        })
    }

    async fn resolve_http_forward_proxy(
        &mut self,
        provider_egress_key: &str,
    ) -> FrameworkResult<ForwardProxyLease> {
        self.ensure_worker_is_live().await?;
        let egresses = self.sync_egresses().await?;
        let available = egresses.egresses.iter().any(|egress| {
            egress.provider_egress_key == provider_egress_key
                && egress.availability == EgressAvailability::Available
        });
        if !available {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "requested provider_egress_key is unavailable",
            ));
        }
        let response = self
            .call(NetworkEgressProviderStdioRequest::AcquireHttpForwardProxy(
                AcquireHttpForwardProxyInput {
                    provider_egress_key: provider_egress_key.to_string(),
                },
            ))
            .await?;
        let lease = match response {
            NetworkEgressProviderStdioResponse::AcquireHttpForwardProxy(lease) => lease,
            _ => {
                return Err(PluginFrameworkError::invalid_provider_contract(
                    "network egress worker returned the wrong result for acquire_http_forward_proxy",
                ));
            }
        };
        let verified = VerifiedForwardProxyLease::verify(lease).await?;
        let lease = verified.lease.clone();
        if self.leases_by_id.contains_key(&lease.lease_id) {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "network egress worker reused an active lease_id",
            ));
        }
        self.leases_by_id.insert(lease.lease_id.clone(), verified);
        Ok(lease)
    }

    fn is_live(&mut self) -> FrameworkResult<bool> {
        Ok(self
            .child
            .try_wait()
            .map_err(|error| {
                PluginFrameworkError::io(Some(&self.executable_path), error.to_string())
            })?
            .is_none())
    }

    async fn ensure_worker_is_live(&mut self) -> FrameworkResult<()> {
        if !self.is_live()? {
            return Err(network_runtime_error("network egress worker exited"));
        }
        Ok(())
    }

    async fn sync_egresses(&mut self) -> FrameworkResult<SyncEgressesResult> {
        let response = self
            .call(NetworkEgressProviderStdioRequest::SyncEgresses(
                SyncEgressesInput {},
            ))
            .await?;
        match response {
            NetworkEgressProviderStdioResponse::SyncEgresses(result) => Ok(result),
            _ => Err(PluginFrameworkError::invalid_provider_contract(
                "network egress worker returned the wrong result for sync_egresses",
            )),
        }
    }

    async fn release_lease(&mut self, lease_id: &str) -> bool {
        let Some(lease) = self.leases_by_id.get(lease_id).cloned() else {
            return false;
        };
        let response = self
            .call(NetworkEgressProviderStdioRequest::ReleaseHttpForwardProxy(
                ReleaseHttpForwardProxyInput {
                    lease_id: lease.lease.lease_id.clone(),
                    cleanup_token: lease.lease.cleanup_token.clone(),
                },
            ))
            .await;
        let release_acknowledged = matches!(
            response,
            Ok(NetworkEgressProviderStdioResponse::ReleaseHttpForwardProxy(receipt))
                if receipt.lease_id == lease.lease.lease_id
        );
        if release_acknowledged {
            self.leases_by_id.remove(lease_id);
        }
        release_acknowledged
    }

    async fn release_all_leases(&mut self) -> bool {
        let lease_ids = self.leases_by_id.keys().cloned().collect::<Vec<_>>();
        let mut all_released = true;
        for lease_id in lease_ids {
            all_released &= self.release_lease(&lease_id).await;
        }
        all_released
    }

    async fn call(
        &mut self,
        request: NetworkEgressProviderStdioRequest,
    ) -> FrameworkResult<NetworkEgressProviderStdioResponse> {
        let expected_operation = match &request {
            NetworkEgressProviderStdioRequest::SyncEgresses(_) => "sync_egresses",
            NetworkEgressProviderStdioRequest::AcquireHttpForwardProxy(_) => {
                "acquire_http_forward_proxy"
            }
            NetworkEgressProviderStdioRequest::ReleaseHttpForwardProxy(_) => {
                "release_http_forward_proxy"
            }
        };
        request.validate()?;
        let payload = serde_json::to_vec(&request)
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
        self.stdin.write_all(&payload).await.map_err(|error| {
            PluginFrameworkError::io(Some(&self.executable_path), error.to_string())
        })?;
        self.stdin.write_all(b"\n").await.map_err(|error| {
            PluginFrameworkError::io(Some(&self.executable_path), error.to_string())
        })?;
        let timeout = Duration::from_millis(self.limits.timeout_ms.unwrap_or(5_000));
        let line = tokio::time::timeout(timeout, self.stdout.next_line())
            .await
            .map_err(|_| network_runtime_error("network egress worker operation timed out"))?
            .map_err(|error| {
                PluginFrameworkError::io(Some(&self.executable_path), error.to_string())
            })?
            .ok_or_else(|| {
                network_runtime_error("network egress worker ended without a response")
            })?;
        if let Ok(error_response) = serde_json::from_str::<NetworkEgressWorkerErrorResponse>(&line)
        {
            error_response.validate()?;
            if error_response.operation != expected_operation {
                return Err(PluginFrameworkError::invalid_provider_contract(
                    "network egress worker error operation does not match the request",
                ));
            }
            return Err(error_response.into_framework_error());
        }
        let response =
            serde_json::from_str::<NetworkEgressProviderStdioResponse>(&line).map_err(|error| {
                PluginFrameworkError::invalid_provider_contract(format!(
                    "invalid network egress stdio response: {error}"
                ))
            })?;
        response.validate()?;
        Ok(response)
    }

    async fn stop(
        mut self,
        runtime_id: &str,
        reason: NetworkEgressCleanupReason,
    ) -> NetworkEgressCleanupReceipt {
        let lease_revoked = !self.leases_by_id.is_empty();
        let release_acknowledged = self.release_all_leases().await;
        let prior_pid = Some(self.process_group_id);
        let process_group_id = Some(self.process_group_id);
        let mut termination_signal_sent = false;
        let mut cleanup_error = (lease_revoked && !release_acknowledged).then_some(
            "network egress lease release did not return a matching receipt".to_string(),
        );
        let leader_already_exited = match self.child.try_wait() {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(error) => {
                cleanup_error.get_or_insert_with(|| error.to_string());
                false
            }
        };
        // The worker is the process-group/session leader, but its descendants can outlive it.
        // Always address the retained PGID even when `try_wait` has already reaped the leader.
        match terminate_process_group(process_group_id, libc::SIGTERM) {
            Ok(sent) => termination_signal_sent |= sent,
            Err(error) => {
                cleanup_error.get_or_insert_with(|| error.to_string());
            }
        }
        let leader_exited = if leader_already_exited {
            true
        } else {
            match tokio::time::timeout(WORKER_SHUTDOWN_TIMEOUT, self.child.wait()).await {
                Ok(Ok(_)) => true,
                Ok(Err(error)) => {
                    cleanup_error.get_or_insert_with(|| error.to_string());
                    false
                }
                Err(_) => false,
            }
        };
        let mut process_group_exited = wait_for_process_group_exit(process_group_id).await;
        if !leader_exited || !process_group_exited {
            match terminate_process_group(process_group_id, libc::SIGKILL) {
                Ok(sent) => termination_signal_sent |= sent,
                Err(error) => {
                    cleanup_error.get_or_insert_with(|| error.to_string());
                }
            }
            if !leader_exited {
                if let Err(error) = self.child.wait().await {
                    cleanup_error.get_or_insert_with(|| error.to_string());
                }
            }
            process_group_exited = wait_for_process_group_exit(process_group_id).await;
        }
        let process_tree_exited = leader_exited && process_group_exited;
        if !process_tree_exited {
            cleanup_error.get_or_insert_with(|| {
                "network egress worker process group did not exit".to_string()
            });
        }
        if !self.config_file.cleanup() {
            cleanup_error.get_or_insert_with(|| {
                "network egress private configuration cleanup failed".to_string()
            });
        }
        NetworkEgressCleanupReceipt {
            runtime_id: runtime_id.to_string(),
            prior_pid,
            process_group_id,
            termination_signal_sent,
            process_tree_exited,
            lease_revoked,
            final_state: if reason == NetworkEgressCleanupReason::RuntimeFailure {
                NetworkEgressWorkerState::Failed
            } else {
                NetworkEgressWorkerState::Inactive
            },
            reason,
            cleanup_error,
        }
    }
}

impl VerifiedForwardProxyLease {
    async fn verify(lease: ForwardProxyLease) -> FrameworkResult<Self> {
        let uri = lease.http_proxy_url.parse::<Uri>().map_err(|_| {
            PluginFrameworkError::invalid_provider_contract(
                "network egress lease http_proxy_url is invalid",
            )
        })?;
        if uri.scheme_str() != Some("http") || uri.path() != "/" || uri.query().is_some() {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "network egress lease must use an http proxy URL without a path or query",
            ));
        }
        let host = uri.host().ok_or_else(|| {
            PluginFrameworkError::invalid_provider_contract(
                "network egress lease has no proxy host",
            )
        })?;
        let address = host.parse::<IpAddr>().map_err(|_| {
            PluginFrameworkError::invalid_provider_contract(
                "network egress lease proxy host must be a loopback IP address",
            )
        })?;
        if !address.is_loopback() {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "network egress lease proxy host must be loopback-only",
            ));
        }
        let port = uri.port_u16().ok_or_else(|| {
            PluginFrameworkError::invalid_provider_contract(
                "network egress lease proxy URL must include an explicit port",
            )
        })?;
        if lease.expires_at <= unix_milliseconds_now() {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "network egress lease expires_at must be in the future",
            ));
        }
        let endpoint = SocketAddr::new(address, port);
        tokio::time::timeout(LEASE_VERIFICATION_TIMEOUT, TcpStream::connect(endpoint))
            .await
            .map_err(|_| {
                network_runtime_error("network egress proxy endpoint did not accept a connection")
            })?
            .map_err(|_| {
                network_runtime_error("network egress proxy endpoint verification failed")
            })?;
        Ok(Self { lease })
    }
}

fn unix_milliseconds_now() -> u64 {
    let milliseconds = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    u64::try_from(milliseconds).unwrap_or(u64::MAX)
}

#[cfg(unix)]
fn configure_worker_process_group(command: &mut Command) -> FrameworkResult<()> {
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn configure_worker_process_group(_command: &mut Command) -> FrameworkResult<()> {
    Ok(())
}

#[cfg(unix)]
fn apply_memory_limit(command: &mut Command, memory_bytes: Option<u64>) -> FrameworkResult<()> {
    if let Some(limit) = memory_bytes {
        unsafe {
            command.pre_exec(move || {
                let limit = libc::rlimit {
                    rlim_cur: limit as libc::rlim_t,
                    rlim_max: limit as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn apply_memory_limit(_command: &mut Command, _memory_bytes: Option<u64>) -> FrameworkResult<()> {
    Ok(())
}

#[cfg(unix)]
fn terminate_process_group(pid: Option<u32>, signal: libc::c_int) -> std::io::Result<bool> {
    let Some(pid) = pid else {
        return Ok(false);
    };
    let result = unsafe { libc::kill(-(pid as libc::pid_t), signal) };
    if result == 0 {
        return Ok(true);
    }
    if std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        return Ok(false);
    }
    Err(std::io::Error::last_os_error())
}

#[cfg(not(unix))]
fn terminate_process_group(_pid: Option<u32>, _signal: libc::c_int) -> std::io::Result<bool> {
    Ok(false)
}

#[cfg(unix)]
fn process_group_exists(pid: Option<u32>) -> std::io::Result<bool> {
    let Some(pid) = pid else {
        return Ok(false);
    };
    let result = unsafe { libc::kill(-(pid as libc::pid_t), 0) };
    if result == 0 {
        return Ok(true);
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(std::io::Error::last_os_error()),
    }
}

#[cfg(not(unix))]
fn process_group_exists(_pid: Option<u32>) -> std::io::Result<bool> {
    Ok(false)
}

async fn wait_for_process_group_exit(pid: Option<u32>) -> bool {
    let deadline = tokio::time::Instant::now() + WORKER_SHUTDOWN_TIMEOUT;
    loop {
        match process_group_exists(pid) {
            Ok(false) => return true,
            Err(_) => return false,
            Ok(true) if tokio::time::Instant::now() >= deadline => return false,
            Ok(true) => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }
}

fn network_runtime_error(message: impl Into<String>) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_contract(format!(
        "network egress runtime: {}",
        message.into()
    ))
}

fn next_host_lease_id(runtime_id: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_HOST_LEASE_ID.fetch_add(1, Ordering::Relaxed);
    format!("{runtime_id}:{timestamp}:{sequence}")
}

fn startup_receipt(runtime_id: &str) -> NetworkEgressCleanupReceipt {
    NetworkEgressCleanupReceipt {
        runtime_id: runtime_id.to_string(),
        prior_pid: None,
        process_group_id: None,
        termination_signal_sent: false,
        process_tree_exited: true,
        lease_revoked: true,
        final_state: NetworkEgressWorkerState::Failed,
        reason: NetworkEgressCleanupReason::RuntimeFailure,
        cleanup_error: None,
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
    };

    use extension_package_runtime::error::PluginFrameworkError;

    use super::{
        ForwardProxyLease, NetworkEgressCleanupReason, NetworkEgressHost, NetworkEgressLeaseOwner,
        NetworkEgressWorkerConfig, NetworkEgressWorkerErrorResponse, VerifiedForwardProxyLease,
    };

    #[cfg(unix)]
    struct EgressWorkerFixture {
        root: PathBuf,
        version: String,
    }

    #[cfg(unix)]
    impl EgressWorkerFixture {
        fn new(version: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "runtime-extension-host-network-egress-generation-{}-{}",
                std::process::id(),
                super::NEXT_CONFIG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("bin")).expect("fixture bin directory must exist");
            fs::create_dir_all(root.join("provider"))
                .expect("fixture provider directory must exist");
            fs::write(
                root.join("manifest.yaml"),
                format!(
                    r#"manifest_version: 1
plugin_id: fixture-egress
publisher_namespace: test
version: {version}
vendor: Test
display_name: Fixture Egress
description: Fixture Egress
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: stateful_runtime_worker
slot_codes:
  - network_egress_provider
binding_targets:
  - workspace
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.network_egress_provider/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/fixture-egress
"#
                ),
            )
            .expect("fixture manifest must be written");
            fs::write(
                root.join("provider/egress-provider.yaml"),
                r#"provider_code: fixture-egress
display_name: Fixture Egress
form_schema:
  schema_version: 1flowbase.plugin.form/v1
  fields:
    - key: subscription_url
      label: Subscription URL
      type: string
      control: url
      required: true
      send_mode: secret
"#,
            )
            .expect("fixture provider definition must be written");
            let executable = root.join("bin/fixture-egress");
            fs::write(
                &executable,
                r#"#!/usr/bin/env bash
while IFS= read -r request; do
  case "$request" in
    *'"operation":"sync_egresses"'*)
      echo '{"operation":"sync_egresses","result":{"egresses":[]}}'
      ;;
    *'"operation":"release_http_forward_proxy"'*)
      echo '{"operation":"release_http_forward_proxy","result":{"lease_id":"worker-lease"}}'
      ;;
  esac
done
"#,
            )
            .expect("fixture executable must be written");
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
                .expect("fixture executable must be private and executable");
            Self {
                root,
                version: version.to_string(),
            }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn plugin_id(&self) -> String {
            format!("fixture-egress@{}", self.version)
        }
    }

    #[cfg(unix)]
    impl Drop for EgressWorkerFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn ac_004_provider_instances_own_isolated_workers_and_secret_files() {
        let fixture = EgressWorkerFixture::new("0.2.8");
        let mut host = NetworkEgressHost::default();
        for (runtime_id, secret) in [("provider-a", "secret-a"), ("provider-b", "secret-b")] {
            host.load_if_needed(
                runtime_id,
                &fixture.plugin_id(),
                fixture.path().to_str().unwrap(),
                "installation=0.2.8",
                NetworkEgressWorkerConfig::from_secret_json(serde_json::json!({
                    "subscription_url": secret
                })),
            )
            .await
            .expect("provider instance worker must start");
        }

        assert_eq!(host.workers.len(), 2);
        let first_generation = host.active_generations["provider-a"].clone();
        let second_generation = host.active_generations["provider-b"].clone();
        assert_ne!(first_generation, second_generation);
        let first_config =
            fs::read_to_string(host.workers[&first_generation].config_file.path()).unwrap();
        let second_config =
            fs::read_to_string(host.workers[&second_generation].config_file.path()).unwrap();
        assert!(first_config.contains("secret-a"));
        assert!(!first_config.contains("secret-b"));
        assert!(second_config.contains("secret-b"));
        assert!(!second_config.contains("secret-a"));

        host.unload("provider-a").await.unwrap();
        host.unload("provider-b").await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn ac_005_version_generation_drains_the_old_worker_after_its_last_lease() {
        let old_fixture = EgressWorkerFixture::new("0.2.5");
        let new_fixture = EgressWorkerFixture::new("0.2.8");
        let mut host = NetworkEgressHost::default();
        host.load_if_needed(
            "provider-a",
            &old_fixture.plugin_id(),
            old_fixture.path().to_str().unwrap(),
            "installation=0.2.5",
            NetworkEgressWorkerConfig::from_secret_json(serde_json::json!({
                "subscription_url": "secret-a"
            })),
        )
        .await
        .unwrap();
        let old_generation = host.active_generations["provider-a"].clone();
        host.workers
            .get_mut(&old_generation)
            .unwrap()
            .leases_by_id
            .insert(
                "worker-lease".into(),
                VerifiedForwardProxyLease {
                    lease: ForwardProxyLease {
                        lease_id: "worker-lease".into(),
                        http_proxy_url: "http://127.0.0.1:1".into(),
                        cleanup_token: "cleanup-token".into(),
                        expires_at: u64::MAX,
                    },
                },
            );
        host.lease_owners.insert(
            "host-lease".into(),
            NetworkEgressLeaseOwner {
                runtime_id: "provider-a".into(),
                generation_id: old_generation.clone(),
                worker_lease_id: "worker-lease".into(),
            },
        );

        host.load_if_needed(
            "provider-a",
            &new_fixture.plugin_id(),
            new_fixture.path().to_str().unwrap(),
            "installation=0.2.8",
            NetworkEgressWorkerConfig::from_secret_json(serde_json::json!({
                "subscription_url": "secret-a"
            })),
        )
        .await
        .unwrap();

        assert_eq!(host.workers.len(), 2, "old generation must remain leased");
        assert!(host.retired_generations.contains(&old_generation));
        assert_ne!(host.active_generations["provider-a"], old_generation);

        host.release_http_forward_proxy("provider-a", "host-lease")
            .await
            .expect("last old-generation lease must release");

        assert_eq!(host.workers.len(), 1);
        assert!(!host.workers.contains_key(&old_generation));
        assert_eq!(
            host.cleanup_receipt("provider-a").unwrap().reason,
            NetworkEgressCleanupReason::Reloaded
        );
        host.unload("provider-a").await.unwrap();
    }

    #[test]
    fn ac_clash_worker_error_envelope_accepts_only_safe_network_egress_codes() {
        let response: NetworkEgressWorkerErrorResponse = serde_json::from_str(
            r#"{"operation":"sync_egresses","error":{"code":"network_egress_proxy_invalid","message":"proxy node is invalid"}}"#,
        )
        .expect("safe error envelope should deserialize");
        response
            .validate()
            .expect("safe error envelope should validate");

        let unsafe_response: NetworkEgressWorkerErrorResponse = serde_json::from_str(
            r#"{"operation":"sync_egresses","error":{"code":"token=secret","message":"proxy node is invalid"}}"#,
        )
        .expect("unsafe fixture shape should deserialize");
        assert!(unsafe_response.validate().is_err());
    }

    #[test]
    fn ac_008_preserves_the_validated_provider_code_and_safe_summary() {
        for (code, summary) in [
            (
                "network_egress_runtime_capacity_exhausted",
                "Proxy runtime capacity is exhausted.",
            ),
            (
                "network_egress_runtime_resource_exhausted",
                "Proxy runtime could not reserve required memory.",
            ),
        ] {
            let response: NetworkEgressWorkerErrorResponse =
                serde_json::from_value(serde_json::json!({
                    "operation": "acquire_http_forward_proxy",
                    "error": {"code": code, "message": summary}
                }))
                .expect("safe runtime error envelope should deserialize");
            response
                .validate()
                .expect("safe runtime error should validate");

            let PluginFrameworkError::RuntimeContract { error } = response.into_framework_error()
            else {
                panic!("provider runtime errors must retain the runtime contract variant");
            };
            assert_eq!(error.provider_summary.as_deref(), Some(summary));
            assert_eq!(error.provider_details.as_ref().unwrap()["code"], code);
        }
    }
}
