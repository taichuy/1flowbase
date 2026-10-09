use super::*;

#[tokio::test]
async fn committed_generation_gc_keeps_cold_attach_opaque_order_and_acked_tool_semantics() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let stream =
        Arc::new(LocalRuntimeEventStream::new().with_recoverable_retention(Duration::ZERO));
    stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let writer = stream.terminal_writer(run.id).await.unwrap();
    let probe = stream.run_probe_for_tests(run.id).unwrap();
    let persister = control_plane::orchestration_runtime::start_runtime_debug_event_persister(
        state.store.clone(),
        stream.clone(),
        run.id,
    )
    .await;
    stream
        .append(run.id, debug_stream_events::flow_started(run.id))
        .await
        .unwrap();
    for index in 0..65 {
        stream
            .append(
                run.id,
                RuntimeEventPayload {
                    event_type: "node_started".into(),
                    source: RuntimeEventSource::Runtime,
                    durability: RuntimeEventDurability::DurableRequired,
                    persist_required: true,
                    trace_visible: true,
                    payload: json!({"index": index, "padding": "x".repeat(2048)}),
                },
            )
            .await
            .unwrap();
    }
    let reasoning = json!({"type": "reasoning", "id": "rs_cold_original", "encrypted_content": "opaque-provider-bytes", "extension": {"numeric_lexeme": "00"}});
    let message = json!({"type": "message", "id": "msg_cold_original", "content": [{"type": "output_text", "text": "recovered"}]});
    let tool =
        seed_pending_committed_delivery(&state, &run, committed_delivery_payload("call-cold-once"))
            .await;
    complete_attached_callback_round(&state, run.id, &reasoning, &message).await;
    run.status = NativeRunStatus::Succeeded;
    let mut terminal = debug_stream_events::flow_finished(run.id, json!({}));
    terminal.persist_required = false;
    terminal.durability = RuntimeEventDurability::Ephemeral;
    stream
        .append_terminal_if_missing_and_close(run.id, terminal)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), persister)
        .await
        .unwrap()
        .unwrap();
    assert!(writer.durable_replay_boundary().is_some());
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if stream.replay(run.id, None, 1).await.is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("proven closed hot replay expires");
    assert_eq!(probe().1, 0);
    assert_eq!(probe().2, 0);
    for expected_tool_count in [1, 0] {
        let dependencies = NativeRunTerminalDependencies::new(
            state.store.clone(),
            state.runtime_engine.clone(),
            state.provider_runtime.clone(),
            state.provider_secret_master_key.clone(),
            state.model_billing_require_provider_usage,
            state.infrastructure.provider_transport_store(),
            stream.clone(),
        );
        let mut attached = attach_compatible_typed_stream_with_replay(
            dependencies,
            stream.clone(),
            run.clone(),
            Some(42),
            Vec::new(),
        )
        .await
        .unwrap();
        let mut items = Vec::new();
        let mut tool_count = 0;
        let mut terminal_count = 0;
        while let Some(input) = tokio::time::timeout(Duration::from_secs(3), attached.events.recv())
            .await
            .unwrap()
        {
            let (_snapshot, event, receipt) = input.into_parts();
            assert!(
                event.sequence > 42,
                "cold projection must not confuse local and DB cursors"
            );
            if let Some(receipt) = receipt {
                tool_count += 1;
                receipt.projected();
            }
            if event.event_type == "provider_output_item_done"
                && !event.payload["committed_delivery"]
                    .as_bool()
                    .unwrap_or(false)
            {
                items.push(event.payload["item"].clone());
            }
            if event.event_type == "flow_finished" {
                terminal_count += 1;
            }
        }
        assert_eq!(items, vec![reasoning.clone(), message.clone()]);
        assert_eq!(tool_count, expected_tool_count);
        assert_eq!(terminal_count, 1);
    }
    assert_eq!(delivery_statuses(&state, &[tool.id]).await, vec!["acked"]);
    assert!(
        state
            .store
            .get_runtime_event_replay_window(run.id)
            .await
            .unwrap()
            .is_some(),
        "hot GC must not delete durable history"
    );
}

#[tokio::test]
async fn cold_native_sse_recovers_completed_answer_without_opening_a_producer() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    run.status = NativeRunStatus::Succeeded;
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: domain::FlowRunStatus::Succeeded,
            output_payload: json!({"answer": "cold durable answer"}),
            error_payload: None,
            finished_at: Some(time::OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
    let stream = Arc::new(LocalRuntimeEventStream::new());
    let dependencies = NativeRunSseDependencies::new(
        stream.clone(),
        NativeRunTerminalDependencies::new(
            state.store.clone(),
            state.runtime_engine.clone(),
            state.provider_runtime.clone(),
            state.provider_secret_master_key.clone(),
            state.model_billing_require_provider_usage,
            state.infrastructure.provider_transport_store(),
            stream.clone(),
        ),
    );
    let (sender, receiver) = mpsc::channel(8);
    let task = tokio::spawn(send_native_runtime_event_stream_with_dependencies(
        dependencies,
        run.clone(),
        IncludeWorkflowEvents::Public,
        None,
        None,
        sender,
    ));
    let response =
        axum::response::sse::Sse::new(tokio_stream::wrappers::ReceiverStream::new(receiver))
            .into_response();
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    task.await.unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("cold durable answer"), "{body}");
    assert!(body.contains("run.completed"), "{body}");
    assert!(
        stream.replay(run.id, None, 1).await.is_err(),
        "cold reads never execute or reopen the run"
    );
}
