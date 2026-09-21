use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as StdMutex, Weak,
    },
    time::Duration,
};

use orchestration_runtime::transport_session::{
    AdmissionRequest, DeadlineKind, InvocationCompletion, InvocationLease, InvocationRequest,
    LifecycleEvent, RegistryError, SafeRegistrySnapshot, SystemTransportClock, TerminationKind,
    TransportClock, TransportFence, TransportFenceStatus, TransportInstant, TransportOwnerId,
    TransportProviderId, TransportRegistryConfig, TransportRuntimeTargetId, TransportSessionId,
    TransportSessionRegistry, TransportSessionState,
};
use plugin_framework::{
    provider_contract::{
        CommitLevel, CursorBinding, ProviderInvocationInput,
        ProviderInvocationTransportClassification, ProviderInvocationTransportOutcome,
        ProviderLogicalSessionState, ProviderRecoveryDirective, ProviderRuntimeError,
        ProviderRuntimeErrorKind, ProviderTransportClosureEvidence,
        ProviderTransportClosureOutcome, ProviderTransportSessionAction,
        ProviderTransportSessionCommand, ProviderTransportSessionDirective,
        ProviderTransportSessionIdentity, ProviderTransportSessionReceipt, ProviderWireOperation,
        RecoveryTransport,
    },
    PluginFrameworkError,
};
use runtime_core::runtime_backend::{RuntimeBackend, RuntimeBackendError};
use sha2::{Digest, Sha256};
use tokio::sync::{broadcast, Mutex, Notify};

use super::ProviderRuntimeExecutionContext;

const CONTROL_DEADLINE: Duration = Duration::from_secs(5);
const CONTROL_OVERALL_DEADLINE: Duration = Duration::from_secs(30);
const CONTROL_MAX_ATTEMPTS: u8 = 5;
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(1);
const HANDOFF_MAX_WAIT: Duration = Duration::from_secs(30);

fn handoff_wait_budget(now: TransportInstant, deadline: Option<TransportInstant>) -> Duration {
    deadline
        .map(|deadline| Duration::from_millis(deadline.as_millis().saturating_sub(now.as_millis())))
        .unwrap_or(HANDOFF_MAX_WAIT)
        .min(HANDOFF_MAX_WAIT)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TransportTerminationNotice {
    pub(crate) owner_id: String,
    pub(crate) fence: TransportFence,
    pub(crate) invocation_sequence: Option<u64>,
    pub(crate) connection_scope_id: Option<String>,
    pub(crate) code: &'static str,
}

/// A socket owns only leases actually admitted for it, never all sessions with
/// the same client thread identity. This is a bounded attachment to the registry.
pub(crate) struct TransportConnectionScope {
    id: String,
    state: StdMutex<TransportConnectionBindings>,
}

#[derive(Default)]
struct TransportConnectionBindings {
    closed: bool,
    leases: BTreeMap<String, InvocationLease>,
}

impl TransportConnectionScope {
    #[cfg(test)]
    pub(crate) fn for_test(lease: InvocationLease) -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::now_v7().to_string(),
            state: StdMutex::new(TransportConnectionBindings {
                closed: false,
                leases: BTreeMap::from([(lease.fence.session_id.as_str().into(), lease)]),
            }),
        })
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn accepts_termination(&self, notice: &TransportTerminationNotice) -> bool {
        let state = self.state.lock().expect("transport connection bindings");
        !state.closed
            && notice.connection_scope_id.as_deref() == Some(self.id.as_str())
            && state
                .leases
                .get(notice.fence.session_id.as_str())
                .is_some_and(|lease| {
                    lease.fence == notice.fence
                        && Some(lease.sequence()) == notice.invocation_sequence
                })
    }
}

pub(crate) fn take_transport_connection_scope(
    input: &mut ProviderInvocationInput,
) -> Option<String> {
    input
        .client_protocol_envelope
        .as_mut()?
        .headers
        .remove(control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER)?
        .into_iter()
        .next()
}

struct LifecycleCommand {
    fence: TransportFence,
    target_id: String,
    command: ProviderTransportSessionCommand,
    termination: Option<TransportTerminationNotice>,
    terminal_close: bool,
    overall_deadline: TransportInstant,
    next_due: TransportInstant,
    attempts: u8,
    state: CloseTaskState,
    last_blocker: Option<&'static str>,
    first_control_failure: Option<&'static str>,
    primary_failure: Option<PrimaryTransportFailure>,
    task_id: String,
}

#[derive(Clone, Copy, Debug)]
struct PrimaryTransportFailure {
    kind: Option<ProviderRuntimeErrorKind>,
    code: &'static str,
    invocation_sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CloseTaskState {
    Pending,
    Released,
    Exhausted,
}

pub(crate) struct PreparedTransportInvocation {
    lease: InvocationLease,
    transport: RecoveryTransport,
    recovery_directive: Option<ProviderRecoveryDirective>,
}

#[async_trait::async_trait]
trait TransportLifecycleRuntime: Send + Sync {
    async fn transport_session(
        &self,
        target_id: &str,
        command: ProviderTransportSessionCommand,
    ) -> Result<ProviderTransportSessionReceipt, RuntimeBackendError>;
}

struct RuntimeBackendTransportLifecycle(Arc<dyn RuntimeBackend>);

#[async_trait::async_trait]
impl TransportLifecycleRuntime for RuntimeBackendTransportLifecycle {
    async fn transport_session(
        &self,
        target_id: &str,
        command: ProviderTransportSessionCommand,
    ) -> Result<ProviderTransportSessionReceipt, RuntimeBackendError> {
        self.0.provider_transport_session(target_id, command).await
    }
}

pub(crate) struct TransportSessionCoordinator<C = SystemTransportClock> {
    registry: Mutex<TransportSessionRegistry<C>>,
    runtime: Arc<dyn TransportLifecycleRuntime>,
    notices: broadcast::Sender<TransportTerminationNotice>,
    shutdown: AtomicBool,
    shutdown_notify: Notify,
    invocation_changed: Notify,
    scheduler: StdMutex<Option<tokio::task::JoinHandle<()>>>,
    dispatcher: Mutex<()>,
    pending_commands: StdMutex<VecDeque<LifecycleCommand>>,
    connection_scopes: StdMutex<BTreeMap<String, Weak<TransportConnectionScope>>>,
}

impl TransportSessionCoordinator<SystemTransportClock> {
    pub(crate) fn new(
        runtime: Arc<dyn RuntimeBackend>,
        config: TransportRegistryConfig,
    ) -> anyhow::Result<Self> {
        Self::new_with_clock(
            Arc::new(RuntimeBackendTransportLifecycle(runtime)),
            config,
            SystemTransportClock::default(),
        )
    }
}

impl<C: TransportClock + 'static> TransportSessionCoordinator<C> {
    fn new_with_clock(
        runtime: Arc<dyn TransportLifecycleRuntime>,
        config: TransportRegistryConfig,
        clock: C,
    ) -> anyhow::Result<Self> {
        let registry = TransportSessionRegistry::new(clock, config)?;
        let (notices, _) = broadcast::channel(256);
        Ok(Self {
            registry: Mutex::new(registry),
            runtime,
            notices,
            shutdown: AtomicBool::new(false),
            shutdown_notify: Notify::new(),
            invocation_changed: Notify::new(),
            scheduler: StdMutex::new(None),
            dispatcher: Mutex::new(()),
            pending_commands: StdMutex::new(VecDeque::new()),
            connection_scopes: StdMutex::new(BTreeMap::new()),
        })
    }

    pub(crate) fn start(self: &Arc<Self>) {
        let mut scheduler = self.scheduler.lock().expect("transport scheduler lock");
        if scheduler.is_some() {
            return;
        }
        let coordinator = Arc::clone(self);
        *scheduler = Some(tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(MAINTENANCE_INTERVAL) => {
                        coordinator.maintain_and_dispatch().await;
                    }
                    _ = coordinator.shutdown_notify.notified() => break,
                }
            }
        }));
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<TransportTerminationNotice> {
        self.notices.subscribe()
    }

    pub(crate) fn open_connection_scope(&self) -> Arc<TransportConnectionScope> {
        let scope = Arc::new(TransportConnectionScope {
            id: uuid::Uuid::now_v7().to_string(),
            state: StdMutex::new(TransportConnectionBindings::default()),
        });
        let mut scopes = self.connection_scopes.lock().expect("transport scopes");
        scopes.retain(|_, scope| scope.strong_count() > 0);
        scopes.insert(scope.id.clone(), Arc::downgrade(&scope));
        scope
    }

    pub(crate) async fn close_connection_scope(&self, scope: &TransportConnectionScope) {
        let _dispatcher = self.dispatcher.lock().await;
        let leases = {
            let mut state = scope.state.lock().expect("transport connection bindings");
            state.closed = true;
            std::mem::take(&mut state.leases)
        };
        self.connection_scopes
            .lock()
            .expect("transport scopes")
            .remove(scope.id());
        let mut registry = self.registry.lock().await;
        for lease in leases.values() {
            if registry.fence_status(&lease.fence) == TransportFenceStatus::Current {
                // Only still-bound inflight work belongs to this disconnected socket.
                // Completed text/tool calls were detached by finish.
                let _ = registry.mark_invocation_orphaned(lease);
            }
        }
        self.invocation_changed.notify_waiters();
    }

    fn detach_lease(&self, lease: &InvocationLease) {
        let mut scopes = self.connection_scopes.lock().expect("transport scopes");
        scopes.retain(|_, scope| {
            let Some(scope) = scope.upgrade() else {
                return false;
            };
            let mut state = scope.state.lock().expect("transport connection bindings");
            let key = lease.fence.session_id.as_str();
            if state.leases.get(key).is_some_and(|bound| bound == lease) {
                state.leases.remove(key);
            }
            true
        });
    }

    /// A read-only liveness check also used before cancellable handoff lock waits.
    /// Closing a scope is still serialized by the dispatcher; until that close
    /// is committed, the handoff deadline bounds contention rather than claiming
    /// that the scope has already closed.
    fn admission_scope(
        &self,
        connection_scope_id: Option<&str>,
        session_id: &TransportSessionId,
    ) -> anyhow::Result<Option<Arc<TransportConnectionScope>>> {
        let scope_requested = connection_scope_id.is_some();
        if self.shutdown.load(Ordering::Acquire) {
            return Err(admission_error(
                "transport_session_shutdown",
                admission_details(session_id, None, None, None, scope_requested),
            ));
        }
        let connection_scope = match connection_scope_id {
            Some(id) => {
                let scope = self
                    .connection_scopes
                    .lock()
                    .expect("transport scopes")
                    .get(id)
                    .and_then(Weak::upgrade);
                let Some(scope) = scope else {
                    // The request names a delivery scope this host no longer owns:
                    // either it was already closed, or the reference is stale.
                    return Err(admission_error(
                        "transport_session_scope_unknown",
                        admission_details(session_id, None, None, None, true),
                    ));
                };
                if scope
                    .state
                    .lock()
                    .expect("transport connection bindings")
                    .closed
                {
                    return Err(admission_error(
                        "transport_session_scope_closed",
                        admission_details(session_id, None, None, None, true),
                    ));
                }
                Some(scope)
            }
            None => None,
        };
        Ok(connection_scope)
    }

    pub(crate) async fn prepare(
        &self,
        target_id: &str,
        input: &mut ProviderInvocationInput,
        context: &ProviderRuntimeExecutionContext,
    ) -> anyhow::Result<Option<PreparedTransportInvocation>> {
        let connection_scope_id = take_transport_connection_scope(input);
        let Some(protocol_session_id) = protocol_session_id(input) else {
            return Ok(None);
        };
        let transport = selected_responses_transport(input)?;
        let recovery_directive = input
            .recovery_directive()
            .map_err(|_| transport_error("provider_recovery_directive_invalid"))?;
        let scope_requested = connection_scope_id.is_some();

        let owner_id = TransportOwnerId::new(protocol_session_id.to_string())?;
        let session_id = TransportSessionId::new(hash_identity(&[
            protocol_session_id,
            &context.workspace_id.to_string(),
            context
                .actor_id
                .map(|id| id.to_string())
                .as_deref()
                .unwrap_or("anonymous"),
            &input.provider_instance_id,
            &input.protocol,
            &input.model,
        ]))?;
        if self.shutdown.load(Ordering::Acquire) {
            return Err(admission_error(
                "transport_session_shutdown",
                admission_details(&session_id, None, None, None, scope_requested),
            ));
        }
        let target = TransportRuntimeTargetId::new(target_id.to_string())?;
        let provider_id = TransportProviderId::new(input.provider_instance_id.clone())?;
        let invocation_deadline = u64::try_from(context.deadline_unix_ms)
            .ok()
            .map(TransportInstant::from_millis);

        let mut handoff_deadline = None;
        loop {
            // Register before observing state; completion between inspection and await
            // must wake this waiter. Never retain registry/dispatcher locks while waiting.
            let changed = self.invocation_changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let _dispatcher = if let Some(deadline) = handoff_deadline {
                let _ = self.admission_scope(connection_scope_id.as_deref(), &session_id)?;
                if tokio::time::Instant::now() >= deadline {
                    return Err(transport_error("transport_invocation_deadline_exceeded"));
                }
                tokio::select! {
                    result = tokio::time::timeout_at(deadline, self.dispatcher.lock()) => {
                        result.map_err(|_| transport_error("transport_invocation_deadline_exceeded"))?
                    }
                    _ = changed.as_mut() => continue,
                }
            } else {
                self.dispatcher.lock().await
            };
            let connection_scope =
                self.admission_scope(connection_scope_id.as_deref(), &session_id)?;
            if handoff_deadline.is_none() {
                // Initial admission retains its existing control owner. A resumed
                // handoff must not drain or await provider Close work: cancelling
                // that dispatched future could lose its command/stdio retirement.
                // Existing finish/maintenance/shutdown owners dispatch those events.
                self.registry.lock().await.maintain();
                self.dispatch_pending_events_locked().await;
            }

            let mut registry = if let Some(deadline) = handoff_deadline {
                tokio::select! {
                    result = tokio::time::timeout_at(deadline, self.registry.lock()) => {
                        result.map_err(|_| transport_error("transport_invocation_deadline_exceeded"))?
                    }
                    _ = changed.as_mut() => continue,
                }
            } else {
                self.registry.lock().await
            };
            if handoff_deadline.is_some() {
                // Leave any newly generated lifecycle events queued for their
                // existing owner; admission still checks the maintained state.
                registry.maintain();
            }
            let now = registry.safe_snapshot().observed_at;
            if invocation_deadline.is_some_and(|deadline| deadline <= now)
                || handoff_deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
            {
                return Err(transport_error("transport_invocation_deadline_exceeded"));
            }
            let fence = if let Some(fence) = registry.fence(&session_id) {
                let state = registry.state(&fence)?;
                if state == TransportSessionState::Orphaned {
                    // Unbinding the delivery is a fact, not a verdict. Only the
                    // execution state decides whether a successor may start here, and
                    // a second execution is never opened while its predecessor runs.
                    // A replacement delivery with the same owner identity and target
                    // may wait for actual completion. Waking restarts admission; the
                    // registry's sequence and inflight checks still arbitrate racers.
                    let inflight = registry.invocation_inflight(&fence)?;
                    if inflight {
                        if registry.runtime_target_id(&fence)? == &target
                            && connection_scope.is_some()
                        {
                            let deadline = *handoff_deadline.get_or_insert_with(|| {
                                tokio::time::Instant::now()
                                    + handoff_wait_budget(now, invocation_deadline)
                            });
                            drop(registry);
                            drop(_dispatcher);
                            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                                return Err(transport_error(
                                    "transport_invocation_deadline_exceeded",
                                ));
                            }
                            continue;
                        }
                        return Err(admission_error(
                            "transport_session_inflight_unbound",
                            admission_details(
                                &session_id,
                                Some(&fence),
                                Some(state),
                                Some(true),
                                true,
                            ),
                        ));
                    }
                }
                if registry.runtime_target_id(&fence)? != &target {
                    return Err(admission_error(
                        "transport_session_evicted",
                        admission_details(
                            &session_id,
                            Some(&fence),
                            Some(state),
                            None,
                            scope_requested,
                        ),
                    ));
                }
                if matches!(
                    state,
                    TransportSessionState::Faulted | TransportSessionState::IdleReleased
                ) {
                    validate_fault_successor(input, recovery_directive.as_ref(), now)?;
                    if self.close_task_exhausted(&fence) {
                        return Err(self.recovery_admission_error(
                            &fence,
                            "provider_physical_connection_close_exhausted",
                        ));
                    }
                    // Only the next invocation gets a fresh physical generation.
                    // Rotation keeps the original logical deadline and sequence.
                    let next = registry.rotate_generation(&fence).map_err(|error| {
                        if matches!(error, RegistryError::InvalidTransition { .. }) {
                            self.recovery_admission_error(
                                &fence,
                                "provider_physical_connection_close_pending",
                            )
                        } else {
                            map_registry_use_error(error)
                        }
                    })?;
                    registry.activate(&next)?;
                    next
                } else {
                    if matches!(
                        state,
                        TransportSessionState::Draining | TransportSessionState::Closing
                    ) {
                        if self.close_task_exhausted(&fence) {
                            return Err(self.recovery_admission_error(
                                &fence,
                                "provider_physical_connection_close_exhausted",
                            ));
                        }
                        return Err(self.recovery_admission_error(
                            &fence,
                            if state == TransportSessionState::Draining {
                                "provider_connection_draining"
                            } else {
                                "provider_connection_closing"
                            },
                        ));
                    }
                    fence
                }
            } else if let Some(receipt) = registry.tombstone(&session_id) {
                return Err(
                    self.recovery_admission_error(&receipt.fence, termination_code(receipt.kind))
                );
            } else {
                let fence = registry
                    .admit(AdmissionRequest {
                        session_id: session_id.clone(),
                        owner_id,
                        provider_id,
                        runtime_target_id: target,
                        provider_hard_deadline: None,
                    })
                    .map_err(map_registry_admission_error)?;
                registry.activate(&fence)?;
                fence
            };
            let lease = registry
                .begin_invocation(
                    &fence,
                    InvocationRequest {
                        deadline: invocation_deadline,
                    },
                )
                .map_err(map_registry_use_error)?;
            let physical_deadline_unix_ms =
                i64::try_from(registry.physical_hard_deadline(&fence)?.as_millis())
                    .unwrap_or(i64::MAX);
            input
                .set_transport_session_directive(ProviderTransportSessionDirective {
                    worker_incarnation: None,
                    logical_session_id: session_id.as_str().to_string(),
                    generation: fence.generation.get(),
                    task_id: format!("invocation-{}-{}", fence.generation.get(), lease.sequence()),
                    state: ProviderLogicalSessionState::Active,
                    physical_deadline_unix_ms,
                })
                .map_err(|_| transport_error("transport_session_directive_invalid"))?;
            drop(registry);
            {
                let mut scopes = self.connection_scopes.lock().expect("transport scopes");
                scopes.retain(|_, scope| {
                    let Some(scope) = scope.upgrade() else {
                        return false;
                    };
                    scope
                        .state
                        .lock()
                        .expect("transport connection bindings")
                        .leases
                        .remove(lease.fence.session_id.as_str());
                    true
                });
            }
            if let Some(scope) = connection_scope {
                scope
                    .state
                    .lock()
                    .expect("transport connection bindings")
                    .leases
                    .insert(lease.fence.session_id.as_str().into(), lease.clone());
            }
            self.dispatch_pending_events_locked().await;
            return Ok(Some(PreparedTransportInvocation {
                lease,
                transport,
                recovery_directive: recovery_directive.clone(),
            }));
        }
    }

    pub(crate) async fn finish(
        &self,
        prepared: PreparedTransportInvocation,
        result: &anyhow::Result<super::ProviderRuntimeInvocationOutput>,
    ) -> anyhow::Result<()> {
        self.detach_lease(&prepared.lease);
        let fence_status = self
            .registry
            .lock()
            .await
            .fence_status(&prepared.lease.fence);
        if matches!(fence_status, TransportFenceStatus::Stale { .. }) {
            tracing::warn!(
                ?fence_status,
                generation = prepared.lease.fence.generation.get(),
                "old provider transport generation was fenced without changing current state"
            );
            return Err(transport_error("provider_transport_stale_generation"));
        }
        let completion = match result {
            Ok(output) => {
                let classification = match output.result.transport_outcome_for_mode(
                    prepared.transport,
                    prepared.lease.fence.generation.get(),
                    prepared.recovery_directive.as_ref(),
                ) {
                    Ok(outcome) => outcome,
                    Err(reason) => {
                        tracing::warn!(
                            generation = prepared.lease.fence.generation.get(),
                            has_recovery_directive = prepared.recovery_directive.is_some(),
                            reason = %reason,
                            "provider transport receipt could not be classified"
                        );
                        self.fault_invocation(&prepared.lease, None).await;
                        return Err(transport_error("provider_transport_receipt_invalid"));
                    }
                };
                if let ProviderInvocationTransportClassification::Session(outcome) = classification
                {
                    match outcome {
                        ProviderInvocationTransportOutcome::Ready { .. }
                        | ProviderInvocationTransportOutcome::HttpFallback { .. } => {}
                        ProviderInvocationTransportOutcome::ReceiptMissing => {
                            self.fault_invocation(&prepared.lease, None).await;
                            return Err(transport_error("provider_transport_receipt_missing"));
                        }
                        ProviderInvocationTransportOutcome::StaleGeneration { .. } => {
                            let registry = self.registry.lock().await;
                            let status = registry.fence_status(&prepared.lease.fence);
                            drop(registry);
                            tracing::warn!(
                            ?status,
                            generation = prepared.lease.fence.generation.get(),
                            "stale provider transport result was fenced without changing session state"
                        );
                            return Err(transport_error("provider_transport_stale_generation"));
                        }
                        ProviderInvocationTransportOutcome::PhysicalConnectionFault { .. } => {
                            self.fault_invocation(&prepared.lease, None).await;
                            return Err(transport_error("provider_physical_connection_fault"));
                        }
                    }
                }
                if output.result.tool_calls.is_empty() {
                    InvocationCompletion::IdleAffinity
                } else {
                    InvocationCompletion::WaitingTool
                }
            }
            Err(error) => {
                self.fault_invocation(
                    &prepared.lease,
                    Some(primary_transport_failure(error, prepared.lease.sequence())),
                )
                .await;
                return Ok(());
            }
        };
        let mut registry = self.registry.lock().await;
        registry
            .finish_invocation(&prepared.lease, completion)
            .map_err(map_registry_use_error)?;
        drop(registry);
        // Successful orphaned completion clears inflight without a StateChanged
        // event. Notify independently of lifecycle commands so it cannot strand
        // a replacement delivery waiting on this lease.
        self.invocation_changed.notify_waiters();
        self.dispatch_pending_events().await;
        Ok(())
    }

    pub(crate) async fn safe_snapshot(
        &self,
    ) -> orchestration_runtime::transport_session::SafeRegistrySnapshot {
        self.registry.lock().await.safe_snapshot()
    }

    pub(crate) async fn shutdown(&self, timeout: Duration) {
        self.shutdown.store(true, Ordering::Release);
        self.invocation_changed.notify_waiters();
        self.registry
            .lock()
            .await
            .terminate_all(TerminationKind::Shutdown);
        self.dispatch_pending_events().await;
        self.shutdown_notify.notify_waiters();
        let handle = self
            .scheduler
            .lock()
            .expect("transport scheduler lock")
            .take();
        if let Some(handle) = handle {
            let _ = tokio::time::timeout(timeout, handle).await;
        }
    }

    async fn fault_invocation(
        &self,
        lease: &InvocationLease,
        primary: Option<PrimaryTransportFailure>,
    ) {
        let result = self
            .registry
            .lock()
            .await
            .finish_invocation(lease, InvocationCompletion::Faulted);
        if let Err(error) = result {
            tracing::warn!(%error, generation = lease.fence.generation.get(),
                "failed invocation did not change a stale or closed transport session");
        }
        let _dispatcher = self.dispatcher.lock().await;
        self.dispatch_pending_events_locked_with_primary(
            primary.map(|failure| (lease.fence.clone(), failure)),
        )
        .await;
        self.invocation_changed.notify_waiters();
    }

    async fn maintain_and_dispatch(&self) {
        self.registry.lock().await.maintain();
        self.dispatch_pending_events().await;
    }

    async fn dispatch_pending_events(&self) {
        let _dispatcher = self.dispatcher.lock().await;
        self.dispatch_pending_events_locked().await;
    }

    async fn dispatch_pending_events_locked(&self) {
        self.dispatch_pending_events_locked_with_primary(None).await;
    }

    async fn dispatch_pending_events_locked_with_primary(
        &self,
        primary: Option<(TransportFence, PrimaryTransportFailure)>,
    ) {
        let (new_commands, snapshot) = {
            let mut registry = self.registry.lock().await;
            let commands = registry
                .drain_events()
                .into_iter()
                .filter_map(|event| lifecycle_command(&registry, event))
                .collect::<Vec<_>>();
            (commands, registry.safe_snapshot())
        };
        if !new_commands.is_empty() {
            self.invocation_changed.notify_waiters();
        }
        let commands = {
            let mut pending = self
                .pending_commands
                .lock()
                .expect("transport pending command lock");
            pending.retain(|task| {
                snapshot
                    .sessions
                    .iter()
                    .any(|session| session.fence == task.fence)
                    || snapshot
                        .tombstones
                        .iter()
                        .any(|receipt| receipt.fence == task.fence)
            });
            for command in new_commands {
                if let Some(existing) = pending.iter_mut().find(|task| task.fence == command.fence)
                {
                    // Drain -> Close / logical termination upgrades the same physical task.
                    // It never renews the original deadline, attempts, or exhausted state.
                    if command.command.action == ProviderTransportSessionAction::Close {
                        existing.command.action = ProviderTransportSessionAction::Close;
                    }
                    if command.terminal_close {
                        existing.terminal_close = true;
                        existing.termination = command.termination;
                    }
                } else {
                    pending.push_back(command);
                }
            }
            if let Some((fence, primary)) = primary {
                if let Some(task) = pending.iter_mut().find(|task| task.fence == fence) {
                    task.primary_failure.get_or_insert(primary);
                }
            }
            pending.drain(..).collect::<Vec<_>>()
        };
        let mut retained = VecDeque::new();
        for mut task in commands {
            if let Some(notice) = task.termination.take() {
                self.publish_termination(notice);
            }
            if task.state != CloseTaskState::Pending {
                retained.push_back(task);
                continue;
            }
            let snapshot = self.registry.lock().await.safe_snapshot();
            let now = snapshot.observed_at;
            if now >= task.overall_deadline || task.attempts >= CONTROL_MAX_ATTEMPTS {
                task.state = CloseTaskState::Exhausted;
                task.last_blocker = Some("provider_physical_connection_close_exhausted");
                tracing::warn!(
                    task_id = %task.task_id, primary = ?task.primary_failure,
                    first_control_failure = task.first_control_failure,
                    generation = task.fence.generation.get(),
                    attempts = task.attempts,
                    overall_deadline_ms = task.overall_deadline.as_millis(),
                    "provider close task exhausted"
                );
                retained.push_back(task);
                continue;
            }
            if now < task.next_due {
                retained.push_back(task);
                continue;
            }
            if !task.terminal_close {
                match task.command.action {
                    ProviderTransportSessionAction::Drain => {
                        match drain_disposition(&snapshot, &task.fence) {
                            DrainDisposition::Stale => continue,
                            DrainDisposition::Defer => {
                                retained.push_back(task);
                                continue;
                            }
                            DrainDisposition::Ready => {}
                        }
                    }
                    ProviderTransportSessionAction::Close => {
                        if !snapshot.sessions.iter().any(|session| {
                            session.fence == task.fence
                                && matches!(
                                    session.state,
                                    TransportSessionState::Faulted
                                        | TransportSessionState::IdleReleased
                                )
                        }) {
                            continue;
                        }
                    }
                }
            }
            task.command.deadline_unix_ms = i64::try_from(
                transport_add(now, CONTROL_DEADLINE)
                    .min(task.overall_deadline)
                    .as_millis(),
            )
            .unwrap_or(i64::MAX);
            task.attempts += 1;
            // The host owns the wire deadline and retirement after a dispatched timeout.
            // Dropping this future externally could leave a late unary response in stdio.
            let result = self
                .runtime
                .transport_session(&task.target_id, task.command.clone())
                .await;
            let evidence = match result {
                Ok(receipt) => accepted_closure_evidence(&task, &receipt),
                Err(error) => Err(control_error_code(&error)),
            };
            let released = match evidence {
                Ok(evidence) => {
                    task.command.worker_incarnation = Some(evidence.identity.worker_incarnation);
                    tracing::info!(task_id = %task.task_id, generation = task.fence.generation.get(),
                        worker_incarnation = evidence.identity.worker_incarnation, source = ?evidence.source,
                        local_released = evidence.local_released, peer_ack = ?evidence.peer_close_acknowledged,
                        no_ack_reason = ?evidence.no_ack_reason, attempt = task.attempts,
                        attempt_deadline_ms = task.command.deadline_unix_ms,
                        overall_deadline_ms = task.overall_deadline.as_millis(),
                        "provider close physical evidence observed");
                    match self
                        .registry
                        .lock()
                        .await
                        .record_closure_evidence(&task.fence, &evidence)
                    {
                        Ok(()) => {
                            task.last_blocker = if evidence.local_released {
                                None
                            } else {
                                Some("provider_transport_close_not_released")
                            };
                            if let Some(code) = task.last_blocker {
                                task.first_control_failure.get_or_insert(code);
                            }
                            evidence.local_released
                        }
                        Err(_) => {
                            task.last_blocker = Some("provider_transport_close_stale_evidence");
                            false
                        }
                    }
                }
                Err(code) => {
                    task.first_control_failure.get_or_insert(code);
                    task.last_blocker = Some(code);
                    false
                }
            };
            if released {
                task.state = CloseTaskState::Released;
                if task.command.action == ProviderTransportSessionAction::Drain {
                    let mut registry = self.registry.lock().await;
                    if drain_disposition(&registry.safe_snapshot(), &task.fence)
                        == DrainDisposition::Ready
                    {
                        match registry.rotate_generation(&task.fence) {
                            Ok(next) => {
                                let _ = registry.activate(&next);
                            }
                            Err(_) => {
                                task.last_blocker =
                                    Some("provider_transport_close_rotation_blocked");
                            }
                        }
                    }
                }
            } else {
                let completed_at = self.registry.lock().await.safe_snapshot().observed_at;
                if task.attempts >= CONTROL_MAX_ATTEMPTS || completed_at >= task.overall_deadline {
                    task.state = CloseTaskState::Exhausted;
                } else {
                    task.next_due = transport_add(completed_at, close_retry_delay(&task))
                        .min(task.overall_deadline);
                }
                tracing::warn!(task_id = %task.task_id, primary = ?task.primary_failure,
                    first_control_failure = task.first_control_failure,
                    generation = task.fence.generation.get(), attempts = task.attempts,
                    next_due_ms = task.next_due.as_millis(), attempt_deadline_ms = task.command.deadline_unix_ms,
                    state = ?task.state, blocker = task.last_blocker, overall_deadline_ms = task.overall_deadline.as_millis(),
                    "provider close task retained its bounded recovery outcome");
            }
            retained.push_back(task);
        }
        self.pending_commands
            .lock()
            .expect("transport pending command lock")
            .extend(retained);
    }

    fn close_task_exhausted(&self, fence: &TransportFence) -> bool {
        self.pending_commands
            .lock()
            .expect("transport pending command lock")
            .iter()
            .any(|task| task.fence == *fence && task.state == CloseTaskState::Exhausted)
    }

    fn recovery_admission_error(
        &self,
        fence: &TransportFence,
        code: &'static str,
    ) -> anyhow::Error {
        let tasks = self
            .pending_commands
            .lock()
            .expect("transport pending command lock");
        let Some(task) = tasks.iter().find(|task| task.fence == *fence) else {
            return transport_error(code);
        };
        let mut error = ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderTransportAdmissionFailed,
            code,
        );
        error.provider_details = Some(serde_json::json!({
            "transport_recovery": {
                "task_id": task.task_id, "generation": fence.generation.get(),
                "worker_incarnation": task.command.worker_incarnation,
                "primary_error": task.primary_failure.map(|failure| serde_json::json!({
                    "kind": failure.kind, "code": failure.code, "invocation_sequence": failure.invocation_sequence,
                })),
                "first_control_failure": task.first_control_failure, "recovery_blocker": task.last_blocker,
                "attempts": task.attempts, "overall_deadline_unix_ms": task.overall_deadline.as_millis(),
            }
        }));
        PluginFrameworkError::runtime(error).into()
    }

    fn publish_termination(&self, mut notice: TransportTerminationNotice) {
        let scopes = self.connection_scopes.lock().expect("transport scopes");
        for scope in scopes.values().filter_map(Weak::upgrade) {
            let state = scope.state.lock().expect("transport connection bindings");
            if let Some(lease) = state.leases.get(notice.fence.session_id.as_str()) {
                if lease.fence == notice.fence {
                    notice.connection_scope_id = Some(scope.id.clone());
                    notice.invocation_sequence = Some(lease.sequence());
                    break;
                }
            }
        }
        let _ = self.notices.send(notice);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DrainDisposition {
    Ready,
    Defer,
    Stale,
}

fn drain_disposition(snapshot: &SafeRegistrySnapshot, fence: &TransportFence) -> DrainDisposition {
    let Some(session) = snapshot
        .sessions
        .iter()
        .find(|session| session.fence == *fence)
    else {
        return DrainDisposition::Stale;
    };
    if session.state != TransportSessionState::Draining {
        return DrainDisposition::Stale;
    }
    if session.inflight {
        DrainDisposition::Defer
    } else {
        DrainDisposition::Ready
    }
}

fn lifecycle_command(
    registry: &TransportSessionRegistry<impl TransportClock>,
    event: LifecycleEvent,
) -> Option<LifecycleCommand> {
    let (fence, target_id, action, termination) = match event {
        LifecycleEvent::StateChanged { fence, to, .. }
            if matches!(
                to,
                TransportSessionState::Draining
                    | TransportSessionState::Faulted
                    | TransportSessionState::IdleReleased
            ) =>
        {
            let target = registry.runtime_target_id(&fence).ok()?.as_str().to_owned();
            let action = if to == TransportSessionState::Draining {
                ProviderTransportSessionAction::Drain
            } else {
                ProviderTransportSessionAction::Close
            };
            (fence, target, action, None)
        }
        LifecycleEvent::Terminated(receipt) => {
            let notice = TransportTerminationNotice {
                owner_id: receipt.owner_id.as_str().to_owned(),
                fence: receipt.fence.clone(),
                invocation_sequence: None,
                connection_scope_id: None,
                code: termination_code(receipt.kind),
            };
            (
                receipt.fence,
                receipt.runtime_target_id.as_str().to_owned(),
                ProviderTransportSessionAction::Close,
                Some(notice),
            )
        }
        _ => return None,
    };
    let snapshot = registry.safe_snapshot();
    let now = snapshot.observed_at;
    let overall_deadline = snapshot
        .sessions
        .iter()
        .find(|session| session.fence == fence)
        .map_or(transport_add(now, CONTROL_OVERALL_DEADLINE), |session| {
            transport_add(now, CONTROL_OVERALL_DEADLINE.min(session.logical_ttl))
        });
    let task_id = hash_identity(&[
        fence.session_id.as_str(),
        &fence.generation.get().to_string(),
    ]);
    Some(LifecycleCommand {
        command: ProviderTransportSessionCommand {
            logical_session_id: fence.session_id.as_str().to_owned(),
            generation: fence.generation.get(),
            worker_incarnation: None,
            action,
            deadline_unix_ms: i64::try_from(
                transport_add(now, CONTROL_DEADLINE)
                    .min(overall_deadline)
                    .as_millis(),
            )
            .unwrap_or(i64::MAX),
        },
        fence,
        target_id,
        terminal_close: termination.is_some(),
        termination,
        overall_deadline,
        next_due: now,
        attempts: 0,
        state: CloseTaskState::Pending,
        last_blocker: None,
        first_control_failure: None,
        primary_failure: None,
        task_id,
    })
}

fn accepted_closure_evidence(
    task: &LifecycleCommand,
    receipt: &ProviderTransportSessionReceipt,
) -> Result<ProviderTransportClosureEvidence, &'static str> {
    receipt
        .validate()
        .map_err(|_| "provider_transport_close_invalid_evidence")?;
    let evidence = receipt
        .closure_evidence
        .as_ref()
        .ok_or("provider_transport_close_evidence_missing")?;
    let expected = ProviderTransportSessionIdentity {
        logical_session_id: task.command.logical_session_id.clone(),
        generation: task.command.generation,
        worker_incarnation: task
            .command
            .worker_incarnation
            .unwrap_or(evidence.identity.worker_incarnation),
    };
    // The runtime-host boundary already checked this worker incarnation against its trusted dispatch binding.
    match receipt
        .closure_outcome(&expected)
        .map_err(|_| "provider_transport_close_invalid_evidence")?
    {
        ProviderTransportClosureOutcome::IdentityMismatch
        | ProviderTransportClosureOutcome::EvidenceMissing => {
            Err("provider_transport_close_identity_mismatch")
        }
        _ => Ok(evidence.clone()),
    }
}

fn primary_transport_failure(
    error: &anyhow::Error,
    invocation_sequence: u64,
) -> PrimaryTransportFailure {
    let typed = match error.downcast_ref::<PluginFrameworkError>() {
        Some(PluginFrameworkError::RuntimeContract { error }) => Some(error.as_ref()),
        _ => error.downcast_ref::<ProviderRuntimeError>(),
    };
    let kind = typed.map(|error| error.kind);
    // Only canonical categories and known fixed messages are retained; no provider body or free-form diagnostic.
    let code = match typed.map(|error| error.message.as_str()) {
        Some("WebSocket continuation interrupted: cursor owner is unavailable") => {
            "provider_cursor_owner_unavailable"
        }
        Some("provider_transport_cursor_unreconstructible") => {
            "provider_transport_cursor_unreconstructible"
        }
        Some("provider_transport_receipt_missing") => "provider_transport_receipt_missing",
        _ => kind
            .map(provider_kind_code)
            .unwrap_or("provider_invocation_failed"),
    };
    PrimaryTransportFailure {
        kind,
        code,
        invocation_sequence,
    }
}

fn provider_kind_code(kind: ProviderRuntimeErrorKind) -> &'static str {
    match kind {
        ProviderRuntimeErrorKind::AuthFailed => "provider_auth_failed",
        ProviderRuntimeErrorKind::EndpointUnreachable => "provider_endpoint_unreachable",
        ProviderRuntimeErrorKind::ModelNotFound => "provider_model_not_found",
        ProviderRuntimeErrorKind::ProviderAffinityMismatch => "provider_affinity_mismatch",
        ProviderRuntimeErrorKind::ProviderTransportUnavailable => "provider_transport_unavailable",
        ProviderRuntimeErrorKind::ProviderTransportAdmissionFailed => {
            "provider_transport_admission_failed"
        }
        ProviderRuntimeErrorKind::SemanticCapabilityUnsupported => {
            "provider_semantic_capability_unsupported"
        }
        ProviderRuntimeErrorKind::RateLimited => "provider_rate_limited",
        ProviderRuntimeErrorKind::ProviderUpstreamError => "provider_upstream_error",
        ProviderRuntimeErrorKind::ProviderInvalidResponse => "provider_invalid_response",
    }
}

fn control_error_code(error: &RuntimeBackendError) -> &'static str {
    match error {
        RuntimeBackendError::InvalidRequest(_) => "control_invalid_request",
        RuntimeBackendError::DuplicateBackend => "control_duplicate_backend",
        RuntimeBackendError::MissingBackend => "control_missing_backend",
        RuntimeBackendError::Unavailable(_) => "control_backend_unavailable",
        RuntimeBackendError::DuplicateRequest(_) => "control_duplicate_request",
        RuntimeBackendError::Cancelled(_) => "control_cancelled",
        RuntimeBackendError::UnsupportedOperation(_) => "control_unsupported_operation",
        RuntimeBackendError::Contract(error) => match error.as_ref() {
            PluginFrameworkError::RuntimeContract { error } => match error.message.as_str() {
                "provider worker ended without response line"
                | "provider worker process exited" => "control_worker_eof",
                "provider transport control deadline exceeded" => "control_queue_deadline",
                message if message.starts_with("provider runtime timed out:") => {
                    "control_stdio_timeout"
                }
                _ => provider_kind_code(error.kind),
            },
            _ => "control_provider_contract_error",
        },
        RuntimeBackendError::CountTokens(_) => "control_unexpected_count_tokens_error",
        RuntimeBackendError::Compact(_) => "control_unexpected_compact_error",
        RuntimeBackendError::Execution { .. } => "control_execution_error",
    }
}

fn transport_add(now: TransportInstant, duration: Duration) -> TransportInstant {
    TransportInstant::from_millis(
        now.as_millis()
            .saturating_add(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)),
    )
}

fn close_retry_delay(task: &LifecycleCommand) -> Duration {
    let base = 250_u64
        .saturating_mul(1_u64 << task.attempts.saturating_sub(1).min(4))
        .min(4_000);
    let mut hash = Sha256::new();
    hash.update(task.fence.session_id.as_str().as_bytes());
    hash.update(task.fence.generation.get().to_le_bytes());
    hash.update([task.attempts]);
    let bytes = hash.finalize();
    let jitter = u64::from(u16::from_le_bytes([bytes[0], bytes[1]])) % (base / 2 + 1);
    Duration::from_millis(base + jitter)
}

// This admits a new invocation; it never resubmits the failed invocation or
// strips a cursor. An opaque/connection-bound cursor without durable proof is
// unusable on the fresh physical generation.
fn validate_fault_successor(
    input: &ProviderInvocationInput,
    recovery: Option<&ProviderRecoveryDirective>,
    now: TransportInstant,
) -> anyhow::Result<()> {
    if recovery
        .is_some_and(|directive| directive.initial_commit_level != CommitLevel::LifecycleOnly)
    {
        return Err(transport_error("provider_transport_committed_invocation"));
    }
    if recovery.is_some_and(|directive| {
        u64::try_from(directive.policy.budget().absolute_deadline_unix_ms)
            .map_or(true, |deadline| deadline <= now.as_millis())
    }) {
        return Err(transport_error("transport_invocation_deadline_exceeded"));
    }
    let binding = recovery
        .and_then(|directive| directive.cursor_provenance)
        .map(|cursor| cursor.binding);
    let has_cursor = input.previous_response_id.is_some()
        || input.native_transport.as_ref().is_some_and(|native| {
            native
                .wire_body
                .get("previous_response_id")
                .is_some_and(|value| !value.is_null())
        });
    if matches!(binding, Some(CursorBinding::ConnectionBound { .. }))
        || (has_cursor && binding != Some(CursorBinding::Durable))
    {
        return Err(transport_error(
            "provider_transport_cursor_unreconstructible",
        ));
    }
    Ok(())
}

/// Freeze the transport selected by the invocation configuration before calling
/// the provider. Node selection wins over the provider-instance setting; `auto`
/// starts with WS and requires a typed recovery receipt for HTTP fallback. This
/// follows the paired OpenAI provider contract, never infers HTTP from metadata.
fn selected_responses_transport(
    input: &ProviderInvocationInput,
) -> anyhow::Result<RecoveryTransport> {
    if input.operation == ProviderWireOperation::Compact {
        return Ok(RecoveryTransport::ProviderHttp);
    }
    match input.model_parameters.get("use_responses_websocket") {
        Some(serde_json::Value::Bool(true)) => return Ok(RecoveryTransport::AiNativeWebSocket),
        Some(serde_json::Value::Bool(false)) => return Ok(RecoveryTransport::ProviderHttp),
        Some(_) => return Err(transport_error("provider_transport_mode_invalid")),
        None => {}
    }
    match input.provider_config.get("transport_mode") {
        None => Ok(RecoveryTransport::ProviderHttp),
        Some(serde_json::Value::Null) => Ok(RecoveryTransport::AiNativeWebSocket),
        Some(serde_json::Value::String(mode)) => match mode.trim().to_ascii_lowercase().as_str() {
            "http_sse" | "sse" | "http" => Ok(RecoveryTransport::ProviderHttp),
            "" | "auto" | "responses_websocket" | "websocket" | "ws" => {
                Ok(RecoveryTransport::AiNativeWebSocket)
            }
            _ => Err(transport_error("provider_transport_mode_invalid")),
        },
        Some(_) => Err(transport_error("provider_transport_mode_invalid")),
    }
}

fn protocol_session_id(input: &ProviderInvocationInput) -> Option<&str> {
    (input.protocol == "openai_responses")
        .then_some(input.client_protocol_envelope.as_ref()?)?
        .headers
        .get("session-id")?
        .as_slice()
        .first()
        .map(String::as_str)
}

fn hash_identity(parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part.len().to_be_bytes());
        digest.update(part.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn termination_code(kind: TerminationKind) -> &'static str {
    match kind {
        TerminationKind::CapacityEvicted => "transport_session_evicted",
        TerminationKind::OwnerOrphaned
        | TerminationKind::OwnerClosed
        | TerminationKind::Shutdown => "transport_session_orphaned",
        TerminationKind::ProviderHardMax => "provider_connection_max_age",
        TerminationKind::ProviderFault => "provider_physical_connection_fault",
        TerminationKind::DeadlineExceeded(DeadlineKind::Task) => {
            "transport_invocation_deadline_exceeded"
        }
        TerminationKind::DeadlineExceeded(DeadlineKind::LogicalAbsolute) => {
            "transport_logical_deadline_exceeded"
        }
        TerminationKind::DeadlineExceeded(DeadlineKind::StateLease) => {
            "transport_state_lease_expired"
        }
        TerminationKind::DeadlineExceeded(DeadlineKind::PhysicalHard) => {
            "provider_connection_max_age"
        }
    }
}

fn map_registry_admission_error(error: RegistryError) -> anyhow::Error {
    match error {
        RegistryError::Capacity(_) => transport_error("capacity_exceeded"),
        RegistryError::DeadlineInPast => transport_error("provider_connection_max_age"),
        RegistryError::InflightExists | RegistryError::AlreadyExists => {
            transport_error("transport_session_busy")
        }
        _ => transport_error("transport_session_evicted"),
    }
}

fn map_registry_use_error(error: RegistryError) -> anyhow::Error {
    match error {
        RegistryError::NotFound | RegistryError::StaleGeneration { .. } => {
            transport_error("transport_session_evicted")
        }
        RegistryError::Capacity(_) => transport_error("capacity_exceeded"),
        RegistryError::DeadlineInPast => transport_error("transport_invocation_deadline_exceeded"),
        RegistryError::InflightExists | RegistryError::InvocationSequenceExhausted => {
            transport_error("transport_session_busy")
        }
        RegistryError::StaleInvocation | RegistryError::NoInflight => {
            transport_error("transport_session_evicted")
        }
        RegistryError::AlreadyExists
        | RegistryError::InvalidTransition { .. }
        | RegistryError::GenerationExhausted
        | RegistryError::InvalidClosureEvidence
        | RegistryError::InvalidConfig => transport_error("transport_session_evicted"),
    }
}

fn transport_error(code: &'static str) -> anyhow::Error {
    PluginFrameworkError::runtime(ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::ProviderTransportAdmissionFailed,
        code,
    ))
    .into()
}

/// Stable, secret-free admission diagnostics.
///
/// `branch` is the exact rejection branch, so an incident can be located without
/// guessing which check refused the request. Only opaque identities, counters,
/// the logical state and booleans may enter here: no prompt, credential, cursor
/// value, payload or raw scope identifier.
fn admission_error(branch: &'static str, details: serde_json::Value) -> anyhow::Error {
    tracing::warn!(
        reject_branch = branch,
        admission = %details,
        "provider transport admission rejected"
    );
    let mut error = ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::ProviderTransportAdmissionFailed,
        branch,
    );
    error.provider_details = Some(serde_json::json!({ "transport_admission": details }));
    PluginFrameworkError::runtime(error).into()
}

fn admission_details(
    session_id: &TransportSessionId,
    fence: Option<&TransportFence>,
    state: Option<TransportSessionState>,
    inflight: Option<bool>,
    connection_scope_bound: bool,
) -> serde_json::Value {
    serde_json::json!({
        "session_id": session_id.as_str(),
        "generation": fence.map(|fence| fence.generation.get()),
        "state": state.map(session_state_code),
        "inflight": inflight,
        "connection_scope_bound": connection_scope_bound,
    })
}

fn session_state_code(state: TransportSessionState) -> &'static str {
    match state {
        TransportSessionState::Opening => "opening",
        TransportSessionState::Active => "active",
        TransportSessionState::WaitingTool => "waiting_tool",
        TransportSessionState::IdleAffinity => "idle_affinity",
        TransportSessionState::IdleReleased => "idle_released",
        TransportSessionState::Orphaned => "orphaned",
        TransportSessionState::Draining => "draining",
        TransportSessionState::Faulted => "faulted",
        TransportSessionState::Closing => "closing",
    }
}

#[allow(dead_code)]
fn _assert_runtime_error_is_send(error: RuntimeBackendError) -> RuntimeBackendError {
    error
}

#[cfg(test)]
#[path = "_tests/transport_session_lifecycle.rs"]
mod tests;
