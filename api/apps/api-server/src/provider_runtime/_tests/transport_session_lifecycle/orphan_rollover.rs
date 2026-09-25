use super::*;

#[tokio::test]
async fn orphaned_tombstone_requires_release_then_admits_same_owner_on_new_socket() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([
        AckBehavior::Matching(false),
        AckBehavior::Matching(true),
    ]));
    let config = TransportRegistryConfig {
        orphan_grace: Duration::from_secs(1),
        idle_affinity_lease: Duration::from_secs(1),
        ..Default::default()
    };
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime.clone(), config, clock.clone())
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
    let old_fence = first.lease.fence.clone();
    coordinator.close_connection_scope(&old_scope).await;
    coordinator
        .finish(first, &successful_output(old_fence.generation.get()))
        .await
        .unwrap();

    clock.advance(Duration::from_secs(1));
    coordinator.maintain_and_dispatch().await;
    let receipt = coordinator
        .safe_snapshot()
        .await
        .tombstones
        .into_iter()
        .find(|receipt| receipt.fence == old_fence)
        .unwrap();
    assert_eq!(receipt.kind, TerminationKind::OwnerOrphaned);
    assert!(!receipt.closure_evidence.unwrap().local_released);

    let new_scope = coordinator.open_connection_scope();
    let mut successor_input = scope_input(&new_scope, "model-a");
    let error = coordinator
        .prepare("runtime-a", &mut successor_input, &context(2_100_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_physical_connection_close_pending"));

    clock.advance(Duration::from_secs(1));
    coordinator.maintain_and_dispatch().await;
    let successor = coordinator
        .prepare("runtime-a", &mut successor_input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    assert!(successor.lease.fence.generation > old_fence.generation);
    assert_eq!(runtime.commands().len(), 2);
}
