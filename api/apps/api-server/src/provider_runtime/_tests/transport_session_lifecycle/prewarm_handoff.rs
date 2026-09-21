use super::*;
use std::{future::Future, pin::Pin, task::Poll};

async fn assert_pending<F: Future>(mut future: Pin<&mut F>) {
    std::future::poll_fn(|cx| match future.as_mut().poll(cx) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(_) => panic!("successor must wait while its predecessor prewarm is executing"),
    })
    .await;
}

async fn orphaned_prewarm(
    coordinator: &TransportSessionCoordinator<FakeClock>,
) -> (Arc<TransportConnectionScope>, PreparedTransportInvocation) {
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "model-a");
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"generate":false,"input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    let first = coordinator
        .prepare("runtime-a", &mut input, &context(2_030_000))
        .await
        .unwrap()
        .unwrap();
    coordinator.close_connection_scope(&scope).await;
    (scope, first)
}

#[tokio::test]
async fn orphaned_prewarm_handoff_waits_without_consuming_lease_and_ignores_late_close() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let (old_scope, first) = orphaned_prewarm(&coordinator).await;
    let first_lease = first.lease.clone();
    let next_scope = coordinator.open_connection_scope();
    let mut input = scope_input(&next_scope, "model-a");
    let ctx = context(2_025_000);
    let mut waiting = Box::pin(coordinator.prepare("runtime-a", &mut input, &ctx));
    assert_pending(waiting.as_mut()).await;
    let before = coordinator.safe_snapshot().await;
    assert!(before.sessions[0].inflight);
    assert_eq!(before.sessions.len(), 1);
    coordinator
        .finish(
            first,
            &successful_output(first_lease.fence.generation.get()),
        )
        .await
        .unwrap();
    let next = tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(next.lease.fence, first_lease.fence);
    assert_eq!(next.lease.sequence(), first_lease.sequence() + 1);
    coordinator.close_connection_scope(&old_scope).await;
    assert_eq!(
        coordinator
            .registry
            .lock()
            .await
            .state(&next.lease.fence)
            .unwrap(),
        TransportSessionState::Active
    );
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&next.lease.fence)
        .unwrap());
    coordinator
        .finish(next, &successful_output(first_lease.fence.generation.get()))
        .await
        .unwrap();
}

#[tokio::test]
async fn orphaned_prewarm_handoff_rechecks_failed_predecessor_and_rotates_only_after_close() {
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let (_, first) = orphaned_prewarm(&coordinator).await;
    let old = first.lease.clone();
    let next_scope = coordinator.open_connection_scope();
    let mut input = scope_input(&next_scope, "model-a");
    let ctx = context(2_025_000);
    let mut waiting = Box::pin(coordinator.prepare("runtime-a", &mut input, &ctx));
    assert_pending(waiting.as_mut()).await;
    coordinator
        .finish(first, &Err(transport_error("prewarm-provider-failure")))
        .await
        .unwrap();
    let next = tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation > old.fence.generation);
    assert_eq!(next.lease.sequence(), old.sequence() + 1);
    assert_eq!(runtime.commands().len(), 1);
    let generation = next.lease.fence.generation.get();
    coordinator
        .finish(next, &successful_output(generation))
        .await
        .unwrap();
}

#[tokio::test]
async fn orphaned_prewarm_handoff_stops_when_new_scope_closes() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let (_, first) = orphaned_prewarm(&coordinator).await;
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "model-a");
    let ctx = context(2_025_000);
    let mut waiting = Box::pin(coordinator.prepare("runtime-a", &mut input, &ctx));
    assert_pending(waiting.as_mut()).await;
    coordinator.close_connection_scope(&scope).await;
    let error = tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_scope_unknown"));
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

#[tokio::test]
async fn orphaned_prewarm_handoff_stops_at_successor_deadline_without_new_lease() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let (_, first) = orphaned_prewarm(&coordinator).await;
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "model-a");
    let ctx = context(2_000_020);
    let error = tokio::time::timeout(
        Duration::from_secs(1),
        coordinator.prepare("runtime-a", &mut input, &ctx),
    )
    .await
    .unwrap()
    .err()
    .unwrap();
    assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&first.lease.fence)
        .unwrap());
}

#[tokio::test]
async fn orphaned_prewarm_handoff_wakes_on_shutdown() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let (_, _) = orphaned_prewarm(&coordinator).await;
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "model-a");
    let ctx = context(2_025_000);
    let mut waiting = Box::pin(coordinator.prepare("runtime-a", &mut input, &ctx));
    assert_pending(waiting.as_mut()).await;
    coordinator.shutdown(Duration::from_secs(1)).await;
    let error = tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_shutdown"));
}

#[tokio::test]
async fn completed_prewarm_is_followed_by_an_independent_normal_handoff() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let (_, first) = orphaned_prewarm(&coordinator).await;
    let generation = first.lease.fence.generation.get();
    coordinator
        .finish(first, &successful_output(generation))
        .await
        .unwrap();
    let scope = coordinator.open_connection_scope();
    let normal = coordinator
        .prepare(
            "runtime-a",
            &mut scope_input(&scope, "model-a"),
            &context(2_025_000),
        )
        .await
        .unwrap()
        .unwrap();
    coordinator.close_connection_scope(&scope).await;
    let new_scope = coordinator.open_connection_scope();
    let mut input = scope_input(&new_scope, "model-a");
    let ctx = context(2_025_000);
    let mut waiting = Box::pin(coordinator.prepare("runtime-a", &mut input, &ctx));
    assert_pending(waiting.as_mut()).await;
    let lease = normal.lease.clone();
    coordinator
        .finish(normal, &successful_output(generation))
        .await
        .unwrap();
    let successor = tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(successor.lease.fence, lease.fence);
    assert_eq!(successor.lease.sequence(), lease.sequence() + 1);
}
