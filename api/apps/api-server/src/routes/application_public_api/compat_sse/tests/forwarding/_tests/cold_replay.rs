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
        (
            "flow_started",
            json!({"type": "flow_started", "response_round_id": callback_task_id, "stream_generation_id": requested_generation}),
        ),
    ] {
        append_compat_sse_runtime_event(&state, run.id, event_type, payload).await;
    }
    // Cross a real durable page boundary before the late previous producer.
    for index in 0..65 {
        append_compat_sse_runtime_event(&state, run.id, "node_started", json!({"index": index}))
            .await;
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

    for expected_tool_count in [1, 0] {
        let mut attached = start_compatible_typed_resume_stream_for_actor(
            dependencies.clone(),
            run.clone(),
            command.clone(),
            actor.clone(),
        )
        .await
        .unwrap();
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
