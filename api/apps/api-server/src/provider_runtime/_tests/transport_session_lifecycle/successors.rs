use super::*;

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
        .prepare_dispatched("runtime-a", &mut compact, &context(1_000_010))
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
        .prepare_dispatched("runtime-a", &mut successor, &context(1_000_100))
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
        .prepare_dispatched("runtime-a", &mut input, &context(2_000_100))
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
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.sequence() > failed_lease.sequence());
    assert!(next.lease.fence.generation > failed_lease.fence.generation);
    let current = coordinator.safe_snapshot().await;
    assert_eq!(current.sessions[0].logical_ttl, Duration::from_secs(59));
    assert_eq!(next.lease.deadline().as_millis(), 2_010_000);
    let old = PreparedTransportInvocation {
        on_abandon: None,
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
        .prepare_dispatched("runtime-a", &mut input, &context(2_000_100))
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
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
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
            .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
            .await
            .err()
            .unwrap();
        assert!(reason(error).contains("provider_transport_committed_invocation"));
        assert_eq!(coordinator.safe_snapshot().await, before);
    }
    directive.initial_commit_level = CommitLevel::LifecycleOnly;
    input.set_recovery_directive(directive).unwrap();
    let next = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
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
        .prepare_dispatched("runtime-a", &mut input, &context(2_000_100))
        .await
        .unwrap()
        .unwrap();
    coordinator
        .finish(failed, &Err(transport_error("primary-provider-error")))
        .await
        .unwrap();
    let before = coordinator.safe_snapshot().await;
    let error = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_020_000))
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
        .prepare_dispatched("runtime-a", &mut input, &context(2_000_000))
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
        .prepare_dispatched("runtime-a", &mut input, &context(2_020_000))
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

