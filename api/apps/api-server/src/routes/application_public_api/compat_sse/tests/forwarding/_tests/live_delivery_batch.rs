use super::*;

#[tokio::test]
async fn typed_responses_stream_drains_live_batch_before_later_notifications() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let (live_sender, live_events) = tokio::sync::mpsc::unbounded_channel();
    let (sender, mut receiver) = mpsc::channel(8);
    let forwarding = spawn_typed_stream(&state, &run, Vec::new(), live_events, sender);
    // Establish that the initial pending scan finished; yield_now is not a barrier.
    live_sender
        .send(RuntimeEventEnvelope::new(
            run.id,
            1,
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
    let barrier = receiver.recv().await.unwrap();
    assert_eq!(barrier.into_parts().1.event_type, "flow_started");

    let payloads = ["call-first", "call-second", "call-third"].map(committed_delivery_payload);
    let mut records = Vec::new();
    for payload in &payloads {
        records.push(seed_pending_committed_delivery(&state, &run, payload.clone()).await);
    }
    live_sender
        .send(committed_delivery_envelope(
            run.id,
            100,
            payloads[0].clone(),
        ))
        .unwrap();
    let mut receipts = Vec::new();
    for (index, payload) in payloads.iter().enumerate() {
        let input = tokio::time::timeout(Duration::from_secs(3), receiver.recv())
            .await
            .expect("one notification must drain the whole committed batch")
            .expect("delivery stream remains open");
        let (_, envelope, receipt) = input.into_parts();
        assert_eq!(
            &envelope.payload, payload,
            "durable batch order must be stable"
        );
        let receipt = receipt.expect("each item retains its writer receipt");
        assert_eq!(receipt.event_id(), Some(records[index].id));
        receipts.push(receipt);
    }
    let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    assert_eq!(
        delivery_statuses(&state, &ids).await,
        vec!["claimed"; 3],
        "internal enqueue must not ACK delivery"
    );
    for receipt in receipts {
        receipt.projected();
    }
    // All echoes, including the wake-up event, must be harmless after the batch drain.
    for (index, payload) in payloads.iter().enumerate() {
        live_sender
            .send(committed_delivery_envelope(
                run.id,
                101 + index as i64,
                payload.clone(),
            ))
            .unwrap();
    }
    drop(live_sender);
    assert!(
        receiver.recv().await.is_none(),
        "live echoes must not execute tools twice"
    );
    forwarding.await.unwrap();
    assert_eq!(delivery_statuses(&state, &ids).await, vec!["acked"; 3]);
    assert_eq!(claimable_delivery_count(&state, run.id).await, 0);
}

#[tokio::test]
async fn native_deltas_survive_real_subscription_typed_cursor_and_transparent_sse() {
    let fragments = [
        "HAND", "OFF", "_", "69", "faf", "c", "3", "c", "-", "3", "ac", "0", "-", "4", "ebb", "-",
        "968", "1", "-", "82", "ef", "452", "b", "358", "1",
    ];
    let expected = "HANDOFF_69fafc3c-3ac0-4ebb-9681-82ef452b3581";
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: domain::FlowRunStatus::Succeeded,
            output_payload: json!({"answer":expected,"__canonical_answer_presentation":true}),
            error_payload: None,
            finished_at: Some(time::OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
    let stream = Arc::new(LocalRuntimeEventStream::with_broadcast_capacity_for_tests(
        1,
    ));
    stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let subscription = stream.subscribe(run.id, Some(0)).await.unwrap();
    let node = Uuid::now_v7();
    // Subscribe first, then hold the consumer while the raw/canonical pairs
    // accumulate. This exercises live lanes, not sorted replay-only projection.
    for fragment in fragments {
        stream
            .append(
                run.id,
                debug_stream_events::provider_responses_output_delta(
                    "llm",
                    node,
                    json!({"type":"response.output_text.delta","delta":fragment}),
                ),
            )
            .await
            .unwrap();
        stream
            .append(
                run.id,
                debug_stream_events::text_delta("llm", node, fragment.into()),
            )
            .await
            .unwrap();
    }
    stream
        .append(
            run.id,
            debug_stream_events::provider_responses_output_delta(
                "llm",
                node,
                json!({"type":"response.output_text.done","text":expected}),
            ),
        )
        .await
        .unwrap();
    stream
        .append_terminal_if_missing_and_close(
            run.id,
            debug_stream_events::flow_finished(run.id, json!({})),
        )
        .await
        .unwrap();
    let (sender, mut receiver) = mpsc::channel(1);
    let dependencies = NativeRunTerminalDependencies::new(
        state.store.clone(),
        state.runtime_engine.clone(),
        state.provider_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.model_billing_require_provider_usage,
        state.infrastructure.provider_transport_store(),
        stream,
    );
    let mut forwarding = tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            terminal_dependencies: dependencies,
            initial_run: run.clone(),
            from_sequence: None,
            ignored_waiting_callback_task_id: None,
            subscription,
            sender,
        },
    ));
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::TransparentProviderResponses,
    );
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let mut projected = Vec::new();
        let mut raw_sequences = Vec::new();
        while let Some(input) = receiver.recv().await {
            let (snapshot, envelope, receipt) = input.into_parts();
            assert!(
                receipt.is_none(),
                "text streaming must not invent durable delivery claims"
            );
            if envelope.event_type == "provider_responses_output_delta"
                && envelope.payload["event"]["type"] == "response.output_text.delta"
            {
                assert_eq!(envelope.durability, RuntimeEventDurability::Ephemeral);
                assert!(!envelope.persist_required);
                raw_sequences.push(envelope.sequence);
            }
            projected.extend(mapper.runtime_event_to_sse(&snapshot, envelope));
            tokio::task::yield_now().await;
        }
        assert_eq!(
            raw_sequences,
            (0..25).map(|index| index * 2 + 1).collect::<Vec<i64>>()
        );
        projected
    })
    .await;
    if result.is_err() {
        forwarding.abort();
        let _ = forwarding.await;
        panic!("live raw deltas must flow through the real shared cursor to terminal");
    }
    tokio::time::timeout(Duration::from_secs(1), &mut forwarding)
        .await
        .unwrap()
        .unwrap();
    let body = axum::body::to_bytes(
        test_projected_events_response(result.unwrap()).into_body(),
        usize::MAX,
    )
    .await
    .unwrap();
    let payloads = std::str::from_utf8(&body)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    let deltas = payloads
        .iter()
        .filter(|event| event["type"] == "response.output_text.delta")
        .map(|event| event["delta"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(deltas, fragments);
    assert_eq!(deltas.concat(), expected);
    assert_eq!(
        payloads
            .iter()
            .filter(|event| event["type"] == "response.completed")
            .count(),
        1
    );
    assert_eq!(payloads.last().unwrap()["type"], "response.completed");
}
