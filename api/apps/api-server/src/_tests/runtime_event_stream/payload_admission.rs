use super::*;

#[tokio::test]
async fn diagnostic_admission_and_trim_preserve_required_gaps_and_paged_replay() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();
    stream
        .open_run(
            run_id,
            RuntimeEventStreamPolicy {
                max_events: 4,
                max_bytes: usize::MAX,
                ..RuntimeEventStreamPolicy::debug_default()
            },
        )
        .await
        .unwrap();
    for event in [
        required_text_delta(1),
        heartbeat(),
        required_text_delta(3),
        heartbeat(),
    ] {
        stream.append(run_id, event).await.unwrap();
    }
    stream.append(run_id, required_text_delta(5)).await.unwrap();
    stream.append(run_id, required_text_delta(6)).await.unwrap();
    assert_eq!(
        stream
            .append(run_id, heartbeat())
            .await
            .unwrap_err()
            .to_string(),
        "runtime event stream event capacity exceeded"
    );
    let mut cursor = 0;
    let mut sequences = Vec::new();
    loop {
        let page = stream.replay(run_id, Some(cursor), 2).await.unwrap();
        let Some(last) = page.last() else { break };
        cursor = last.sequence;
        sequences.extend(page.iter().map(|event| event.sequence));
    }
    assert_eq!(sequences, [1, 3, 5, 6]);
    assert!(stream.replay(run_id, Some(0), 0).await.unwrap().is_empty());
    assert!(stream
        .replay(run_id, Some(i64::MAX), 2)
        .await
        .unwrap()
        .is_empty());
    stream
        .trim(
            run_id,
            RuntimeEventTrimPolicy {
                before_sequence: Some(4),
                keep_required: false,
            },
        )
        .await
        .unwrap();
    assert!(stream.replay(run_id, Some(0), 2).await.is_err());
    assert_eq!(
        stream.append(run_id, heartbeat()).await.unwrap().sequence,
        7
    );
    stream.append(run_id, heartbeat()).await.unwrap();
    stream.append(run_id, required_text_delta(9)).await.unwrap();
    assert_eq!(
        stream
            .replay(run_id, Some(4), 10)
            .await
            .unwrap()
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [5, 6, 8, 9]
    );
}

#[tokio::test]
async fn byte_pressure_removes_consecutive_diagnostics_before_admitting_required_payload() {
    let stream = LocalRuntimeEventStream::new();
    let run_id = Uuid::now_v7();
    stream
        .open_run(
            run_id,
            RuntimeEventStreamPolicy {
                max_events: 100,
                max_bytes: 10_000,
                ..RuntimeEventStreamPolicy::debug_default()
            },
        )
        .await
        .unwrap();
    for event in [
        required_text_delta(1),
        heartbeat(),
        heartbeat(),
        required_text_delta(4),
    ] {
        stream.append(run_id, event).await.unwrap();
    }
    stream
        .append(
            run_id,
            blob_event(20_000, RuntimeEventDurability::DurableRequired),
        )
        .await
        .unwrap();
    assert_eq!(
        stream
            .append(run_id, heartbeat())
            .await
            .unwrap_err()
            .to_string(),
        "runtime event stream byte capacity exceeded"
    );
    assert_eq!(
        stream
            .replay(run_id, Some(0), 10)
            .await
            .unwrap()
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        [1, 4, 5]
    );
    assert_eq!(
        stream
            .append(run_id, required_text_delta(6))
            .await
            .unwrap()
            .sequence,
        6
    );
}

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
