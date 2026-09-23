use super::*;

fn two_hour_config() -> TransportRegistryConfig {
    TransportRegistryConfig {
        logical_max_age: Duration::from_secs(2 * 60 * 60),
        idle_affinity_lease: Duration::from_secs(3 * 60 * 60),
        physical_soft_drain_age: Duration::from_secs(3 * 60 * 60),
        physical_max_age: Duration::from_secs(4 * 60 * 60),
        ..transport_config()
    }
}

#[tokio::test]
async fn two_hour_rollover_does_not_close_an_active_provider_call() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        two_hour_config(),
        clock.clone(),
    )
    .unwrap();
    let mut input = invocation_input("long-active-call", ProviderWireOperation::Generate);
    let running = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    let old_generation = running.lease.fence.generation.get();

    clock.advance(Duration::from_secs(2 * 60 * 60));
    coordinator.maintain_and_dispatch().await;
    assert!(
        runtime.commands().is_empty(),
        "Close cannot overtake the active call"
    );
    assert!(coordinator.safe_snapshot().await.tombstones.is_empty());

    coordinator
        .finish(running, &successful_output(old_generation))
        .await
        .unwrap();
    assert_eq!(runtime.commands().len(), 1);
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation.get() > old_generation);
}

#[tokio::test]
async fn completed_session_gets_a_new_bounded_generation_after_two_hours() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([
        AckBehavior::Matching(true),
        AckBehavior::Matching(true),
    ]));
    let mut config = two_hour_config();
    config.physical_soft_drain_age = Duration::from_secs(50 * 60);
    config.physical_max_age = Duration::from_secs(120 * 60 + 1);
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime.clone(), config, clock.clone())
            .unwrap();
    let mut input = invocation_input("long-lived-codex-thread", ProviderWireOperation::Generate);
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

    // The physical transport rotates during the long conversation, while the
    // logical deadline remains anchored to the first admission.
    clock.advance(Duration::from_secs(50 * 60));
    coordinator.maintain_and_dispatch().await;
    let physical_fence = coordinator.safe_snapshot().await.sessions[0].fence.clone();
    assert!(physical_fence.generation > old.fence.generation);
    clock.advance(Duration::from_secs(70 * 60));
    input.previous_response_id = Some("provider-owned-cursor".into());
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    let snapshot = coordinator.safe_snapshot().await;
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(
        snapshot.sessions[0].logical_ttl,
        Duration::from_secs(2 * 60 * 60)
    );
    assert!(next.lease.fence.generation > physical_fence.generation);
    assert_eq!(
        input.previous_response_id.as_deref(),
        Some("provider-owned-cursor")
    );
    assert_eq!(runtime.commands().len(), 2, "old sockets must be released");
    assert!(snapshot
        .tombstones
        .iter()
        .any(|receipt| receipt.fence == physical_fence));

    let stale = PreparedTransportInvocation {
        lease: old,
        transport: RecoveryTransport::AiNativeWebSocket,
        recovery_directive: None,
    };
    assert!(reason(
        coordinator
            .finish(stale, &successful_output(1))
            .await
            .unwrap_err()
    )
    .contains("provider_transport_stale_generation"));
}

#[tokio::test]
async fn expired_inflight_session_cannot_admit_a_parallel_successor() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime, two_hour_config(), clock.clone())
            .unwrap();
    let mut input = invocation_input("inflight-at-limit", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    clock.advance(Duration::from_secs(2 * 60 * 60));
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_busy"));
    let snapshot = coordinator.safe_snapshot().await;
    assert_eq!(snapshot.sessions.len(), 1);
    assert!(snapshot.sessions[0].inflight);
    assert!(snapshot.tombstones.is_empty());

    // The ordinary five-minute diagnostic TTL must not erase an invocation
    // that can still finish under its separate task deadline.
    clock.advance(Duration::from_secs(6 * 60));
    coordinator.maintain_and_dispatch().await;
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_busy"));
    let old_generation = first.lease.fence.generation.get();
    coordinator
        .finish(first, &successful_output(old_generation))
        .await
        .unwrap();
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation.get() > old_generation);
}

#[tokio::test]
async fn expired_inflight_session_can_roll_over_at_its_task_deadline() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime, two_hour_config(), clock.clone())
            .unwrap();
    let mut input = invocation_input("inflight-task-deadline", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare("runtime-a", &mut input, &context(9_300_000))
        .await
        .unwrap()
        .unwrap();
    clock.advance(Duration::from_secs(2 * 60 * 60));
    assert!(reason(
        coordinator
            .prepare("runtime-a", &mut input, &context(10_000_000))
            .await
            .err()
            .unwrap()
    )
    .contains("transport_session_busy"));

    // The old invocation's own deadline arrives before the tombstone TTL.
    clock.advance(Duration::from_secs(100));
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation > first.lease.fence.generation);
}

#[tokio::test]
async fn expired_session_waits_for_physical_release_and_preserves_cursor_guards() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([
        AckBehavior::Matching(false),
        AckBehavior::Matching(true),
    ]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        two_hour_config(),
        clock.clone(),
    )
    .unwrap();
    let mut input = invocation_input("release-before-renewal", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    coordinator
        .finish(
            first,
            &successful_output(
                input
                    .transport_session_directive()
                    .unwrap()
                    .unwrap()
                    .generation,
            ),
        )
        .await
        .unwrap();
    clock.advance(Duration::from_secs(2 * 60 * 60));
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_physical_connection_close_pending"));
    assert!(coordinator.safe_snapshot().await.sessions.is_empty());

    clock.advance(Duration::from_secs(1));
    coordinator.maintain_and_dispatch().await;
    assert_eq!(runtime.commands().len(), 2);
    let error = coordinator
        .prepare("runtime-b", &mut input, &context(10_000_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_evicted"));

    input.previous_response_id = Some("connection-bound-cursor".into());
    let epoch = TransportEpoch::new(17).unwrap();
    let mut directive = recovery_directive(epoch);
    directive.policy = RecoveryPolicy::NativeOpaque {
        budget: RecoveryBudget {
            max_inner_attempts: 2,
            absolute_deadline_unix_ms: 10_000_000,
        },
    };
    directive.cursor_provenance = Some(CursorProvenance::connection_bound(
        epoch,
        plugin_framework::provider_contract::SocketIncarnation::new(1).unwrap(),
    ));
    input.set_recovery_directive(directive).unwrap();
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_transport_cursor_unreconstructible"));
    assert!(coordinator.safe_snapshot().await.sessions.is_empty());

    input.previous_response_id = Some("durable-provider-cursor".into());
    let mut directive = recovery_directive(epoch);
    directive.policy = RecoveryPolicy::NativeOpaque {
        budget: RecoveryBudget {
            max_inner_attempts: 2,
            absolute_deadline_unix_ms: 10_000_000,
        },
    };
    input.set_recovery_directive(directive).unwrap();
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        input.previous_response_id.as_deref(),
        Some("durable-provider-cursor")
    );
    assert!(next.lease.fence.generation.get() > 1);
}
