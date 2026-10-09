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

#[tokio::test]
async fn cold_callback_replay_ignores_late_foreign_terminal_and_latest_round_anchor() {
    assert_cold_callback_replay_with_late_foreign_terminal(false).await;
}

#[tokio::test]
async fn cold_callback_follower_waits_for_identity_after_anchor_and_foreign_terminal() {
    assert_cold_callback_replay_with_late_foreign_terminal(true).await;
}

async fn assert_cold_callback_replay_with_late_foreign_terminal(delayed_identity: bool) {
    use control_plane::application_public_api::{
        api_keys::ApplicationApiKeyActor,
        callback_resume::{
            PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
            ResumePublishedCallbackCommand,
        },
    };
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    let callback_task_id = Uuid::now_v7();
    let requested_generation = Uuid::now_v7();
    let old_generation = Uuid::now_v7();
    let later_round = Uuid::now_v7();
    run.metadata["response_round_id"] = json!(callback_task_id);
    run.metadata["active_callback_attach"] = json!(true);
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    for (event_type, payload) in [
        (
            "runtime_stream_opened",
            json!({"stream_generation_id": old_generation}),
        ),
        (
            "waiting_callback",
            json!({"callback_task_id": callback_task_id}),
        ),
        (
            "runtime_stream_opened",
            json!({"stream_generation_id": requested_generation}),
        ),
    ] {
        append_compat_sse_runtime_event(&state, run.id, event_type, payload).await;
    }
    if !delayed_identity {
        append_compat_sse_runtime_event(
            &state,
            run.id,
            "flow_started",
            json!({"type": "flow_started", "response_round_id": callback_task_id, "stream_generation_id": requested_generation}),
        ).await;
    }
    // Cross a real durable page boundary before the late previous producer.
    if !delayed_identity {
        for index in 0..65 {
            append_compat_sse_runtime_event(
                &state,
                run.id,
                "node_started",
                json!({"index": index}),
            )
            .await;
        }
    }
    for (event_type, payload) in [
        (
            "provider_output_item_done",
            json!({"output_index": 0, "stream_generation_id": old_generation, "item": {"type": "message", "id": "late-old-output"}}),
        ),
        // Generation exclusion is required even without a response_round_id.
        (
            "flow_finished",
            json!({"type": "flow_finished", "stream_generation_id": old_generation, "status": "succeeded", "marker": "late-old-terminal"}),
        ),
        (
            "provider_output_item_done",
            json!({"output_index": 0, "stream_generation_id": requested_generation, "response_round_id": later_round, "item": {"type": "message", "id": "foreign-round-output"}}),
        ),
        // Round exclusion also precedes terminal detection in a shared generation.
        (
            "flow_finished",
            json!({"type": "flow_finished", "stream_generation_id": requested_generation, "response_round_id": later_round, "status": "succeeded", "marker": "foreign-round-terminal"}),
        ),
    ] {
        append_compat_sse_runtime_event(&state, run.id, event_type, payload).await;
    }
    let reasoning = json!({"type": "reasoning", "id": "rs_callback_original", "encrypted_content": "opaque-callback-bytes", "extension": {"numeric_lexeme": "00"}});
    let message = json!({"type": "message", "id": "msg_callback_original", "content": [{"type": "output_text", "text": "requested callback"}]});
    let stream = Arc::new(LocalRuntimeEventStream::new());
    let native = crate::routes::application_public_api::native::ApplicationNativeRunDependencies {
        store: state.store.clone(),
        cache_store: state.infrastructure.cache_store(),
        published_plan_cache: state.infrastructure.published_plan_cache(),
        published_publication_cache: state.infrastructure.published_publication_cache(),
        runtime_engine: state.runtime_engine.clone(),
        provider_runtime: state.provider_runtime.clone(),
        network_egress: Arc::new(state.network_egress_http_clients()),
        provider_secret_master_key: state.provider_secret_master_key.clone(),
        model_billing_require_provider_usage: state.model_billing_require_provider_usage,
        api_node_id: state.api_node_id.clone(),
        provider_install_root: state.provider_install_root.clone(),
        file_storage_registry: state.file_storage_registry.clone(),
        task_queue: state.infrastructure.task_queue(),
        provider_transport_store: state.infrastructure.provider_transport_store(),
        runtime_event_stream: stream.clone(),
        runtime_activity: state.runtime_activity.clone(),
        runtime_invoker_factory: Arc::new(RejectAttachExecutor),
    };
    let dependencies = crate::routes::application_public_api::compatibility_interface::CompatibilityExecutionDependencies {
        provider_transport_store: state.infrastructure.provider_transport_store(),
        native,
    };
    let actor = ApplicationApiKeyActor {
        api_key_id: run.api_key_id,
        application_id: run.application_id,
        creator_user_id: Uuid::nil(),
        tenant_id: Uuid::nil(),
        workspace_id: Uuid::nil(),
        actor: domain::ActorContext::root(Uuid::nil(), Uuid::nil(), "root"),
    };
    let command = ResumePublishedCallbackCommand {
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask { callback_task_id },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: json!({"tool_results": []}),
        response_mode: Some("streaming".into()),
        native_transport: None,
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
    };

    let mut pending_attachment = if delayed_identity {
        assert!(matches!(
            durable_compatible_round_replay(&dependencies, &run, callback_task_id)
                .await
                .unwrap(),
            DurableCompatibleRoundReplay::PendingIdentity,
        ));
        assert!(
            compatible_round_replay(&dependencies, &run, callback_task_id)
                .await
                .is_err(),
            "cold recovery must not accept an anchored round without identity",
        );
        let mut attached = start_compatible_typed_resume_stream_for_actor(
            dependencies.clone(),
            run.clone(),
            command.clone(),
            actor.clone(),
        )
        .await
        .unwrap();
        // Keep writes frozen across a complete follower poll: the old terminal
        // must neither end this subscription nor cause an executor to be built.
        assert!(
            tokio::time::timeout(Duration::from_millis(1100), attached.events.recv())
                .await
                .is_err(),
            "an anchor awaiting its marker must keep the subscriber alive",
        );
        append_compat_sse_runtime_event(
            &state,
            run.id,
            "flow_started",
            json!({"type": "flow_started", "response_round_id": callback_task_id, "stream_generation_id": requested_generation}),
        ).await;
        for index in 0..65 {
            append_compat_sse_runtime_event(
                &state,
                run.id,
                "node_started",
                json!({"index": index}),
            )
            .await;
        }
        Some(attached)
    } else {
        None
    };
    let tool = seed_pending_committed_delivery(
        &state,
        &run,
        committed_delivery_payload("call-callback-once"),
    )
    .await;
    // These business-owner output and terminal rows are legitimately untagged.
    complete_attached_callback_round(&state, run.id, &reasoning, &message).await;
    for (event_type, payload) in [
        (
            "runtime_stream_opened",
            json!({"stream_generation_id": Uuid::now_v7()}),
        ),
        (
            "flow_started",
            json!({"type": "flow_started", "response_round_id": later_round}),
        ),
        (
            "provider_output_item_done",
            json!({"output_index": 0, "item": {"type": "message", "id": "later-output"}}),
        ),
        (
            "flow_finished",
            json!({"type": "flow_finished", "status": "succeeded", "marker": "later-terminal"}),
        ),
    ] {
        append_compat_sse_runtime_event(&state, run.id, event_type, payload).await;
    }

    for expected_tool_count in [1, 0] {
        let mut attached = if let Some(attached) = pending_attachment.take() {
            attached
        } else {
            start_compatible_typed_resume_stream_for_actor(
                dependencies.clone(),
                run.clone(),
                command.clone(),
                actor.clone(),
            )
            .await
            .unwrap()
        };
        let mut items = Vec::new();
        let mut terminal_count = 0;
        let mut tool_count = 0;
        while let Some(input) = tokio::time::timeout(Duration::from_secs(3), attached.events.recv())
            .await
            .unwrap()
        {
            let (_snapshot, event, receipt) = input.into_parts();
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
                assert!(
                    event.payload["marker"].is_null(),
                    "foreign terminal cut requested prefix"
                );
                terminal_count += 1;
            }
        }
        assert_eq!(items, vec![reasoning.clone(), message.clone()]);
        assert_eq!(terminal_count, 1);
        assert_eq!(tool_count, expected_tool_count);
    }
    assert_eq!(delivery_statuses(&state, &[tool.id]).await, vec!["acked"]);
    let attempt_count: i64 = sqlx::query_scalar(
        "select count(*) from flow_run_callback_resume_attempts where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        attempt_count, 0,
        "cold callback replay must not reserve another attempt"
    );
    assert!(
        stream.replay(run.id, None, 1).await.is_err(),
        "cold callback replay must not open a producer"
    );
}

#[tokio::test]
async fn cold_anchored_admission_rejects_previous_waiting_and_uses_current_terminal() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let cold_stream = Arc::new(LocalRuntimeEventStream::new());
    let dependencies = NativeRunTerminalDependencies::new(
        state.store.clone(),
        state.runtime_engine.clone(),
        state.provider_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.model_billing_require_provider_usage,
        state.infrastructure.provider_transport_store(),
        cold_stream.clone(),
    );
    // Explicit legacy positive: without an anchor, business Waiting still
    // supplies the recoverable terminal. The seeded run is waiting_callback.
    let legacy = cold_runtime_event_subscription(&dependencies, &run, None)
        .await
        .unwrap();
    assert_eq!(legacy.replay.len(), 1);
    assert!(matches!(
        legacy.replay[0].event_type.as_str(),
        "waiting_human" | "waiting_callback"
    ));
    assert_eq!(
        legacy.closure.borrow().unwrap().reason,
        RuntimeEventCloseReason::from_terminal_event_type(&legacy.replay[0].event_type).unwrap(),
    );

    let old_generation = Uuid::now_v7();
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "runtime_stream_opened",
        json!({"stream_generation_id": old_generation}),
    )
    .await;
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "waiting_callback",
        json!({"stream_generation_id": old_generation, "status": "waiting_callback"}),
    )
    .await;
    // Model the production admission boundary: G2 is open, and its anchor and
    // start are durable, while its producer has not updated G1's Waiting state.
    let live_stream = Arc::new(LocalRuntimeEventStream::new());
    live_stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let live_writer = live_stream.terminal_writer(run.id).await.unwrap();
    let generation = live_writer.generation_id().unwrap();
    live_stream
        .append(run.id, debug_stream_events::flow_started(run.id))
        .await
        .unwrap();
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "runtime_stream_opened",
        json!({"stream_generation_id": generation}),
    )
    .await;
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "flow_started",
        json!({"type": "flow_started", "stream_generation_id": generation, "stream_sequence": 1}),
    )
    .await;
    // A late previous terminal after the new anchor cannot admit a cold close.
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "waiting_callback",
        json!({"stream_generation_id": old_generation, "status": "waiting_callback"}),
    )
    .await;
    assert!(cold_runtime_event_subscription(&dependencies, &run, None)
        .await
        .is_err());
    assert!(
        attach_compatible_typed_stream_with_replay(
            dependencies.clone(),
            cold_stream.clone(),
            run.clone(),
            None,
            Vec::new(),
        )
        .await
        .is_err(),
        "generic compatible cold consumer must not fabricate Waiting"
    );
    let (sender, mut receiver) = mpsc::channel(8);
    send_native_runtime_event_stream_with_dependencies(
        NativeRunSseDependencies::new(cold_stream.clone(), dependencies.clone()),
        run.clone(),
        IncludeWorkflowEvents::Public,
        None,
        None,
        sender,
    )
    .await;
    assert!(
        receiver.recv().await.is_none(),
        "native cold consumer must not publish stale Waiting"
    );
    assert!(
        live_writer.closure().is_none(),
        "cold admission must preserve active G2"
    );
    assert_eq!(live_stream.replay(run.id, None, 8).await.unwrap().len(), 1);

    let live_dependencies = NativeRunTerminalDependencies::new(
        state.store.clone(),
        state.runtime_engine.clone(),
        state.provider_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.model_billing_require_provider_usage,
        state.infrastructure.provider_transport_store(),
        live_stream.clone(),
    );
    let mut reattached = cold_runtime_event_subscription(&live_dependencies, &run, None)
        .await
        .unwrap();
    assert_eq!(reattached.terminal_writer.generation_id(), Some(generation));
    assert!(reattached.closure.borrow().is_none());
    assert!(reattached
        .replay
        .iter()
        .all(|event| !event_forwarding::is_public_terminal_runtime_event(&event.event_type)));
    // The generic consumer's first (remote/missing) stream fails subscription,
    // then cold admission safely reattaches the already-open matching G2.
    let mut admitted = attach_compatible_typed_stream_with_replay(
        live_dependencies,
        cold_stream.clone(),
        run.clone(),
        None,
        Vec::new(),
    )
    .await
    .unwrap();
    let started = tokio::time::timeout(Duration::from_secs(3), admitted.events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(started.envelope.event_type, "flow_started");

    for index in 0..65 {
        append_compat_sse_runtime_event(&state, run.id, "node_started", json!({"index": index, "stream_generation_id": generation, "stream_sequence": index + 2})).await;
    }
    let reasoning = json!({"type": "reasoning", "id": "rs_admission_original", "encrypted_content": "opaque-admission-bytes"});
    let message = json!({"type": "message", "id": "msg_admission_original", "content": [{"type": "output_text", "text": "current generation"}]});
    for (index, item) in [(1, &message), (0, &reasoning)] {
        append_compat_sse_runtime_event(
            &state,
            run.id,
            "provider_output_item_done",
            json!({"output_index": index, "item": item}),
        )
        .await;
    }
    let terminal_payload = json!({"type": "flow_finished", "stream_generation_id": generation, "stream_sequence": 70, "status": "succeeded", "marker": "current-terminal"});
    append_compat_sse_runtime_event(&state, run.id, "flow_finished", terminal_payload.clone())
        .await;
    // The business snapshot deliberately remains Waiting. Closure must come
    // from the current durable terminal, including when the cursor is past it.
    for cursor in [None, Some(99)] {
        let subscription = cold_runtime_event_subscription(&dependencies, &run, cursor)
            .await
            .unwrap();
        let terminals: Vec<_> = subscription
            .replay
            .iter()
            .filter(|event| event_forwarding::is_public_terminal_runtime_event(&event.event_type))
            .collect();
        assert_eq!(terminals.len(), 1);
        assert_eq!(terminals[0].event_type, "flow_finished");
        assert_eq!(terminals[0].payload, terminal_payload);
        let closure = subscription.closure.borrow().unwrap();
        assert_eq!(closure.reason, RuntimeEventCloseReason::Finished);
        assert_eq!(
            closure.final_sequence,
            subscription.replay.last().unwrap().sequence
        );
        assert!(subscription
            .replay
            .iter()
            .all(|event| event.sequence > cursor.unwrap_or(0)));
        let items: Vec<_> = subscription
            .replay
            .iter()
            .filter(|event| event.event_type == "provider_output_item_done")
            .map(|event| event.payload["item"].clone())
            .collect();
        assert_eq!(items, vec![reasoning.clone(), message.clone()]);
        assert!(subscription
            .terminal_writer
            .append_terminal_if_missing_and_close(debug_stream_events::flow_finished(
                run.id,
                json!({})
            ))
            .await
            .is_err());
    }
    // Once business state catches up, the production compatible consumer emits
    // exactly the current terminal; it cannot resurrect the previous Waiting.
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: domain::FlowRunStatus::Succeeded,
            output_payload: json!({}),
            error_payload: None,
            finished_at: Some(time::OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
    let mut attached = attach_compatible_typed_stream_with_replay(
        dependencies.clone(),
        cold_stream.clone(),
        run.clone(),
        Some(99),
        Vec::new(),
    )
    .await
    .unwrap();
    let mut terminal_types = Vec::new();
    let mut items = Vec::new();
    while let Some(input) = tokio::time::timeout(Duration::from_secs(3), attached.events.recv())
        .await
        .unwrap()
    {
        let (_snapshot, event, receipt) = input.into_parts();
        assert!(receipt.is_none());
        if event_forwarding::is_public_terminal_runtime_event(&event.event_type) {
            terminal_types.push(event.event_type.clone());
        }
        if event.event_type == "provider_output_item_done" {
            items.push(event.payload["item"].clone());
        }
    }
    assert_eq!(terminal_types, vec!["flow_finished"]);
    assert_eq!(items, vec![reasoning, message]);
    assert!(
        cold_stream.replay(run.id, None, 1).await.is_err(),
        "cold reads never open a producer"
    );
    let attempts: i64 = sqlx::query_scalar(
        "select count(*) from flow_run_callback_resume_attempts where flow_run_id = $1",
    )
    .bind(run.id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(attempts, 0);
    assert!(live_writer.closure().is_none());
    live_writer
        .append_terminal_if_missing_and_close(debug_stream_events::flow_finished(run.id, json!({})))
        .await
        .unwrap();
    let terminal = tokio::time::timeout(Duration::from_secs(3), reattached.live_events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(terminal.event_type, "flow_finished");
    assert_eq!(
        reattached.closure.borrow().unwrap().reason,
        RuntimeEventCloseReason::Finished
    );
    let mut admitted_terminals = Vec::new();
    while let Some(input) = tokio::time::timeout(Duration::from_secs(3), admitted.events.recv())
        .await
        .unwrap()
    {
        if event_forwarding::is_public_terminal_runtime_event(&input.envelope.event_type) {
            admitted_terminals.push(input.envelope.event_type);
        }
    }
    assert_eq!(admitted_terminals, vec!["flow_finished"]);
}
