//! Physical sessions own independent serial stdio carriers. Capacity waits never
//! hold the registry lock; only confirmed child cleanup returns an owned slot.
//! At most 64 physical-session children share the Host's bulkhead; unbound
//! operations retain the legacy per-plugin carrier. Bindings retain the existing
//! physical-deadline receipt window, while live workers are never evicted.
//! Reload epochs invalidate queued old-package admission, and cleanup batches
//! quiesce children concurrently under the existing per-worker 5s budget.
use super::operations::{
    bind_transport_worker_locked, lock_provider_worker_registry, transport_binding_error,
};
use super::*;

#[derive(Debug)]
pub(super) struct SessionWorkerCapacity(Arc<Semaphore>);
impl Default for SessionWorkerCapacity {
    fn default() -> Self {
        Self(Arc::new(Semaphore::new(64)))
    }
}

pub(super) fn admission_timeout() -> PluginFrameworkError {
    transport_binding_error("provider invocation admission deadline exceeded")
}

pub(super) fn epoch(workers: &ProviderWorkerRegistry, plugin: &str) -> FrameworkResult<u64> {
    Ok(*lock_provider_worker_registry(workers)?
        .epochs
        .get(plugin)
        .unwrap_or(&0))
}

pub(super) async fn select_worker(
    workers: &ProviderWorkerRegistry,
    plugin: &str,
    loaded: &LoadedProviderPackage,
    input: &mut ProviderInvocationInput,
    deadline: tokio::time::Instant,
    expected_epoch: u64,
) -> FrameworkResult<ProviderWorkerHandle> {
    let directive = input
        .transport_session_directive()
        .map_err(PluginFrameworkError::invalid_provider_contract)?;
    let Some(directive) = directive else {
        let mut registry = lock_provider_worker_registry(workers)?;
        check_epoch(&registry, plugin, expected_epoch)?;
        return super::operations::provider_worker_handle_locked(
            &mut registry,
            plugin.to_owned(),
            loaded,
        );
    };
    let unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64;
    if directive.physical_deadline_unix_ms <= unix_ms {
        return Err(transport_binding_error(
            "transport binding physical deadline has expired",
        ));
    }
    let key = (
        plugin.to_owned(),
        directive.logical_session_id.clone(),
        directive.generation,
    );
    let capacity = {
        let mut registry = lock_provider_worker_registry(workers)?;
        check_epoch(&registry, plugin, expected_epoch)?;
        super::operations::prune_transport_bindings(&mut registry);
        if let Some(worker) = existing(&registry, &key)? {
            bind_transport_worker_locked(&mut registry, plugin, &worker, input)?;
            return Ok(worker);
        }
        registry.session_capacity.0.clone()
    };
    let slot = tokio::time::timeout_at(deadline, capacity.acquire_owned())
        .await
        .map_err(|_| admission_timeout())?
        .map_err(|_| transport_binding_error("session worker capacity closed"))?;
    let mut registry = lock_provider_worker_registry(workers)?;
    check_epoch(&registry, plugin, expected_epoch)?;
    super::operations::prune_transport_bindings(&mut registry);
    if let Some(worker) = existing(&registry, &key)? {
        bind_transport_worker_locked(&mut registry, plugin, &worker, input)?;
        return Ok(worker);
    }
    // Validate admission before spawning a child. Binding capacity is also bounded;
    // closed bindings retain their existing physical-deadline retention window.
    if registry.transport_bindings.len() >= super::operations::TRANSPORT_BINDING_CAPACITY {
        return Err(transport_binding_error(
            "transport worker binding capacity exhausted",
        ));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64;
    if directive.physical_deadline_unix_ms <= now {
        return Err(transport_binding_error(
            "transport binding physical deadline has expired",
        ));
    }
    let incarnation = *registry
        .next_generation
        .entry(plugin.to_owned())
        .or_insert(1);
    let worker = ProviderWorkerSupervisor::activate(
        loaded.runtime_executable.clone(),
        loaded.package.manifest.runtime.limits.clone(),
        incarnation,
    )?;
    worker.retain_capacity(slot);
    registry
        .next_generation
        .insert(plugin.to_owned(), incarnation.saturating_add(1));
    registry.session_workers.insert(key, worker.clone());
    if let Err(error) = bind_transport_worker_locked(&mut registry, plugin, &worker, input) {
        // Even a deadline crossing during synchronous process activation owns a
        // child: retain its slot until a detached cleanup owner proves exit.
        worker.begin_quiesce()?;
        tokio::spawn(cleanup_batch(
            workers.clone(),
            plugin.to_owned(),
            vec![worker],
        ));
        return Err(error);
    }
    Ok(worker)
}

fn check_epoch(
    registry: &ProviderWorkerRegistryState,
    plugin: &str,
    expected: u64,
) -> FrameworkResult<()> {
    if *registry.epochs.get(plugin).unwrap_or(&0) != expected {
        return Err(transport_binding_error(
            "provider package changed during admission",
        ));
    }
    Ok(())
}

fn existing(
    registry: &ProviderWorkerRegistryState,
    key: &(String, String, u64),
) -> FrameworkResult<Option<ProviderWorkerHandle>> {
    if let Some(binding) = registry.transport_bindings.get(key) {
        if binding.released_receipt.is_some() {
            return Err(transport_binding_error(
                "transport generation is already closed",
            ));
        }
        if binding.worker.snapshot()?.state != ProviderWorkerLifecycleState::Active {
            return Err(transport_binding_error(
                "transport generation belongs to a previous worker",
            ));
        }
        return Ok(Some(binding.worker.clone()));
    }
    Ok(None)
}

pub(super) fn release_worker(
    workers: &ProviderWorkerRegistry,
    plugin: &str,
    binding: &TransportWorkerBinding,
    receipt: ProviderTransportSessionReceipt,
) -> FrameworkResult<()> {
    let key = (
        plugin.to_owned(),
        binding.identity.logical_session_id.clone(),
        binding.identity.generation,
    );
    let worker = {
        let mut registry = lock_provider_worker_registry(workers)?;
        let current = registry
            .transport_bindings
            .get_mut(&key)
            .ok_or_else(|| transport_binding_error("transport release binding disappeared"))?;
        if current.identity != binding.identity || !Arc::ptr_eq(&current.worker, &binding.worker) {
            return Err(transport_binding_error("transport release binding changed"));
        }
        current.released_receipt.get_or_insert(receipt);
        let Some(worker) = registry.session_workers.get(&key) else {
            return Ok(());
        };
        if worker.snapshot()?.state == ProviderWorkerLifecycleState::Quiescing {
            return Ok(());
        }
        worker.begin_quiesce()?;
        worker.clone()
    };
    let workers = workers.clone();
    let plugin = plugin.to_owned();
    tokio::spawn(async move {
        if let Ok(receipt) = worker
            .finish_quiesce(
                PROVIDER_WORKER_QUIESCE_DEADLINE,
                ProviderWorkerCleanupReason::Restarted,
            )
            .await
        {
            if let Ok(mut registry) = lock_provider_worker_registry(&workers) {
                registry.cleanup_receipts.insert(plugin, receipt.clone());
                if receipt.exited
                    && registry
                        .session_workers
                        .get(&key)
                        .is_some_and(|current| Arc::ptr_eq(current, &worker))
                {
                    registry.session_workers.remove(&key);
                }
            }
        }
    });
    Ok(())
}

fn take_all(
    workers: &ProviderWorkerRegistry,
    plugin: &str,
) -> FrameworkResult<Vec<ProviderWorkerHandle>> {
    let mut registry = lock_provider_worker_registry(workers)?;
    let epoch = registry.epochs.entry(plugin.to_owned()).or_default();
    *epoch = epoch.saturating_add(1);
    let mut all = Vec::new();
    if let Some(worker) = registry.workers.remove(plugin) {
        all.push(worker);
    }
    let keys: Vec<_> = registry
        .session_workers
        .keys()
        .filter(|key| key.0 == plugin)
        .cloned()
        .collect();
    for key in keys {
        all.push(
            registry
                .session_workers
                .get(&key)
                .expect("collected worker")
                .clone(),
        );
    }
    for worker in &all {
        worker.begin_quiesce()?;
    }
    Ok(all)
}

impl ProviderHost {
    pub(super) async fn quiesce_all_provider_workers(&self) -> FrameworkResult<()> {
        // Fence every plugin and close admission before awaiting any child.
        let mut batches = Vec::new();
        for plugin in self.loaded_packages.keys() {
            batches.push((plugin.clone(), take_all(&self.provider_workers, plugin)?));
        }
        let workers = self.provider_workers.clone();
        // The detached owner joins all plugin batches even if stop_all is cancelled
        // or one batch fails. Their existing quiesce budgets run concurrently.
        tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            for (plugin, all) in batches {
                tasks.spawn(cleanup_batch(workers.clone(), plugin, all));
            }
            let mut failure = None;
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        failure.get_or_insert(error);
                    }
                    Err(_) => {
                        failure.get_or_insert_with(|| {
                            transport_binding_error("provider cleanup batch failed")
                        });
                    }
                }
            }
            match failure {
                Some(error) => Err(error),
                None => Ok(()),
            }
        })
        .await
        .map_err(|_| transport_binding_error("provider cleanup owner failed"))?
    }

    pub(super) async fn quiesce_provider_worker(
        &self,
        plugin: &str,
    ) -> FrameworkResult<Option<ProviderWorkerCleanupReceipt>> {
        let all = take_all(&self.provider_workers, plugin)?;
        let workers = self.provider_workers.clone();
        let plugin = plugin.to_owned();
        // Cleanup ownership survives cancellation of reload/unload's caller.
        tokio::spawn(cleanup_batch(workers, plugin, all))
            .await
            .map_err(|_| transport_binding_error("provider cleanup owner failed"))?
    }

    pub(super) fn retire_provider_worker_in_background(&self, plugin: &str) -> FrameworkResult<()> {
        let all = take_all(&self.provider_workers, plugin)?;
        if all.is_empty() {
            return Ok(());
        }
        let workers = self.provider_workers.clone();
        let plugin = plugin.to_owned();
        tokio::runtime::Handle::try_current()
            .map_err(|_| {
                transport_binding_error("provider worker cleanup requires an async runtime")
            })?
            .spawn(cleanup_batch(workers, plugin, all));
        Ok(())
    }
}

impl ProviderHost {
    pub(super) fn active_stream_event_observer(
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

    pub(super) async fn remove_active_stream(
        active_streams: &Arc<Mutex<HashMap<String, ActiveProviderStreamRecord>>>,
        invocation_id: &str,
    ) {
        active_streams.lock().await.remove(invocation_id);
    }

    pub(super) async fn acquire_active_invocation_lease(
        active_invocation_leases: &Arc<Mutex<HashMap<String, std::sync::Weak<Semaphore>>>>,
        plugin_id: &str,
        input: &ProviderInvocationInput,
    ) -> FrameworkResult<ActiveProviderInvocationLease> {
        let provider_pool_key = match input
            .transport_session_directive()
            .map_err(PluginFrameworkError::invalid_provider_contract)?
        {
            Some(directive) => serde_json::to_string(&(
                "physical",
                plugin_id,
                directive.logical_session_id,
                directive.generation,
            ))
            .expect("string tuple"),
            None => provider_pool_key(input),
        };
        let semaphore = {
            let mut leases = active_invocation_leases.lock().await;
            leases.retain(|_, semaphore| semaphore.strong_count() > 0);
            match leases
                .get(&provider_pool_key)
                .and_then(std::sync::Weak::upgrade)
            {
                Some(semaphore) => semaphore,
                None => {
                    let semaphore = Arc::new(Semaphore::new(1));
                    leases.insert(provider_pool_key.clone(), Arc::downgrade(&semaphore));
                    semaphore
                }
            }
        };
        tracing::debug!(
            provider_pool_key = %provider_pool_key,
            "active provider invocation lease acquiring"
        );
        let permit = semaphore.acquire_owned().await.map_err(|_| {
            PluginFrameworkError::runtime(
                extension_package_runtime::provider_contract::ProviderRuntimeError::normalize(
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
}

fn remove_exited(
    workers: &ProviderWorkerRegistry,
    worker: &ProviderWorkerHandle,
    receipt: &ProviderWorkerCleanupReceipt,
) -> FrameworkResult<()> {
    if receipt.exited {
        lock_provider_worker_registry(workers)?
            .session_workers
            .retain(|_, current| !Arc::ptr_eq(current, worker));
    }
    Ok(())
}

async fn cleanup_batch(
    workers: ProviderWorkerRegistry,
    plugin: String,
    all: Vec<ProviderWorkerHandle>,
) -> FrameworkResult<Option<ProviderWorkerCleanupReceipt>> {
    let mut tasks = tokio::task::JoinSet::new();
    for worker in all {
        let workers = workers.clone();
        let plugin = plugin.clone();
        tasks.spawn(async move {
            let receipt = worker
                .finish_quiesce(
                    PROVIDER_WORKER_QUIESCE_DEADLINE,
                    ProviderWorkerCleanupReason::Restarted,
                )
                .await?;
            remove_exited(&workers, &worker, &receipt)?;
            record_provider_worker_cleanup(&workers, &plugin, receipt.clone())?;
            Ok::<_, PluginFrameworkError>(receipt)
        });
    }
    let mut last = None;
    let mut failure = None;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(receipt)) => {
                last = Some(receipt);
            }
            Ok(Err(error)) => {
                failure.get_or_insert(error);
            }
            Err(_) => {
                failure
                    .get_or_insert_with(|| transport_binding_error("provider cleanup task failed"));
            }
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(last),
    }
}

#[cfg(test)]
impl SessionWorkerCapacity {
    pub(super) fn for_test(capacity: usize) -> Self {
        Self(Arc::new(Semaphore::new(capacity)))
    }
}
