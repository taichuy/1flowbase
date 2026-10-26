use super::*;

impl NetworkEgressWorker {
    pub(super) fn active_lease_count(&self) -> usize {
        self.leases_by_id.len()
    }

    pub(super) fn start(
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

    pub(super) async fn resolve_http_forward_proxy(
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

    pub(super) fn is_live(&mut self) -> FrameworkResult<bool> {
        Ok(self
            .child
            .try_wait()
            .map_err(|error| {
                PluginFrameworkError::io(Some(&self.executable_path), error.to_string())
            })?
            .is_none())
    }

    pub(super) async fn ensure_worker_is_live(&mut self) -> FrameworkResult<()> {
        if !self.is_live()? {
            return Err(network_runtime_error("network egress worker exited"));
        }
        Ok(())
    }

    pub(super) async fn sync_egresses(&mut self) -> FrameworkResult<SyncEgressesResult> {
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

    pub(super) async fn release_lease(&mut self, lease_id: &str) -> bool {
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

    pub(super) async fn release_all_leases(&mut self) -> bool {
        let lease_ids = self.leases_by_id.keys().cloned().collect::<Vec<_>>();
        let mut all_released = true;
        for lease_id in lease_ids {
            all_released &= self.release_lease(&lease_id).await;
        }
        all_released
    }

    pub(super) async fn call(
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

    pub(super) async fn stop(
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
        // Non-Unix platforms do not implement POSIX process-group signals. Terminate
        // the owned child handle instead of waiting forever on the no-op group adapter.
        #[cfg(not(unix))]
        if !leader_already_exited {
            match self.child.start_kill() {
                Ok(()) => termination_signal_sent = true,
                Err(error) => {
                    cleanup_error.get_or_insert_with(|| error.to_string());
                }
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
            #[cfg(unix)]
            match terminate_process_group(process_group_id, libc::SIGKILL) {
                Ok(sent) => termination_signal_sent |= sent,
                Err(error) => {
                    cleanup_error.get_or_insert_with(|| error.to_string());
                }
            }
            #[cfg(not(unix))]
            if let Err(error) = self.child.start_kill() {
                cleanup_error.get_or_insert_with(|| error.to_string());
            }
            if !leader_exited {
                if let Err(error) = self.child.wait().await {
                    cleanup_error.get_or_insert_with(|| error.to_string());
                }
            }
            #[cfg(unix)]
            {
                process_group_exited = wait_for_process_group_exit(process_group_id).await;
            }
        }
        // A reaped Windows child is not proof that all of its descendants exited.
        // Preserve that distinction until the host owns a Windows Job Object.
        #[cfg(not(unix))]
        {
            process_group_exited = false;
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
    pub(super) async fn verify(lease: ForwardProxyLease) -> FrameworkResult<Self> {
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
