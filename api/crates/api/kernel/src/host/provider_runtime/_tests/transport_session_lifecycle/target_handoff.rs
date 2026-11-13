use super::logical_rollover::two_hour_config;
use super::*;

#[tokio::test]
async fn target_handoff_resumes_idle_compaction_then_generation() {
    for idle_released in [false, true] {
        let clock = FakeClock::new(2_000_000);
        let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
        let coordinator = TransportSessionCoordinator::new_with_clock(
            runtime.clone(),
            transport_config(),
            clock.clone(),
        )
        .unwrap();
        let mut input = invocation_input("upgrade-compaction", ProviderWireOperation::Generate);
        let first = coordinator
            .prepare_dispatched("openai@old", &mut input, &context(2_100_000))
            .await
            .unwrap()
            .unwrap();
        let old = first.lease.clone();
        coordinator
            .finish(first, &successful_output(old.fence.generation.get()))
            .await
            .unwrap();
        let previous = coordinator.safe_snapshot().await.sessions[0].clone();
        if idle_released {
            clock.advance(Duration::from_secs(31));
            coordinator.maintain_and_dispatch().await;
            assert_eq!(
                coordinator.safe_snapshot().await.sessions[0].state,
                TransportSessionState::IdleReleased
            );
        }
        let mut compact = invocation_input("upgrade-compaction", ProviderWireOperation::Compact);
        compact.profile = Some(ProviderCompactProfile::ResponsesCompactionV2);
        let next = coordinator
            .prepare_dispatched("openai@new", &mut compact, &context(2_100_000))
            .await
            .unwrap()
            .unwrap();
        let lease = next.lease.clone();
        assert_eq!(lease.fence.session_id, old.fence.session_id);
        assert!(lease.fence.generation > old.fence.generation);
        assert_eq!(lease.sequence(), old.sequence() + 1);
        let current = coordinator.safe_snapshot().await.sessions[0].clone();
        assert_eq!(
            current.logical_ttl,
            previous.logical_ttl.saturating_sub(if idle_released {
                Duration::from_secs(31)
            } else {
                Duration::ZERO
            })
        );
        assert_eq!(current.owner_id, previous.owner_id);
        assert_eq!(
            runtime.commands().len(),
            1,
            "old-target release must precede dispatch to the replacement"
        );
        assert_eq!(
            coordinator
                .registry
                .lock()
                .await
                .runtime_target_id(&lease.fence)
                .unwrap()
                .as_str(),
            "openai@new"
        );
        assert!(runtime
            .probes
            .lock()
            .unwrap()
            .iter()
            .any(|(target, _, generation)| target == "openai@old"
                && *generation == old.fence.generation.get()));
        assert_eq!(next.transport, RecoveryTransport::ProviderHttp);
        coordinator
            .finish(
                next,
                &output_with_result(ProviderInvocationResult::default()),
            )
            .await
            .unwrap();
        let mut resume = invocation_input("upgrade-compaction", ProviderWireOperation::Generate);
        let continued = coordinator
            .prepare_dispatched("openai@new", &mut resume, &context(2_100_000))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(continued.lease.fence, lease.fence);
        assert_eq!(continued.lease.sequence(), lease.sequence() + 1);
        coordinator
            .finish(continued, &successful_output(lease.fence.generation.get()))
            .await
            .unwrap();
        assert_eq!(runtime.commands().len(), 1);
    }
}

#[tokio::test]
async fn target_handoff_never_overtakes_an_inflight_invocation() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime.clone(), transport_config(), clock)
            .unwrap();
    let mut input = invocation_input("upgrade-active", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare_dispatched("old", &mut input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    let old = first.lease.clone();
    let before = coordinator.safe_snapshot().await;
    let mut compact = invocation_input("upgrade-active", ProviderWireOperation::Compact);
    let error = coordinator
        .prepare_dispatched("new", &mut compact, &context(2_100_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_busy"));
    assert_eq!(coordinator.safe_snapshot().await, before);
    assert!(runtime.commands().is_empty());
    assert_eq!(runtime.bindings.lock().unwrap().len(), 1);
    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    let next = coordinator
        .prepare_dispatched("new", &mut compact, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.transport, RecoveryTransport::ProviderHttp);
    coordinator
        .finish(
            next,
            &output_with_result(ProviderInvocationResult::default()),
        )
        .await
        .unwrap();
    assert_eq!(runtime.commands().len(), 1);
}

#[tokio::test]
async fn target_handoff_retries_old_target_close_before_rebinding() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([
        AckBehavior::Matching(false),
        AckBehavior::Matching(true),
    ]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let mut input = invocation_input("upgrade-close-pending", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare_dispatched("old", &mut input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    let old = first.lease.clone();
    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    let mut compact = invocation_input("upgrade-close-pending", ProviderWireOperation::Compact);
    let error = coordinator
        .prepare_dispatched("new", &mut compact, &context(2_100_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_connection_draining"));
    assert_eq!(
        coordinator
            .registry
            .lock()
            .await
            .runtime_target_id(&old.fence)
            .unwrap()
            .as_str(),
        "old"
    );
    assert_eq!(runtime.bindings.lock().unwrap().len(), 1);
    clock.advance(Duration::from_secs(1));
    coordinator.maintain_and_dispatch().await;
    let next = coordinator
        .prepare_dispatched("new", &mut compact, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation > old.fence.generation);
    assert_eq!(runtime.commands().len(), 2);
    assert_eq!(next.transport, RecoveryTransport::ProviderHttp);
    coordinator
        .finish(
            next,
            &output_with_result(ProviderInvocationResult::default()),
        )
        .await
        .unwrap();
    assert_eq!(runtime.commands().len(), 2);
}

#[tokio::test]
async fn target_handoff_preserves_connection_bound_cursor_guard() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime.clone(), transport_config(), clock)
            .unwrap();
    let mut input = invocation_input("upgrade-cursor", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare_dispatched("old", &mut input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    let old = first.lease.clone();
    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    input.previous_response_id = Some("connection-bound-cursor".into());
    let epoch = TransportEpoch::new(17).unwrap();
    let mut directive = recovery_directive(epoch);
    directive.cursor_provenance = Some(CursorProvenance::connection_bound(
        epoch,
        plugin_framework::provider_contract::SocketIncarnation::new(1).unwrap(),
    ));
    input.set_recovery_directive(directive).unwrap();
    let before = coordinator.safe_snapshot().await;
    let error = coordinator
        .prepare_dispatched("new", &mut input, &context(2_100_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_transport_cursor_unreconstructible"));
    assert_eq!(coordinator.safe_snapshot().await, before);
    assert_eq!(
        input.previous_response_id.as_deref(),
        Some("connection-bound-cursor")
    );
    assert!(runtime.commands().is_empty());
}

#[tokio::test]
async fn target_handoff_recovers_released_tombstone_without_forking_an_active_call() {
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
    let mut input = invocation_input("upgrade-expired", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare_dispatched("old", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    let old = first.lease.clone();
    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    clock.advance(Duration::from_secs(2 * 60 * 60));
    let error = coordinator
        .prepare_dispatched("new", &mut input, &context(10_000_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_physical_connection_close_pending"));
    assert!(coordinator.safe_snapshot().await.sessions.is_empty());
    clock.advance(Duration::from_secs(1));
    coordinator.maintain_and_dispatch().await;
    let next = coordinator
        .prepare_dispatched("new", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.lease.fence.session_id, old.fence.session_id);
    assert!(next.lease.fence.generation > old.fence.generation);
    coordinator
        .finish(
            next,
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
}
