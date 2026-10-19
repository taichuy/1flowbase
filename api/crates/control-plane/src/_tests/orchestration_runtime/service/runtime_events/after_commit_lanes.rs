use super::support::{
    after_commit_graph, after_commit_lane, durable_lane_event, AfterCommitBehavior,
};
use super::*;

#[tokio::test]
async fn one_flow_uses_required_diagnostic_and_after_commit_lanes_with_distinct_semantics() {
    let run_id = Uuid::now_v7();
    let (required, diagnostic, mut receiver) = RuntimeEventReceiver::bounded_lanes(1);
    required.send(durable_lane_event(run_id, 1)).await.unwrap();
    let blocked_required = required.send(durable_lane_event(run_id, 2));
    tokio::pin!(blocked_required);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut blocked_required)
            .await
            .is_err()
    );

    assert_eq!(receiver.recv().await.unwrap().sequence, 1);
    blocked_required.await.unwrap();
    assert_eq!(receiver.recv().await.unwrap().sequence, 2);

    assert_eq!(
        diagnostic
            .try_send(RuntimeEventEnvelope::new(
                run_id,
                3,
                debug_stream_events::heartbeat()
            ))
            .status,
        RuntimeEventDiagnosticDeliveryStatus::Delivered
    );
    let dropped = diagnostic.try_send(RuntimeEventEnvelope::new(
        run_id,
        4,
        debug_stream_events::heartbeat(),
    ));
    assert_eq!(
        dropped.status,
        RuntimeEventDiagnosticDeliveryStatus::Dropped(
            RuntimeEventDiagnosticDropReason::ReceiverFull
        )
    );

    let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lane = after_commit_lane(
        AfterCommitBehavior::Success,
        Arc::clone(&calls),
        Duration::from_secs(1),
        1,
    );
    let receipts = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![durable_lane_event(run_id, 5)],
        &lane,
    )
    .await
    .unwrap();

    assert_eq!(
        receipts[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Delivered
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(
        repository
            .list_runtime_events(run_id, 0)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn after_commit_runs_only_after_durable_append_and_suppresses_duplicate_keys() {
    let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let event = durable_lane_event(run_id, 7);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lane = after_commit_lane(
        AfterCommitBehavior::Success,
        Arc::clone(&calls),
        Duration::from_secs(1),
        1,
    );

    repository.fail_next_runtime_event_append();
    assert!(control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event.clone()],
        &lane,
    )
    .await
    .is_err());
    assert!(calls.lock().unwrap().is_empty());

    let first = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event.clone()],
        &lane,
    )
    .await
    .unwrap();
    let duplicate = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event],
        &lane,
    )
    .await
    .unwrap();

    assert_eq!(
        first[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Delivered
    );
    assert_eq!(
        duplicate[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::DuplicateSuppressed
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn after_commit_timeout_and_error_retries_are_finite_and_do_not_rollback_append() {
    for (behavior, expected_reason) in [
        (
            AfterCommitBehavior::Timeout,
            RuntimeEventAfterCommitFailureReason::Timeout,
        ),
        (
            AfterCommitBehavior::Error,
            RuntimeEventAfterCommitFailureReason::SubscriberError,
        ),
    ] {
        let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
        let run_id = Uuid::now_v7();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let lane = after_commit_lane(behavior, Arc::clone(&calls), Duration::from_millis(1), 2);

        let receipts = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
            &repository,
            vec![durable_lane_event(run_id, 1)],
            &lane,
        )
        .await
        .unwrap();
        let receipt = &receipts[0].subscribers[0];

        assert_eq!(
            receipt.status,
            RuntimeEventAfterCommitDeliveryStatus::Failed
        );
        assert_eq!(receipt.attempts, 2);
        assert_eq!(receipt.failure_reason, Some(expected_reason));
        assert_eq!(calls.lock().unwrap().len(), 2);
        assert_eq!(
            repository
                .list_runtime_events(run_id, 0)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}

#[tokio::test]
async fn failed_after_commit_claim_is_released_for_replay_and_completed_claim_is_suppressed() {
    let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let event = durable_lane_event(run_id, 9);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lane = after_commit_lane(
        AfterCommitBehavior::ErrorOnce,
        Arc::clone(&calls),
        Duration::from_secs(1),
        1,
    );

    let first = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event.clone()],
        &lane,
    )
    .await
    .unwrap();
    let replay = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event.clone()],
        &lane,
    )
    .await
    .unwrap();
    let completed_duplicate = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event],
        &lane,
    )
    .await
    .unwrap();

    assert_eq!(
        first[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Failed
    );
    assert_eq!(
        replay[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Delivered
    );
    assert_eq!(
        completed_duplicate[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::DuplicateSuppressed
    );
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn ephemeral_delta_creates_neither_history_nor_after_commit_delivery() {
    let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lane = after_commit_lane(
        AfterCommitBehavior::Success,
        Arc::clone(&calls),
        Duration::from_secs(1),
        1,
    );
    let event = RuntimeEventEnvelope::new(
        run_id,
        1,
        debug_stream_events::text_delta("node-llm", Uuid::now_v7(), "history".into()),
    );

    let receipts = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event],
        &lane,
    )
    .await
    .unwrap();

    assert!(receipts.is_empty());
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(
        repository
            .list_runtime_events(run_id, 0)
            .await
            .unwrap()
            .len(),
        0
    );
}
