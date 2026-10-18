use super::*;

#[tokio::test]
async fn required_large_payloads_survive_live_backfill_replay_and_terminal_reclamation() {
    let stream = LocalRuntimeEventStream::with_broadcast_capacity_for_tests(1)
        .with_recoverable_retention(Duration::ZERO);
    let run_id = Uuid::now_v7();
    stream
        .open_run(
            run_id,
            RuntimeEventStreamPolicy {
                max_events: 1,
                ..RuntimeEventStreamPolicy::debug_default()
            },
        )
        .await
        .unwrap();
    let writer = stream.terminal_writer(run_id).await.unwrap();
    writer.set_durable_replay_boundary(0).unwrap();
    let mut subscription = stream.subscribe(run_id, Some(0)).await.unwrap();
    let text = format!("{}完整🙂", "x".repeat(16 * 1024 * 1024 + 1));
    // These events are transient, but their delivery is required. Durability
    // alone must not make a paused consumer lose a text delta or output item.
    for kind in ["text_delta", "provider_output_item_done"] {
        stream
            .append(
                run_id,
                RuntimeEventPayload {
                    event_type: kind.into(),
                    source: RuntimeEventSource::Provider,
                    durability: RuntimeEventDurability::Ephemeral,
                    persist_required: false,
                    trace_visible: true,
                    payload: json!({"value": text}),
                },
            )
            .await
            .unwrap();
    }
    stream
        .append_terminal_if_missing_and_close(
            run_id,
            RuntimeEventPayload {
                event_type: "flow_finished".into(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({"value": text}),
            },
        )
        .await
        .unwrap();
    let replay = stream.replay(run_id, Some(0), 10).await.unwrap();
    assert_eq!(replay.len(), 3);
    for (index, expected) in replay.iter().enumerate() {
        assert_eq!(expected.sequence, index as i64 + 1);
        assert_eq!(expected.payload["value"].as_str(), Some(text.as_str()));
        let live = tokio::time::timeout(Duration::from_secs(10), subscription.live_events.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(live, *expected);
    }
    assert!(
        tokio::time::timeout(Duration::from_secs(10), subscription.live_events.recv())
            .await
            .unwrap()
            .is_none()
    );
    drop(replay);
    drop(subscription);
    writer.confirm_terminal_persisted(3).await.unwrap();
    wait_for_idle_reclamation(&stream, run_id).await;
    assert!(stream.replay(run_id, Some(0), 10).await.is_err());
}
