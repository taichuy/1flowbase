use super::*;

#[tokio::test]
async fn reserved_generation_settles_without_fabricated_worker_evidence_and_fences_handoff() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "reserved-not-dispatched");
    let prepared = coordinator
        .prepare("runtime-a", &mut input, &context(2_030_000))
        .await
        .unwrap()
        .unwrap();
    let fence = prepared.lease.fence.clone();
    clock.advance(Duration::from_secs(20));
    coordinator.maintain_and_dispatch().await;
    let snapshot = coordinator.safe_snapshot().await;
    assert_eq!(snapshot.sessions[0].fence, fence);
    assert!(snapshot.sessions[0].never_dispatched_settled);
    assert!(!snapshot.sessions[0].dispatch_claimed);
    assert!(!snapshot.sessions[0].inflight);
    assert!(snapshot.sessions[0].closure_evidence.is_none());
    assert!(runtime.commands().is_empty());
    assert!(coordinator.claim_dispatch(&prepared).await.is_err());
    assert!(runtime.bindings.lock().unwrap().is_empty());
    let mut input = scope_input(&scope, "reserved-not-dispatched");
    let next = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_040_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation > fence.generation);
    assert!(coordinator.claim_dispatch(&prepared).await.is_err());
    drop(prepared);
    assert_eq!(
        scope
            .state
            .lock()
            .unwrap()
            .leases
            .get(next.lease.fence.session_id.as_str()),
        Some(&next.lease),
        "late abandoned owner cannot detach its successor"
    );
}

#[tokio::test]
async fn dropping_prepare_reservation_reclaims_lease_and_recovers_same_identity() {
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "abandoned-before-execution");
    let prepared = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    let old = prepared.lease.fence.clone();
    assert_eq!(
        scope
            .state
            .lock()
            .unwrap()
            .leases
            .get(old.session_id.as_str()),
        Some(&prepared.lease)
    );
    drop(prepared);
    assert!(
        scope.state.lock().unwrap().leases.is_empty(),
        "Drop detaches delivery before asynchronous registry cleanup"
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while coordinator.safe_snapshot().await.sessions[0].inflight {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    coordinator.maintain_and_dispatch().await;
    let snapshot = coordinator.safe_snapshot().await;
    assert!(snapshot.sessions[0].never_dispatched_settled);
    assert!(snapshot.sessions[0].closure_evidence.is_none());
    assert!(runtime.commands().is_empty());
    let mut input = scope_input(&scope, "abandoned-before-execution");
    let next = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.lease.fence.session_id, old.session_id);
    assert!(next.lease.fence.generation > old.generation);
    assert_eq!(
        scope
            .state
            .lock()
            .unwrap()
            .leases
            .get(old.session_id.as_str()),
        Some(&next.lease)
    );
}

#[tokio::test]
async fn request_construction_failure_preserves_original_error_without_runtime_close() {
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let mut input = invocation_input("invalid-runtime-request", ProviderWireOperation::Generate);
    let prepared = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    let original = Err(anyhow::anyhow!("original request construction failure"));
    let completion = coordinator.finish(prepared, &original).await;
    let error = super::super::super::preserve_provider_invocation_outcome(original, completion)
        .unwrap_err();
    assert_eq!(error.to_string(), "original request construction failure");
    assert!(runtime.commands().is_empty());
    assert!(coordinator.safe_snapshot().await.sessions[0].never_dispatched_settled);
}

#[tokio::test]
async fn two_hour_expiry_of_reserved_generation_has_no_close_exhausted_poison() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        logical_rollover::two_hour_config(),
        clock.clone(),
    )
    .unwrap();
    let mut input = invocation_input("reserved-expiry", ProviderWireOperation::Generate);
    let prepared = coordinator
        .prepare("runtime-a", &mut input, &context(9_200_000))
        .await
        .unwrap()
        .unwrap();
    let old = prepared.lease.fence.clone();
    clock.advance(Duration::from_secs(2 * 60 * 60));
    coordinator.maintain_and_dispatch().await;
    assert!(coordinator.claim_dispatch(&prepared).await.is_err());
    let snapshot = coordinator.safe_snapshot().await;
    assert!(snapshot.tombstones[0].never_dispatched_settled);
    assert!(snapshot.tombstones[0].closure_evidence.is_none());
    let next = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(10_000_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.lease.fence.session_id, old.session_id);
    assert!(next.lease.fence.generation > old.generation);
    assert!(runtime.commands().is_empty());
}

#[tokio::test]
async fn dispatched_generation_missing_real_binding_stays_fail_closed() {
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let clock = FakeClock::new(2_000_000);
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let mut input = invocation_input("missing-real-binding", ProviderWireOperation::Generate);
    let prepared = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert!(
        coordinator.claim_dispatch(&prepared).await.is_err(),
        "single dispatch"
    );
    runtime.bindings.lock().unwrap().clear();
    coordinator
        .finish(prepared, &Err(transport_error("original upstream failure")))
        .await
        .unwrap();
    let snapshot = coordinator.safe_snapshot().await;
    assert!(snapshot.sessions[0].dispatch_claimed);
    assert!(!snapshot.sessions[0].never_dispatched_settled);
    assert!(snapshot.sessions[0].closure_evidence.is_none());
    assert_eq!(runtime.commands().len(), 1);
    assert!(reason(
        coordinator
            .prepare("runtime-a", &mut input, &context(2_010_000))
            .await
            .err()
            .unwrap()
    )
    .contains("close_pending"));
    clock.advance(Duration::from_secs(31));
    coordinator.maintain_and_dispatch().await;
    assert!(reason(
        coordinator
            .prepare("runtime-a", &mut input, &context(2_050_000))
            .await
            .err()
            .unwrap()
    )
    .contains("close_exhausted"));
}

#[tokio::test]
async fn fake_runtime_rejects_wrong_target_stale_generation_and_wrong_worker_binding() {
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(false)]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let mut input = invocation_input("real-binding-fences", ProviderWireOperation::Generate);
    let prepared = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    coordinator
        .finish(prepared, &Err(transport_error("primary")))
        .await
        .unwrap();
    let command = runtime.commands()[0].clone();
    assert!(runtime
        .transport_session("runtime-b", command.clone())
        .await
        .is_err());
    let mut stale = command.clone();
    stale.generation += 1;
    assert!(runtime.transport_session("runtime-a", stale).await.is_err());
    let mut wrong_worker = command;
    wrong_worker.worker_incarnation = Some(2);
    assert!(runtime
        .transport_session("runtime-a", wrong_worker)
        .await
        .is_err());
    assert!(coordinator.safe_snapshot().await.sessions[0]
        .closure_evidence
        .as_ref()
        .is_some_and(|e| !e.local_released));
}

#[tokio::test]
async fn aborted_execution_owner_returns_undispatched_reservation() {
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let coordinator = Arc::new(
        TransportSessionCoordinator::new_with_clock(
            runtime.clone(),
            transport_config(),
            FakeClock::new(2_000_000),
        )
        .unwrap(),
    );
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "aborted-owner");
    let prepared = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        scope
            .state
            .lock()
            .unwrap()
            .leases
            .get(prepared.lease.fence.session_id.as_str()),
        Some(&prepared.lease)
    );
    let entered = Arc::new(Notify::new());
    let observed = entered.notified();
    let task_entered = entered.clone();
    let task = tokio::spawn(async move {
        let _reservation = prepared;
        task_entered.notify_one();
        std::future::pending::<()>().await;
    });
    observed.await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(
        scope.state.lock().unwrap().leases.is_empty(),
        "aborting the owner detaches delivery"
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        while coordinator.safe_snapshot().await.sessions[0].inflight {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    coordinator.maintain_and_dispatch().await;
    assert!(coordinator.safe_snapshot().await.sessions[0].never_dispatched_settled);
    assert!(runtime.commands().is_empty());
    let mut input = scope_input(&scope, "aborted-owner");
    let successor = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        scope
            .state
            .lock()
            .unwrap()
            .leases
            .get(successor.lease.fence.session_id.as_str()),
        Some(&successor.lease)
    );
}
