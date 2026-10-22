use super::*;

#[tokio::test]
async fn uncertain_native_tools_replay_exactly_without_delivery_receipts() {
    assert_exact_native_tool_replay(false).await;
}

#[tokio::test]
async fn acked_native_tools_replay_exactly_without_delivery_receipts() {
    assert_exact_native_tool_replay(true).await;
}

async fn assert_exact_native_tool_replay(acked: bool) {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    run.metadata["response_round_id"] = json!(Uuid::now_v7());
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    // Preserve the provider's argument bytes, namespace and unknown fields.
    // Reconstructing a semantic function_call would lose these native facts.
    let arguments = "{ \"target\" : \"/root\", \"message\" : \"hello\\nworld\" }";
    let items = vec![
        json!({
            "id": "provider_function_original",
            "type": "function_call", "call_id": "native_function_call",
            "name": "send_message", "namespace": "collaboration",
            "arguments": arguments, "status": "completed",
            "opaque_provider_state": {"bytes": "AQID==", "flags": [3, 1]}
        }),
        json!({
            "id": "provider_custom_original",
            "type": "custom_tool_call", "call_id": "native_custom_call",
            "name": "apply_patch", "input": "*** Begin Patch\n*** End Patch\n",
            "opaque_extension": ["preserve", {"nested": true}]
        }),
    ];
    let payloads = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            json!({
                "output_index": index, "committed_delivery": true, "item": item
            })
        })
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    for payload in &payloads {
        records.push(seed_pending_committed_delivery(&state, &run, payload.clone()).await);
    }
    // Establish the status through the real receipt owner, as after a completed
    // write or a disconnected writer. No callback is submitted by this replay.
    let (live_sender, live_events) = mpsc::unbounded_channel();
    drop(live_sender);
    let (sender, mut receiver) = mpsc::channel(8);
    let forwarding = spawn_typed_stream(&state, &run, Vec::new(), live_events, sender);
    let mut receipt_count = 0;
    while let Some(input) = receiver.recv().await {
        let (_, _, receipt) = input.into_parts();
        let mut receipt = receipt.expect("initial delivery owns a claim");
        receipt_count += 1;
        if acked {
            receipt.projected();
        } else {
            receipt.begin_write();
            drop(receipt);
        }
    }
    forwarding.await.unwrap();
    assert_eq!(receipt_count, items.len());
    let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    let expected_status = if acked { "acked" } else { "uncertain" };
    assert_eq!(
        delivery_statuses(&state, &ids).await,
        vec![expected_status; 2]
    );

    for lane in ["durable", "subscription", "live"] {
        let envelopes = |start| {
            payloads
                .iter()
                .enumerate()
                .map(|(index, payload)| {
                    committed_delivery_envelope(run.id, start + index as i64, payload.clone())
                })
                .collect::<Vec<_>>()
        };
        let durable = if lane == "durable" {
            envelopes(100)
        } else {
            Vec::new()
        };
        let replay = if lane != "live" {
            envelopes(1)
        } else {
            Vec::new()
        };
        let (live_sender, live_events) = mpsc::unbounded_channel();
        // Durable-follow attachment suffixes use this same live lane. Repeated
        // notifications must not produce duplicate output items in one response.
        for event in envelopes(3).into_iter().chain(envelopes(5)) {
            live_sender.send(event).unwrap();
        }
        live_sender
            .send(RuntimeEventEnvelope::new(
                run.id,
                7,
                RuntimeEventPayload {
                    event_type: "waiting_callback".into(),
                    source: RuntimeEventSource::Runtime,
                    durability: RuntimeEventDurability::DurableRequired,
                    persist_required: true,
                    trace_visible: true,
                    payload: json!({
                        "callback_task_id": Uuid::now_v7(),
                        "callback_kind": "llm_tool_calls",
                        "tool_calls": [{
                            "id": "native_function_call", "name": "send_message",
                            "arguments": arguments
                        }]
                    }),
                },
            ))
            .unwrap();
        drop(live_sender);
        let (sender, mut receiver) = mpsc::channel(8);
        let forwarding =
            spawn_typed_stream_with_durable(&state, &run, durable, replay, live_events, sender);
        let mut mapper = OpenAiResponseStreamMapper::with_mode(
            "fixture".into(),
            None,
            ResponsesProjectionMode::TransparentProviderResponses,
        );
        let mut projected = Vec::new();
        let mut forwarded_items = Vec::new();
        while let Some(input) = receiver.recv().await {
            let (snapshot, envelope, receipt) = input.into_parts();
            assert!(
                receipt.is_none(),
                "{lane}: replay must not acquire delivery ownership"
            );
            if envelope.event_type == "provider_output_item_done" {
                forwarded_items.push(envelope.payload["item"].clone());
            }
            projected.extend(mapper.runtime_event_to_sse(&snapshot, envelope));
        }
        forwarding.await.unwrap();
        assert_eq!(
            forwarded_items, items,
            "{lane}: one exact fact per item per response"
        );
        let response = axum::response::sse::Sse::new(tokio_stream::iter(projected)).into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = std::str::from_utf8(&body).unwrap();
        let events = body
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        let done = events
            .iter()
            .filter(|event| event["type"] == "response.output_item.done")
            .map(|event| event["item"].clone())
            .collect::<Vec<_>>();
        assert_eq!(done, items, "{lane}: native item event equality");
        let completed = events
            .iter()
            .filter(|event| event["type"] == "response.completed")
            .collect::<Vec<_>>();
        assert_eq!(completed.len(), 1, "{lane}: {body}");
        assert_eq!(completed[0]["response"]["output"], json!(items), "{lane}");
        assert!(!body.contains("fc_native_function_call"), "{lane}: {body}");
        assert_eq!(
            delivery_statuses(&state, &ids).await,
            vec![expected_status; 2]
        );
        assert_eq!(claimable_delivery_count(&state, run.id).await, 0);
    }
}

#[tokio::test]
async fn a_later_claim_batch_releases_an_already_replayed_fact_without_acking_it() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let original = committed_delivery_payload("already-read");
    let first = seed_pending_committed_delivery(&state, &run, original.clone()).await;
    sqlx::query("update runtime_events set delivery_status = 'uncertain' where id = $1")
        .bind(first.id)
        .execute(state.store.pool())
        .await
        .unwrap();
    let (live_sender, live_events) = mpsc::unbounded_channel();
    let (sender, mut receiver) = mpsc::channel(8);
    let forwarding = spawn_typed_stream(&state, &run, Vec::new(), live_events, sender);
    live_sender
        .send(committed_delivery_envelope(run.id, 1, original.clone()))
        .unwrap();
    let (_, envelope, receipt) = receiver.recv().await.unwrap().into_parts();
    assert_eq!(envelope.payload, original);
    assert!(receipt.is_none());

    // A later recovery can make the previously read fact claimable. The next
    // live wake-up then claims both rows; only its new fact should be emitted.
    sqlx::query("update runtime_events set delivery_status = 'pending' where id = $1")
        .bind(first.id)
        .execute(state.store.pool())
        .await
        .unwrap();
    let fresh = committed_delivery_payload("new-delivery");
    let second = seed_pending_committed_delivery(&state, &run, fresh.clone()).await;
    live_sender
        .send(committed_delivery_envelope(run.id, 2, fresh.clone()))
        .unwrap();
    let (_, envelope, receipt) = receiver.recv().await.unwrap().into_parts();
    assert_eq!(envelope.payload, fresh);
    receipt
        .expect("new fact retains its delivery owner")
        .projected();
    drop(live_sender);
    assert!(
        receiver.recv().await.is_none(),
        "the old fact must not be sent twice"
    );
    forwarding.await.unwrap();
    assert_eq!(
        delivery_statuses(&state, &[first.id, second.id]).await,
        vec!["pending", "acked"],
        "an already-read fact's unused claim is released, not falsely ACKed"
    );
}
