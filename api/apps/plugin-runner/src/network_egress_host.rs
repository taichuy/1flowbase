use std::{
    collections::HashMap,
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

use axum::http::Uri;
use plugin_framework::{
    error::{FrameworkResult, PluginFrameworkError},
    AcquireHttpForwardProxyInput, EgressAvailability, EgressDescriptor, ForwardProxyLease,
    NetworkEgressProviderPackage, NetworkEgressProviderStdioRequest,
    NetworkEgressProviderStdioResponse, PluginRuntimeLimits, ReleaseHttpForwardProxyInput,
    SyncEgressesInput, SyncEgressesResult,
};
use serde::Serialize;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::TcpStream,
    process::{Child, ChildStdin, ChildStdout, Command},
};

const LEASE_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(1);
const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
static NEXT_CONFIG_ID: AtomicU64 = AtomicU64::new(0);

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
    pub plugin_id: String,
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
    workers: HashMap<String, NetworkEgressWorker>,
    sources: HashMap<String, NetworkEgressSource>,
    cleanup_receipts: HashMap<String, NetworkEgressCleanupReceipt>,
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
    /// Registers the stateful runtime worker only. A lease is intentionally not acquired until
    /// the resolver supplies the provider-owned egress key selected by the caller.
    pub async fn load_if_needed(
        &mut self,
        plugin_id: &str,
        package_root: &str,
        source_identity: &str,
        config: NetworkEgressWorkerConfig,
    ) -> FrameworkResult<()> {
        let requested = NetworkEgressSource::resolve(package_root, source_identity)?;
        if self.sources.get(plugin_id) == Some(&requested) {
            return self.ensure_worker_is_live(plugin_id).await;
        }

        self.unload_with_reason(plugin_id, NetworkEgressCleanupReason::Reloaded)
            .await?;
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
        .map_err(|error| {
            self.cleanup_receipts
                .insert(plugin_id.to_string(), startup_receipt(plugin_id));
            error
        })?;
        self.sources.insert(plugin_id.to_string(), requested);
        self.workers.insert(plugin_id.to_string(), worker);
        Ok(())
    }

    /// Resolves and validates a fresh HTTP forward-proxy lease for a caller-selected egress.
    /// The provider configuration never crosses this boundary.
    pub async fn resolve_http_forward_proxy(
        &mut self,
        plugin_id: &str,
        provider_egress_key: &str,
    ) -> FrameworkResult<ForwardProxyLease> {
        let worker = self.workers.get_mut(plugin_id).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "network egress package is not loaded: {plugin_id}"
            ))
        })?;
        let result = worker.resolve_http_forward_proxy(provider_egress_key).await;
        if result.is_err() && !worker.is_live()? {
            self.retire_failed_worker(plugin_id).await;
        }
        result
    }

    /// Returns the provider-owned egress catalog through the validated v1 worker operation.
    /// Descriptors deliberately contain only display and availability data, never provider
    /// configuration, proxy capabilities, or secrets.
    pub async fn sync_egresses(
        &mut self,
        plugin_id: &str,
    ) -> FrameworkResult<Vec<EgressDescriptor>> {
        let worker = self.workers.get_mut(plugin_id).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "network egress package is not loaded: {plugin_id}"
            ))
        })?;
        let result = worker.sync_egresses().await.map(|result| result.egresses);
        if result.is_err() && !worker.is_live()? {
            self.retire_failed_worker(plugin_id).await;
        }
        result
    }

    /// Releases the current lease after a Core consumer finishes its HTTP request. The opaque
    /// cleanup capability remains inside the worker so callers cannot retain or replay it.
    pub async fn release_http_forward_proxy(&mut self, plugin_id: &str) -> FrameworkResult<()> {
        let worker = self.workers.get_mut(plugin_id).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(format!(
                "network egress package is not loaded: {plugin_id}"
            ))
        })?;
        if worker.release_lease().await {
            Ok(())
        } else {
            Err(network_runtime_error(
                "network egress lease release did not return a matching receipt",
            ))
        }
    }

    pub async fn unload(&mut self, plugin_id: &str) -> FrameworkResult<()> {
        self.unload_with_reason(plugin_id, NetworkEgressCleanupReason::Stopped)
            .await
    }

    pub fn cleanup_receipt(&self, plugin_id: &str) -> Option<&NetworkEgressCleanupReceipt> {
        self.cleanup_receipts.get(plugin_id)
    }

    async fn ensure_worker_is_live(&mut self, plugin_id: &str) -> FrameworkResult<()> {
        let is_live = self
            .workers
            .get_mut(plugin_id)
            .ok_or_else(|| {
                PluginFrameworkError::invalid_provider_package(format!(
                    "network egress package is not loaded: {plugin_id}"
                ))
            })?
            .is_live()?;
        if is_live {
            return Ok(());
        }
        self.retire_failed_worker(plugin_id).await;
        Err(network_runtime_error("network egress worker exited"))
    }

    async fn retire_failed_worker(&mut self, plugin_id: &str) {
        if let Some(worker) = self.workers.remove(plugin_id) {
            let receipt = worker
                .stop(plugin_id, NetworkEgressCleanupReason::RuntimeFailure)
                .await;
            self.cleanup_receipts.insert(plugin_id.to_string(), receipt);
        }
        self.sources.remove(plugin_id);
    }

    async fn unload_with_reason(
        &mut self,
        plugin_id: &str,
        reason: NetworkEgressCleanupReason,
    ) -> FrameworkResult<()> {
        if let Some(worker) = self.workers.remove(plugin_id) {
            let receipt = worker.stop(plugin_id, reason).await;
            let release_failed = receipt.cleanup_error.is_some();
            self.cleanup_receipts.insert(plugin_id.to_string(), receipt);
            self.sources.remove(plugin_id);
            if release_failed {
                return Err(network_runtime_error(
                    "network egress worker cleanup did not receive a lease release receipt",
                ));
            }
        }
        self.sources.remove(plugin_id);
        Ok(())
    }
}

struct NetworkEgressWorker {
    executable_path: PathBuf,
    limits: PluginRuntimeLimits,
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    lease: Option<VerifiedForwardProxyLease>,
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
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            lease: None,
            config_file,
        })
    }

    async fn resolve_http_forward_proxy(
        &mut self,
        provider_egress_key: &str,
    ) -> FrameworkResult<ForwardProxyLease> {
        self.ensure_lease_is_current().await?;
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
            _ => return Err(PluginFrameworkError::invalid_provider_contract(
                "network egress worker returned the wrong result for acquire_http_forward_proxy",
            )),
        };
        let verified = VerifiedForwardProxyLease::verify(lease).await?;
        let lease = verified.lease.clone();
        self.lease = Some(verified);
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

    async fn ensure_lease_is_current(&mut self) -> FrameworkResult<()> {
        if !self.is_live()? {
            return Err(network_runtime_error("network egress worker exited"));
        }
        if self.lease.as_ref().is_some_and(|lease| lease.is_expired()) {
            self.lease = None;
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

    async fn release_lease(&mut self) -> bool {
        let Some(lease) = self.lease.take() else {
            return true;
        };
        let response = self
            .call(NetworkEgressProviderStdioRequest::ReleaseHttpForwardProxy(
                ReleaseHttpForwardProxyInput {
                    lease_id: lease.lease.lease_id.clone(),
                    cleanup_token: lease.lease.cleanup_token.clone(),
                },
            ))
            .await;
        matches!(
            response,
            Ok(NetworkEgressProviderStdioResponse::ReleaseHttpForwardProxy(receipt))
                if receipt.lease_id == lease.lease.lease_id
        )
    }

    async fn call(
        &mut self,
        request: NetworkEgressProviderStdioRequest,
    ) -> FrameworkResult<NetworkEgressProviderStdioResponse> {
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
        plugin_id: &str,
        reason: NetworkEgressCleanupReason,
    ) -> NetworkEgressCleanupReceipt {
        let lease_revoked = self.lease.is_some();
        let release_acknowledged = self.release_lease().await;
        let prior_pid = self.child.id();
        let mut termination_signal_sent = false;
        let mut cleanup_error = (lease_revoked && !release_acknowledged).then_some(
            "network egress lease release did not return a matching receipt".to_string(),
        );
        let process_tree_exited = match self.child.try_wait() {
            Ok(Some(_)) => true,
            Ok(None) => {
                match terminate_process_group(prior_pid, libc::SIGTERM) {
                    Ok(sent) => termination_signal_sent = sent,
                    Err(error) => {
                        cleanup_error.get_or_insert_with(|| error.to_string());
                    }
                }
                match tokio::time::timeout(WORKER_SHUTDOWN_TIMEOUT, self.child.wait()).await {
                    Ok(Ok(_)) => true,
                    Ok(Err(error)) => {
                        cleanup_error.get_or_insert_with(|| error.to_string());
                        false
                    }
                    Err(_) => {
                        match terminate_process_group(prior_pid, libc::SIGKILL) {
                            Ok(sent) => termination_signal_sent |= sent,
                            Err(error) => {
                                cleanup_error.get_or_insert_with(|| error.to_string());
                            }
                        }
                        self.child.wait().await.is_ok()
                    }
                }
            }
            Err(error) => {
                cleanup_error = Some(error.to_string());
                false
            }
        };
        if !self.config_file.cleanup() {
            cleanup_error.get_or_insert_with(|| {
                "network egress private configuration cleanup failed".to_string()
            });
        }
        NetworkEgressCleanupReceipt {
            plugin_id: plugin_id.to_string(),
            prior_pid,
            process_group_id: prior_pid,
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

    fn is_expired(&self) -> bool {
        self.lease.expires_at <= unix_milliseconds_now()
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
    if result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        return Ok(true);
    }
    Err(std::io::Error::last_os_error())
}

#[cfg(not(unix))]
fn terminate_process_group(_pid: Option<u32>, _signal: libc::c_int) -> std::io::Result<bool> {
    Ok(false)
}

fn network_runtime_error(message: impl Into<String>) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_contract(format!(
        "network egress runtime: {}",
        message.into()
    ))
}

fn startup_receipt(plugin_id: &str) -> NetworkEgressCleanupReceipt {
    NetworkEgressCleanupReceipt {
        plugin_id: plugin_id.to_string(),
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
