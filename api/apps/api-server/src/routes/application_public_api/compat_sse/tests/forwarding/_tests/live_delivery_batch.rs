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
