//! A trusted logical session owns its serial child across physical generations.
//! Closed physical bindings stay fenced; dormant children retain provider-owned
//! context until the existing binding deadline, reload, or resource eviction.
//! Only confirmed child exit returns its Host-wide memory reservation.
use super::operations::{
    bind_transport_worker_locked, lock_provider_worker_registry, transport_binding_error,
};
use super::*;

mod capacity;
mod shared_capacity;
pub(super) use capacity::{SessionWorkerCapacity, SessionWorkerPermit};
pub(super) use shared_capacity::{SharedInvocationPermit, SharedWorkerCapacity};

#[derive(Debug)]
pub(super) struct LogicalSessionWorker {
    pub(super) worker: ProviderWorkerHandle,
    pub(super) generation: u64,
    dormant_since: Option<std::time::Instant>,
    dormant_until: Option<std::time::Instant>,
    expiry_task: Option<tokio::task::AbortHandle>,
    shared: bool,
}
impl LogicalSessionWorker {
    fn cancel_expiry(&mut self) {
        if let Some(task) = self.expiry_task.take() {
            task.abort();
        }
        self.dormant_since = None;
        self.dormant_until = None;
    }
    fn retire(&mut self) -> FrameworkResult<ProviderWorkerHandle> {
        self.worker.begin_quiesce()?;
        self.cancel_expiry();
        Ok(self.worker.clone())
    }
}

impl Drop for LogicalSessionWorker {
    fn drop(&mut self) {
        self.cancel_expiry();
    }
}

pub(super) fn admission_timeout() -> PluginFrameworkError {
    PluginFrameworkError::runtime(ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::ProviderTransportAdmissionFailed,
        "provider invocation admission deadline exceeded",
    ))
}

pub(super) async fn acquire_shared_invocation(
    workers: &ProviderWorkerRegistry,
    worker: &ProviderWorkerHandle,
    request: &ProviderStdioRequest,
    deadline: tokio::time::Instant,
) -> FrameworkResult<SharedInvocationPermit> {
    let capacity = lock_provider_worker_registry(workers)?
        .shared_capacity
        .clone();
    let pid = worker
        .snapshot()?
        .pid
        .ok_or_else(|| transport_binding_error("shared worker has no process"))?;
    let bytes = serde_json::to_vec(request)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))?
        .len();
    capacity.acquire(pid, bytes, deadline).await
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
    if loaded.package.manifest.runtime.protocol == extension_contracts::STDIO_JSON_MULTIPLEX_V1 {
        return select_shared_worker(workers, plugin, loaded, input, expected_epoch);
    }
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
    let key = (plugin.to_owned(), directive.logical_session_id.clone());
    let physical_key = (key.0.clone(), key.1.clone(), directive.generation);
    let capacity = lock_provider_worker_registry(workers)?
        .session_capacity
        .clone();
    let admission_started = std::time::Instant::now();
    let mut waiting_for_memory = false;
    loop {
        // Register before checking registry state: a concurrent Close must wake
        // admissions already waiting behind the all-active capacity boundary.
        let changed = capacity.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        let retiring;
        {
            let mut registry = lock_provider_worker_registry(workers)?;
            check_epoch(&registry, plugin, expected_epoch)?;
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
            super::operations::prune_transport_bindings(&mut registry);
            if let Some(worker) = existing(&registry, &physical_key)? {
                bind_transport_worker_locked(&mut registry, plugin, &worker, input)?;
                return Ok(worker);
            }
            if let Some(session) = registry.session_workers.get_mut(&key) {
                if session
                    .dormant_until
                    .is_some_and(|until| until <= std::time::Instant::now())
                {
                    let worker = session.retire()?;
                    tokio::spawn(cleanup_batch(
                        workers.clone(),
                        plugin.to_owned(),
                        vec![worker],
                    ));
                }
            }
            retiring = registry
                .session_workers
                .get(&key)
                .map(|session| {
                    session
                        .worker
                        .snapshot()
                        .map(|snapshot| snapshot.state != ProviderWorkerLifecycleState::Active)
                })
                .transpose()?
                .unwrap_or(false);
            if !retiring {
                if let Some(session) = registry.session_workers.get(&key) {
                    if directive.generation <= session.generation {
                        return Err(transport_binding_error(
                            "transport generation is already closed",
                        ));
                    }
                    if session.dormant_since.is_none() {
                        return Err(transport_binding_error(
                            "previous transport generation is still active",
                        ));
                    }
                    let worker = session.worker.clone();
                    bind_transport_worker_locked(&mut registry, plugin, &worker, input)?;
                    let session = registry
                        .session_workers
                        .get_mut(&key)
                        .expect("locked logical session");
                    session.cancel_expiry();
                    session.generation = directive.generation;
                    return Ok(worker);
                }
                if let Some(slot) =
                    capacity.try_acquire(loaded.package.manifest.runtime.limits.memory_bytes)?
                {
                    if waiting_for_memory {
                        tracing::info!(
                            plugin,
                            wait_ms = admission_started.elapsed().as_millis() as u64,
                            "session worker memory admission resumed"
                        );
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
                    registry.session_workers.insert(
                        key.clone(),
                        LogicalSessionWorker {
                            worker: worker.clone(),
                            generation: directive.generation,
                            dormant_since: None,
                            dormant_until: None,
                            expiry_task: None,
                            shared: false,
                        },
                    );
                    if let Err(error) =
                        bind_transport_worker_locked(&mut registry, plugin, &worker, input)
                    {
                        worker.begin_quiesce()?;
                        tokio::spawn(cleanup_batch(
                            workers.clone(),
                            plugin.to_owned(),
                            vec![worker],
                        ));
                        return Err(error);
                    }
                    return Ok(worker);
                }
                waiting_for_memory = true;
                // Never evict an active physical generation. A retirement stays
                // in the registry and keeps its permit until confirmed child exit.
                let oldest = registry
                    .session_workers
                    .iter()
                    .filter_map(|(key, session)| {
                        session.dormant_since.map(|since| (key.clone(), since))
                    })
                    .min_by_key(|(_, since)| *since)
                    .map(|(key, _)| key);
                if let Some(oldest) = oldest {
                    let worker = registry
                        .session_workers
                        .get_mut(&oldest)
                        .expect("locked dormant session")
                        .retire()?;
                    tokio::spawn(cleanup_batch(workers.clone(), oldest.0, vec![worker]));
                }
            }
        }
        let wait = async {
            // Memory outside this Host can become available without a worker
            // notification. Poll it at a bounded interval while honoring Close.
            tokio::select! {
                _ = &mut changed => {},
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {},
            }
        };
        tokio::time::timeout_at(deadline, wait).await.map_err(|_| {
            if waiting_for_memory {
                tracing::warn!(
                    plugin,
                    wait_ms = admission_started.elapsed().as_millis() as u64,
                    "session worker memory admission deadline exceeded"
                );
            }
            admission_timeout()
        })?;
    }
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

/// Logical sessions share the loaded artifact's worker. A binding still points to
/// its original incarnation: replacing the worker never migrates an old cursor.
fn select_shared_worker(
    workers: &ProviderWorkerRegistry,
    plugin: &str,
    loaded: &LoadedProviderPackage,
    input: &mut ProviderInvocationInput,
    expected_epoch: u64,
) -> FrameworkResult<ProviderWorkerHandle> {
    let directive = input
        .transport_session_directive()
        .map_err(PluginFrameworkError::invalid_provider_contract)?;
    let mut registry = lock_provider_worker_registry(workers)?;
    check_epoch(&registry, plugin, expected_epoch)?;
    super::operations::prune_transport_bindings(&mut registry);
    if let Some(directive) = &directive {
        let key = (plugin.to_owned(), directive.logical_session_id.clone());
        if let Some(worker) = existing(
            &registry,
            &(key.0.clone(), key.1.clone(), directive.generation),
        )? {
            bind_transport_worker_locked(&mut registry, plugin, &worker, input)?;
            return Ok(worker);
        }
        if let Some(session) = registry.session_workers.get(&key) {
            if directive.generation <= session.generation {
                return Err(transport_binding_error(
                    "transport generation is already closed",
                ));
            }
            if session.dormant_since.is_none() {
                return Err(transport_binding_error(
                    "previous transport generation is still active",
                ));
            }
        }
    }
    let worker =
        super::operations::provider_worker_handle_locked(&mut registry, plugin.to_owned(), loaded)?;
    bind_transport_worker_locked(&mut registry, plugin, &worker, input)?;
    if let Some(directive) = directive {
        registry.session_workers.insert(
            (plugin.to_owned(), directive.logical_session_id),
            LogicalSessionWorker {
                worker: worker.clone(),
                generation: directive.generation,
                dormant_since: None,
                dormant_until: None,
                expiry_task: None,
                shared: true,
            },
        );
    }
    Ok(worker)
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
    );
    let physical_key = (key.0.clone(), key.1.clone(), binding.identity.generation);
    let mut registry = lock_provider_worker_registry(workers)?;
    let current = registry
        .transport_bindings
        .get_mut(&physical_key)
        .ok_or_else(|| transport_binding_error("transport release binding disappeared"))?;
    if current.identity != binding.identity || !Arc::ptr_eq(&current.worker, &binding.worker) {
        return Err(transport_binding_error("transport release binding changed"));
    }
    current.released_receipt.get_or_insert(receipt);
    let expires_at = current.expires_at;
    let Some(session) = registry.session_workers.get_mut(&key) else {
        return Ok(());
    };
    // A repeated late release belongs only to its physical generation. It cannot
    // make a reactivated successor dormant or schedule that successor's death.
    if session.generation != binding.identity.generation
        || !Arc::ptr_eq(&session.worker, &binding.worker)
    {
        return Ok(());
    }
    let state = session.worker.snapshot()?.state;
    if state != ProviderWorkerLifecycleState::Active {
        if session.shared {
            registry.session_workers.remove(&key);
            return Ok(());
        }
        if state != ProviderWorkerLifecycleState::Quiescing {
            let worker = session.retire()?;
            tokio::spawn(cleanup_batch(
                workers.clone(),
                plugin.to_owned(),
                vec![worker],
            ));
        }
        return Ok(());
    }
    if session.dormant_since.is_some() {
        return Ok(());
    }
    session.dormant_since = Some(std::time::Instant::now());
    session.dormant_until = Some(expires_at);
    let expected_worker = Arc::downgrade(&session.worker);
    let expected_generation = session.generation;
    let weak_registry = Arc::downgrade(workers);
    let task = tokio::spawn(async move {
        tokio::time::sleep_until(tokio::time::Instant::from_std(expires_at)).await;
        let (Some(workers), Some(expected_worker)) =
            (weak_registry.upgrade(), expected_worker.upgrade())
        else {
            return;
        };
        let worker = {
            let Ok(mut registry) = lock_provider_worker_registry(&workers) else {
                return;
            };
            let Some(session) = registry.session_workers.get_mut(&key) else {
                return;
            };
            if session.generation != expected_generation
                || session.dormant_since.is_none()
                || !Arc::ptr_eq(&session.worker, &expected_worker)
            {
                return;
            }
            if session.shared {
                // Expiring one logical owner must never terminate its neighbours.
                session.expiry_task.take();
                registry.session_workers.remove(&key);
                return;
            }
            // Do not abort this timer after it becomes the cleanup owner.
            session.expiry_task.take();
            let Ok(worker) = session.retire() else {
                return;
            };
            worker
        };
        let _ = cleanup_batch(workers, key.0, vec![worker]).await;
    });
    session.expiry_task = Some(task.abort_handle());
    registry.session_capacity.changed.notify_waiters();
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
        let worker = registry
            .session_workers
            .get(&key)
            .expect("collected worker")
            .worker
            .clone();
        if !all.iter().any(|existing| Arc::ptr_eq(existing, &worker)) {
            all.push(worker);
        }
    }
    for (key, session) in &mut registry.session_workers {
        if key.0 == plugin {
            session.cancel_expiry();
        }
    }
    for worker in &all {
        worker.begin_quiesce()?;
    }
    registry.session_capacity.changed.notify_waiters();
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
        multiplex: bool,
    ) -> FrameworkResult<Option<ActiveProviderInvocationLease>> {
        let provider_pool_key = match input
            .transport_session_directive()
            .map_err(PluginFrameworkError::invalid_provider_contract)?
        {
            Some(directive) => {
                serde_json::to_string(&("logical", plugin_id, directive.logical_session_id))
                    .expect("string tuple")
            }
            None if multiplex => return Ok(None),
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
        Ok(Some(ActiveProviderInvocationLease {
            provider_pool_key,
            _permit: permit,
        }))
    }
}

fn remove_exited(
    workers: &ProviderWorkerRegistry,
    worker: &ProviderWorkerHandle,
    receipt: &ProviderWorkerCleanupReceipt,
) -> FrameworkResult<()> {
    if receipt.exited {
        let mut registry = lock_provider_worker_registry(workers)?;
        registry
            .session_workers
            .retain(|_, current| !Arc::ptr_eq(&current.worker, worker));
        registry.session_capacity.changed.notify_waiters();
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
