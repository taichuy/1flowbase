use super::*;

fn hard_max_config() -> TransportRegistryConfig {
    TransportRegistryConfig {
        physical_soft_drain_age: Duration::from_secs(9),
        physical_max_age: Duration::from_secs(10),
        ..Default::default()
    }
}

#[tokio::test]
async fn hard_max_release_allows_only_a_new_invocation_and_fences_delayed_results() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime, hard_max_config(), clock.clone())
            .unwrap();
    let mut input = invocation_input("hard-max-successor", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    let old = first.lease.clone();
    let mut notices = coordinator.subscribe();
    clock.advance(Duration::from_secs(10));
    coordinator.maintain_and_dispatch().await;
    let receipt = coordinator.safe_snapshot().await.tombstones[0].clone();
    assert_eq!(receipt.kind, TerminationKind::ProviderHardMax);
    assert!(receipt.closure_evidence.unwrap().local_released);
    // Even a released transport cannot admit over the unsettled old execution.
    assert!(reason(
        coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_100_000))
            .await
            .err()
            .unwrap()
    )
    .contains("provider_connection_max_age"));
    clock.advance(Duration::from_secs(1));
    // The registry clears the tombstone's lease and still returns NotFound by contract.
    assert!(coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .is_err());
    assert!(coordinator.safe_snapshot().await.tombstones[0]
        .unsettled_invocation
        .is_none());
    clock.advance(Duration::from_secs(1));
    input.previous_response_id = Some("durable-provider-cursor".into());
    let next = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation > old.fence.generation);
    assert_eq!(next.lease.deadline().as_millis(), 2_100_000);
    assert_eq!(
        input.previous_response_id.as_deref(),
        Some("durable-provider-cursor")
    );
    let current = coordinator.safe_snapshot().await;
    assert_eq!(current.sessions[0].physical_ttl, Duration::from_secs(10));
    let notice = notices.try_recv().unwrap();
    assert_eq!(notice.fence, old.fence);
    assert_ne!(notice.fence, next.lease.fence);
    let delayed = PreparedTransportInvocation {
        on_abandon: None,
        lease: old,
        transport: RecoveryTransport::AiNativeWebSocket,
        recovery_directive: None,
    };
    assert!(reason(
        coordinator
            .finish(delayed, &successful_output(1))
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
async fn hard_max_successor_keeps_release_and_recovery_guards() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([
        AckBehavior::Matching(false),
        AckBehavior::Matching(true),
    ]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        hard_max_config(),
        clock.clone(),
    )
    .unwrap();
    let mut input = invocation_input("hard-max-guards", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    let generation = first.lease.fence.generation.get();
    coordinator
        .finish(first, &successful_output(generation))
        .await
        .unwrap();
    clock.advance(Duration::from_secs(10));
    coordinator.maintain_and_dispatch().await;
    assert!(reason(
        coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_100_000))
            .await
            .err()
            .unwrap()
    )
    .contains("provider_physical_connection_close_pending"));
    clock.advance(Duration::from_secs(1));
    coordinator.maintain_and_dispatch().await;
    assert!(
        coordinator.safe_snapshot().await.tombstones[0]
            .closure_evidence
            .as_ref()
            .unwrap()
            .local_released
    );
    let before = coordinator.safe_snapshot().await;
    let epoch = TransportEpoch::new(17).unwrap();
    let mut directive = recovery_directive(epoch);
    directive.cursor_provenance = Some(CursorProvenance::connection_bound(
        epoch,
        plugin_framework::provider_contract::SocketIncarnation::new(1).unwrap(),
    ));
    input.set_recovery_directive(directive.clone()).unwrap();
    assert!(reason(
        coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_100_000))
            .await
            .err()
            .unwrap()
    )
    .contains("provider_transport_cursor_unreconstructible"));
    assert_eq!(coordinator.safe_snapshot().await, before);
    directive.cursor_provenance = Some(CursorProvenance::durable());
    for commit in [CommitLevel::SemanticCommitted, CommitLevel::Terminal] {
        directive.initial_commit_level = commit;
        input.set_recovery_directive(directive.clone()).unwrap();
        assert!(reason(
            coordinator
                .prepare_dispatched("runtime-a", &mut input, &context(2_100_000))
                .await
                .err()
                .unwrap()
        )
        .contains("provider_transport_committed_invocation"));
        assert_eq!(coordinator.safe_snapshot().await, before);
    }
    directive.initial_commit_level = CommitLevel::LifecycleOnly;
    directive.policy = RecoveryPolicy::SemanticMapped {
        budget: RecoveryBudget {
            max_inner_attempts: 2,
            absolute_deadline_unix_ms: 2_011_000,
        },
    };
    input.set_recovery_directive(directive.clone()).unwrap();
    assert!(reason(
        coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_100_000))
            .await
            .err()
            .unwrap()
    )
    .contains("transport_invocation_deadline_exceeded"));
    assert_eq!(coordinator.safe_snapshot().await, before);
    directive.policy = RecoveryPolicy::SemanticMapped {
        budget: RecoveryBudget {
            max_inner_attempts: 2,
            absolute_deadline_unix_ms: 2_100_000,
        },
    };
    input.set_recovery_directive(directive).unwrap();
    let next = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation.get() > generation);
    assert_eq!(runtime.commands().len(), 2);
}

#[tokio::test]
async fn hard_max_successor_rejects_wrong_owner_or_provider_and_accepts_physical_deadline() {
    for kind in [
        TerminationKind::ProviderHardMax,
        TerminationKind::DeadlineExceeded(DeadlineKind::PhysicalHard),
    ] {
        for mismatch in [0, 1, 2] {
            let clock = FakeClock::new(2_000_000);
            let coordinator = TransportSessionCoordinator::new_with_clock(
                Arc::new(FakeTransportRuntime::new([])),
                hard_max_config(),
                clock,
            )
            .unwrap();
            let mut input = invocation_input("hard-max-identity", ProviderWireOperation::Generate);
            let context = context(2_100_000);
            // Seed the exact session through the real admission owner, then reuse its
            // opaque identity with an intentionally foreign registry owner/provider.
            let reserved = coordinator
                .prepare("runtime-a", &mut input, &context)
                .await
                .unwrap()
                .unwrap();
            let session_id = reserved.lease.fence.session_id.clone();
            coordinator
                .finish(reserved, &Err(transport_error("never dispatched")))
                .await
                .unwrap();
            let mut registry = coordinator.registry.lock().await;
            let old = registry.safe_snapshot().sessions[0].fence.clone();
            registry
                .terminate(&old, TerminationKind::OwnerClosed)
                .unwrap();
            let fence = registry
                .admit(AdmissionRequest {
                    session_id,
                    owner_id: TransportOwnerId::new(if mismatch == 1 {
                        "foreign-owner"
                    } else {
                        "hard-max-identity"
                    })
                    .unwrap(),
                    provider_id: TransportProviderId::new(if mismatch == 2 {
                        "foreign-provider"
                    } else {
                        "provider-instance"
                    })
                    .unwrap(),
                    runtime_target_id: TransportRuntimeTargetId::new("runtime-a").unwrap(),
                    provider_hard_deadline: None,
                })
                .unwrap();
            registry.activate(&fence).unwrap();
            registry.settle_never_dispatched(&fence).unwrap();
            registry.terminate(&fence, kind).unwrap();
            drop(registry);
            let result = coordinator
                .prepare_dispatched("runtime-a", &mut input, &context)
                .await;
            if mismatch == 0 {
                assert!(result.unwrap().unwrap().lease.fence.generation > fence.generation);
            } else {
                assert!(reason(result.err().unwrap()).contains("transport_session_evicted"));
                assert!(coordinator.safe_snapshot().await.sessions.is_empty());
            }
        }
    }
}
