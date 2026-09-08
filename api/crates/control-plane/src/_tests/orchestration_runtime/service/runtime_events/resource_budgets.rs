use super::*;

#[tokio::test]
async fn root_2007_ac_010_lane_budgets() {
    use std::future::{poll_fn, Future};
    use std::task::Poll;
    let run = Uuid::now_v7();
    let (required, diagnostic, mut receiver) = RuntimeEventReceiver::bounded_lanes(1);
    required.send(durable_lane_event(run, 1)).await.unwrap();
    let waiting = required.send(durable_lane_event(run, 2));
    tokio::pin!(waiting);
    poll_fn(|cx| {
        assert!(waiting.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(receiver.recv().await.unwrap().sequence, 1);
    waiting.await.unwrap();
    assert_eq!(receiver.recv().await.unwrap().sequence, 2);
    diagnostic.try_send(durable_lane_event(run, 3));
    let full = diagnostic.try_send(durable_lane_event(run, 4));
    assert_eq!(
        full.status,
        RuntimeEventDiagnosticDeliveryStatus::Dropped(
            RuntimeEventDiagnosticDropReason::ReceiverFull
        )
    );
    required.send(durable_lane_event(run, 5)).await.unwrap();
    let closing = required.send(durable_lane_event(run, 6));
    tokio::pin!(closing);
    poll_fn(|cx| {
        assert!(closing.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(receiver);
    assert!(closing.await.is_err());
    let closed = diagnostic.try_send(durable_lane_event(run, 7));
    assert_eq!(
        closed.status,
        RuntimeEventDiagnosticDeliveryStatus::Dropped(
            RuntimeEventDiagnosticDropReason::ReceiverClosed
        )
    );
    let snapshot = diagnostic.snapshot();
    assert_eq!(
        (
            snapshot.dropped_total,
            snapshot.receiver_full,
            snapshot.receiver_closed
        ),
        (2, 1, 1)
    );
}

#[tokio::test]
async fn root_2007_ac_010_lane_budgets_temporary_after_commit_scope() {
    let run = Uuid::now_v7();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lane = after_commit_lane(
        AfterCommitBehavior::Success,
        calls.clone(),
        Duration::from_secs(1),
        5,
    );
    for sequence in 1..=4096 {
        assert_eq!(
            lane.deliver_after_commit(durable_lane_event(run, sequence))
                .await
                .subscribers[0]
                .status,
            RuntimeEventAfterCommitDeliveryStatus::Delivered
        );
    }
    let full = lane
        .deliver_after_commit(durable_lane_event(run, 4097))
        .await;
    assert_eq!(
        full.subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Failed
    );
    assert_eq!(
        full.subscribers[0].failure_reason,
        Some(RuntimeEventAfterCommitFailureReason::CapacityExhausted)
    );
    assert_eq!(full.subscribers[0].attempts, 0);
    assert_eq!(calls.lock().unwrap().len(), 4096);
    assert_eq!(lane.retained_claim_count(), 4096);
    assert_eq!(
        lane.deliver_after_commit(durable_lane_event(run, 1))
            .await
            .subscribers[0]
            .status,
        RuntimeEventAfterCommitDeliveryStatus::DuplicateSuppressed
    );
    let owner = lane.scope_owner();
    drop(owner); // The persister owns this exact guard on normal exit, failure and cancellation.
    lane.wait_closed(Duration::from_secs(1)).await.unwrap();
    assert_eq!(lane.retained_claim_count(), 0);
    let closed = lane.deliver_after_commit(durable_lane_event(run, 1)).await;
    assert_eq!(
        closed.subscribers[0].failure_reason,
        Some(RuntimeEventAfterCommitFailureReason::ScopeClosed)
    );
    assert_eq!(closed.subscribers[0].attempts, 0);

    struct Barrier {
        started: tokio::sync::mpsc::Sender<()>,
        release: Arc<tokio::sync::Semaphore>,
    }
    #[async_trait::async_trait]
    impl RuntimeEventAfterCommitSubscriber for Barrier {
        async fn deliver(&self, _: RuntimeEventAfterCommitDelivery) -> anyhow::Result<()> {
            self.started.send(()).await.unwrap();
            self.release.acquire().await.unwrap().forget();
            Ok(())
        }
    }
    let (started, mut entered) = tokio::sync::mpsc::channel(1);
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let lane = RuntimeEventAfterCommitLane::from_graph(
        after_commit_graph(),
        vec![TrustedRuntimeEventAfterCommitRegistration {
            contribution_id: "fixture.runtime-event.after-commit".into(),
            subscriber_id: RuntimeEventAfterCommitSubscriberId::new("fixture.subscriber").unwrap(),
            timeout: Duration::from_secs(5),
            max_attempts: 5,
            subscriber: Arc::new(Barrier {
                started,
                release: release.clone(),
            }),
        }],
    )
    .unwrap();
    let waiter = {
        let lane = lane.clone();
        tokio::spawn(async move { lane.deliver_after_commit(durable_lane_event(run, 1)).await })
    };
    entered.recv().await.unwrap();
    assert_eq!(lane.retained_claim_count(), 1);
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert_eq!(
        lane.retained_claim_count(),
        0,
        "cancelling this temporary attempt releases its claim, not a durable Outbox row"
    );
    let waiter = {
        let lane = lane.clone();
        tokio::spawn(async move { lane.deliver_after_commit(durable_lane_event(run, 1)).await })
    };
    entered.recv().await.unwrap();
    lane.close();
    assert!(lane.wait_closed(Duration::ZERO).await.is_err());
    assert_eq!(lane.retained_claim_count(), 1);
    release.add_permits(1);
    assert_eq!(
        waiter.await.unwrap().subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Delivered
    );
    lane.wait_closed(Duration::from_secs(1)).await.unwrap();
    assert_eq!(lane.retained_claim_count(), 0);
}
