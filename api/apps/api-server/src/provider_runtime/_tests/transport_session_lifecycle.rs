use super::*;
use orchestration_runtime::transport_session::CapacityRejection;
use plugin_framework::provider_contract::{
    CommitLevel, CursorProvenance, ProtocolContextEnvelope, ProviderCompactProfile,
    ProviderInvocationResult, ProviderPhysicalTransportState, ProviderRecoveryReceipt,
    ProviderTransportSessionCloseReason, ProviderWireOperation, RecoveryBudget,
    RecoveryDisposition, RecoveryPolicy, RecoveryReason, RecoveryTransport, TransportEpoch,
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
    ReleasedWithoutAck,
    ControlError,
}

struct FakeTransportRuntime {
    bindings: StdMutex<BTreeMap<(String, u64), (String, u64)>>,
    responses: StdMutex<VecDeque<AckBehavior>>,
    commands: StdMutex<Vec<ProviderTransportSessionCommand>>,
    worker_exit_evidence: StdMutex<Option<ProviderTransportClosureEvidence>>,
    probes: StdMutex<Vec<(String, String, u64)>>,
}

impl FakeTransportRuntime {
    fn new(responses: impl IntoIterator<Item = AckBehavior>) -> Self {
        Self {
            responses: StdMutex::new(responses.into_iter().collect()),
            bindings: StdMutex::new(BTreeMap::new()),
            commands: StdMutex::new(Vec::new()),
            worker_exit_evidence: StdMutex::new(None),
            probes: StdMutex::new(Vec::new()),
        }
    }

    fn commands(&self) -> Vec<ProviderTransportSessionCommand> {
        self.commands.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl TransportLifecycleRuntime for FakeTransportRuntime {
    fn observe_dispatch(&self, target_id: &str, lease: &InvocationLease) {
        self.bindings.lock().unwrap().insert(
            (
                lease.fence.session_id.as_str().into(),
                lease.fence.generation.get(),
            ),
            (target_id.into(), 1),
        );
    }
    async fn transport_worker_exit_evidence(
        &self,
        target_id: &str,
        logical_session_id: &str,
        generation: u64,
    ) -> Result<Option<ProviderTransportClosureEvidence>, RuntimeBackendError> {
        self.probes
            .lock()
            .unwrap()
            .push((target_id.into(), logical_session_id.into(), generation));
        Ok(self.worker_exit_evidence.lock().unwrap().clone())
    }

    async fn transport_session(
        &self,
        target_id: &str,
        command: ProviderTransportSessionCommand,
    ) -> Result<ProviderTransportSessionReceipt, RuntimeBackendError> {
        self.commands.lock().unwrap().push(command.clone());
        let binding = self
            .bindings
            .lock()
            .unwrap()
            .get(&(command.logical_session_id.clone(), command.generation))
            .cloned();
        let Some((bound_target, incarnation)) = binding else {
            return Err(PluginFrameworkError::runtime(ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::ProviderTransportUnavailable,
                "transport worker binding evidence is unavailable",
            ))
            .into());
        };
        if bound_target != target_id || command.worker_incarnation.is_some_and(|i| i != incarnation)
        {
            return Err(PluginFrameworkError::runtime(ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::ProviderTransportUnavailable,
                "transport control binding target or worker incarnation mismatch",
            ))
            .into());
        }
        let behavior = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("a fake ACK behavior must be configured for every command");
        let (generation, close_acknowledged) = match behavior {
            AckBehavior::Matching(acknowledged) => (command.generation, Some(acknowledged)),
            AckBehavior::Mismatched => (command.generation + 1, Some(true)),
            AckBehavior::ReleasedWithoutAck => (command.generation, Some(false)),
            AckBehavior::ControlError => {
                return Err(RuntimeBackendError::Execution {
                    target_id: "fixture".into(),
                    message: "never-log-private-control-body".into(),
                })
            }
        };
        if matches!(
            behavior,
            AckBehavior::Matching(true) | AckBehavior::ReleasedWithoutAck
        ) {
            self.bindings
                .lock()
                .unwrap()
                .remove(&(command.logical_session_id.clone(), command.generation));
        }
        Ok(ProviderTransportSessionReceipt {
            closure_evidence: Some(ProviderTransportClosureEvidence {
                source: plugin_framework::provider_contract::ProviderTransportClosureSource::ProviderLocalRelease,
                no_ack_reason: if close_acknowledged == Some(false) { Some(plugin_framework::provider_contract::ProviderTransportNoAckReason::Timeout) } else { None },
                identity: ProviderTransportSessionIdentity {
                    logical_session_id: command.logical_session_id.clone(), generation,
                    worker_incarnation: incarnation,
                },
                local_released: !matches!(behavior, AckBehavior::Matching(false)),
                peer_close_acknowledged: close_acknowledged,
            }),
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

fn invocation_input(session_id: &str, operation: ProviderWireOperation) -> ProviderInvocationInput {
    ProviderInvocationInput {
        operation,
        provider_instance_id: "provider-instance".to_string(),
        provider_code: "provider-code".to_string(),
        protocol: "openai_responses".to_string(),
        model: "model-a".to_string(),
        model_parameters: BTreeMap::from([(
            "use_responses_websocket".into(),
            serde_json::json!(true),
        )]),
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
            closure_evidence: None,
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

fn output_with_result(
    result: ProviderInvocationResult,
) -> anyhow::Result<super::super::ProviderRuntimeInvocationOutput> {
    Ok(super::super::ProviderRuntimeInvocationOutput {
        events: Vec::new(),
        result,
    })
}

fn recovery_directive(epoch: TransportEpoch) -> ProviderRecoveryDirective {
    ProviderRecoveryDirective {
        policy: RecoveryPolicy::SemanticMapped {
            budget: RecoveryBudget {
                max_inner_attempts: 2,
                absolute_deadline_unix_ms: 2_100_000,
            },
        },
        transport_epoch: epoch,
        initial_commit_level: CommitLevel::LifecycleOnly,
        cursor_provenance: Some(CursorProvenance::durable()),
    }
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
        "provider_physical_connection_fault"
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
async fn invocation_transport_outcomes_preserve_missing_stale_fault_and_http_fallback() {
    let clock = FakeClock::new(2_000_000);

    let missing_runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let missing_coordinator = TransportSessionCoordinator::new_with_clock(
        missing_runtime,
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let mut missing_input = invocation_input("missing-receipt", ProviderWireOperation::Generate);
    let missing = missing_coordinator
        .prepare_dispatched("runtime-a", &mut missing_input, &context(2_000_100))
        .await
        .unwrap()
        .unwrap();
    let missing_error = missing_coordinator
        .finish(
            missing,
            &output_with_result(ProviderInvocationResult::default()),
        )
        .await
        .unwrap_err();
    assert!(reason(missing_error).contains("provider_transport_receipt_missing"));
    assert_eq!(
        missing_coordinator.safe_snapshot().await.sessions[0].state,
        TransportSessionState::Faulted
    );

    let stale_coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let mut stale_input = invocation_input("stale-receipt", ProviderWireOperation::Generate);
    let stale = stale_coordinator
        .prepare_dispatched("runtime-a", &mut stale_input, &context(2_000_100))
        .await
        .unwrap()
        .unwrap();
    let stale_generation = stale.lease.fence.generation.get();
    let before_stale = stale_coordinator.safe_snapshot().await;
    let stale_error = stale_coordinator
        .finish(stale, &successful_output(stale_generation + 1))
        .await
        .unwrap_err();
    assert!(reason(stale_error).contains("provider_transport_stale_generation"));
    assert_eq!(stale_coordinator.safe_snapshot().await, before_stale);

    let old_generation_coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let mut old_generation_input =
        invocation_input("old-generation", ProviderWireOperation::Generate);
    let old_generation = old_generation_coordinator
        .prepare_dispatched("runtime-a", &mut old_generation_input, &context(2_000_100))
        .await
        .unwrap()
        .unwrap();
    let old_generation_number = old_generation.lease.fence.generation.get();
    {
        let mut registry = old_generation_coordinator.registry.lock().await;
        registry
            .finish_invocation(&old_generation.lease, InvocationCompletion::Active)
            .unwrap();
        registry
            .transition(&old_generation.lease.fence, TransportSessionState::Draining)
            .unwrap();
        registry.record_closure_evidence(&old_generation.lease.fence, &ProviderTransportClosureEvidence {
            identity: ProviderTransportSessionIdentity {
                logical_session_id: old_generation.lease.fence.session_id.as_str().into(),
                generation: old_generation_number, worker_incarnation: 1,
            }, source: plugin_framework::provider_contract::ProviderTransportClosureSource::ProviderLocalRelease,
            local_released: true, peer_close_acknowledged: Some(true), no_ack_reason: None,
        }).unwrap();
        let next = registry
            .rotate_generation(&old_generation.lease.fence)
            .unwrap();
        registry.activate(&next).unwrap();
    }
    let current_generation = old_generation_coordinator.safe_snapshot().await;
    let old_generation_error = old_generation_coordinator
        .finish(old_generation, &successful_output(old_generation_number))
        .await
        .unwrap_err();
    assert!(reason(old_generation_error).contains("provider_transport_stale_generation"));
    assert_eq!(
        old_generation_coordinator.safe_snapshot().await,
        current_generation
    );

    let fault_runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let fault_coordinator = TransportSessionCoordinator::new_with_clock(
        fault_runtime,
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let mut fault_input = invocation_input("physical-fault", ProviderWireOperation::Generate);
    let fault = fault_coordinator
        .prepare_dispatched("runtime-a", &mut fault_input, &context(2_000_100))
        .await
        .unwrap()
        .unwrap();
    let mut fault_result = ProviderInvocationResult {
        provider_metadata: serde_json::json!({}),
        ..ProviderInvocationResult::default()
    };
    fault_result
        .set_transport_session_receipt(ProviderTransportSessionReceipt {
            closure_evidence: None,
            generation: fault.lease.fence.generation.get(),
            reused: true,
            physical_state: ProviderPhysicalTransportState::Faulted,
            connection_age_ms: 1,
            ttl_remaining_ms: 0,
            close_reason: Some(ProviderTransportSessionCloseReason::TransportFault),
            close_acknowledged: None,
        })
        .unwrap();
    let fault_error = fault_coordinator
        .finish(fault, &output_with_result(fault_result))
        .await
        .unwrap_err();
    assert!(reason(fault_error).contains("provider_physical_connection_fault"));
    assert_eq!(
        fault_coordinator.safe_snapshot().await.sessions[0].state,
        TransportSessionState::Faulted
    );

    let fallback_coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        clock,
    )
    .unwrap();
    let epoch = TransportEpoch::new(9).unwrap();
    let directive = recovery_directive(epoch);
    let mut fallback_input = invocation_input("http-fallback", ProviderWireOperation::Generate);
    fallback_input
        .set_recovery_directive(directive.clone())
        .unwrap();
    let fallback = fallback_coordinator
        .prepare_dispatched("runtime-a", &mut fallback_input, &context(2_000_100))
        .await
        .unwrap()
        .unwrap();
    let mut fallback_result = ProviderInvocationResult {
        provider_metadata: serde_json::json!({}),
        ..ProviderInvocationResult::default()
    };
    fallback_result
        .set_recovery_receipt(ProviderRecoveryReceipt {
            attempt: 0,
            transport: RecoveryTransport::ProviderHttp,
            transport_epoch: epoch,
            socket_incarnation: None,
            commit_level: CommitLevel::LifecycleOnly,
            disposition: RecoveryDisposition::PreCommitHttpFallback,
            reason: RecoveryReason::TransportDisconnected,
        })
        .unwrap();
    fallback_coordinator
        .finish(fallback, &output_with_result(fallback_result))
        .await
        .unwrap();
    let fallback_snapshot = fallback_coordinator.safe_snapshot().await;
    assert_eq!(fallback_snapshot.sessions.len(), 1);
    assert_eq!(
        fallback_snapshot.sessions[0].state,
        TransportSessionState::IdleAffinity
    );
    assert!(fallback_snapshot.tombstones.is_empty());
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
        .prepare_dispatched("runtime-a", &mut input, &context(2_030_000))
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
async fn inflight_drain_defers_then_matching_ack_rotates_only_on_next_invocation() {
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
    assert_eq!(snapshot.sessions[0].fence, old_fence);
    assert_eq!(snapshot.sessions[0].state, TransportSessionState::Draining);
    assert!(
        snapshot.sessions[0]
            .closure_evidence
            .as_ref()
            .unwrap()
            .local_released
    );
    let mut input = invocation_input("drain-session", ProviderWireOperation::Generate);
    let next = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_040_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation.get() > generation);
    assert_eq!(runtime.commands().len(), 1);

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

#[test]
fn execution_mode_is_selected_from_input_not_missing_receipts() {
    let mut input = invocation_input("mode-selection", ProviderWireOperation::Generate);
    input.provider_config = serde_json::json!({"transport_mode":"http_sse"});
    assert_eq!(
        selected_responses_transport(&input).unwrap(),
        RecoveryTransport::AiNativeWebSocket
    );
    input
        .model_parameters
        .insert("use_responses_websocket".into(), serde_json::json!(false));
    input.provider_config = serde_json::json!({"transport_mode":"responses_websocket"});
    assert_eq!(
        selected_responses_transport(&input).unwrap(),
        RecoveryTransport::AiNativeWebSocket
    );
    input.model_parameters.clear();
    for mode in ["auto", "responses_websocket", "websocket", "ws", ""] {
        input.provider_config = serde_json::json!({"transport_mode":mode});
        assert_eq!(
            selected_responses_transport(&input).unwrap(),
            RecoveryTransport::AiNativeWebSocket
        );
    }
    for mode in ["http_sse", "sse", "http"] {
        input.provider_config = serde_json::json!({"transport_mode":mode});
        assert_eq!(
            selected_responses_transport(&input).unwrap(),
            RecoveryTransport::ProviderHttp
        );
    }
    input.provider_config = serde_json::json!({});
    assert_eq!(
        selected_responses_transport(&input).unwrap(),
        RecoveryTransport::ProviderHttp
    );
    input.provider_config = serde_json::json!({"transport_mode":null});
    assert_eq!(
        selected_responses_transport(&input).unwrap(),
        RecoveryTransport::AiNativeWebSocket
    );
    input
        .model_parameters
        .insert("use_responses_websocket".into(), serde_json::json!("true"));
    assert!(selected_responses_transport(&input).is_err());
    input.operation = ProviderWireOperation::Compact;
    assert_eq!(
        selected_responses_transport(&input).unwrap(),
        RecoveryTransport::ProviderHttp
    );
}

#[test]
fn node_policy_follows_host_observed_client_transport_or_forces_selection() {
    let mut input = invocation_input("policy-selection", ProviderWireOperation::Generate);
    input.model_parameters.clear();
    input.provider_config = serde_json::json!({"transport_mode":"http_sse"});
    input.client_transport = Some(ProviderClientTransport::Websocket);
    for (policy, expected) in [
        ("inherit", RecoveryTransport::AiNativeWebSocket),
        ("force_http_sse", RecoveryTransport::ProviderHttp),
        ("force_websocket", RecoveryTransport::AiNativeWebSocket),
    ] {
        input.model_parameters.insert(
            "responses_transport_policy".into(),
            serde_json::json!(policy),
        );
        assert_eq!(selected_responses_transport(&input).unwrap(), expected);
    }
    input.client_transport = Some(ProviderClientTransport::Http);
    input.model_parameters.insert(
        "responses_transport_policy".into(),
        serde_json::json!("inherit"),
    );
    assert_eq!(
        selected_responses_transport(&input).unwrap(),
        RecoveryTransport::ProviderHttp
    );
    input.model_parameters.insert(
        "responses_transport_policy".into(),
        serde_json::json!("invalid"),
    );
    assert!(selected_responses_transport(&input).is_err());
}

#[tokio::test]
async fn selected_http_success_without_websocket_receipt_is_accepted() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let mut input = invocation_input("selected-http", ProviderWireOperation::Generate);
    input.provider_config = serde_json::json!({"transport_mode":"http_sse"});
    input
        .model_parameters
        .insert("use_responses_websocket".into(), serde_json::json!(false));
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"model":"model-a","input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    let prepared = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(input.client_transport, Some(ProviderClientTransport::Http));
    assert_eq!(prepared.transport, RecoveryTransport::ProviderHttp);
    coordinator
        .finish(
            prepared,
            &output_with_result(ProviderInvocationResult::default()),
        )
        .await
        .unwrap();
    let snapshot = coordinator.safe_snapshot().await;
    assert_eq!(
        snapshot.sessions[0].state,
        TransportSessionState::IdleAffinity
    );
    assert!(snapshot.tombstones.is_empty());
}

#[tokio::test]
async fn http_responses_native_continuation_keeps_provider_websocket_choice() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let mut input = invocation_input("http-ws-continuation", ProviderWireOperation::Generate);
    input.model_parameters.clear();
    input.provider_config = serde_json::json!({"transport_mode":"responses_websocket"});
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"model":"model-a","previous_response_id":"resp_previous","input":"next"}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    let prepared = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(input.client_transport, None);
    assert_eq!(prepared.transport, RecoveryTransport::AiNativeWebSocket);
}

#[tokio::test]
async fn draining_and_closing_are_not_reported_as_hard_max_age() {
    for (state, expected) in [
        (
            TransportSessionState::Draining,
            "provider_connection_draining",
        ),
        (
            TransportSessionState::Closing,
            "provider_connection_closing",
        ),
    ] {
        let coordinator = TransportSessionCoordinator::new_with_clock(
            Arc::new(FakeTransportRuntime::new([])),
            transport_config(),
            FakeClock::new(2_000_000),
        )
        .unwrap();
        let mut input = invocation_input("closing-reason", ProviderWireOperation::Generate);
        let prepared = coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
            .await
            .unwrap()
            .unwrap();
        coordinator
            .registry
            .lock()
            .await
            .transition(&prepared.lease.fence, state)
            .unwrap();
        let error = coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
            .await
            .err()
            .unwrap();
        assert!(reason(error).contains(expected));
    }
}

#[tokio::test]
async fn idle_release_admits_new_call_after_90_seconds_without_replaying_expired_call() {
    for elapsed in [89, 90, 91] {
        let clock = FakeClock::new(2_000_000);
        let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
        let config = TransportRegistryConfig {
            logical_max_age: Duration::from_secs(200),
            ..Default::default()
        };
        let coordinator =
            TransportSessionCoordinator::new_with_clock(runtime.clone(), config, clock.clone())
                .unwrap();
        let mut notices = coordinator.subscribe();
        let mut input = invocation_input("idle-successor", ProviderWireOperation::Generate);
        let first = coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
            .await
            .unwrap()
            .unwrap();
        let old = first.lease.clone();
        coordinator
            .finish(first, &successful_output(old.fence.generation.get()))
            .await
            .unwrap();
        clock.advance(Duration::from_secs(elapsed));
        let error = coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
            .await
            .err()
            .unwrap();
        assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
        // The provider owns the opaque cursor. The host must preserve it across
        // idle rotation instead of guessing that missing provenance is invalid.
        input.previous_response_id = Some("provider-owned-cursor".into());
        let second = coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_150_000))
            .await
            .unwrap()
            .unwrap();
        assert!(second.lease.sequence() > old.sequence());
        assert_eq!(
            input.previous_response_id.as_deref(),
            Some("provider-owned-cursor")
        );
        assert_eq!(
            second.lease.fence.generation > old.fence.generation,
            elapsed >= 90
        );
        assert_eq!(
            coordinator.safe_snapshot().await.sessions[0].logical_ttl,
            Duration::from_secs(200 - elapsed)
        );
        assert!(
            notices.try_recv().is_err(),
            "idle resource release is not logical termination"
        );
        assert_eq!(runtime.commands().len(), usize::from(elapsed >= 90));
        if elapsed >= 90 {
            let late = PreparedTransportInvocation {
                on_abandon: None,
                lease: old,
                transport: RecoveryTransport::AiNativeWebSocket,
                recovery_directive: None,
            };
            assert!(reason(
                coordinator
                    .finish(late, &successful_output(1))
                    .await
                    .unwrap_err()
            )
            .contains("provider_transport_stale_generation"));
        }
    }
}

#[tokio::test]
async fn idle_release_close_ack_cursor_and_commit_guards_are_composed() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([
        AckBehavior::Matching(false),
        AckBehavior::Mismatched,
        AckBehavior::Matching(true),
    ]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        TransportRegistryConfig::default(),
        clock.clone(),
    )
    .unwrap();
    let mut input = invocation_input("idle-guards", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    let old = first.lease.clone();
    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    clock.advance(Duration::from_secs(90));
    for _ in 0..2 {
        let error = coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_150_000))
            .await
            .err()
            .unwrap();
        assert!(reason(error).contains("provider_physical_connection_close_pending"));
        assert_eq!(
            coordinator.safe_snapshot().await.sessions[0].fence,
            old.fence
        );
        clock.advance(Duration::from_secs(1));
    }
    let epoch = TransportEpoch::new(17).unwrap();
    let mut directive = recovery_directive(epoch);
    directive.policy = RecoveryPolicy::NativeOpaque {
        budget: RecoveryBudget {
            max_inner_attempts: 2,
            absolute_deadline_unix_ms: 2_150_000,
        },
    };
    directive.initial_commit_level = CommitLevel::Terminal;
    input.set_recovery_directive(directive.clone()).unwrap();
    let error = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_150_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_transport_committed_invocation"));
    directive.initial_commit_level = CommitLevel::LifecycleOnly;
    directive.cursor_provenance = Some(CursorProvenance::connection_bound(
        epoch,
        plugin_framework::provider_contract::SocketIncarnation::new(1).unwrap(),
    ));
    input.previous_response_id = Some("cursor".into());
    input.set_recovery_directive(directive.clone()).unwrap();
    let error = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_150_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_transport_cursor_unreconstructible"));
    directive.cursor_provenance = Some(CursorProvenance::durable());
    input.set_recovery_directive(directive.clone()).unwrap();
    let next = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_150_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation > old.fence.generation);
    assert_eq!(input.previous_response_id.as_deref(), Some("cursor"));
    assert_eq!(input.recovery_directive().unwrap(), Some(directive));
    assert_eq!(runtime.commands().len(), 3);
}

fn scope_input(scope: &TransportConnectionScope, model: &str) -> ProviderInvocationInput {
    let mut input = invocation_input("shared-thread", ProviderWireOperation::Generate);
    input.model = model.into();
    input
        .client_protocol_envelope
        .as_mut()
        .unwrap()
        .headers
        .insert(
            control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER.into(),
            vec![scope.id().into()],
        );
    input
}

/// Typed, secret-free admission diagnostics: the exact rejection branch plus the
/// opaque session identity, generation, logical state and execution flag.
fn admission_diagnostics(error: &anyhow::Error) -> serde_json::Value {
    let contract = error
        .downcast_ref::<PluginFrameworkError>()
        .expect("an admission rejection must stay a typed provider error");
    let PluginFrameworkError::RuntimeContract { error } = contract else {
        panic!("unexpected contract error shape");
    };
    error
        .provider_details
        .as_ref()
        .and_then(|details| details.get("transport_admission"))
        .cloned()
        .expect("admission diagnostics must be attached")
}

#[path = "transport_session_lifecycle/close_control.rs"]
mod close_control;

#[path = "transport_session_lifecycle/prewarm_handoff.rs"]
mod prewarm_handoff;

#[path = "transport_session_lifecycle/invocation_handoff.rs"]
mod invocation_handoff;

#[path = "transport_session_lifecycle/logical_rollover.rs"]
mod logical_rollover;

#[path = "transport_session_lifecycle/orphan_rollover.rs"]
mod orphan_rollover;

#[path = "transport_session_lifecycle/worker_exit_recovery.rs"]
mod worker_exit_recovery;

impl<C: TransportClock + 'static> TransportSessionCoordinator<C> {
    async fn prepare_dispatched(
        &self,
        target_id: &str,
        input: &mut ProviderInvocationInput,
        context: &ProviderRuntimeExecutionContext,
    ) -> anyhow::Result<Option<PreparedTransportInvocation>> {
        let prepared = self.prepare(target_id, input, context).await?;
        if let Some(prepared) = prepared.as_ref() {
            self.claim_dispatch(prepared).await?;
        }
        Ok(prepared)
    }
}

#[path = "transport_session_lifecycle/dispatch_settlement.rs"]
mod dispatch_settlement;

#[path = "transport_session_lifecycle/target_handoff.rs"]
mod target_handoff;

#[path = "transport_session_lifecycle/http_carrier.rs"]
mod http_carrier;

mod connection_scope;
#[path = "transport_session_lifecycle/successors.rs"]
mod successors;
