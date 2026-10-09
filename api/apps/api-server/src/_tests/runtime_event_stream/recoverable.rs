use super::*;

async fn closed_generation(
    stream: &LocalRuntimeEventStream,
    kind: &str,
) -> (
    Uuid,
    std::sync::Arc<dyn control_plane::ports::RuntimeEventTerminalWriter>,
) {
    let run_id = Uuid::now_v7();
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let writer = stream.terminal_writer(run_id).await.unwrap();
    stream
        .append_terminal_if_missing_and_close(
            run_id,
            RuntimeEventPayload {
                event_type: kind.into(),
                source: RuntimeEventSource::System,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({"type": kind}),
            },
        )
        .await
        .unwrap();
    (run_id, writer)
}

#[tokio::test]
async fn closed_terminal_needs_bound_anchor_before_short_retention() {
    let stream = LocalRuntimeEventStream::new().with_recoverable_retention(Duration::ZERO);
    let (run_id, writer) = closed_generation(&stream, "flow_finished").await;
    assert!(writer.confirm_terminal_persisted(1).await.is_err());
    assert!(stream.replay(run_id, None, 10).await.is_ok());
    writer.set_durable_replay_boundary(41).unwrap();
    writer.set_durable_replay_boundary(41).unwrap();
    assert!(writer.set_durable_replay_boundary(42).is_err());
    assert_eq!(writer.durable_replay_boundary(), Some(41));
    assert!(writer.confirm_terminal_persisted(0).await.is_err());
    assert!(stream.replay(run_id, None, 10).await.is_ok());
    writer.confirm_terminal_persisted(1).await.unwrap();
    wait_for_idle_reclamation(&stream, run_id).await;
    assert!(stream.replay(run_id, None, 10).await.is_err());
}

#[tokio::test]
async fn sticky_persistence_failure_excludes_generation_from_short_retention() {
    let stream = LocalRuntimeEventStream::new()
        .with_recoverable_retention(Duration::ZERO)
        .with_recoverable_byte_budget(0);
    let (run_id, writer) = closed_generation(&stream, "flow_finished").await;
    writer.set_durable_replay_boundary(1).unwrap();
    writer.record_persistence_failure();
    assert!(writer.confirm_terminal_persisted(1).await.is_err());
    writer.set_durable_replay_boundary(1).unwrap();
    assert!(writer.confirm_terminal_persisted(1).await.is_err());
    assert_eq!(stream.list_ephemeral_entries().await.unwrap().len(), 1);
    assert!(stream.replay(run_id, None, 10).await.is_ok());
}

#[tokio::test]
async fn open_and_closed_without_terminal_cannot_claim_durable_confirmation() {
    let stream = LocalRuntimeEventStream::new().with_recoverable_retention(Duration::ZERO);
    let run_id = Uuid::now_v7();
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let writer = stream.terminal_writer(run_id).await.unwrap();
    writer.set_durable_replay_boundary(0).unwrap();
    assert!(writer.confirm_terminal_persisted(0).await.is_err());
    stream.append(run_id, heartbeat()).await.unwrap();
    stream
        .close_run(run_id, RuntimeEventCloseReason::Finished)
        .await
        .unwrap();
    assert!(writer.confirm_terminal_persisted(1).await.is_err());
    assert!(stream.replay(run_id, None, 10).await.is_ok());
}

#[tokio::test]
async fn default_confirmation_window_is_five_minutes_and_repeat_does_not_extend_it() {
    let stream = LocalRuntimeEventStream::new();
    let (run_id, writer) = closed_generation(&stream, "flow_finished").await;
    writer.set_durable_replay_boundary(20).unwrap();
    writer.confirm_terminal_persisted(1).await.unwrap();
    let deadline = stream.retention_deadline_for_tests(run_id).unwrap();
    let remaining = deadline - OffsetDateTime::now_utc();
    assert!(remaining <= TimeDuration::minutes(5) && remaining > TimeDuration::minutes(4));
    writer.confirm_terminal_persisted(1).await.unwrap();
    assert_eq!(
        stream.retention_deadline_for_tests(run_id).unwrap(),
        deadline
    );
    stream
        .set_confirmation_timestamp_for_tests(
            run_id,
            OffsetDateTime::now_utc() - TimeDuration::minutes(6),
        )
        .unwrap();
    wait_for_idle_reclamation(&stream, run_id).await;
}

#[tokio::test]
async fn confirmed_waiting_generation_expires_hot_data_without_shortening_unproven_waiting() {
    let stream = LocalRuntimeEventStream::new().with_recoverable_retention(Duration::ZERO);
    for kind in ["waiting_human", "waiting_callback"] {
        let (run_id, writer) = closed_generation(&stream, kind).await;
        let deadline = stream.retention_deadline_for_tests(run_id).unwrap();
        assert!(deadline - OffsetDateTime::now_utc() > TimeDuration::hours(23));
        assert!(writer.confirm_terminal_persisted(1).await.is_err());
        writer.set_durable_replay_boundary(10).unwrap();
        writer.confirm_terminal_persisted(1).await.unwrap();
        wait_for_idle_reclamation(&stream, run_id).await;
        assert_eq!(writer.closure().unwrap().final_sequence, 1);
    }
}

#[tokio::test]
async fn stale_confirmation_and_failure_cannot_affect_reopened_generation() {
    let stream = LocalRuntimeEventStream::new()
        .with_recoverable_retention(Duration::ZERO)
        .with_recoverable_byte_budget(0);
    let (run_id, old) = closed_generation(&stream, "waiting_human").await;
    old.set_durable_replay_boundary(8).unwrap();
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let current = stream.terminal_writer(run_id).await.unwrap();
    assert_ne!(old.generation_id(), current.generation_id());
    assert_eq!(current.durable_replay_boundary(), None);
    old.confirm_terminal_persisted(1).await.unwrap();
    old.record_persistence_failure();
    assert_eq!(
        stream
            .append(run_id, required_text_delta(1))
            .await
            .unwrap()
            .sequence,
        1
    );
    assert!(current.closure().is_none());
    assert_eq!(stream.list_ephemeral_entries().await.unwrap().len(), 1);
}

#[tokio::test]
async fn byte_budget_evicts_oldest_confirmed_closed_only_without_admission_rejection() {
    let stream = LocalRuntimeEventStream::new().with_recoverable_byte_budget(12_000);
    let mut generations = Vec::new();
    for _ in 0..3 {
        let run_id = Uuid::now_v7();
        stream
            .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        let writer = stream.terminal_writer(run_id).await.unwrap();
        stream
            .append(
                run_id,
                blob_event(8_000, RuntimeEventDurability::DurableRequired),
            )
            .await
            .unwrap();
        stream
            .append_terminal_if_missing_and_close(
                run_id,
                RuntimeEventPayload {
                    event_type: "flow_finished".into(),
                    source: RuntimeEventSource::System,
                    durability: RuntimeEventDurability::DurableRequired,
                    persist_required: true,
                    trace_visible: true,
                    payload: json!({"type":"flow_finished"}),
                },
            )
            .await
            .unwrap();
        generations.push((run_id, writer));
    }
    let (old_id, old) = generations.remove(0);
    let (new_id, new) = generations.remove(0);
    let (unproven_id, _) = generations.remove(0);
    let open_id = Uuid::now_v7();
    stream
        .open_run(open_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream
        .append(open_id, required_text_delta(0))
        .await
        .unwrap();
    let old_probe = stream.run_probe_for_tests(old_id).unwrap();
    old.set_durable_replay_boundary(1).unwrap();
    old.confirm_terminal_persisted(2).await.unwrap();
    new.set_durable_replay_boundary(2).unwrap();
    new.confirm_terminal_persisted(2).await.unwrap();
    stream.list_ephemeral_entries().await.unwrap();
    assert!(!stream.contains_run_without_purge_for_tests(old_id));
    assert!(stream.contains_run_without_purge_for_tests(new_id));
    assert!(stream.contains_run_without_purge_for_tests(unproven_id));
    assert!(stream.contains_run_without_purge_for_tests(open_id));
    assert_eq!((old_probe().1, old_probe().2), (0, 0));
    assert!(stream.append(open_id, required_text_delta(1)).await.is_ok());
}

#[tokio::test]
async fn zero_budget_confirmation_wakes_idle_gc_and_preserves_live_pending_pages() {
    let stream = LocalRuntimeEventStream::with_broadcast_capacity_for_tests(1)
        .with_recoverable_byte_budget(0);
    let run_id = Uuid::now_v7();
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let mut subscription = stream.subscribe(run_id, None).await.unwrap();
    let probe = stream.run_probe_for_tests(run_id).unwrap();
    subscription
        .terminal_writer
        .set_durable_replay_boundary(1)
        .unwrap();
    for index in 0..129 {
        stream
            .append(run_id, required_text_delta(index))
            .await
            .unwrap();
    }
    stream
        .append_terminal_if_missing_and_close(
            run_id,
            RuntimeEventPayload {
                event_type: "flow_finished".into(),
                source: RuntimeEventSource::System,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({"type":"flow_finished"}),
            },
        )
        .await
        .unwrap();
    subscription
        .terminal_writer
        .confirm_terminal_persisted(130)
        .await
        .unwrap();
    wait_for_idle_reclamation(&stream, run_id).await;
    assert_eq!((probe().1, probe().2), (0, 0));
    for sequence in 1..=130 {
        let event = tokio::time::timeout(Duration::from_secs(1), subscription.live_events.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.sequence, sequence);
    }
    assert!(subscription.live_events.recv().await.is_none());
}

#[tokio::test]
async fn failure_after_confirmation_restores_original_unproven_retention() {
    let stream = LocalRuntimeEventStream::new();
    let (run_id, writer) = closed_generation(&stream, "flow_finished").await;
    writer.set_durable_replay_boundary(3).unwrap();
    writer.confirm_terminal_persisted(1).await.unwrap();
    assert!(
        stream.retention_deadline_for_tests(run_id).unwrap() - OffsetDateTime::now_utc()
            < TimeDuration::minutes(6)
    );
    writer.record_persistence_failure();
    assert!(
        stream.retention_deadline_for_tests(run_id).unwrap() - OffsetDateTime::now_utc()
            > TimeDuration::minutes(119)
    );
    assert!(writer.confirm_terminal_persisted(1).await.is_err());
}

#[tokio::test]
async fn persistence_owner_claim_is_once_per_generation_and_independent_after_reopen() {
    let stream = LocalRuntimeEventStream::new();
    let (run_id, old) = closed_generation(&stream, "flow_finished").await;
    let another_old = stream.terminal_writer(run_id).await.unwrap();
    assert!(old.claim_persistence_owner());
    assert!(!old.claim_persistence_owner());
    assert!(!another_old.claim_persistence_owner());
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let current = stream.terminal_writer(run_id).await.unwrap();
    assert_ne!(old.generation_id(), current.generation_id());
    assert!(current.claim_persistence_owner());
    assert!(!current.claim_persistence_owner());
    assert!(!old.claim_persistence_owner());
}
