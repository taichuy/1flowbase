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
            AckBehavior::ReleasedWithoutAck => (command.generation, Some(false)),
            AckBehavior::ControlError => {
                return Err(RuntimeBackendError::Execution {
                    target_id: "fixture".into(),
                    message: "never-log-private-control-body".into(),
                })
            }
        };
        Ok(ProviderTransportSessionReceipt {
            closure_evidence: Some(ProviderTransportClosureEvidence {
                source: plugin_framework::provider_contract::ProviderTransportClosureSource::ProviderLocalRelease,
                no_ack_reason: if close_acknowledged == Some(false) { Some(plugin_framework::provider_contract::ProviderTransportNoAckReason::Timeout) } else { None },
                identity: ProviderTransportSessionIdentity {
                    logical_session_id: command.logical_session_id.clone(), generation,
                    worker_incarnation: command.worker_incarnation.unwrap_or(1),
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
        .finish(
            first,
            &output_with_result(ProviderInvocationResult::default()),
        )
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
        .prepare("runtime-a", &mut missing_input, &context(2_000_100))
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
        .prepare("runtime-a", &mut stale_input, &context(2_000_100))
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
        .prepare("runtime-a", &mut old_generation_input, &context(2_000_100))
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
        .prepare("runtime-a", &mut fault_input, &context(2_000_100))
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
        .prepare("runtime-a", &mut fallback_input, &context(2_000_100))
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

#[tokio::test]
async fn physical_fault_successor_keeps_logical_deadline_and_fences_failed_invocation() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let mut notices = coordinator.subscribe();
    let mut input = invocation_input("fault-successor", ProviderWireOperation::Generate);
    let failed = coordinator
        .prepare("runtime-a", &mut input, &context(2_000_100))
        .await
        .unwrap()
        .unwrap();
    let failed_lease = failed.lease.clone();
    coordinator
        .finish(failed, &Err(transport_error("primary-provider-error")))
        .await
        .unwrap();
    let faulted = coordinator.safe_snapshot().await;
    assert_eq!(faulted.sessions[0].state, TransportSessionState::Faulted);
    assert!(!faulted.sessions[0].inflight);
    assert!(faulted.tombstones.is_empty());
    assert!(
        notices.try_recv().is_err(),
        "physical failure is not logical owner termination"
    );
    assert_eq!(
        runtime.commands()[0].action,
        ProviderTransportSessionAction::Close
    );

    // The old invocation deadline has passed; this is a distinct authorized call.
    clock.advance(Duration::from_secs(1));
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.sequence() > failed_lease.sequence());
    assert!(next.lease.fence.generation > failed_lease.fence.generation);
    let current = coordinator.safe_snapshot().await;
    assert_eq!(current.sessions[0].logical_ttl, Duration::from_secs(59));
    assert_eq!(next.lease.deadline().as_millis(), 2_010_000);
    let old = PreparedTransportInvocation {
        lease: failed_lease,
        transport: RecoveryTransport::AiNativeWebSocket,
        recovery_directive: None,
    };
    assert!(reason(
        coordinator
            .finish(old, &Err(transport_error("late-old-error")))
            .await
            .unwrap_err()
    )
    .contains("provider_transport_stale_generation"));
    assert_eq!(coordinator.safe_snapshot().await, current);
    let generation = next.lease.fence.generation.get();
    coordinator
        .finish(next, &successful_output(generation))
        .await
        .unwrap();
}

#[tokio::test]
async fn physical_fault_successor_rejects_bound_cursor_and_committed_replay_without_mutation() {
    let clock = FakeClock::new(2_000_000);
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)])),
        transport_config(),
        clock,
    )
    .unwrap();
    let mut input = invocation_input("cursor-fault", ProviderWireOperation::Generate);
    let failed = coordinator
        .prepare("runtime-a", &mut input, &context(2_000_100))
        .await
        .unwrap()
        .unwrap();
    coordinator
        .finish(failed, &Err(transport_error("primary-provider-error")))
        .await
        .unwrap();
    let before = coordinator.safe_snapshot().await;
    input.previous_response_id = Some("opaque-cursor-fixture".into());
    let epoch = TransportEpoch::new(17).unwrap();
    let mut directive = recovery_directive(epoch);
    directive.cursor_provenance = Some(CursorProvenance::connection_bound(
        epoch,
        plugin_framework::provider_contract::SocketIncarnation::new(1).unwrap(),
    ));
    input.set_recovery_directive(directive.clone()).unwrap();
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_transport_cursor_unreconstructible"));
    assert_eq!(coordinator.safe_snapshot().await, before);

    directive.cursor_provenance = Some(CursorProvenance::durable());
    for commit in [CommitLevel::SemanticCommitted, CommitLevel::Terminal] {
        directive.initial_commit_level = commit;
        input.set_recovery_directive(directive.clone()).unwrap();
        let error = coordinator
            .prepare("runtime-a", &mut input, &context(2_010_000))
            .await
            .err()
            .unwrap();
        assert!(reason(error).contains("provider_transport_committed_invocation"));
        assert_eq!(coordinator.safe_snapshot().await, before);
    }
    directive.initial_commit_level = CommitLevel::LifecycleOnly;
    input.set_recovery_directive(directive).unwrap();
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        input.previous_response_id.as_deref(),
        Some("opaque-cursor-fixture")
    );
    assert!(next.lease.fence.generation > before.sessions[0].fence.generation);
}

#[tokio::test]
async fn physical_fault_successor_requires_matching_close_ack_and_unexpired_deadline() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([
        AckBehavior::Matching(false),
        AckBehavior::Mismatched,
        AckBehavior::Matching(true),
    ]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let mut input = invocation_input("pending-close-fault", ProviderWireOperation::Generate);
    let failed = coordinator
        .prepare("runtime-a", &mut input, &context(2_000_100))
        .await
        .unwrap()
        .unwrap();
    coordinator
        .finish(failed, &Err(transport_error("primary-provider-error")))
        .await
        .unwrap();
    let before = coordinator.safe_snapshot().await;
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(2_020_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_physical_connection_close_pending"));
    assert_eq!(coordinator.safe_snapshot().await, before);
    clock.advance(Duration::from_secs(1));
    coordinator.maintain_and_dispatch().await;
    clock.advance(Duration::from_secs(1));
    // A valid ACK still cannot admit an already-expired invocation or rotate its fence.
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(2_000_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
    assert_eq!(
        coordinator.safe_snapshot().await.sessions[0].fence,
        before.sessions[0].fence
    );
    assert_eq!(runtime.commands().len(), 3);
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(2_020_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation > before.sessions[0].fence.generation);
}

#[test]
fn physical_fault_successor_checks_native_cursor_without_rewriting_payload_or_budget() {
    let mut input = invocation_input("native-cursor-fault", ProviderWireOperation::Generate);
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"previous_response_id":"opaque-native-cursor","input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    let original = input.native_transport.clone();
    let now = TransportInstant::from_millis(2_000_000);
    assert!(validate_fault_successor(None, now).is_ok());
    let mut directive = recovery_directive(TransportEpoch::new(9).unwrap());
    directive.cursor_provenance = None;
    assert!(validate_fault_successor(Some(&directive), now).is_ok());
    directive.policy = RecoveryPolicy::NativeOpaque {
        budget: RecoveryBudget {
            max_inner_attempts: 2,
            absolute_deadline_unix_ms: 2_000_000,
        },
    };
    assert!(
        reason(validate_fault_successor(Some(&directive), now).unwrap_err())
            .contains("transport_invocation_deadline_exceeded")
    );
    assert_eq!(input.native_transport, original);
    assert_eq!(
        directive.policy.budget().absolute_deadline_unix_ms,
        2_000_000
    );
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
        .prepare("runtime-a", &mut input, &context(2_010_000))
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
            .prepare("runtime-a", &mut input, &context(2_010_000))
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
            .prepare("runtime-a", &mut input, &context(2_010_000))
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
        let mut config = TransportRegistryConfig::default();
        config.logical_max_age = Duration::from_secs(200);
        let coordinator =
            TransportSessionCoordinator::new_with_clock(runtime.clone(), config, clock.clone())
                .unwrap();
        let mut notices = coordinator.subscribe();
        let mut input = invocation_input("idle-successor", ProviderWireOperation::Generate);
        let first = coordinator
            .prepare("runtime-a", &mut input, &context(2_010_000))
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
            .prepare("runtime-a", &mut input, &context(2_010_000))
            .await
            .err()
            .unwrap();
        assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
        // The provider owns the opaque cursor. The host must preserve it across
        // idle rotation instead of guessing that missing provenance is invalid.
        input.previous_response_id = Some("provider-owned-cursor".into());
        let second = coordinator
            .prepare("runtime-a", &mut input, &context(2_150_000))
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
        .prepare("runtime-a", &mut input, &context(2_010_000))
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
            .prepare("runtime-a", &mut input, &context(2_150_000))
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
        .prepare("runtime-a", &mut input, &context(2_150_000))
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
        .prepare("runtime-a", &mut input, &context(2_150_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_transport_cursor_unreconstructible"));
    directive.cursor_provenance = Some(CursorProvenance::durable());
    input.set_recovery_directive(directive.clone()).unwrap();
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(2_150_000))
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

#[tokio::test]
async fn scoped_termination_matches_actual_model_fence_and_invocation_only() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)])),
        TransportRegistryConfig::default(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let a = coordinator.open_connection_scope();
    let b = coordinator.open_connection_scope();
    let mut input_a = scope_input(&a, "model-a");
    input_a.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"model":"model-a","input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    let first = coordinator
        .prepare("runtime-a", &mut input_a, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        input_a.client_transport,
        Some(ProviderClientTransport::Websocket)
    );
    assert!(!input_a
        .client_protocol_envelope
        .as_ref()
        .unwrap()
        .headers
        .contains_key(
            control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER
        ));
    let mut input_b = scope_input(&b, "model-b");
    let second = coordinator
        .prepare("runtime-a", &mut input_b, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        input_b.client_transport,
        Some(ProviderClientTransport::Websocket)
    );
    let mut notices = coordinator.subscribe();
    coordinator
        .registry
        .lock()
        .await
        .terminate(&first.lease.fence, TerminationKind::ProviderHardMax)
        .unwrap();
    coordinator.dispatch_pending_events().await;
    let notice = notices.try_recv().unwrap();
    assert!(a.accepts_termination(&notice));
    assert!(!b.accepts_termination(&notice));
    assert_eq!(notice.fence, first.lease.fence);
    assert_eq!(notice.invocation_sequence, Some(first.lease.sequence()));
    coordinator.close_connection_scope(&a).await;
    assert_eq!(
        coordinator
            .registry
            .lock()
            .await
            .state(&second.lease.fence)
            .unwrap(),
        TransportSessionState::Active
    );
}

#[tokio::test]
async fn completed_connection_close_and_old_notices_do_not_orphan_successor() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        TransportRegistryConfig::default(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let old = coordinator.open_connection_scope();
    let first = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&old, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let old_lease = first.lease.clone();
    coordinator
        .finish(first, &successful_output(old_lease.fence.generation.get()))
        .await
        .unwrap();
    assert!(old.state.lock().unwrap().leases.is_empty());
    let current = coordinator.open_connection_scope();
    let second = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&current, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.lease.fence, old_lease.fence);
    assert!(second.lease.sequence() > old_lease.sequence());
    let stale_notice = TransportTerminationNotice {
        owner_id: "shared-thread".into(),
        fence: old_lease.fence.clone(),
        invocation_sequence: Some(old_lease.sequence()),
        connection_scope_id: Some(current.id().into()),
        code: "provider_connection_max_age",
    };
    assert!(
        !current.accepts_termination(&stale_notice),
        "same physical generation does not identify the new invocation"
    );
    coordinator.close_connection_scope(&old).await;
    assert_eq!(
        coordinator
            .registry
            .lock()
            .await
            .state(&second.lease.fence)
            .unwrap(),
        TransportSessionState::Active
    );
    let fence = second.lease.fence.clone();
    coordinator
        .finish(second, &successful_output(fence.generation.get()))
        .await
        .unwrap();
    coordinator.close_connection_scope(&current).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::IdleAffinity
    );
    assert!(coordinator.connection_scopes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn active_scope_disconnect_orphans_only_its_current_lease_and_rejects_forgery() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        TransportRegistryConfig::default(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let mut forged = scope_input(&scope, "model-a");
    forged
        .client_protocol_envelope
        .as_mut()
        .unwrap()
        .headers
        .insert(
            control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER.into(),
            vec!["client-forged".into()],
        );
    assert!(coordinator
        .prepare("runtime-a", &mut forged, &context(2_100_000))
        .await
        .is_err());
    assert!(!forged
        .client_protocol_envelope
        .as_ref()
        .unwrap()
        .headers
        .contains_key(
            control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER
        ));
    assert!(coordinator.safe_snapshot().await.sessions.is_empty());
    let first = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    coordinator.close_connection_scope(&scope).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );
    coordinator
        .finish(first, &successful_output(fence.generation.get()))
        .await
        .unwrap();
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );
}

#[tokio::test]
async fn completed_waiting_tool_retains_business_state_after_socket_close() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        TransportRegistryConfig::default(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    let mut output = successful_output(fence.generation.get()).unwrap();
    output.result.tool_calls = vec![plugin_framework::provider_contract::ProviderToolCall {
        id: "committed-tool".into(),
        name: "read".into(),
        arguments: serde_json::json!({}),
        provider_metadata: serde_json::json!({}),
    }];
    coordinator.finish(first, &Ok(output)).await.unwrap();
    assert!(scope.state.lock().unwrap().leases.is_empty());
    coordinator.close_connection_scope(&scope).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::WaitingTool
    );
}

/// #2085 orphan/mailbox: a delivery unbind must not permanently mark the call as
/// orphaned. This is the exact incident shape - a mailbox-style client
/// disconnect while the model call runs, the original call completing
/// successfully afterwards, and the next round arriving on a new connection.
#[tokio::test]
async fn unbound_delivery_admits_the_successor_round_after_the_original_completes() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    let first_sequence = first.lease.sequence();

    // The client drops the delivery while the model call is still running.
    coordinator.close_connection_scope(&scope).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );

    // Kernel completion is independent of the delivery and still succeeds.
    let mut output = successful_output(fence.generation.get()).unwrap();
    output.result.tool_calls = vec![plugin_framework::provider_contract::ProviderToolCall {
        id: "committed-tool".into(),
        name: "read".into(),
        arguments: serde_json::json!({}),
        provider_metadata: serde_json::json!({}),
    }];
    coordinator.finish(first, &Ok(output)).await.unwrap();
    let snapshot = coordinator.safe_snapshot().await;
    assert_eq!(
        snapshot.sessions[0].state,
        TransportSessionState::Orphaned,
        "the unbound delivery stays visible as a fact"
    );
    assert!(
        !snapshot.sessions[0].inflight,
        "but the execution itself is terminal"
    );

    // The tool result returns on a new connection: same owner identity, same
    // logical session, same physical generation, and no second model call.
    let next_scope = coordinator.open_connection_scope();
    let successor = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&next_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .expect("a legitimate successor must be admitted")
        .unwrap();
    assert_eq!(successor.lease.fence, fence);
    assert!(successor.lease.sequence() > first_sequence);
}

/// Ordering coverage: the successor is admitted on the reclaimed session first,
/// and only then does the *old* delivery finish closing. A late close must not
/// rebind the delivery fact onto the running successor.
#[tokio::test]
async fn a_late_close_of_the_old_delivery_cannot_orphan_the_admitted_successor() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let old_scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&old_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    coordinator.close_connection_scope(&old_scope).await;
    coordinator
        .finish(first, &successful_output(fence.generation.get()))
        .await
        .unwrap();

    let new_scope = coordinator.open_connection_scope();
    let successor = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&new_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Active
    );

    // The old connection's close completes last, with no leases left to detach.
    coordinator.close_connection_scope(&old_scope).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Active,
        "a late old-delivery close must not orphan the running successor"
    );
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&successor.lease.fence)
        .unwrap());
    coordinator
        .finish(successor, &successful_output(fence.generation.get()))
        .await
        .unwrap();
}

#[tokio::test]
async fn unbound_inflight_execution_without_replacement_scope_is_refused() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    let first_sequence = first.lease.sequence();
    coordinator.close_connection_scope(&scope).await;

    let next_scope = coordinator.open_connection_scope();
    let mut unscoped = scope_input(&next_scope, "model-a");
    take_transport_connection_scope(&mut unscoped);
    let error = coordinator
        .prepare("runtime-a", &mut unscoped, &context(2_100_000))
        .await
        .err()
        .expect("an unbound in-flight execution must not start a second call");
    let details = admission_diagnostics(&error);
    assert!(reason(error).contains("transport_session_inflight_unbound"));
    assert_eq!(details["state"], serde_json::json!("orphaned"));
    assert_eq!(details["inflight"], serde_json::json!(true));
    assert!(details["generation"].as_u64().unwrap() >= 1);

    // The refused call consumed nothing: the real successor still gets the very
    // next invocation sequence after the original call finishes.
    let snapshot = coordinator.safe_snapshot().await;
    assert!(snapshot.sessions[0].inflight);
    coordinator
        .finish(first, &successful_output(fence.generation.get()))
        .await
        .unwrap();
    let successor = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&next_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(successor.lease.sequence(), first_sequence + 1);
}

#[tokio::test]
async fn admission_rejections_are_locatable_by_their_exact_branch() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();

    // A delivery scope this host no longer owns: an expired/closed reference is
    // its own branch, not the same fact as a registry orphan.
    let stale_scope = coordinator.open_connection_scope();
    coordinator.close_connection_scope(&stale_scope).await;
    let error = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&stale_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .err()
        .unwrap();
    let details = admission_diagnostics(&error);
    assert!(reason(error).contains("transport_session_scope_unknown"));
    assert_eq!(details["connection_scope_bound"], serde_json::json!(true));
    assert_eq!(details["state"], serde_json::Value::Null);

    // Host shutdown is its own branch as well.
    coordinator.shutdown.store(true, Ordering::Release);
    let error = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&coordinator.open_connection_scope(), "model-a"),
            &context(2_100_000),
        )
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_shutdown"));
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
