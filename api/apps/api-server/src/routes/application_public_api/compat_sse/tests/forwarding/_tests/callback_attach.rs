use super::*;

// Ordinary native facts precede the atomic tool+upper wait. They are never
// assigned delivery_status; only the real waiting writer creates executable M.
pub(super) async fn append_tool_wait_round(
    state: &ApiState,
    run: &NativeRunResult,
    first: &Value,
    second: &Value,
    tool: &Value,
) -> control_plane::ports::PersistedWaitingState {
    super::delivery_proof::resume_waiting_callback(state, run).await;
    for (index, item) in [(1, second), (0, first)] {
        append_compat_sse_runtime_event(
            state,
            run.id,
            "provider_output_item_done",
            json!({"output_index":index,"item":item}),
        )
        .await;
    }
    super::delivery_proof::commit_wait(state, run, &[tool.clone()]).await
}

#[tokio::test]
async fn callback_attach_replays_exact_output_and_disconnect_leaves_round_open() {
    let mut run = native_run();
    let callback_task_id = Uuid::now_v7();
    run.metadata["response_round_id"] = json!(callback_task_id);
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    super::response_scoped_delivery::seed_scope_boundary(&state, &run, Some(callback_task_id))
        .await;
    let stream = Arc::new(LocalRuntimeEventStream::new());
    stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let mut started = debug_stream_events::flow_started(run.id);
    started.payload["response_round_id"] = json!(callback_task_id);
    stream.append(run.id, started).await.unwrap();
    let exact_output = json!({
        "output_index": 0,
        "item": {"type": "message", "content": [{"type": "output_text", "text": "exact reply"}]}
    });
    stream
        .append(
            run.id,
            RuntimeEventPayload {
                event_type: "provider_output_item_done".into(),
                source: RuntimeEventSource::Provider,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: false,
                trace_visible: true,
                payload: exact_output.clone(),
            },
        )
        .await
        .unwrap();
    let terminal_dependencies = NativeRunTerminalDependencies::new(
        state.store.clone(),
        state.runtime_engine.clone(),
        state.provider_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.model_billing_require_provider_usage,
        state.infrastructure.provider_transport_store(),
        stream.clone(),
    );
    let mut attached = attach_compatible_typed_stream_with_replay(
        terminal_dependencies,
        stream.clone(),
        run.clone(),
        Some(0),
        Vec::new(),
    )
    .await
    .unwrap();
    let first = tokio::time::timeout(Duration::from_secs(2), attached.events.recv())
        .await
        .unwrap()
        .unwrap();
    let second = tokio::time::timeout(Duration::from_secs(2), attached.events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.envelope.event_type, "flow_started");
    assert_eq!(second.envelope.event_type, "provider_output_item_done");
    assert_eq!(second.envelope.payload, exact_output);
    drop(attached);

    let next = stream
        .append(
            run.id,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "still running".into(),
                0,
                None,
                None,
                None,
            ),
        )
        .await
        .expect("subscriber disconnect must not close the shared round");
    assert_eq!(next.sequence, 3);
    let survivor = stream.subscribe(run.id, Some(2)).await.unwrap();
    assert_eq!(survivor.replay.len(), 1);
    assert_eq!(survivor.replay[0].payload["text"], "still running");
}

#[tokio::test]
async fn terminal_callback_attach_replays_ordered_durable_round_without_executor() {
    assert_terminal_callback_attach(false, 0).await;
}

#[tokio::test]
async fn missing_local_round_follows_later_durable_terminal_without_executor() {
    assert_terminal_callback_attach(true, 0).await;
}

#[tokio::test]
async fn callback_attach_follows_durable_completion_with_stalled_live_generation() {
    assert_terminal_callback_attach(true, 1).await;
}

#[tokio::test]
async fn callback_attach_partial_live_then_durable_does_not_repeat_items_or_deltas() {
    assert_terminal_callback_attach_scenario(true, 2, false).await;
    assert_terminal_callback_attach_scenario(true, 2, true).await;
}

#[tokio::test]
async fn callback_attach_retains_terminal_observation_until_durable_identity_arrives() {
    assert_terminal_callback_attach(true, 3).await;
}

#[tokio::test]
async fn missing_callback_round_retains_terminal_observation_until_identity_only_page() {
    assert_terminal_callback_attach(true, 5).await;
}

async fn assert_terminal_callback_attach(terminal_after_subscribe: bool, live_mode: u8) {
    assert_terminal_callback_attach_scenario(terminal_after_subscribe, live_mode, false).await;
}

async fn assert_terminal_callback_attach_scenario(
    terminal_after_subscribe: bool,
    live_mode: u8,
    tool_wait: bool,
) {
    use control_plane::application_public_api::{
        api_keys::ApplicationApiKeyActor,
        callback_resume::{
            PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
            ResumePublishedCallbackCommand,
        },
    };
    let delayed_identity = matches!(live_mode, 3 | 5);
    let live_enabled = matches!(live_mode, 1..=3);
    let mut run = native_run();
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let callback_task_id = if tool_wait {
        let old = super::delivery_proof::commit_wait(
            &state,
            &run,
            &[committed_delivery_payload("prior-callback")],
        )
        .await;
        old.callback_task.unwrap().id
    } else {
        let id = Uuid::now_v7();
        super::response_scoped_delivery::seed_scope_boundary(&state, &run, Some(id)).await;
        id
    };
    run.metadata["response_round_id"] = json!(callback_task_id);
    run.metadata["active_callback_attach"] = json!(true);
    let generation = Uuid::now_v7();
    if delayed_identity {
        append_compat_sse_runtime_event(
            &state,
            run.id,
            "runtime_stream_opened",
            json!({"stream_generation_id": generation}),
        )
        .await;
    }
    let first_item =
        json!({"type": "message", "content": [{"type": "output_text", "text": "first"}]});
    let second_item =
        json!({"type": "message", "content": [{"type": "output_text", "text": "second"}]});
    if !terminal_after_subscribe {
        complete_attached_callback_round(&state, run.id, &first_item, &second_item).await;
    }

    let tool_payload = json!({"output_index": 2, "committed_delivery": true,
        "item": {"type": "function_call", "call_id": "attach-once", "name": "lookup"}});
    let tool = if tool_wait {
        let waiting =
            append_tool_wait_round(&state, &run, &first_item, &second_item, &tool_payload).await;
        Some(waiting.tool_delivery_events[0].clone())
    } else {
        None
    };
    let stream = Arc::new(LocalRuntimeEventStream::new());
    if live_enabled {
        stream
            .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        let mut started = debug_stream_events::flow_started(run.id);
        started.payload["response_round_id"] = json!(callback_task_id);
        stream.append(run.id, started).await.unwrap();
        if live_mode == 2 {
            stream
                .append(
                    run.id,
                    RuntimeEventPayload {
                        event_type: "provider_output_item_done".into(),
                        source: RuntimeEventSource::Provider,
                        durability: RuntimeEventDurability::DurableRequired,
                        persist_required: false,
                        trace_visible: true,
                        payload: json!({"output_index": 0, "item": first_item}),
                    },
                )
                .await
                .unwrap();
            stream
                .append(
                    run.id,
                    debug_stream_events::answer_text_delta(
                        "assistant",
                        "partial live".into(),
                        0,
                        None,
                        None,
                        None,
                    ),
                )
                .await
                .unwrap();
        }
    }
    if tool_wait {
        stream
            .append(
                run.id,
                RuntimeEventPayload {
                    event_type: "provider_output_item_done".into(),
                    source: RuntimeEventSource::Provider,
                    durability: RuntimeEventDurability::DurableRequired,
                    persist_required: false,
                    trace_visible: true,
                    payload: tool_payload.clone(),
                },
            )
            .await
            .unwrap();
    }
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
        notification_sender: state.notification_service.clone(),
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
    let mut attached = start_compatible_typed_resume_stream_for_actor(
        dependencies.clone(),
        run,
        command,
        actor,
        None,
    )
    .await
    .unwrap();
    let mut events = Vec::new();
    if live_enabled {
        for _ in 0..if live_mode == 2 {
            if tool_wait {
                4
            } else {
                3
            }
        } else {
            1
        } {
            let input = tokio::time::timeout(Duration::from_secs(2), attached.events.recv())
                .await
                .expect("live delivery must remain immediate")
                .unwrap();
            let (_, event, receipt) = input.into_parts();
            if let Some(receipt) = receipt {
                receipt.projected();
            }
            events.push(event);
        }
    }
    if terminal_after_subscribe && !tool_wait {
        assert!(
            tokio::time::timeout(Duration::from_millis(50), attached.events.recv())
                .await
                .is_err(),
            "a missing local marker must wait for durable completion"
        );
        // Diagnostic persistence can expose the terminal fact before its
        // flow_started identity, while the business run is still open. Keep
        // that legal write window until the delayed identity has committed.
        append_attached_callback_round_events(
            &state,
            attached.initial_run.id,
            &first_item,
            &second_item,
        )
        .await;
        if delayed_identity {
            assert!(matches!(
                durable_compatible_round_replay(
                    &dependencies,
                    &attached.initial_run,
                    callback_task_id,
                )
                .await
                .unwrap(),
                DurableCompatibleRoundReplay::PendingIdentity
            ));
            // Let the bridge consume the terminal page while its durable
            // identity remains Pending. The next write contains no terminal.
            assert!(
                tokio::time::timeout(Duration::from_millis(1100), attached.events.recv())
                    .await
                    .is_err(),
                "unidentified terminal must not close attachment"
            );
            append_compat_sse_runtime_event(
                &state,
                attached.initial_run.id,
                "flow_started",
                json!({"response_round_id": callback_task_id, "stream_generation_id": generation}),
            )
            .await;
        }
        // Seal only after all diagnostic writes: the repository correctly
        // rejects late flow_started writes once this status is terminal.
        seal_attached_callback_round(&state, attached.initial_run.id).await;
    }
    while let Some(input) = tokio::time::timeout(Duration::from_secs(2), attached.events.recv())
        .await
        .expect("terminal replay must finish")
    {
        events.push(input.envelope);
    }
    let types = events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    if live_mode == 2 {
        let expected = if tool_wait {
            vec![
                "flow_started",
                "provider_output_item_done",
                "text_delta",
                "provider_output_item_done",
                "provider_output_item_done",
                "waiting_callback",
            ]
        } else {
            vec![
                "flow_started",
                "provider_output_item_done",
                "text_delta",
                "provider_output_item_done",
                "flow_finished",
            ]
        };
        assert_eq!(types, expected);
        assert_eq!(events[2].text.as_deref(), Some("partial live"));
    } else {
        assert_eq!(
            types,
            vec![
                "flow_started",
                "provider_output_item_done",
                "provider_output_item_done",
                "flow_finished"
            ]
        );
    }
    let items = events
        .iter()
        .filter(|event| event.event_type == "provider_output_item_done")
        .map(|event| event.payload["item"].clone())
        .collect::<Vec<_>>();
    if tool_wait {
        assert_eq!(
            items,
            vec![first_item, tool_payload["item"].clone(), second_item]
        );
        let tool = tool.unwrap();
        super::cold_replay::await_tool_ack(&state, tool.id).await;
        assert_eq!(delivery_statuses(&state, &[tool.id]).await, vec!["acked"]);
    } else {
        assert_eq!(items, vec![first_item, second_item]);
    }
    let attempt_count: i64 = sqlx::query_scalar(
        "select count(*) from flow_run_callback_resume_attempts where flow_run_id = $1",
    )
    .bind(attached.initial_run.id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(attempt_count, 0, "attach must not reserve another attempt");
    if !live_enabled {
        assert!(
            stream
                .replay(attached.initial_run.id, None, 1)
                .await
                .is_err(),
            "durable terminal replay must not create an empty local stream"
        );
    } else {
        assert!(
            stream
                .subscribe(attached.initial_run.id, None)
                .await
                .unwrap()
                .closure
                .borrow()
                .is_none(),
            "read-only attachment must not close the original producer generation"
        );
    }
}
