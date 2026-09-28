//! Host-owned deadline queue. Admission, bindings and retirement are fenced by
//! the original supervisor and registry; timers never infer transport release.
use super::*;
use std::{sync::Weak, time::Duration};
use tokio::sync::mpsc;
use tokio_util::time::{delay_queue::Key, DelayQueue};

#[derive(Debug)]
pub(super) struct ReservedWorker {
    pub(super) worker: ProviderWorkerHandle,
    _lease: supervisor::ProviderWorkerInvocationLease,
}
impl ReservedWorker {
    pub(super) fn new(worker: ProviderWorkerHandle) -> FrameworkResult<Self> {
        let lease = worker.admit()?;
        Ok(Self {
            worker,
            _lease: lease,
        })
    }
}
impl std::ops::Deref for ReservedWorker {
    type Target = ProviderWorkerHandle;
    fn deref(&self) -> &Self::Target {
        &self.worker
    }
}

#[derive(Debug)]
pub(super) struct IdleWorkerPolicy {
    grace: Duration,
    sender: Option<mpsc::UnboundedSender<WorkerNotifier>>,
    registry: Weak<StdMutex<ProviderWorkerRegistryState>>,
    demand: HashMap<String, (u64, Option<bool>)>,
}
impl Default for IdleWorkerPolicy {
    fn default() -> Self {
        Self {
            grace: Duration::from_secs(90),
            sender: None,
            registry: Weak::new(),
            demand: HashMap::new(),
        }
    }
}
#[derive(Debug, Clone)]
pub(super) struct WorkerNotifier {
    plugin: String,
    generation: u64,
    worker: Weak<ProviderWorkerSupervisor>,
    sender: mpsc::UnboundedSender<WorkerNotifier>,
}
impl WorkerNotifier {
    pub(super) fn changed(&self) {
        let _ = self.sender.send(self.clone());
    }
}

pub(super) fn ensure_scheduler(workers: &ProviderWorkerRegistry) -> FrameworkResult<()> {
    let mut registry = operations::lock_provider_worker_registry(workers)?;
    if registry.idle.sender.is_some() {
        return Ok(());
    }
    let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
        operations::transport_binding_error("worker lifecycle requires async runtime")
    })?;
    let (sender, receiver) = mpsc::unbounded_channel();
    registry.idle.sender = Some(sender);
    registry.idle.registry = Arc::downgrade(workers);
    runtime.spawn(run(Arc::downgrade(workers), receiver));
    Ok(())
}

pub(super) fn register_worker(
    registry: &ProviderWorkerRegistryState,
    plugin: &str,
    worker: &ProviderWorkerHandle,
) {
    let Some(sender) = registry.idle.sender.as_ref() else {
        return;
    };
    let Ok(generation) = worker.incarnation() else {
        return;
    };
    let notifier = WorkerNotifier {
        plugin: plugin.to_owned(),
        generation,
        worker: Arc::downgrade(worker),
        sender: sender.clone(),
    };
    worker.attach_idle_notifier(notifier.clone());
    notifier.changed();
    // Observe a real child exit without writing to healthy stdio. Weak ownership
    // makes this observer disappear after retirement and never pins a Host.
    let weak = Arc::downgrade(worker);
    let control = worker.exit_control();
    let task = tokio::spawn(async move {
        control.wait_for_exit_notification().await;
        if let Some(worker) = weak.upgrade() {
            if worker.observe_exit().await.unwrap_or(false) {
                notifier.changed();
            }
        }
    });
    worker.retain_exit_observer(task.abort_handle());
}

async fn run(
    registry: Weak<StdMutex<ProviderWorkerRegistryState>>,
    mut receiver: mpsc::UnboundedReceiver<WorkerNotifier>,
) {
    let mut deadlines = DelayQueue::<(WorkerNotifier, u64)>::new();
    let mut keys = HashMap::<(String, u64), Key>::new();
    loop {
        tokio::select! {
            message = receiver.recv() => {
                let Some(message) = message else { return; };
                let id = (message.plugin.clone(), message.generation);
                if let Some(key) = keys.remove(&id) { deadlines.remove(&key); }
                let (Some(worker), Some(registry)) = (message.worker.upgrade(), registry.upgrade()) else { continue; };
                let Ok(state) = operations::lock_provider_worker_registry(&registry) else { continue; };
                let exited = worker.last_cleanup_receipt().ok().flatten().is_some_and(|receipt| receipt.exited);
                let lifecycle = worker.snapshot().ok().map(|snapshot| snapshot.state);
                if exited && matches!(lifecycle, Some(ProviderWorkerLifecycleState::Failed | ProviderWorkerLifecycleState::Inactive)) {
                    let retiring = lifecycle == Some(ProviderWorkerLifecycleState::Inactive) || worker.begin_quiesce().is_ok();
                    drop(state);
                    if retiring { tokio::spawn(session_workers::cleanup_batch(registry, message.plugin, vec![worker])); }
                    continue;
                }
                if let Ok(Some((revision, idle_since))) = worker.idle_revision() {
                    let grace = if state.idle.demand.get(&message.plugin).is_some_and(|(_, selectable)| *selectable == Some(false)) { Duration::ZERO } else { state.idle.grace };
                    let deadline = if grace.is_zero() { tokio::time::Instant::now() } else { tokio::time::Instant::from_std(idle_since + grace) };
                    keys.insert(id, deadlines.insert_at((message, revision), deadline));
                }
            }
            expired = std::future::poll_fn(|cx| deadlines.poll_expired(cx)), if !deadlines.is_empty() => {
                let Some(expired) = expired else { continue; };
                let (message, revision) = expired.into_inner();
                keys.remove(&(message.plugin.clone(), message.generation));
                let (Some(worker), Some(workers)) = (message.worker.upgrade(), registry.upgrade()) else { continue; };
                let retiring = {
                    let Ok(_state) = operations::lock_provider_worker_registry(&workers) else { continue; };
                    worker.incarnation().ok() == Some(message.generation) && worker.begin_idle_quiesce(revision).unwrap_or(false)
                };
                if retiring {
                    tokio::spawn(session_workers::cleanup_batch(workers, message.plugin, vec![worker]));
                }
            }
        }
    }
}

impl ProviderHost {
    pub(crate) fn with_worker_idle_grace(grace: Duration) -> Self {
        let host = Self::default();
        host.provider_workers
            .lock()
            .expect("new registry")
            .idle
            .grace = grace;
        host
    }

    pub(crate) fn reconcile_worker_demand(
        &self,
        plugin_id: &str,
        revision: u64,
        selectable: Option<bool>,
    ) -> FrameworkResult<()> {
        let mut registry = operations::lock_provider_worker_registry(&self.provider_workers)?;
        if registry
            .idle
            .demand
            .get(plugin_id)
            .is_some_and(|(current, _)| *current >= revision)
        {
            return Ok(());
        }
        registry
            .idle
            .demand
            .insert(plugin_id.to_owned(), (revision, selectable));
        if let Some(worker) = registry.workers.get(plugin_id) {
            register_worker_notification(&registry, plugin_id, worker);
        }
        // Demand reconciliation is rare and plugin-scoped; invocation paths never scan.
        for ((plugin, _), session) in &registry.session_workers {
            if plugin == plugin_id {
                register_worker_notification(&registry, plugin_id, &session.worker);
            }
        }
        Ok(())
    }

    pub(crate) async fn transport_worker_exit_evidence(
        &self,
        plugin_id: &str,
        logical_session_id: &str,
        generation: u64,
    ) -> FrameworkResult<Option<ProviderTransportClosureEvidence>> {
        let binding = operations::lock_provider_worker_registry(&self.provider_workers)?
            .transport_bindings
            .get(&(
                plugin_id.to_owned(),
                logical_session_id.to_owned(),
                generation,
            ))
            .cloned();
        let Some(binding) = binding else {
            return Ok(None);
        };
        if binding.worker.incarnation()? != binding.identity.worker_incarnation
            || !binding.worker.confirmed_process_exit().await
        {
            return Ok(None);
        }
        binding.worker.observe_exit().await?;
        if let Some(receipt) =
            operations::confirmed_exit_receipt(&binding, ProviderTransportSessionAction::Close)?
        {
            session_workers::release_worker(&self.provider_workers, plugin_id, &binding, receipt)?;
        }
        Ok(Some(ProviderTransportClosureEvidence {
            identity: binding.identity,
            source: ProviderTransportClosureSource::ConfirmedWorkerExit,
            local_released: true,
            peer_close_acknowledged: None,
            no_ack_reason: Some(ProviderTransportNoAckReason::Unknown),
        }))
    }
}

fn register_worker_notification(
    registry: &ProviderWorkerRegistryState,
    plugin: &str,
    worker: &ProviderWorkerHandle,
) {
    if let (Some(sender), Ok(generation)) = (&registry.idle.sender, worker.incarnation()) {
        WorkerNotifier {
            plugin: plugin.to_owned(),
            generation,
            worker: Arc::downgrade(worker),
            sender: sender.clone(),
        }
        .changed();
    }
}
