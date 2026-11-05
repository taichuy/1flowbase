use super::*;
use std::{future::Future, pin::Pin, task::Poll};

type Coordinator = TransportSessionCoordinator<FakeClock>;
type Waiter = tokio::task::JoinHandle<anyhow::Result<Option<PreparedTransportInvocation>>>;

async fn assert_pending<F: Future>(mut future: Pin<&mut F>) {
    std::future::poll_fn(|cx| match future.as_mut().poll(cx) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(_) => panic!("the original execution still owns the only invocation lease"),
    })
    .await;
}

async fn bound_execution(
    responses: impl IntoIterator<Item = AckBehavior>,
) -> (
    Arc<Coordinator>,
    Arc<FakeTransportRuntime>,
    Arc<TransportConnectionScope>,
    PreparedTransportInvocation,
) {
    let runtime = Arc::new(FakeTransportRuntime::new(responses));
    let coordinator = Arc::new(
        Coordinator::new_with_clock(
            runtime.clone(),
            transport_config(),
            FakeClock::new(2_000_000),
        )
        .unwrap(),
    );
    let old_scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&old_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    (coordinator, runtime, old_scope, first)
}

async fn orphaned_execution(
    responses: impl IntoIterator<Item = AckBehavior>,
) -> (
    Arc<Coordinator>,
    Arc<FakeTransportRuntime>,
    Arc<TransportConnectionScope>,
    PreparedTransportInvocation,
) {
    let (coordinator, runtime, old_scope, first) = bound_execution(responses).await;
    coordinator.close_connection_scope(&old_scope).await;
    (coordinator, runtime, old_scope, first)
}

async fn parked_waiter(
    coordinator: &Arc<Coordinator>,
    scope: &Arc<TransportConnectionScope>,
    deadline_ms: u64,
) -> Waiter {
    let coordinator = coordinator.clone();
    let scope = scope.clone();
    let (parked, ready) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut input = scope_input(&scope, "model-a");
        let ctx = context(deadline_ms);
        let mut waiting = Box::pin(coordinator.prepare_dispatched("runtime-a", &mut input, &ctx));
        assert_pending(waiting.as_mut()).await;
        parked.send(()).unwrap();
        // Polling and parking happen in this task before it yields. The parent
        // must wake the registered waiter, not manually repoll it after finish.
        waiting.await
    });
    ready.await.unwrap();
    task
}

async fn waiter_result(mut waiter: Waiter) -> anyhow::Result<Option<PreparedTransportInvocation>> {
    match tokio::time::timeout(Duration::from_secs(1), &mut waiter).await {
        Ok(result) => result.unwrap(),
        Err(_) => {
            waiter.abort();
            let _ = waiter.await;
            panic!("state changes must wake a parked admission waiter");
        }
    }
}

#[tokio::test]
async fn separate_delivery_waits_before_predecessor_socket_closes() {
    for close_predecessor in [false, true] {
        let (coordinator, runtime, old_scope, first) = bound_execution([]).await;
        let old = first.lease.clone();
        let scope = coordinator.open_connection_scope();
        let waiting = parked_waiter(&coordinator, &scope, 2_025_000).await;
        assert!(runtime.commands().is_empty());
        assert!(coordinator.safe_snapshot().await.sessions[0].inflight);
        if close_predecessor {
            coordinator.close_connection_scope(&old_scope).await;
        }
        coordinator
            .finish(first, &successful_output(old.fence.generation.get()))
            .await
            .unwrap();
        let next = waiter_result(waiting).await.unwrap().unwrap();
        assert_eq!(next.lease.fence, old.fence);
        assert_eq!(next.lease.sequence(), old.sequence() + 1);
        coordinator.close_connection_scope(&old_scope).await;
        assert!(coordinator.safe_snapshot().await.sessions[0].inflight);
        coordinator
            .finish(next, &successful_output(old.fence.generation.get()))
            .await
            .unwrap();
        assert!(runtime.commands().is_empty());
    }
}

#[tokio::test]
async fn bound_delivery_still_rejects_its_own_concurrent_invocation() {
    let (coordinator, _, old_scope, first) = bound_execution([]).await;
    let error = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&old_scope, "model-a"),
            &context(2_025_000),
        )
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_busy"));
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

#[tokio::test]
async fn separate_delivery_wait_expires_without_cancelling_live_predecessor() {
    let (coordinator, _, _old_scope, first) = bound_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_000_020).await;
    assert!(reason(waiter_result(waiting).await.err().unwrap())
        .contains("transport_invocation_deadline_exceeded"));
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

#[tokio::test]
async fn ordinary_success_wakes_parked_waiter_without_state_change_or_early_lease() {
    let (coordinator, runtime, old_scope, first) = orphaned_execution([]).await;
    let old = first.lease.clone();
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_025_000).await;
    let before = coordinator.safe_snapshot().await;
    assert_eq!(before.sessions.len(), 1);
    assert!(before.sessions[0].inflight);
    assert_eq!(before.sessions[0].fence, old.fence);
    assert!(!waiting.is_finished());
    assert!(runtime.commands().is_empty());

    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    let next = waiter_result(waiting).await.unwrap().unwrap();
    assert_eq!(next.lease.fence, old.fence);
    assert_eq!(next.lease.sequence(), old.sequence() + 1);
    coordinator.close_connection_scope(&old_scope).await;
    // Neither late scope close nor an out-of-order completion can release the
    // replacement invocation. The real old completion was already consumed.
    let after = coordinator.safe_snapshot().await;
    assert!(after.sessions[0].inflight);
    assert!(coordinator
        .registry
        .lock()
        .await
        .finish_invocation(&old, InvocationCompletion::IdleAffinity)
        .is_err());
    assert_eq!(coordinator.safe_snapshot().await, after);
}

#[tokio::test]
async fn ordinary_fault_handoff_requires_close_proof_before_rotation() {
    for released in [false, true] {
        let (coordinator, runtime, _, first) =
            orphaned_execution([AckBehavior::Matching(released)]).await;
        let old = first.lease.clone();
        let scope = coordinator.open_connection_scope();
        let waiting = parked_waiter(&coordinator, &scope, 2_025_000).await;
        coordinator
            .finish(first, &Err(transport_error("ordinary-provider-failure")))
            .await
            .unwrap();
        let result = waiter_result(waiting).await;
        assert_eq!(runtime.commands().len(), 1);
        if released {
            let next = result.unwrap().unwrap();
            assert!(next.lease.fence.generation > old.fence.generation);
            assert_eq!(next.lease.sequence(), old.sequence() + 1);
        } else {
            assert!(reason(result.err().unwrap())
                .contains("provider_physical_connection_close_pending"));
            let snapshot = coordinator.safe_snapshot().await;
            assert_eq!(snapshot.sessions[0].fence, old.fence);
            assert!(!snapshot.sessions[0].inflight);
        }
    }
}

#[tokio::test]
async fn ordinary_handoff_scope_close_wakes_without_releasing_predecessor() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_025_000).await;
    coordinator.close_connection_scope(&scope).await;
    assert!(reason(waiter_result(waiting).await.err().unwrap())
        .contains("transport_session_scope_unknown"));
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

#[tokio::test]
async fn ordinary_handoff_shutdown_wakes_without_admitting_successor() {
    let (coordinator, runtime, _, first) = orphaned_execution([AckBehavior::Matching(true)]).await;
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_025_000).await;
    coordinator.shutdown(Duration::from_secs(1)).await;
    assert!(
        reason(waiter_result(waiting).await.err().unwrap()).contains("transport_session_shutdown")
    );
    assert_eq!(runtime.commands().len(), 1);
    drop(first);
}

#[tokio::test]
async fn ordinary_handoff_deadline_does_not_reset_after_notifications() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "model-a");
    let ctx = context(2_000_020);
    let mut waiting = Box::pin(coordinator.prepare_dispatched("runtime-a", &mut input, &ctx));
    assert_pending(waiting.as_mut()).await;
    // The fake registry clock does not move: only the retained monotonic total
    // deadline can stop this wait when unrelated changes keep notifying it.
    let notify = async {
        loop {
            coordinator.invocation_changed.notify_waiters();
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    };
    let result = tokio::time::timeout(Duration::from_millis(200), async {
        tokio::select! {
            result = waiting => result,
            _ = notify => unreachable!(),
        }
    })
    .await
    .expect("notifications must not renew the original 20 ms wait budget");
    assert!(reason(result.err().unwrap()).contains("transport_invocation_deadline_exceeded"));
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

#[test]
fn handoff_budget_uses_request_deadline_or_existing_predecessor_deadline() {
    let now = TransportInstant::from_millis(2_000_000);
    for (deadline, expected_ms) in [
        (None, 100_000),
        (Some(2_100_000), 100_000),
        (Some(2_300_000), 300_000),
        (Some(2_030_000), 30_000),
        (Some(2_000_020), 20),
        (Some(1_999_999), 0),
    ] {
        assert_eq!(
            handoff_wait_budget(
                now,
                deadline.map(TransportInstant::from_millis),
                TransportInstant::from_millis(2_100_000),
            ),
            Duration::from_millis(expected_ms),
        );
    }
}

#[tokio::test]
async fn grace_expiry_does_not_close_active_orphan_before_successful_handoff() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let config = TransportRegistryConfig {
        orphan_grace: Duration::from_secs(1),
        physical_soft_drain_age: Duration::from_secs(150),
        physical_max_age: Duration::from_secs(180),
        ..TransportRegistryConfig::default()
    };
    let coordinator =
        Arc::new(Coordinator::new_with_clock(runtime.clone(), config, clock.clone()).unwrap());
    let old_scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&old_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let old = first.lease.clone();
    coordinator.close_connection_scope(&old_scope).await;
    let next_scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &next_scope, 2_120_000).await;
    clock.advance(Duration::from_secs(65));
    coordinator.maintain_and_dispatch().await;
    let retained = coordinator.safe_snapshot().await;
    assert_eq!(retained.sessions.len(), 1);
    assert_eq!(retained.sessions[0].fence, old.fence);
    assert!(retained.sessions[0].inflight);
    assert!(retained.tombstones.is_empty());
    assert!(
        runtime.commands().is_empty(),
        "Close cannot contend with the admitted Generate"
    );
    assert!(!waiting.is_finished());

    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    let next = waiter_result(waiting).await.unwrap().unwrap();
    assert_eq!(next.lease.fence, old.fence);
    assert_eq!(next.lease.sequence(), old.sequence() + 1);
    assert!(runtime.commands().is_empty());
}

#[tokio::test]
async fn two_waiters_acquire_successive_leases_without_overlapping_execution() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let old = first.lease.clone();
    let a_scope = coordinator.open_connection_scope();
    let b_scope = coordinator.open_connection_scope();
    let mut a = parked_waiter(&coordinator, &a_scope, 2_025_000).await;
    let mut b = parked_waiter(&coordinator, &b_scope, 2_025_000).await;
    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    let (result, remaining) = tokio::select! {
        result = &mut a => (result.unwrap(), b),
        result = &mut b => (result.unwrap(), a),
    };
    let next = result.unwrap().unwrap();
    assert_eq!(next.lease.fence, old.fence);
    assert_eq!(next.lease.sequence(), old.sequence() + 1);
    assert!(!remaining.is_finished());
    coordinator
        .finish(next, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    let last = waiter_result(remaining).await.unwrap().unwrap();
    assert_eq!(last.lease.fence, old.fence);
    assert_eq!(last.lease.sequence(), old.sequence() + 2);
    assert!(coordinator.safe_snapshot().await.sessions[0].inflight);
}

#[tokio::test]
async fn different_target_cannot_wait_for_or_take_over_orphaned_execution() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let error = coordinator
        .prepare_dispatched(
            "runtime-b",
            &mut scope_input(&scope, "model-a"),
            &context(2_025_000),
        )
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_inflight_unbound"));
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

#[tokio::test]
async fn handoff_deadline_expires_while_notified_waiter_contends_for_dispatcher() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_000_020).await;
    let _dispatcher = coordinator.dispatcher.lock().await;
    coordinator.invocation_changed.notify_waiters();
    let error = waiter_result(waiting).await.err().unwrap();
    assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

#[tokio::test]
async fn handoff_deadline_expires_while_notified_waiter_contends_for_registry() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_000_020).await;
    let registry = coordinator.registry.lock().await;
    coordinator.invocation_changed.notify_waiters();
    let error = waiter_result(waiting).await.err().unwrap();
    assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
    assert!(registry.invocation_inflight(&first.lease.fence).unwrap());
    assert!(coordinator.dispatcher.try_lock().is_ok());
}

#[tokio::test]
async fn shutdown_wakes_handoff_before_blocked_control_owner_can_finish() {
    let (coordinator, runtime, _, first) = orphaned_execution([AckBehavior::Matching(true)]).await;
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_025_000).await;
    let dispatcher = coordinator.dispatcher.lock().await;
    coordinator.invocation_changed.notify_waiters();
    let shutdown = {
        let coordinator = coordinator.clone();
        tokio::spawn(async move { coordinator.shutdown(Duration::from_secs(1)).await })
    };
    let error = waiter_result(waiting).await.err().unwrap();
    assert!(reason(error).contains("transport_session_shutdown"));
    assert!(!shutdown.is_finished());
    drop(dispatcher);
    tokio::time::timeout(Duration::from_secs(1), shutdown)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(runtime.commands().len(), 1);
    drop(first);
}

#[tokio::test]
async fn committed_scope_close_is_observed_before_contended_dispatcher() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_025_000).await;
    coordinator.close_connection_scope(&scope).await;
    let _dispatcher = coordinator.dispatcher.lock().await;
    let error = waiter_result(waiting).await.err().unwrap();
    assert!(reason(error).contains("transport_session_scope_unknown"));
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

#[tokio::test]
async fn uncommitted_scope_close_keeps_original_handoff_deadline() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_000_020).await;
    let dispatcher = coordinator.dispatcher.lock().await;
    let closing = {
        let coordinator = coordinator.clone();
        let scope = scope.clone();
        tokio::spawn(async move { coordinator.close_connection_scope(&scope).await })
    };
    coordinator.invocation_changed.notify_waiters();
    let error = waiter_result(waiting).await.err().unwrap();
    assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
    assert!(!closing.is_finished());
    drop(dispatcher);
    tokio::time::timeout(Duration::from_secs(1), closing)
        .await
        .unwrap()
        .unwrap();
    assert!(scope.state.lock().unwrap().closed);
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

struct GatedCloseRuntime {
    runtime: FakeTransportRuntime,
    started: AtomicU64,
    entered: Notify,
    release: Notify,
}

impl GatedCloseRuntime {
    fn new() -> Self {
        Self {
            runtime: FakeTransportRuntime::new([AckBehavior::Matching(true)]),
            started: AtomicU64::new(0),
            entered: Notify::new(),
            release: Notify::new(),
        }
    }
}

#[async_trait::async_trait]
impl TransportLifecycleRuntime for GatedCloseRuntime {
    fn observe_dispatch(&self, target_id: &str, lease: &InvocationLease) {
        self.runtime.observe_dispatch(target_id, lease);
    }
    async fn transport_worker_exit_evidence(
        &self,
        target_id: &str,
        logical_session_id: &str,
        generation: u64,
    ) -> Result<Option<ProviderTransportClosureEvidence>, RuntimeBackendError> {
        self.runtime
            .transport_worker_exit_evidence(target_id, logical_session_id, generation)
            .await
    }

    async fn transport_session(
        &self,
        target_id: &str,
        command: ProviderTransportSessionCommand,
    ) -> Result<ProviderTransportSessionReceipt, RuntimeBackendError> {
        self.started.fetch_add(1, AtomicOrdering::SeqCst);
        self.entered.notify_one();
        self.release.notified().await;
        self.runtime.transport_session(target_id, command).await
    }
}

async fn unrelated_close_fixture() -> (
    Arc<Coordinator>,
    Arc<GatedCloseRuntime>,
    PreparedTransportInvocation,
    PreparedTransportInvocation,
) {
    let runtime = Arc::new(GatedCloseRuntime::new());
    let coordinator = Arc::new(
        Coordinator::new_with_clock(
            runtime.clone(),
            transport_config(),
            FakeClock::new(2_000_000),
        )
        .unwrap(),
    );
    let old_scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&old_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let unrelated = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut invocation_input("unrelated-thread", ProviderWireOperation::Generate),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    coordinator.close_connection_scope(&old_scope).await;
    (coordinator, runtime, first, unrelated)
}

async fn assert_close_retained(
    coordinator: &Coordinator,
    runtime: &GatedCloseRuntime,
    fence: &TransportFence,
) {
    assert_eq!(runtime.started.load(AtomicOrdering::SeqCst), 1);
    assert_eq!(runtime.runtime.commands().len(), 1);
    let snapshot = coordinator.safe_snapshot().await;
    let session = snapshot
        .sessions
        .iter()
        .find(|s| s.fence == *fence)
        .unwrap();
    assert!(session.closure_evidence.as_ref().unwrap().local_released);
    assert!(coordinator
        .pending_commands
        .lock()
        .unwrap()
        .iter()
        .any(|task| task.fence == *fence && task.state == CloseTaskState::Released));
}

#[tokio::test]
async fn slow_unrelated_close_keeps_its_owner_after_handoff_deadline() {
    let (coordinator, runtime, first, unrelated) = unrelated_close_fixture().await;
    let close_fence = unrelated.lease.fence.clone();
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_000_020).await;
    let completion = {
        let coordinator = coordinator.clone();
        tokio::spawn(async move {
            coordinator
                .finish(unrelated, &Err(transport_error("unrelated-provider-fault")))
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(1), runtime.entered.notified())
        .await
        .unwrap();
    let error = waiter_result(waiting).await.err().unwrap();
    assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
    assert!(!completion.is_finished());
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
    runtime.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), completion)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_close_retained(&coordinator, &runtime, &close_fence).await;
}

#[tokio::test]
async fn resumed_handoff_leaves_queued_close_for_existing_dispatch_owner() {
    let (coordinator, runtime, first, unrelated) = unrelated_close_fixture().await;
    let close_fence = unrelated.lease.fence.clone();
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_000_020).await;
    // Expose the same queued StateChanged event that a real finish produces,
    // before its independent dispatcher owner has acquired the dispatcher.
    coordinator
        .registry
        .lock()
        .await
        .finish_invocation(&unrelated.lease, InvocationCompletion::Faulted)
        .unwrap();
    coordinator.invocation_changed.notify_waiters();
    let error = waiter_result(waiting).await.err().unwrap();
    assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
    assert_eq!(runtime.started.load(AtomicOrdering::SeqCst), 0);
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
    let maintenance = {
        let coordinator = coordinator.clone();
        tokio::spawn(async move { coordinator.maintain_and_dispatch().await })
    };
    tokio::time::timeout(Duration::from_secs(1), runtime.entered.notified())
        .await
        .unwrap();
    runtime.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), maintenance)
        .await
        .unwrap()
        .unwrap();
    assert_close_retained(&coordinator, &runtime, &close_fence).await;
}

#[tokio::test]
async fn successful_handoff_returns_lease_before_unrelated_close_is_dispatched() {
    let (coordinator, runtime, first, unrelated) = unrelated_close_fixture().await;
    let close_fence = unrelated.lease.fence.clone();
    let old = first.lease.clone();
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_025_000).await;
    // Reproduce completion before its independent dispatcher runs. Unlike the
    // timeout case above, this waiter can now enter the successful return path.
    {
        let mut registry = coordinator.registry.lock().await;
        registry
            .finish_invocation(&old, InvocationCompletion::IdleAffinity)
            .unwrap();
        registry
            .finish_invocation(&unrelated.lease, InvocationCompletion::Faulted)
            .unwrap();
    }
    coordinator.invocation_changed.notify_waiters();
    let next = waiter_result(waiting).await.unwrap().unwrap();
    assert_eq!(next.lease.fence, old.fence);
    assert_eq!(next.lease.sequence(), old.sequence() + 1);
    assert_eq!(runtime.started.load(AtomicOrdering::SeqCst), 0);
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&next.lease.fence)
        .unwrap());

    // The admission waiter neither drains nor cancels the unrelated Close.
    // Its original maintenance owner still executes it once to real release.
    let maintenance = {
        let coordinator = coordinator.clone();
        tokio::spawn(async move { coordinator.maintain_and_dispatch().await })
    };
    tokio::time::timeout(Duration::from_secs(1), runtime.entered.notified())
        .await
        .unwrap();
    assert!(!maintenance.is_finished());
    runtime.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), maintenance)
        .await
        .unwrap()
        .unwrap();
    assert_close_retained(&coordinator, &runtime, &close_fence).await;
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&next.lease.fence)
        .unwrap());
}

#[tokio::test]
async fn independent_native_request_admits_before_predecessor_completes() {
    let (coordinator, runtime, old_scope, first) = bound_execution([]).await;
    let old = first.lease.clone();
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "model-a");
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"model":"model-a","input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    // Timeout is only a deadlock fence: no predecessor finish or clock advance
    // occurs until the production admission path has returned a distinct lease.
    let next = tokio::time::timeout(
        Duration::from_secs(1),
        coordinator.prepare_dispatched("runtime-a", &mut input, &context(2_100_000)),
    )
    .await
    .expect("an independent native request must not wait for the predecessor")
    .unwrap()
    .unwrap();
    assert_ne!(next.lease.fence.session_id, old.fence.session_id);
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&old.fence)
        .unwrap());
    assert!(
        runtime.commands().is_empty(),
        "delivery replacement never cancels execution"
    );
    coordinator.close_connection_scope(&old_scope).await;
    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&next.lease.fence)
        .unwrap());
    let generation = next.lease.fence.generation.get();
    coordinator
        .finish(next, &successful_output(generation))
        .await
        .unwrap();
    assert!(runtime.commands().is_empty());
}

fn full_native_input(scope: &TransportConnectionScope) -> ProviderInvocationInput {
    let mut input = scope_input(scope, "model-a");
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"model":"model-a", "input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    input
}

async fn finish_with_cursor(
    coordinator: &Coordinator,
    prepared: PreparedTransportInvocation,
    cursor: &str,
) {
    let mut output = successful_output(prepared.lease.fence.generation.get()).unwrap();
    output.result.response_id = Some(cursor.into());
    coordinator.finish(prepared, &Ok(output)).await.unwrap();
}

async fn admit_full(
    coordinator: &Coordinator,
    scope: &TransportConnectionScope,
) -> PreparedTransportInvocation {
    tokio::time::timeout(
        Duration::from_secs(1),
        coordinator.prepare_dispatched(
            "runtime-a",
            &mut full_native_input(scope),
            &context(2_100_000),
        ),
    )
    .await
    .expect("independent admission must not wait")
    .unwrap()
    .unwrap()
}

#[tokio::test]
async fn independent_scope_affinity_survives_finish_and_rejects_concurrent_full_requests() {
    let (coordinator, runtime, _old_scope, first) = bound_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let branch = admit_full(&coordinator, &scope).await;
    let fence = branch.lease.fence.clone();
    let error = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut full_native_input(&scope),
            &context(2_100_000),
        )
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_busy"));
    finish_with_cursor(&coordinator, branch, "branch-first").await;
    assert!(scope.state.lock().unwrap().leases.is_empty());
    let next = admit_full(&coordinator, &scope).await;
    assert_eq!(
        next.lease.fence, fence,
        "scope affinity outlives each invocation attachment"
    );
    finish_with_cursor(&coordinator, next, "branch-second").await;
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
    assert!(runtime.commands().is_empty());
}

#[tokio::test]
async fn branch_response_cursor_continues_on_same_and_reconnected_scopes() {
    for semantic in [false, true] {
        let (coordinator, runtime, _old_scope, first) = bound_execution([]).await;
        let scope = coordinator.open_connection_scope();
        let branch = admit_full(&coordinator, &scope).await;
        let fence = branch.lease.fence.clone();
        finish_with_cursor(&coordinator, branch, "branch-cursor").await;
        for reconnect in [false, true] {
            let delivery = if reconnect {
                coordinator.close_connection_scope(&scope).await;
                assert!(scope.state.lock().unwrap().affinity.is_empty());
                coordinator.open_connection_scope()
            } else {
                scope.clone()
            };
            let mut input = full_native_input(&delivery);
            if semantic {
                input.native_transport = None;
                input.previous_response_id = Some("branch-cursor".into());
            } else {
                input.native_transport.as_mut().unwrap().wire_body["previous_response_id"] =
                    serde_json::json!("branch-cursor");
            }
            let next = tokio::time::timeout(
                Duration::from_secs(1),
                coordinator.prepare_dispatched("runtime-a", &mut input, &context(2_100_000)),
            )
            .await
            .expect("known branch cursor must not wait for base")
            .unwrap()
            .unwrap();
            assert_eq!(next.lease.fence, fence);
            finish_with_cursor(&coordinator, next, "branch-cursor").await;
        }
        assert!(coordinator
            .registry
            .lock()
            .await
            .invocation_inflight(&first.lease.fence)
            .unwrap());
        assert!(runtime.commands().is_empty());
    }
}

#[tokio::test]
async fn bound_cursor_waits_for_actual_owner_instead_of_scope_branch() {
    let (coordinator, runtime, old_scope, first) = bound_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let branch = admit_full(&coordinator, &scope).await;
    let branch_fence = branch.lease.fence.clone();
    finish_with_cursor(&coordinator, branch, "branch-cursor").await;
    let base_fence = first.lease.fence.clone();
    finish_with_cursor(&coordinator, first, "base-cursor").await;
    let base = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&old_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let branch = admit_full(&coordinator, &scope).await;
    let mut input = full_native_input(&scope);
    input.native_transport.as_mut().unwrap().wire_body["previous_response_id"] =
        serde_json::json!("base-cursor");
    let ctx = context(2_100_000);
    let mut waiting = Box::pin(coordinator.prepare_dispatched("runtime-a", &mut input, &ctx));
    assert_pending(waiting.as_mut()).await;
    finish_with_cursor(&coordinator, base, "base-next").await;
    let next = tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(next.lease.fence, base_fence);
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&branch_fence)
        .unwrap());
    finish_with_cursor(&coordinator, next, "base-final").await;
    finish_with_cursor(&coordinator, branch, "branch-final").await;
    let branch_again = admit_full(&coordinator, &scope).await;
    assert_eq!(branch_again.lease.fence, branch_fence);
    finish_with_cursor(&coordinator, branch_again, "branch-after-owner-override").await;
    assert!(runtime.commands().is_empty());
}

#[tokio::test]
async fn unknown_and_conflicting_cursors_preserve_base_handoff() {
    for (cursor, semantic) in [
        (serde_json::json!("unknown"), None),
        (serde_json::json!(17), None),
        (
            serde_json::json!("known-branch"),
            Some("conflicting-semantic"),
        ),
    ] {
        let (coordinator, runtime, _old_scope, first) = bound_execution([]).await;
        let scope = coordinator.open_connection_scope();
        let branch = admit_full(&coordinator, &scope).await;
        let branch_fence = branch.lease.fence.clone();
        finish_with_cursor(&coordinator, branch, "known-branch").await;
        let mut input = full_native_input(&scope);
        input.native_transport.as_mut().unwrap().wire_body["previous_response_id"] = cursor;
        input.previous_response_id = semantic.map(str::to_string);
        let ctx = context(2_100_000);
        let mut waiting = Box::pin(coordinator.prepare_dispatched("runtime-a", &mut input, &ctx));
        assert_pending(waiting.as_mut()).await;
        let base_fence = first.lease.fence.clone();
        finish_with_cursor(&coordinator, first, "base-response").await;
        let next = tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(next.lease.fence, base_fence);
        assert_ne!(next.lease.fence, branch_fence);
        finish_with_cursor(&coordinator, next, "base-next").await;
        assert!(runtime.commands().is_empty());
    }
}

#[tokio::test]
async fn cursor_bindings_are_isolated_by_actor_workspace_provider_and_model() {
    let (coordinator, _, _old_scope, _first) = bound_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let branch = admit_full(&coordinator, &scope).await;
    let branch_fence = branch.lease.fence.clone();
    finish_with_cursor(&coordinator, branch, "same-cursor").await;
    for dimension in 0..4 {
        let delivery = coordinator.open_connection_scope();
        let mut input = full_native_input(&delivery);
        input.native_transport.as_mut().unwrap().wire_body["previous_response_id"] =
            serde_json::json!("same-cursor");
        let mut ctx = context(2_100_000);
        match dimension {
            0 => ctx.actor_id = Some(uuid::Uuid::now_v7()),
            1 => ctx.workspace_id = uuid::Uuid::now_v7(),
            2 => input.provider_instance_id = "different-provider".into(),
            _ => input.model = "different-model".into(),
        }
        let next = tokio::time::timeout(
            Duration::from_secs(1),
            coordinator.prepare_dispatched("runtime-a", &mut input, &ctx),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        assert_ne!(next.lease.fence.session_id, branch_fence.session_id);
        let expected = hash_identity(&[
            "shared-thread",
            &ctx.workspace_id.to_string(),
            &ctx.actor_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "anonymous".into()),
            &input.provider_instance_id,
            &input.protocol,
            &input.model,
        ]);
        assert_eq!(next.lease.fence.session_id.as_str(), expected);
        finish_with_cursor(&coordinator, next, "same-cursor").await;
    }
}

#[tokio::test]
async fn native_branch_cannot_bypass_target_change_or_incomplete_payload() {
    let (coordinator, _, old_scope, first) = bound_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let error = coordinator
        .prepare_dispatched(
            "runtime-b",
            &mut full_native_input(&scope),
            &context(2_100_000),
        )
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_busy"));
    let error = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut full_native_input(&old_scope),
            &context(2_100_000),
        )
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_busy"));
    let mut input = full_native_input(&scope);
    input.native_transport.as_mut().unwrap().wire_body = serde_json::json!({"model":"model-a"});
    let ctx = context(2_100_000);
    let mut waiting = Box::pin(coordinator.prepare_dispatched("runtime-a", &mut input, &ctx));
    assert_pending(waiting.as_mut()).await;
    let base_fence = first.lease.fence.clone();
    finish_with_cursor(&coordinator, first, "base").await;
    let next = tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(next.lease.fence, base_fence);
    finish_with_cursor(&coordinator, next, "base-next").await;
}

#[tokio::test]
async fn cursor_bindings_follow_registry_sessions_and_tombstones() {
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let clock = FakeClock::new(2_000_000);
    let coordinator =
        Coordinator::new_with_clock(runtime, transport_config(), clock.clone()).unwrap();
    let scope = coordinator.open_connection_scope();
    let first = admit_full(&coordinator, &scope).await;
    let fence = first.lease.fence.clone();
    finish_with_cursor(&coordinator, first, "retained-cursor").await;
    assert_eq!(coordinator.affinity.lock().unwrap().cursors.len(), 1);
    coordinator
        .registry
        .lock()
        .await
        .terminate(&fence, TerminationKind::OwnerClosed)
        .unwrap();
    coordinator.dispatch_pending_events().await;
    assert_eq!(
        coordinator.affinity.lock().unwrap().cursors.len(),
        1,
        "live tombstone retains cursor owner"
    );
    clock.advance(transport_config().tombstone_ttl + Duration::from_secs(1));
    coordinator.maintain_and_dispatch().await;
    let snapshot = coordinator.safe_snapshot().await;
    assert!(snapshot.sessions.is_empty());
    assert!(snapshot.tombstones.is_empty());
    assert!(coordinator.affinity.lock().unwrap().cursors.is_empty());
    assert!(coordinator.affinity.lock().unwrap().bases.is_empty());
    assert!(scope.state.lock().unwrap().affinity.is_empty());
}
