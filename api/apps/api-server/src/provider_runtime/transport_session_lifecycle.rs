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
        CommitLevel, CursorBinding, ProviderClientTransport, ProviderInvocationInput,
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

#[path = "transport_session_lifecycle/admission.rs"]
mod admission;

const CONTROL_DEADLINE: Duration = Duration::from_secs(5);
const CONTROL_OVERALL_DEADLINE: Duration = Duration::from_secs(30);
const CONTROL_MAX_ATTEMPTS: u8 = 5;
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(1);
fn handoff_wait_budget(
    now: TransportInstant,
    deadline: Option<TransportInstant>,
    predecessor_deadline: TransportInstant,
) -> Duration {
    Duration::from_millis(
        deadline
            .unwrap_or(predecessor_deadline)
            .as_millis()
            .saturating_sub(now.as_millis()),
    )
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
    carrier: ProviderClientTransport,
    state: StdMutex<TransportConnectionBindings>,
}

/// The delivery owns this guard, not the execution or a clone of the scope.
/// Closing may await coordinator locks, so Drop transfers the whole close future
/// to an independent task rather than letting request cancellation interrupt it.
pub(crate) struct TransportConnectionScopeGuard {
    scope: Arc<TransportConnectionScope>,
    on_close: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl TransportConnectionScopeGuard {
    pub(crate) fn id(&self) -> &str {
        self.scope.id()
    }

    pub(crate) fn close(&mut self) {
        if let Some(close) = self.on_close.take() {
            close();
        }
    }
}

impl Drop for TransportConnectionScopeGuard {
    fn drop(&mut self) {
        self.close();
    }
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
            carrier: ProviderClientTransport::Websocket,
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
    /// Valid after the first eligible control attempt is claimed (attempts > 0).
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
    on_abandon: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl Drop for PreparedTransportInvocation {
    fn drop(&mut self) {
        if let Some(abandon) = self.on_abandon.take() {
            abandon();
        }
    }
}

#[async_trait::async_trait]
trait TransportLifecycleRuntime: Send + Sync {
    #[cfg(test)]
    fn observe_dispatch(&self, _target_id: &str, _lease: &InvocationLease) {}

    async fn transport_worker_exit_evidence(
        &self,
        target_id: &str,
        logical_session_id: &str,
        generation: u64,
    ) -> Result<Option<ProviderTransportClosureEvidence>, RuntimeBackendError>;

    async fn transport_session(
        &self,
        target_id: &str,
        command: ProviderTransportSessionCommand,
    ) -> Result<ProviderTransportSessionReceipt, RuntimeBackendError>;
}

struct RuntimeBackendTransportLifecycle(Arc<dyn RuntimeBackend>);

#[async_trait::async_trait]
impl TransportLifecycleRuntime for RuntimeBackendTransportLifecycle {
    async fn transport_worker_exit_evidence(
        &self,
        target_id: &str,
        logical_session_id: &str,
        generation: u64,
    ) -> Result<Option<ProviderTransportClosureEvidence>, RuntimeBackendError> {
        self.0
            .provider_transport_worker_exit_evidence(target_id, logical_session_id, generation)
            .await
    }

    async fn transport_session(
        &self,
        target_id: &str,
        command: ProviderTransportSessionCommand,
    ) -> Result<ProviderTransportSessionReceipt, RuntimeBackendError> {
        self.0.provider_transport_session(target_id, command).await
    }
}

pub(crate) struct TransportSessionCoordinator<C = SystemTransportClock> {
    registry: Arc<Mutex<TransportSessionRegistry<C>>>,
    runtime: Arc<dyn TransportLifecycleRuntime>,
    notices: broadcast::Sender<TransportTerminationNotice>,
    shutdown: AtomicBool,
    shutdown_notify: Notify,
    invocation_changed: Arc<Notify>,
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
            registry: Arc::new(Mutex::new(registry)),
            runtime,
            notices,
            shutdown: AtomicBool::new(false),
            shutdown_notify: Notify::new(),
            invocation_changed: Arc::new(Notify::new()),
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
        self.open_scope(ProviderClientTransport::Websocket)
    }

    pub(crate) fn open_http_connection_scope(self: &Arc<Self>) -> TransportConnectionScopeGuard {
        let scope = self.open_scope(ProviderClientTransport::Http);
        let coordinator = Arc::clone(self);
        let closing_scope = Arc::clone(&scope);
        // Capture the runtime while opening the request. The response body may
        // later be dropped outside its task's runtime context.
        let runtime = tokio::runtime::Handle::current();
        TransportConnectionScopeGuard {
            scope,
            on_close: Some(Box::new(move || {
                runtime.spawn(async move {
                    coordinator.close_connection_scope(&closing_scope).await;
                });
            })),
        }
    }

    fn open_scope(&self, carrier: ProviderClientTransport) -> Arc<TransportConnectionScope> {
        let scope = Arc::new(TransportConnectionScope {
            id: uuid::Uuid::now_v7().to_string(),
            carrier,
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

    pub(crate) async fn claim_dispatch(
        &self,
        prepared: &PreparedTransportInvocation,
    ) -> anyhow::Result<()> {
        let mut registry = self.registry.lock().await;
        registry
            .claim_invocation_dispatch(&prepared.lease)
            .map_err(map_registry_use_error)?;
        #[cfg(test)]
        self.runtime.observe_dispatch(
            registry.runtime_target_id(&prepared.lease.fence)?.as_str(),
            &prepared.lease,
        );
        Ok(())
    }

    pub(crate) async fn finish(
        &self,
        mut prepared: PreparedTransportInvocation,
        result: &anyhow::Result<super::ProviderRuntimeInvocationOutput>,
    ) -> anyhow::Result<()> {
        let completion = self.finish_prepared(&prepared, result).await;
        prepared.on_abandon = None;
        completion
    }

    async fn finish_prepared(
        &self,
        prepared: &PreparedTransportInvocation,
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
                                "provider transport receipt generation mismatched its admitted invocation"
                            );
                            // The runtime invocation returned, so its host-owned lease
                            // must settle even though the provider receipt is rejected.
                            // Registry fencing protects a newer generation or sequence.
                            self.fault_invocation(&prepared.lease, None).await;
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
        // A logical lifetime may have elapsed while this call was in flight.
        // Close its generation only after the invocation has settled.
        registry.maintain();
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
            // Dispatch fills the wire deadline only after eligibility checks.
            deadline_unix_ms: 0,
        },
        fence,
        target_id,
        terminal_close: termination.is_some(),
        termination,
        overall_deadline: now,
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
// strips a cursor. Explicit connection binding fences a fresh generation; absent
// provenance leaves cursor validation and complete-context recovery to its provider.
fn validate_fault_successor(
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
    if matches!(binding, Some(CursorBinding::ConnectionBound { .. })) {
        return Err(transport_error(
            "provider_transport_cursor_unreconstructible",
        ));
    }
    Ok(())
}

/// Freeze the selected transport before calling the provider. The ingress
/// transport is observed by the host, not inferred from client headers.
fn selected_responses_transport(
    input: &ProviderInvocationInput,
) -> anyhow::Result<RecoveryTransport> {
    if input.operation == ProviderWireOperation::Compact {
        return Ok(RecoveryTransport::ProviderHttp);
    }
    match input.model_parameters.get("responses_transport_policy") {
        Some(serde_json::Value::String(policy)) => match policy.as_str() {
            "force_http_sse" => return Ok(RecoveryTransport::ProviderHttp),
            "force_websocket" => return Ok(RecoveryTransport::AiNativeWebSocket),
            "inherit" => {}
            _ => return Err(transport_error("provider_transport_mode_invalid")),
        },
        Some(_) => return Err(transport_error("provider_transport_mode_invalid")),
        None => match input.model_parameters.get("use_responses_websocket") {
            Some(serde_json::Value::Bool(true)) => return Ok(RecoveryTransport::AiNativeWebSocket),
            Some(serde_json::Value::Bool(false)) | None => {}
            Some(_) => return Err(transport_error("provider_transport_mode_invalid")),
        },
    }
    if let Some(client_transport) = input.client_transport {
        return Ok(match client_transport {
            ProviderClientTransport::Http => RecoveryTransport::ProviderHttp,
            ProviderClientTransport::Websocket => RecoveryTransport::AiNativeWebSocket,
        });
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
    format!("{:x}", base16ct::HexDisplay(&digest.finalize()))
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

#[path = "transport_session_lifecycle/termination_dispatch.rs"]
mod termination_dispatch;
