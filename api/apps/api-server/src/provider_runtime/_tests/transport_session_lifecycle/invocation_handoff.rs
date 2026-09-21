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

async fn orphaned_execution(
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
        .prepare(
            "runtime-a",
            &mut scope_input(&old_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
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
        let mut waiting = Box::pin(coordinator.prepare("runtime-a", &mut input, &ctx));
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
    let (coordinator, runtime, _, _) = orphaned_execution([AckBehavior::Matching(true)]).await;
    let scope = coordinator.open_connection_scope();
    let waiting = parked_waiter(&coordinator, &scope, 2_025_000).await;
    coordinator.shutdown(Duration::from_secs(1)).await;
    assert!(
        reason(waiter_result(waiting).await.err().unwrap()).contains("transport_session_shutdown")
    );
    assert_eq!(runtime.commands().len(), 1);
}

#[tokio::test]
async fn ordinary_handoff_deadline_does_not_reset_after_notifications() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "model-a");
    let ctx = context(2_000_020);
    let mut waiting = Box::pin(coordinator.prepare("runtime-a", &mut input, &ctx));
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
fn handoff_budget_caps_long_requests_at_thirty_seconds() {
    let now = TransportInstant::from_millis(2_000_000);
    for (deadline, expected_ms) in [
        (None, 30_000),
        (Some(2_100_000), 30_000),
        (Some(2_030_000), 30_000),
        (Some(2_000_020), 20),
        (Some(1_999_999), 0),
    ] {
        assert_eq!(
            handoff_wait_budget(now, deadline.map(TransportInstant::from_millis)),
            Duration::from_millis(expected_ms),
        );
    }
}

#[tokio::test]
async fn two_waiters_compete_for_only_one_successor_lease() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let old = first.lease.clone();
    let a_scope = coordinator.open_connection_scope();
    let b_scope = coordinator.open_connection_scope();
    let a = parked_waiter(&coordinator, &a_scope, 2_025_000).await;
    let b = parked_waiter(&coordinator, &b_scope, 2_025_000).await;
    coordinator
        .finish(first, &successful_output(old.fence.generation.get()))
        .await
        .unwrap();
    let outcomes = [waiter_result(a).await, waiter_result(b).await];
    let mut admitted = 0;
    let mut refused = 0;
    for outcome in outcomes {
        match outcome {
            Ok(Some(next)) => {
                admitted += 1;
                assert_eq!(next.lease.fence, old.fence);
                assert_eq!(next.lease.sequence(), old.sequence() + 1);
            }
            Err(error) => {
                refused += 1;
                assert!(reason(error).contains("transport_session_busy"));
            }
            Ok(None) => panic!("the request must retain its session identity"),
        }
    }
    assert_eq!((admitted, refused), (1, 1));
    assert!(coordinator.safe_snapshot().await.sessions[0].inflight);
}

#[tokio::test]
async fn different_target_cannot_wait_for_or_take_over_orphaned_execution() {
    let (coordinator, _, _, first) = orphaned_execution([]).await;
    let scope = coordinator.open_connection_scope();
    let error = coordinator
        .prepare(
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
    let (coordinator, runtime, _, _) = orphaned_execution([AckBehavior::Matching(true)]).await;
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
        .prepare(
            "runtime-a",
            &mut scope_input(&old_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let unrelated = coordinator
        .prepare(
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
