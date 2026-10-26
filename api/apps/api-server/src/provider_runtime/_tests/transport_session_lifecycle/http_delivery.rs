use super::*;
use crate::routes::application_public_api::openai::http_delivery::bind_response;
use axum::{
    body::{Body, Bytes},
    response::Response,
};
use http_body::{Body as HttpBody, Frame};
use std::{future::Future, pin::Pin, task::Poll};

type Coordinator = TransportSessionCoordinator<FakeClock>;
type Waiter = tokio::task::JoinHandle<anyhow::Result<Option<PreparedTransportInvocation>>>;

async fn frame(body: &mut Body) -> Option<Result<Frame<Bytes>, axum::Error>> {
    std::future::poll_fn(|cx| Pin::new(&mut *body).poll_frame(cx)).await
}

fn coordinator() -> Arc<Coordinator> {
    Arc::new(
        Coordinator::new_with_clock(
            Arc::new(FakeTransportRuntime::new([])),
            transport_config(),
            FakeClock::new(2_000_000),
        )
        .unwrap(),
    )
}

async fn pending<F: Future>(mut future: Pin<&mut F>) {
    std::future::poll_fn(|cx| match future.as_mut().poll(cx) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(_) => panic!("the predecessor must retain its invocation"),
    })
    .await;
}

async fn closed(coordinator: &Coordinator, scope: &TransportConnectionScope) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let changed = coordinator.invocation_changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let removed = !coordinator
                .connection_scopes
                .lock()
                .unwrap()
                .contains_key(scope.id());
            if removed {
                // Close owns dispatcher through the registry transition and notification.
                drop(coordinator.dispatcher.lock().await);
                return;
            }
            changed.await;
        }
    })
    .await
    .expect("the owned close task must complete");
}

fn delivery_input(scope: &TransportConnectionScope) -> ProviderInvocationInput {
    let mut input = scope_input(scope, "model-a");
    input.model_parameters.clear();
    input.provider_config = serde_json::json!({"transport_mode":"responses_websocket"});
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"model":"model-a","input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    input
}

fn http_output() -> anyhow::Result<super::super::super::ProviderRuntimeInvocationOutput> {
    output_with_result(ProviderInvocationResult::default())
}

async fn start(
    coordinator: &Arc<Coordinator>,
) -> (TransportConnectionScopeGuard, PreparedTransportInvocation) {
    let guard = coordinator.open_http_connection_scope();
    let invocation = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut delivery_input(&guard.scope),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(invocation.transport, RecoveryTransport::ProviderHttp);
    (guard, invocation)
}

async fn assert_orphaned(coordinator: &Coordinator, invocation: &PreparedTransportInvocation) {
    let registry = coordinator.registry.lock().await;
    assert_eq!(
        registry.state(&invocation.lease.fence).unwrap(),
        TransportSessionState::Orphaned
    );
    assert!(registry
        .invocation_inflight(&invocation.lease.fence)
        .unwrap());
}

async fn waiter(coordinator: &Arc<Coordinator>, scope: &Arc<TransportConnectionScope>) -> Waiter {
    let coordinator = coordinator.clone();
    let scope = scope.clone();
    let (parked, ready) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut input = delivery_input(&scope);
        let original = input.native_transport.clone();
        let ctx = context(2_100_000);
        let result = {
            let mut future =
                Box::pin(coordinator.prepare_dispatched("runtime-a", &mut input, &ctx));
            pending(future.as_mut()).await;
            parked.send(()).unwrap();
            future.await
        };
        assert_eq!(input.native_transport, original);
        result
    });
    ready.await.unwrap();
    task
}

async fn waiter_result(mut task: Waiter) -> anyhow::Result<Option<PreparedTransportInvocation>> {
    match tokio::time::timeout(Duration::from_secs(1), &mut task).await {
        Ok(result) => result.unwrap(),
        Err(_) => {
            task.abort();
            let _ = task.await;
            panic!("committed lifecycle transition must wake HTTP admission");
        }
    }
}

#[tokio::test]
async fn http_body_drop_detaches_unpolled_silent_and_partial_deliveries_without_cancelling_execution(
) {
    for polled_frames in [0, 1, 2] {
        let coordinator = coordinator();
        let (guard, invocation) = start(&coordinator).await;
        let scope = guard.scope.clone();
        let source = futures_util::stream::iter([Ok::<_, std::io::Error>(Bytes::from_static(
            b"data: first\n\n",
        ))]);
        let source = futures_util::StreamExt::chain(source, futures_util::stream::pending());
        let mut body = bind_response(Response::new(Body::from_stream(source)), guard).into_body();
        if polled_frames > 0 {
            assert_eq!(
                frame(&mut body)
                    .await
                    .unwrap()
                    .unwrap()
                    .into_data()
                    .unwrap(),
                "data: first\n\n"
            );
        }
        if polled_frames > 1 {
            let frame = frame(&mut body);
            tokio::pin!(frame);
            pending(frame.as_mut()).await;
        }
        drop(body);
        closed(&coordinator, &scope).await;
        assert_orphaned(&coordinator, &invocation).await;
        coordinator
            .finish(invocation, &http_output())
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn http_body_eof_and_error_close_once_while_preserving_response_and_frames() {
    for fail in [false, true] {
        let coordinator = coordinator();
        let (guard, invocation) = start(&coordinator).await;
        let scope = guard.scope.clone();
        let frames = if fail {
            vec![Err(std::io::Error::other("fixture body failure"))]
        } else {
            vec![Ok(Bytes::from_static(b"complete body"))]
        };
        let response = Response::builder()
            .status(202)
            .header("x-fixture", "preserved")
            .body(Body::from_stream(futures_util::stream::iter(frames)))
            .unwrap();
        let response = bind_response(response, guard);
        assert_eq!(response.status(), 202);
        assert_eq!(response.headers()["x-fixture"], "preserved");
        let mut body = response.into_body();
        if fail {
            assert!(frame(&mut body)
                .await
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("fixture body failure"));
        } else {
            assert_eq!(
                frame(&mut body)
                    .await
                    .unwrap()
                    .unwrap()
                    .into_data()
                    .unwrap(),
                "complete body"
            );
            assert!(frame(&mut body).await.is_none());
        }
        closed(&coordinator, &scope).await;
        assert_orphaned(&coordinator, &invocation).await;
        let next_scope = coordinator.open_http_connection_scope();
        coordinator
            .finish(invocation, &http_output())
            .await
            .unwrap();
        let next = coordinator
            .prepare_dispatched(
                "runtime-a",
                &mut delivery_input(&next_scope.scope),
                &context(2_100_000),
            )
            .await
            .unwrap()
            .unwrap();
        drop(body); // A second close cannot orphan the successor's sequence.
        assert_eq!(
            coordinator
                .registry
                .lock()
                .await
                .state(&next.lease.fence)
                .unwrap(),
            TransportSessionState::Active
        );
        coordinator.finish(next, &http_output()).await.unwrap();
    }
}

#[tokio::test]
async fn cancelled_request_future_before_headers_keeps_a_close_owner_across_coordinator_contention()
{
    for hold_registry in [false, true] {
        let coordinator = coordinator();
        let (guard, invocation) = start(&coordinator).await;
        let scope = guard.scope.clone();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let request = tokio::spawn(async move {
            let _delivery_owner = guard;
            entered.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        ready.await.unwrap();
        let dispatcher = if hold_registry {
            None
        } else {
            Some(coordinator.dispatcher.lock().await)
        };
        let registry = if hold_registry {
            Some(coordinator.registry.lock().await)
        } else {
            None
        };
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        // Request cancellation must not own or abort the asynchronously parked close.
        drop(dispatcher);
        drop(registry);
        closed(&coordinator, &scope).await;
        assert_orphaned(&coordinator, &invocation).await;
        coordinator
            .finish(invocation, &http_output())
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn http_guard_drop_outside_runtime_context_still_uses_the_captured_close_owner() {
    let coordinator = coordinator();
    let (guard, invocation) = start(&coordinator).await;
    let scope = guard.scope.clone();
    std::thread::spawn(move || drop(guard)).join().unwrap();
    closed(&coordinator, &scope).await;
    assert_orphaned(&coordinator, &invocation).await;
    coordinator
        .finish(invocation, &http_output())
        .await
        .unwrap();
}

#[tokio::test]
async fn http_successor_waits_for_detached_http_or_websocket_predecessor_and_preserves_sequence() {
    for predecessor_http in [true, false] {
        let coordinator = coordinator();
        let old_guard = predecessor_http.then(|| coordinator.open_http_connection_scope());
        let old_scope = old_guard
            .as_ref()
            .map(|guard| guard.scope.clone())
            .unwrap_or_else(|| coordinator.open_connection_scope());
        let first = coordinator
            .prepare_dispatched(
                "runtime-a",
                &mut delivery_input(&old_scope),
                &context(2_100_000),
            )
            .await
            .unwrap()
            .unwrap();
        let old = first.lease.clone();
        if let Some(guard) = old_guard {
            drop(bind_response(Response::new(Body::empty()), guard));
            closed(&coordinator, &old_scope).await;
        } else {
            coordinator.close_connection_scope(&old_scope).await;
        }
        let successor = coordinator.open_http_connection_scope();
        let waiting = waiter(&coordinator, &successor.scope).await;
        assert_orphaned(&coordinator, &first).await;
        assert!(!waiting.is_finished());
        // An unrelated identity remains independently admissible.
        let unrelated_scope = coordinator.open_http_connection_scope();
        let mut unrelated = delivery_input(&unrelated_scope.scope);
        unrelated
            .client_protocol_envelope
            .as_mut()
            .unwrap()
            .headers
            .insert("session-id".into(), vec!["unrelated".into()]);
        let unrelated = coordinator
            .prepare_dispatched("runtime-a", &mut unrelated, &context(2_100_000))
            .await
            .unwrap()
            .unwrap();
        coordinator.finish(unrelated, &http_output()).await.unwrap();
        let output = if predecessor_http {
            http_output()
        } else {
            successful_output(old.fence.generation.get())
        };
        coordinator.finish(first, &output).await.unwrap();
        let next = waiter_result(waiting).await.unwrap().unwrap();
        assert_eq!(next.transport, RecoveryTransport::ProviderHttp);
        assert_eq!(next.lease.fence, old.fence);
        assert_eq!(next.lease.sequence(), old.sequence() + 1);
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
            .finish_invocation(&old, InvocationCompletion::IdleAffinity)
            .is_err());
        coordinator.finish(next, &http_output()).await.unwrap();
    }
}

#[tokio::test]
async fn detached_http_waiter_is_woken_without_dispatch_while_predecessor_is_live() {
    let coordinator = coordinator();
    let (guard, first) = start(&coordinator).await;
    let successor = coordinator.open_http_connection_scope();
    let waiting = waiter(&coordinator, &successor.scope).await;
    drop(successor);
    assert!(
        reason(waiter_result(waiting).await.err().unwrap())
            .contains("transport_session_scope_unknown")
    );
    let snapshot = coordinator.safe_snapshot().await;
    assert_eq!(snapshot.sessions[0].state, TransportSessionState::Active);
    assert!(snapshot.sessions[0].inflight);
    coordinator.finish(first, &http_output()).await.unwrap();
    drop(guard);
}

#[tokio::test]
async fn two_http_waiters_dispatch_successors_serially() {
    let coordinator = coordinator();
    let (guard, first) = start(&coordinator).await;
    let old_scope = guard.scope.clone();
    let old = first.lease.clone();
    drop(guard);
    closed(&coordinator, &old_scope).await;
    let a = coordinator.open_http_connection_scope();
    let b = coordinator.open_http_connection_scope();
    let mut a_waiter = waiter(&coordinator, &a.scope).await;
    let mut b_waiter = waiter(&coordinator, &b.scope).await;
    coordinator.finish(first, &http_output()).await.unwrap();
    let (result, remaining) = tokio::select! {
        result = &mut a_waiter => (result.unwrap(), b_waiter),
        result = &mut b_waiter => (result.unwrap(), a_waiter),
    };
    let next = result.unwrap().unwrap();
    assert_eq!(next.lease.fence, old.fence);
    assert_eq!(next.lease.sequence(), old.sequence() + 1);
    assert!(!remaining.is_finished());
    coordinator.finish(next, &http_output()).await.unwrap();
    let last = waiter_result(remaining).await.unwrap().unwrap();
    assert_eq!(last.lease.fence, old.fence);
    assert_eq!(last.lease.sequence(), old.sequence() + 2);
    coordinator.finish(last, &http_output()).await.unwrap();
}

#[tokio::test]
async fn completed_http_delivery_can_drop_late_without_orphaning_successor() {
    let coordinator = coordinator();
    let (guard, first) = start(&coordinator).await;
    let old_scope = guard.scope.clone();
    let body = bind_response(Response::new(Body::empty()), guard);
    coordinator.finish(first, &http_output()).await.unwrap();
    let (_next_guard, next) = start(&coordinator).await;
    drop(body);
    closed(&coordinator, &old_scope).await;
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
    coordinator.finish(next, &http_output()).await.unwrap();
}
