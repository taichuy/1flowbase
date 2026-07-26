use std::time::Duration;

use control_plane::ports::{
    RuntimeEventCloseReason, RuntimeEventDurability, RuntimeEventPayload, RuntimeEventSource,
    RuntimeEventStream, RuntimeEventStreamPolicy, RuntimeEventTrimPolicy,
};
use serde_json::json;
use time::{Duration as TimeDuration, OffsetDateTime};
use uuid::Uuid;

use crate::host_infrastructure::LocalRuntimeEventStream;

fn heartbeat() -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "heartbeat".to_string(),
        source: RuntimeEventSource::System,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: false,
        trace_visible: false,
        payload: json!({ "type": "heartbeat" }),
    }
}

fn required_text_delta(index: usize) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "text_delta".to_string(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({ "index": index }),
    }
}

fn blob_event(size: usize, durability: RuntimeEventDurability) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "debug_blob".to_string(),
        source: RuntimeEventSource::Runtime,
        durability,
        persist_required: durability != RuntimeEventDurability::Ephemeral,
        trace_visible: true,
        payload: json!({ "blob": "x".repeat(size) }),
    }
}

fn persist_requested_ephemeral_blob(size: usize) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "debug_blob".to_string(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::Ephemeral,
        persist_required: true,
        trace_visible: true,
        payload: json!({ "blob": "x".repeat(size) }),
    }
}

#[tokio::test]
async fn local_runtime_event_stream_assigns_monotonic_sequence() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let first = stream.append(run_id, heartbeat()).await.unwrap();
    let second = stream.append(run_id, heartbeat()).await.unwrap();

    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);
    assert_ne!(first.event_id, second.event_id);
}

#[tokio::test]
async fn local_runtime_event_stream_envelope_exposes_delta_metadata() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let event = stream
        .append(
            run_id,
            RuntimeEventPayload {
                event_type: "reasoning_delta".to_string(),
                source: RuntimeEventSource::Provider,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: false,
                payload: json!({
                    "type": "reasoning_delta",
                    "node_run_id": node_run_id,
                    "node_id": "node-llm",
                    "text": "thinking"
                }),
            },
        )
        .await
        .unwrap();

    assert_eq!(event.event_id, format!("{run_id}:1"));
    assert_eq!(event.run_id, run_id);
    assert_eq!(event.node_run_id, Some(node_run_id));
    assert_eq!(event.sequence, 1);
    assert_eq!(event.delta_index, Some(1));
    assert_eq!(event.content_type.as_deref(), Some("reasoning"));
    assert_eq!(event.text.as_deref(), Some("thinking"));
}

#[tokio::test]
async fn local_runtime_event_stream_replays_then_subscribes_live() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    let mut subscription = stream.subscribe(run_id, Some(0)).await.unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();

    assert_eq!(subscription.replay.len(), 1);
    assert_eq!(subscription.replay[0].sequence, 1);
    let live = subscription.live_events.recv().await.unwrap();
    assert_eq!(live.sequence, 2);
}

#[tokio::test]
async fn local_runtime_event_stream_reports_replay_expired_after_trim() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream
        .trim(
            run_id,
            RuntimeEventTrimPolicy {
                before_sequence: Some(2),
                keep_required: false,
            },
        )
        .await
        .unwrap();

    let err = match stream.subscribe(run_id, Some(0)).await {
        Ok(_) => panic!("expected replay expired error"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("runtime event replay expired"));
}

#[tokio::test]
async fn local_runtime_event_stream_overflow_preserves_required_events() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();
    let policy = RuntimeEventStreamPolicy {
        max_events: 3,
        ..RuntimeEventStreamPolicy::debug_default()
    };

    stream.open_run(run_id, policy).await.unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream.append(run_id, required_text_delta(1)).await.unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream.append(run_id, required_text_delta(2)).await.unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();

    let replay = stream.replay(run_id, Some(1), 10).await.unwrap();
    let sequences = replay
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>();

    assert!(sequences.contains(&2));
    assert!(sequences.contains(&4));
    assert!(!sequences.contains(&3));
}

#[tokio::test]
async fn local_runtime_event_stream_byte_budget_evicts_old_ephemeral_events() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();
    let policy = RuntimeEventStreamPolicy {
        max_events: 10,
        max_bytes: 3_000,
        ..RuntimeEventStreamPolicy::debug_default()
    };

    stream.open_run(run_id, policy).await.unwrap();
    stream
        .append(run_id, blob_event(1_800, RuntimeEventDurability::Ephemeral))
        .await
        .unwrap();
    let retained = stream
        .append(run_id, blob_event(1_800, RuntimeEventDurability::Ephemeral))
        .await
        .unwrap();

    let retained_entries = stream.list_ephemeral_entries().await.unwrap();
    assert_eq!(retained_entries.len(), 1);
    assert_eq!(retained_entries[0].metadata["sequence"], retained.sequence);
}

#[tokio::test]
async fn local_runtime_event_stream_can_evict_ephemeral_history_waiting_for_persistence() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();
    let policy = RuntimeEventStreamPolicy {
        max_events: 10,
        max_bytes: 3_000,
        ..RuntimeEventStreamPolicy::debug_default()
    };

    stream.open_run(run_id, policy).await.unwrap();
    stream
        .append(run_id, persist_requested_ephemeral_blob(1_800))
        .await
        .unwrap();
    let retained = stream
        .append(run_id, persist_requested_ephemeral_blob(1_800))
        .await
        .unwrap();

    let replay = stream.replay(run_id, Some(1), 10).await.unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].sequence, retained.sequence);
    assert!(replay[0].persist_required);
}

#[tokio::test]
async fn local_runtime_event_stream_byte_budget_rejects_required_overflow() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();
    let policy = RuntimeEventStreamPolicy {
        max_events: 10,
        max_bytes: 3_000,
        ..RuntimeEventStreamPolicy::debug_default()
    };

    stream.open_run(run_id, policy).await.unwrap();
    stream
        .append(
            run_id,
            blob_event(1_800, RuntimeEventDurability::DurableRequired),
        )
        .await
        .unwrap();
    let error = stream
        .append(
            run_id,
            blob_event(1_800, RuntimeEventDurability::DurableRequired),
        )
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("runtime event stream byte capacity exceeded"));
    let replay = stream.replay(run_id, Some(0), 10).await.unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].sequence, 1);
}

#[tokio::test]
async fn local_runtime_event_stream_trim_keep_required_preserves_required_events() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream.append(run_id, required_text_delta(1)).await.unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();

    stream
        .trim(
            run_id,
            RuntimeEventTrimPolicy {
                before_sequence: Some(4),
                keep_required: true,
            },
        )
        .await
        .unwrap();

    let replay = stream.replay(run_id, Some(1), 10).await.unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].sequence, 2);
    assert!(replay[0].persist_required);
}

#[tokio::test]
async fn local_runtime_event_stream_backfills_retained_events_after_live_lag() {
    let stream = LocalRuntimeEventStream::with_broadcast_capacity_for_tests(1);
    let run_id = Uuid::now_v7();
    let policy = RuntimeEventStreamPolicy {
        max_events: 10,
        ..RuntimeEventStreamPolicy::debug_default()
    };

    stream.open_run(run_id, policy).await.unwrap();
    let mut subscription = stream.subscribe(run_id, Some(0)).await.unwrap();
    assert!(subscription.replay.is_empty());

    for index in 0..5 {
        stream
            .append(run_id, required_text_delta(index))
            .await
            .unwrap();
    }

    let mut sequences = Vec::new();
    for _ in 0..5 {
        let event = tokio::time::timeout(Duration::from_secs(1), subscription.live_events.recv())
            .await
            .unwrap()
            .unwrap();
        sequences.push(event.sequence);
    }

    assert_eq!(sequences, vec![1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn local_runtime_event_stream_required_lane_preserves_duplicates_at_capacity_one() {
    let stream = LocalRuntimeEventStream::with_broadcast_capacity_for_tests(1);
    let run_id = Uuid::now_v7();
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let mut subscription = stream.subscribe(run_id, Some(0)).await.unwrap();

    stream.append(run_id, required_text_delta(7)).await.unwrap();
    stream.append(run_id, required_text_delta(7)).await.unwrap();

    let first = subscription.live_events.recv().await.unwrap();
    let second = subscription.live_events.recv().await.unwrap();
    assert_eq!((first.sequence, second.sequence), (1, 2));
    assert_eq!(first.payload, second.payload);
}

#[tokio::test]
async fn local_runtime_event_stream_diagnostic_saturation_does_not_block_required_lane() {
    let stream = LocalRuntimeEventStream::with_broadcast_capacity_for_tests(1);
    let run_id = Uuid::now_v7();
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let mut subscription = stream.subscribe(run_id, Some(0)).await.unwrap();

    stream.append(run_id, heartbeat()).await.unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream.append(run_id, required_text_delta(1)).await.unwrap();

    let required = subscription.live_events.recv().await.unwrap();
    assert_eq!(required.event_type, "text_delta");
    assert_eq!(required.sequence, 3);
}

#[tokio::test]
async fn local_runtime_event_stream_rejects_append_after_close() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream
        .close_run(run_id, RuntimeEventCloseReason::Finished)
        .await
        .unwrap();

    let err = stream.append(run_id, heartbeat()).await.unwrap_err();
    assert!(err.to_string().contains("runtime event stream is closed"));
}

#[tokio::test]
async fn local_runtime_event_stream_rejects_oversized_payloads() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();

    let err = stream
        .append(
            run_id,
            RuntimeEventPayload {
                event_type: "debug_blob".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::Ephemeral,
                persist_required: false,
                trace_visible: true,
                payload: json!({ "blob": "x".repeat(2 * 1024 * 1024) }),
            },
        )
        .await
        .unwrap_err();

    assert!(err.to_string().contains("ephemeral_payload_too_large"));
    assert!(stream.replay(run_id, Some(0), 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn local_runtime_event_stream_open_run_reopens_closed_run_for_resume_phase() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream
        .close_run(run_id, RuntimeEventCloseReason::WaitingCallback)
        .await
        .unwrap();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let resumed = stream.append(run_id, required_text_delta(1)).await.unwrap();
    let subscription = stream.subscribe(run_id, Some(0)).await.unwrap();

    assert_eq!(resumed.sequence, 1);
    assert_eq!(subscription.replay.len(), 1);
    assert_eq!(subscription.replay[0].event_type, "text_delta");
}

#[tokio::test]
async fn local_runtime_event_stream_subscribe_after_closed_cursor_finishes() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream
        .close_run(run_id, RuntimeEventCloseReason::Finished)
        .await
        .unwrap();

    let mut subscription = stream.subscribe(run_id, Some(1)).await.unwrap();
    assert!(subscription.replay.is_empty());
    assert!(subscription.live_events.recv().await.is_none());
}

#[tokio::test]
async fn local_runtime_event_stream_subscribe_after_closed_replay_finishes() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream
        .close_run(run_id, RuntimeEventCloseReason::Finished)
        .await
        .unwrap();

    let mut subscription = stream.subscribe(run_id, Some(0)).await.unwrap();
    assert_eq!(subscription.replay.len(), 1);
    assert!(subscription.live_events.recv().await.is_none());
}

#[tokio::test]
async fn local_runtime_event_stream_close_wakes_live_subscription() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let mut subscription = stream.subscribe(run_id, Some(0)).await.unwrap();
    assert!(subscription.replay.is_empty());

    stream
        .close_run(run_id, RuntimeEventCloseReason::Finished)
        .await
        .unwrap();

    let closed = tokio::time::timeout(Duration::from_secs(1), subscription.live_events.recv())
        .await
        .expect("close_run should wake live subscribers");
    assert!(closed.is_none());

    subscription
        .closure
        .changed()
        .await
        .expect("close_run should publish a typed closure");
    let closure = *subscription.closure.borrow();
    assert_eq!(closure.unwrap().reason, RuntimeEventCloseReason::Finished);
    assert_eq!(closure.unwrap().final_sequence, 0);
}

#[tokio::test]
async fn local_runtime_event_stream_subscribe_after_close_exposes_typed_closure_snapshot() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream
        .close_run(run_id, RuntimeEventCloseReason::WaitingCallback)
        .await
        .unwrap();

    let subscription = stream.subscribe(run_id, Some(1)).await.unwrap();
    let closure = (*subscription.closure.borrow()).unwrap();

    assert_eq!(closure.reason, RuntimeEventCloseReason::WaitingCallback);
    assert_eq!(closure.final_sequence, 1);
}

#[test]
fn runtime_event_stream_debug_default_keeps_closed_runs_for_two_hours() {
    assert_eq!(
        RuntimeEventStreamPolicy::debug_default().ttl,
        TimeDuration::hours(2)
    );
}

#[tokio::test]
async fn local_runtime_event_stream_expires_finished_run_after_closed_retention() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream
        .close_run(run_id, RuntimeEventCloseReason::Finished)
        .await
        .unwrap();
    stream
        .set_run_timestamps_for_tests(
            run_id,
            OffsetDateTime::now_utc() - TimeDuration::hours(3),
            Some(OffsetDateTime::now_utc() - TimeDuration::hours(2) - TimeDuration::seconds(1)),
        )
        .unwrap();

    assert!(stream.list_ephemeral_entries().await.unwrap().is_empty());
    let err = match stream.subscribe(run_id, Some(0)).await {
        Ok(_) => panic!("expected expired stream to be removed"),
        Err(error) => error,
    };
    assert!(err.to_string().contains("runtime event stream is not open"));
}

#[tokio::test]
async fn local_runtime_event_stream_keeps_waiting_run_for_twenty_four_hours() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream
        .close_run(run_id, RuntimeEventCloseReason::WaitingCallback)
        .await
        .unwrap();
    stream
        .set_run_timestamps_for_tests(
            run_id,
            OffsetDateTime::now_utc() - TimeDuration::hours(3),
            Some(OffsetDateTime::now_utc() - TimeDuration::hours(23)),
        )
        .unwrap();

    assert_eq!(stream.list_ephemeral_entries().await.unwrap().len(), 1);

    stream
        .set_run_timestamps_for_tests(
            run_id,
            OffsetDateTime::now_utc() - TimeDuration::hours(25),
            Some(OffsetDateTime::now_utc() - TimeDuration::hours(24) - TimeDuration::seconds(1)),
        )
        .unwrap();

    assert!(stream.list_ephemeral_entries().await.unwrap().is_empty());
}

#[tokio::test]
async fn local_runtime_event_stream_expires_orphan_open_run_after_seventy_two_hours() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();

    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(run_id, heartbeat()).await.unwrap();
    stream
        .set_run_timestamps_for_tests(
            run_id,
            OffsetDateTime::now_utc() - TimeDuration::hours(72) - TimeDuration::seconds(1),
            None,
        )
        .unwrap();

    assert!(stream.list_ephemeral_entries().await.unwrap().is_empty());
}
