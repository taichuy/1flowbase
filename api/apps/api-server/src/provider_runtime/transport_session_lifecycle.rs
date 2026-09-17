use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as StdMutex,
    },
    time::Duration,
};

use orchestration_runtime::transport_session::{
    AdmissionRequest, DeadlineKind, InvocationCompletion, InvocationLease, InvocationRequest,
    LifecycleEvent, RegistryError, SafeRegistrySnapshot, SystemTransportClock, TerminationKind,
    TransportClock, TransportFence, TransportInstant, TransportOwnerId, TransportProviderId,
    TransportRegistryConfig, TransportRuntimeTargetId, TransportSessionId,
    TransportSessionRegistry, TransportSessionState,
};
use plugin_framework::{
    provider_contract::{
        ProviderInvocationInput, ProviderLogicalSessionState, ProviderPhysicalTransportState,
        ProviderRuntimeError, ProviderRuntimeErrorKind, ProviderTransportSessionAction,
        ProviderTransportSessionCommand, ProviderTransportSessionDirective,
        ProviderTransportSessionReceipt,
    },
    PluginFrameworkError,
};
use runtime_core::runtime_backend::{RuntimeBackend, RuntimeBackendError};
use sha2::{Digest, Sha256};
use tokio::sync::{broadcast, Mutex, Notify};

use super::ProviderRuntimeExecutionContext;

const CONTROL_DEADLINE: Duration = Duration::from_secs(5);
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TransportTerminationNotice {
    pub(crate) owner_id: String,
    pub(crate) code: &'static str,
}

struct LifecycleCommand {
    fence: TransportFence,
    target_id: String,
    command: ProviderTransportSessionCommand,
    termination: Option<TransportTerminationNotice>,
    close_fence: Option<TransportFence>,
}

pub(crate) struct PreparedTransportInvocation {
    lease: InvocationLease,
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
    scheduler: StdMutex<Option<tokio::task::JoinHandle<()>>>,
    dispatcher: Mutex<()>,
    pending_commands: StdMutex<VecDeque<LifecycleCommand>>,
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
            scheduler: StdMutex::new(None),
            dispatcher: Mutex::new(()),
            pending_commands: StdMutex::new(VecDeque::new()),
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

    pub(crate) async fn prepare(
        &self,
        target_id: &str,
        input: &mut ProviderInvocationInput,
        context: &ProviderRuntimeExecutionContext,
    ) -> anyhow::Result<Option<PreparedTransportInvocation>> {
        let Some(protocol_session_id) = protocol_session_id(input) else {
            return Ok(None);
        };
        if self.shutdown.load(Ordering::Acquire) {
            return Err(transport_error("transport_session_orphaned"));
        }

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
        let target = TransportRuntimeTargetId::new(target_id.to_string())?;
        let provider_id = TransportProviderId::new(input.provider_instance_id.clone())?;
        let invocation_deadline = u64::try_from(context.deadline_unix_ms)
            .ok()
            .map(TransportInstant::from_millis);

        let _dispatcher = self.dispatcher.lock().await;
        self.registry.lock().await.maintain();
        self.dispatch_pending_events_locked().await;

        let mut registry = self.registry.lock().await;
        let fence = if let Some(fence) = registry.fence(&session_id) {
            let state = registry.state(&fence)?;
            if state == TransportSessionState::Orphaned {
                return Err(transport_error("transport_session_orphaned"));
            }
            if matches!(
                state,
                TransportSessionState::Draining
                    | TransportSessionState::Faulted
                    | TransportSessionState::Closing
            ) {
                return Err(transport_error("provider_connection_max_age"));
            }
            if registry.runtime_target_id(&fence)? != &target {
                return Err(transport_error("transport_session_evicted"));
            }
            fence
        } else if let Some(receipt) = registry.tombstone(&session_id) {
            return Err(transport_error(termination_code(receipt.kind)));
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
            i64::try_from(registry.physical_hard_deadline(&fence)?.as_millis()).unwrap_or(i64::MAX);
        input
            .set_transport_session_directive(ProviderTransportSessionDirective {
                logical_session_id: session_id.as_str().to_string(),
                generation: fence.generation.get(),
                task_id: format!("invocation-{}-{}", fence.generation.get(), lease.sequence()),
                state: ProviderLogicalSessionState::Active,
                physical_deadline_unix_ms,
            })
            .map_err(|_| transport_error("transport_session_directive_invalid"))?;
        drop(registry);
        self.dispatch_pending_events_locked().await;
        Ok(Some(PreparedTransportInvocation { lease }))
    }

    pub(crate) async fn finish(
        &self,
        prepared: PreparedTransportInvocation,
        result: &anyhow::Result<super::ProviderRuntimeInvocationOutput>,
    ) -> anyhow::Result<()> {
        let completion = match result {
            Ok(output) => {
                let receipt = output
                    .result
                    .transport_session_receipt()
                    .map_err(|_| transport_error("provider_connection_max_age"))?
                    .ok_or_else(|| transport_error("provider_connection_max_age"))?;
                if receipt.generation != prepared.lease.fence.generation.get()
                    || receipt.physical_state != ProviderPhysicalTransportState::Ready
                {
                    self.terminate(&prepared.lease.fence, TerminationKind::ProviderFault)
                        .await;
                    return Err(transport_error("provider_connection_max_age"));
                }
                if output.result.tool_calls.is_empty() {
                    InvocationCompletion::IdleAffinity
                } else {
                    InvocationCompletion::WaitingTool
                }
            }
            Err(_) => {
                self.terminate(&prepared.lease.fence, TerminationKind::ProviderFault)
                    .await;
                return Ok(());
            }
        };
        let mut registry = self.registry.lock().await;
        registry
            .finish_invocation(&prepared.lease, completion)
            .map_err(map_registry_use_error)?;
        drop(registry);
        self.dispatch_pending_events().await;
        Ok(())
    }

    pub(crate) async fn mark_owner_orphaned(&self, owner_id: &str) {
        let Ok(owner_id) = TransportOwnerId::new(owner_id.to_string()) else {
            return;
        };
        self.registry.lock().await.mark_owner_orphaned(&owner_id);
    }

    pub(crate) async fn safe_snapshot(
        &self,
    ) -> orchestration_runtime::transport_session::SafeRegistrySnapshot {
        self.registry.lock().await.safe_snapshot()
    }

    pub(crate) async fn shutdown(&self, timeout: Duration) {
        self.shutdown.store(true, Ordering::Release);
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

    async fn terminate(&self, fence: &TransportFence, kind: TerminationKind) {
        let _ = self.registry.lock().await.terminate(fence, kind);
        self.dispatch_pending_events().await;
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
        let new_commands = {
            let mut registry = self.registry.lock().await;
            registry
                .drain_events()
                .into_iter()
                .filter_map(|event| lifecycle_command(&registry, event))
                .collect::<Vec<_>>()
        };
        let commands = {
            let mut pending = self
                .pending_commands
                .lock()
                .expect("transport pending command lock");
            pending.extend(new_commands);
            pending.drain(..).collect::<Vec<_>>()
        };
        let mut deferred = VecDeque::new();
        for command in commands {
            if command.command.action == ProviderTransportSessionAction::Drain {
                let snapshot = self.registry.lock().await.safe_snapshot();
                match drain_disposition(&snapshot, &command.fence) {
                    DrainDisposition::Defer => {
                        deferred.push_back(command);
                        continue;
                    }
                    DrainDisposition::Stale => continue,
                    DrainDisposition::Ready => {}
                }
            }
            let result = tokio::time::timeout(
                CONTROL_DEADLINE,
                self.runtime
                    .transport_session(&command.target_id, command.command.clone()),
            )
            .await;
            let acknowledged = matches!(
                &result,
                Ok(Ok(receipt))
                    if receipt.generation == command.command.generation
                        && receipt.close_acknowledged == Some(true)
            );
            if !acknowledged {
                tracing::warn!(
                    target_id = %command.target_id,
                    generation = command.command.generation,
                    "provider transport lifecycle command did not return a matching ACK"
                );
            }
            if let Some(fence) = &command.close_fence {
                let _ = self
                    .registry
                    .lock()
                    .await
                    .record_close_acknowledgement(fence, acknowledged);
            }
            if command.command.action == ProviderTransportSessionAction::Drain {
                if !acknowledged {
                    deferred.push_back(command);
                    continue;
                }
                let mut registry = self.registry.lock().await;
                let snapshot = registry.safe_snapshot();
                if drain_disposition(&snapshot, &command.fence) == DrainDisposition::Ready {
                    match registry.rotate_generation(&command.fence) {
                        Ok(next_fence) => {
                            if let Err(error) = registry.activate(&next_fence) {
                                tracing::warn!(
                                    session_id = next_fence.session_id.as_str(),
                                    generation = next_fence.generation.get(),
                                    %error,
                                    "rotated provider transport generation could not be activated"
                                );
                            }
                        }
                        Err(error) => tracing::warn!(
                            session_id = command.fence.session_id.as_str(),
                            generation = command.fence.generation.get(),
                            %error,
                            "provider transport generation could not rotate after close ACK"
                        ),
                    }
                }
            }
            if let Some(notice) = command.termination {
                let _ = self.notices.send(notice);
            }
        }
        self.pending_commands
            .lock()
            .expect("transport pending command lock")
            .extend(deferred);
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
    match event {
        LifecycleEvent::StateChanged { fence, to, .. } if to == TransportSessionState::Draining => {
            let target_id = registry
                .runtime_target_id(&fence)
                .ok()?
                .as_str()
                .to_string();
            Some(LifecycleCommand {
                fence: fence.clone(),
                target_id,
                command: ProviderTransportSessionCommand {
                    logical_session_id: fence.session_id.as_str().to_string(),
                    generation: fence.generation.get(),
                    action: ProviderTransportSessionAction::Drain,
                    deadline_unix_ms: control_deadline_unix_ms(),
                },
                termination: None,
                close_fence: None,
            })
        }
        LifecycleEvent::Terminated(receipt) => Some(LifecycleCommand {
            fence: receipt.fence.clone(),
            target_id: receipt.runtime_target_id.as_str().to_string(),
            command: ProviderTransportSessionCommand {
                logical_session_id: receipt.fence.session_id.as_str().to_string(),
                generation: receipt.fence.generation.get(),
                action: ProviderTransportSessionAction::Close,
                deadline_unix_ms: control_deadline_unix_ms(),
            },
            termination: Some(TransportTerminationNotice {
                owner_id: receipt.owner_id.as_str().to_string(),
                code: termination_code(receipt.kind),
            }),
            close_fence: Some(receipt.fence),
        }),
        _ => None,
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

fn control_deadline_unix_ms() -> i64 {
    i64::try_from(
        (time::OffsetDateTime::now_utc() + time::Duration::seconds(5)).unix_timestamp_nanos()
            / 1_000_000,
    )
    .unwrap_or(i64::MAX)
}

fn termination_code(kind: TerminationKind) -> &'static str {
    match kind {
        TerminationKind::CapacityEvicted => "transport_session_evicted",
        TerminationKind::OwnerOrphaned
        | TerminationKind::OwnerClosed
        | TerminationKind::Shutdown => "transport_session_orphaned",
        TerminationKind::ProviderHardMax | TerminationKind::ProviderFault => {
            "provider_connection_max_age"
        }
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

#[allow(dead_code)]
fn _assert_runtime_error_is_send(error: RuntimeBackendError) -> RuntimeBackendError {
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestration_runtime::transport_session::CapacityRejection;
    use plugin_framework::provider_contract::{
        ProtocolContextEnvelope, ProviderCompactProfile, ProviderInvocationResult,
        ProviderTransportSessionCloseReason, ProviderWireOperation,
    };
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    };

    #[derive(Clone)]
    struct FakeClock(Arc<AtomicU64>);

    impl FakeClock {
        fn new(now_ms: u64) -> Self {
            Self(Arc::new(AtomicU64::new(now_ms)))
        }

        fn advance(&self, duration: Duration) {
            self.0.fetch_add(
                u64::try_from(duration.as_millis()).unwrap(),
                AtomicOrdering::SeqCst,
            );
        }
    }

    impl TransportClock for FakeClock {
        fn now(&self) -> TransportInstant {
            TransportInstant::from_millis(self.0.load(AtomicOrdering::SeqCst))
        }
    }

    #[derive(Clone, Copy)]
    enum AckBehavior {
        Matching(bool),
        Mismatched,
    }

    struct FakeTransportRuntime {
        responses: StdMutex<VecDeque<AckBehavior>>,
        commands: StdMutex<Vec<ProviderTransportSessionCommand>>,
    }

    impl FakeTransportRuntime {
        fn new(responses: impl IntoIterator<Item = AckBehavior>) -> Self {
            Self {
                responses: StdMutex::new(responses.into_iter().collect()),
                commands: StdMutex::new(Vec::new()),
            }
        }

        fn commands(&self) -> Vec<ProviderTransportSessionCommand> {
            self.commands.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl TransportLifecycleRuntime for FakeTransportRuntime {
        async fn transport_session(
            &self,
            _target_id: &str,
            command: ProviderTransportSessionCommand,
        ) -> Result<ProviderTransportSessionReceipt, RuntimeBackendError> {
            self.commands.lock().unwrap().push(command.clone());
            let behavior = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("a fake ACK behavior must be configured for every command");
            let (generation, close_acknowledged) = match behavior {
                AckBehavior::Matching(acknowledged) => (command.generation, Some(acknowledged)),
                AckBehavior::Mismatched => (command.generation + 1, Some(true)),
            };
            Ok(ProviderTransportSessionReceipt {
                generation,
                reused: true,
                physical_state: ProviderPhysicalTransportState::Closed,
                connection_age_ms: 1,
                ttl_remaining_ms: 0,
                close_reason: Some(ProviderTransportSessionCloseReason::RequestedDrain),
                close_acknowledged,
            })
        }
    }

    fn transport_config() -> TransportRegistryConfig {
        TransportRegistryConfig {
            logical_max_age: Duration::from_secs(60),
            invocation_default: Duration::from_secs(10),
            idle_affinity_lease: Duration::from_secs(30),
            physical_soft_drain_age: Duration::from_secs(20),
            physical_max_age: Duration::from_secs(40),
            ..TransportRegistryConfig::default()
        }
    }

    fn invocation_input(
        session_id: &str,
        operation: ProviderWireOperation,
    ) -> ProviderInvocationInput {
        ProviderInvocationInput {
            operation,
            provider_instance_id: "provider-instance".to_string(),
            provider_code: "provider-code".to_string(),
            protocol: "openai_responses".to_string(),
            model: "model-a".to_string(),
            client_protocol_envelope: Some(ProtocolContextEnvelope {
                source_protocol: "openai_responses".to_string(),
                headers: BTreeMap::from([("session-id".to_string(), vec![session_id.to_string()])]),
                ..ProtocolContextEnvelope::default()
            }),
            ..ProviderInvocationInput::default()
        }
    }

    fn context(deadline_ms: u64) -> ProviderRuntimeExecutionContext {
        ProviderRuntimeExecutionContext {
            workspace_id: uuid::Uuid::nil(),
            actor_id: None,
            deadline_unix_ms: i64::try_from(deadline_ms).unwrap(),
        }
    }

    fn successful_output(
        generation: u64,
    ) -> anyhow::Result<super::super::ProviderRuntimeInvocationOutput> {
        let mut result = ProviderInvocationResult {
            provider_metadata: serde_json::json!({}),
            ..ProviderInvocationResult::default()
        };
        result
            .set_transport_session_receipt(ProviderTransportSessionReceipt {
                generation,
                reused: true,
                physical_state: ProviderPhysicalTransportState::Ready,
                connection_age_ms: 1,
                ttl_remaining_ms: 1_000,
                close_reason: None,
                close_acknowledged: None,
            })
            .unwrap();
        Ok(super::super::ProviderRuntimeInvocationOutput {
            events: Vec::new(),
            result,
        })
    }

    fn reason(error: anyhow::Error) -> String {
        error.to_string()
    }

    #[test]
    fn termination_reasons_preserve_the_transport_failure_domain() {
        assert_eq!(
            termination_code(TerminationKind::DeadlineExceeded(DeadlineKind::Task)),
            "transport_invocation_deadline_exceeded"
        );
        assert_eq!(
            termination_code(TerminationKind::DeadlineExceeded(
                DeadlineKind::LogicalAbsolute
            )),
            "transport_logical_deadline_exceeded"
        );
        assert_eq!(
            termination_code(TerminationKind::DeadlineExceeded(DeadlineKind::StateLease)),
            "transport_state_lease_expired"
        );
        assert_eq!(
            termination_code(TerminationKind::ProviderHardMax),
            "provider_connection_max_age"
        );
        assert_eq!(
            termination_code(TerminationKind::ProviderFault),
            "provider_connection_max_age"
        );
        assert_eq!(
            termination_code(TerminationKind::CapacityEvicted),
            "transport_session_evicted"
        );
        assert_eq!(
            termination_code(TerminationKind::OwnerOrphaned),
            "transport_session_orphaned"
        );
    }

    #[test]
    fn registry_rejections_have_stable_safe_reasons() {
        assert!(
            reason(map_registry_use_error(RegistryError::DeadlineInPast))
                .contains("transport_invocation_deadline_exceeded")
        );
        assert!(
            reason(map_registry_use_error(RegistryError::InflightExists))
                .contains("transport_session_busy")
        );
        assert!(reason(map_registry_use_error(RegistryError::NotFound))
            .contains("transport_session_evicted"));
        assert!(reason(map_registry_admission_error(RegistryError::Capacity(
            CapacityRejection {
                capacity: 1,
                active: 1,
            }
        )))
        .contains("capacity_exceeded"));
        assert!(
            reason(map_registry_admission_error(RegistryError::DeadlineInPast))
                .contains("provider_connection_max_age")
        );
    }

    #[tokio::test]
    async fn compaction_successor_reuses_logical_session_without_inheriting_deadline() {
        let clock = FakeClock::new(1_000_000);
        let runtime = Arc::new(FakeTransportRuntime::new([]));
        let coordinator =
            TransportSessionCoordinator::new_with_clock(runtime, transport_config(), clock.clone())
                .unwrap();
        let mut compact = invocation_input("stable-session", ProviderWireOperation::Compact);
        compact.profile = Some(ProviderCompactProfile::ResponsesCompactionV2);
        let first = coordinator
            .prepare("runtime-a", &mut compact, &context(1_000_010))
            .await
            .unwrap()
            .unwrap();
        let first_directive = compact.transport_session_directive().unwrap().unwrap();
        coordinator
            .finish(first, &successful_output(first_directive.generation))
            .await
            .unwrap();

        clock.advance(Duration::from_millis(11));
        let mut successor = invocation_input("stable-session", ProviderWireOperation::Generate);
        let second = coordinator
            .prepare("runtime-a", &mut successor, &context(1_000_100))
            .await
            .unwrap()
            .unwrap();
        let second_directive = successor.transport_session_directive().unwrap().unwrap();
        let snapshot = coordinator.safe_snapshot().await;

        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(
            first_directive.logical_session_id,
            second_directive.logical_session_id
        );
        assert_eq!(first_directive.generation, second_directive.generation);
        assert_eq!(second.lease.sequence(), 2);
        assert_eq!(
            snapshot.sessions[0].invocation_deadline,
            Some(TransportInstant::from_millis(1_000_100))
        );
        assert_eq!(
            snapshot.sessions[0].invocation_ttl,
            Some(Duration::from_millis(89))
        );
    }

    async fn coordinator_at_inflight_soft_drain(
        behavior: AckBehavior,
    ) -> (
        TransportSessionCoordinator<FakeClock>,
        Arc<FakeTransportRuntime>,
        FakeClock,
        PreparedTransportInvocation,
        u64,
    ) {
        let clock = FakeClock::new(2_000_000);
        let runtime = Arc::new(FakeTransportRuntime::new([behavior]));
        let coordinator = TransportSessionCoordinator::new_with_clock(
            runtime.clone(),
            transport_config(),
            clock.clone(),
        )
        .unwrap();
        let mut input = invocation_input("drain-session", ProviderWireOperation::Generate);
        let prepared = coordinator
            .prepare("runtime-a", &mut input, &context(2_030_000))
            .await
            .unwrap()
            .unwrap();
        let generation = input
            .transport_session_directive()
            .unwrap()
            .unwrap()
            .generation;

        clock.advance(Duration::from_secs(20));
        coordinator.maintain_and_dispatch().await;
        assert!(runtime.commands().is_empty());
        let snapshot = coordinator.safe_snapshot().await;
        assert_eq!(snapshot.sessions[0].state, TransportSessionState::Draining);
        assert!(snapshot.sessions[0].inflight);
        (coordinator, runtime, clock, prepared, generation)
    }

    #[tokio::test]
    async fn inflight_drain_defers_then_matching_ack_rotates_and_fences_old_generation() {
        let (coordinator, runtime, clock, prepared, generation) =
            coordinator_at_inflight_soft_drain(AckBehavior::Matching(true)).await;
        let old_fence = prepared.lease.fence.clone();
        coordinator
            .finish(prepared, &successful_output(generation))
            .await
            .unwrap();

        let commands = runtime.commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].action, ProviderTransportSessionAction::Drain);
        assert_eq!(commands[0].generation, generation);
        let snapshot = coordinator.safe_snapshot().await;
        assert_eq!(snapshot.sessions[0].fence.session_id, old_fence.session_id);
        assert!(snapshot.sessions[0].fence.generation.get() > generation);
        assert_eq!(snapshot.sessions[0].state, TransportSessionState::Active);

        let mut registry = coordinator.registry.lock().await;
        assert!(matches!(
            registry.activate(&old_fence),
            Err(RegistryError::StaleGeneration { .. })
        ));
        assert!(lifecycle_command(
            &registry,
            LifecycleEvent::StateChanged {
                fence: old_fence,
                from: TransportSessionState::Active,
                to: TransportSessionState::Draining,
                at: clock.now(),
            }
        )
        .is_none());
    }

    #[tokio::test]
    async fn false_or_mismatched_drain_ack_never_rotates_generation() {
        for behavior in [AckBehavior::Matching(false), AckBehavior::Mismatched] {
            let (coordinator, runtime, _clock, prepared, generation) =
                coordinator_at_inflight_soft_drain(behavior).await;
            coordinator
                .finish(prepared, &successful_output(generation))
                .await
                .unwrap();

            assert_eq!(runtime.commands().len(), 1);
            let snapshot = coordinator.safe_snapshot().await;
            assert_eq!(snapshot.sessions[0].fence.generation.get(), generation);
            assert_eq!(snapshot.sessions[0].state, TransportSessionState::Draining);
            assert!(!snapshot.sessions[0].inflight);
        }
    }
}
