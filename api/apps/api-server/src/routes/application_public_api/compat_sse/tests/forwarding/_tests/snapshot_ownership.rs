use super::*;

#[tokio::test]
async fn typed_stream_shares_immutable_initial_history_between_live_frames() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    run.node_input_payload = json!({"history_blob": "history".repeat(8192)});
    run.metadata = json!({"response_round_id": "round-original"});
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let (live_sender, live_events) = tokio::sync::mpsc::unbounded_channel();
    let (sender, mut receiver) = mpsc::channel(8);
    let forwarding = spawn_typed_stream(&state, &run, Vec::new(), live_events, sender);
    for sequence in 1..=2 {
        live_sender
            .send(RuntimeEventEnvelope::new(
                run.id,
                sequence,
                RuntimeEventPayload {
                    event_type: "flow_started".into(),
                    source: RuntimeEventSource::Runtime,
                    durability: RuntimeEventDurability::DurableRequired,
                    persist_required: true,
                    trace_visible: true,
                    payload: json!({}),
                },
            ))
            .unwrap();
    }
    let first = tokio::time::timeout(Duration::from_secs(3), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    let second = tokio::time::timeout(Duration::from_secs(3), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    let (first_run, first_event, _) = first.into_parts();
    let (second_run, second_event, _) = second.into_parts();
    assert_eq!((first_event.sequence, second_event.sequence), (1, 2));
    assert_eq!(first_run.node_input_payload, run.node_input_payload);
    assert_eq!(second_run.metadata, run.metadata);
    assert_eq!(
        first_run.node_input_payload["history_blob"]
            .as_str()
            .unwrap()
            .as_ptr(),
        second_run.node_input_payload["history_blob"]
            .as_str()
            .unwrap()
            .as_ptr(),
        "ordinary frames must reuse immutable history rather than deep-clone it"
    );
    drop(live_sender);
    while receiver.recv().await.is_some() {}
    forwarding.await.unwrap();
}
