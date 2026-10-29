use super::*;
use control_plane::ports::{
    AckRuntimeEventDeliveryInput, AppendRuntimeEventInput, ClaimRuntimeEventDeliveriesInput,
    MarkRuntimeEventDeliveryUncertainInput, PersistWaitingCallbackTaskInput, PersistWaitingKind,
    PersistWaitingStateInput, PersistedWaitingState, RuntimeEventSubscription,
};

// Use the actual atomic waiting writer: tool facts, original wait, context and
// recovery anchor commit together, unlike asynchronous diagnostic append copies.
async fn commit_wait(
    state: &ApiState,
    run: &NativeRunResult,
    payloads: &[Value],
) -> PersistedWaitingState {
    let current = state
        .store
        .get_flow_run(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    let scope_id: Uuid = sqlx::query_scalar("select scope_id from flow_runs where id=$1")
        .bind(run.id)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let node = state
        .store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run.id,
            node_id: format!("llm-{}", Uuid::now_v7()),
            node_type: "llm".into(),
            node_alias: "Delivery proof".into(),
            status: domain::NodeRunStatus::Running,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let callback = Uuid::now_v7();
    let event = |event_type: &str, payload: Value| AppendRuntimeEventInput {
        flow_run_id: run.id,
        node_run_id: Some(node.id),
        span_id: None,
        parent_span_id: None,
        event_type: event_type.into(),
        layer: domain::RuntimeEventLayer::AgentTransition,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload,
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
    };
    state
        .store
        .persist_waiting_state(&PersistWaitingStateInput {
            checkpoint_id: Uuid::now_v7(),
            scope_id,
            application_id: run.application_id,
            flow_run_id: run.id,
            node_run_id: node.id,
            expected_status: current.status,
            output_payload: json!({}),
            checkpoint_status: "waiting_callback".into(),
            checkpoint_reason: "delivery-proof".into(),
            locator_payload: json!({"node_id":node.node_id}),
            variable_snapshot: json!({}),
            checkpoint_external_ref_payload: None,
            context_content: json!({"format":"runtime_snapshot_v1","variable_pool":{}}),
            parent_context_version_id: None,
            context_transition_kind: domain::ContextTransitionKind::Append,
            recovery_idempotency_key: format!("waiting_callback:{callback}"),
            resume_claim_id: None,
            resume_claim_token: None,
            waiting_event: event("waiting_callback", json!({"callback_task_id":callback})),
            tool_delivery_events: payloads
                .iter()
                .map(|payload| event("provider_output_item_done", payload.clone()))
                .collect(),
            kind: PersistWaitingKind::Callback(PersistWaitingCallbackTaskInput {
                id: callback,
                callback_kind: "llm_tool_calls".into(),
                external_ref_payload: None,
                request_payload: json!({"tool_calls":payloads.iter().map(|payload| json!({
                "id":payload["item"]["call_id"],"name":"exec","arguments":{}
            })).collect::<Vec<_>>()}),
            }),
        })
        .await
        .unwrap()
        .unwrap()
}

fn exact_payload(index: usize) -> Value {
    let mut item = if index % 2 == 0 {
        json!({"id":format!("ctc-{index}"),"type":"custom_tool_call", "call_id":format!("call-{index}"),
            "name":"exec","input":"  exact\nbytes\r\n\"quoted\"  "})
    } else {
        json!({"id":format!("fc-{index}"),"type":"function_call","call_id":format!("call-{index}"),
            "name":"lookup","namespace":"original.namespace", "arguments":"{ \"n\" : 1.00, \"s\": \"\\u0061\" }"})
    };
    item["opaque_extension"] = json!({"retain":[index,"00",{"nested":true}]});
    json!({"output_index":index,"committed_delivery":true,"item":item})
}

fn terminal_dependencies(state: &ApiState) -> NativeRunTerminalDependencies {
    NativeRunTerminalDependencies::new(
        state.store.clone(),
        state.runtime_engine.clone(),
        state.provider_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.model_billing_require_provider_usage,
        state.infrastructure.provider_transport_store(),
        state.runtime_event_stream.clone(),
    )
}

fn empty_subscription() -> RuntimeEventSubscription {
    let (tx, live) = mpsc::unbounded_channel();
    drop(tx);
    let (_closure_sender, closure) = tokio::sync::watch::channel(None);
    RuntimeEventSubscription {
        terminal_writer: Arc::new(UnusedTerminalWriter),
        replay: Vec::new(),
        live_events: control_plane::ports::RuntimeEventReceiver::from_unbounded(live),
        closure,
    }
}

async fn collect_projection(
    mut receiver: mpsc::Receiver<CompatibleProjectionInput>,
) -> (Vec<Value>, usize) {
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::TransparentProviderResponses,
    );
    let mut frames = Vec::new();
    let mut claims = 0;
    while let Some(input) = tokio::time::timeout(Duration::from_secs(10), receiver.recv())
        .await
        .expect("forwarder must terminate")
    {
        let (run, event, receipt) = input.into_parts();
        frames.extend(mapper.runtime_event_to_sse(&run, event));
        if let Some(receipt) = receipt {
            claims += 1;
            receipt.projected();
        }
    }
    let response = axum::response::sse::Sse::new(tokio_stream::iter(frames)).into_response();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|line| serde_json::from_str(line).unwrap())
            .collect(),
        claims,
    )
}

fn assert_exact_response(events: &[Value], expected: &[Value], round: Uuid) {
    let done = events
        .iter()
        .filter(|event| event["type"] == "response.output_item.done")
        .map(|event| event["item"].clone())
        .collect::<Vec<_>>();
    let items = expected
        .iter()
        .map(|payload| payload["item"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        done, items,
        "all exact facts once, including unknown fields/namespace/argument bytes"
    );
    assert!(!events
        .iter()
        .any(|event| event["type"] == "response.failed"));
    let terminal = events
        .iter()
        .find(|event| event["type"] == "response.completed")
        .expect("complete tool response");
    assert_eq!(terminal["response"]["output"], json!(items));
    assert_eq!(terminal["response"]["id"], format!("resp_{round}"));
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "response.completed")
            .count(),
        1
    );
}

#[tokio::test]
async fn closed_proof_reads_385_facts_once_across_four_real_pg_forwarder_batches() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let old = commit_wait(&state, &run, &[committed_delivery_payload("foreign-old")]).await;
    let callback = old.callback_task.as_ref().unwrap().id;
    run.metadata["response_round_id"] = json!(callback);
    let payloads = (0..385).map(exact_payload).collect::<Vec<_>>();
    let own = commit_wait(&state, &run, &payloads).await;
    let future = commit_wait(
        &state,
        &run,
        &[committed_delivery_payload("foreign-future")],
    )
    .await;
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut dependencies = terminal_dependencies(&state);
    dependencies.delivery_read_samples = Some(samples.clone());
    let (tx, live) = mpsc::unbounded_channel();
    for (index, event) in own.tool_delivery_events.iter().enumerate() {
        tx.send(committed_delivery_envelope(
            run.id,
            70000 + index as i64,
            event.payload.clone(),
        ))
        .unwrap();
    }
    let mut terminal = crate::routes::application_public_api::stream_terminal_fallback::durable_record_to_runtime_event_envelope(own.waiting_event.clone());
    terminal.sequence = 90000;
    tx.send(terminal).unwrap();
    drop(tx);
    let (sender, receiver) = mpsc::channel(8);
    let mut subscription = empty_subscription();
    subscription.live_events = control_plane::ports::RuntimeEventReceiver::from_unbounded(live);
    let task = tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            terminal_dependencies: dependencies,
            initial_run: run.clone(),
            delivery_scope: RuntimeEventDeliveryScope::Resume {
                callback_task_id: callback,
            },
            from_sequence: None,
            durable_round_replay: Vec::new(),
            ignored_waiting_callback_task_id: Some(callback),
            subscription,
            sender,
        },
    ));
    let (events, claims) = collect_projection(receiver).await;
    task.await.unwrap();
    assert_exact_response(&events, &payloads, callback);
    assert_eq!(claims, 385);
    let reads = samples.lock().unwrap().clone();
    eprintln!("delivery proof (closed, proof facts, claims, reserialized proof bytes): {reads:?}");
    assert_eq!(
        reads.iter().map(|r| (r.0, r.1, r.2)).collect::<Vec<_>>(),
        vec![
            (true, 385, 128),
            (true, 0, 128),
            (true, 0, 128),
            (true, 0, 1)
        ]
    );
    assert!(reads[0].3 > 0);
    assert!(reads[1..].iter().all(|r| r.3 == 0));
    let ids = vec![
        old.tool_delivery_events[0].id,
        future.tool_delivery_events[0].id,
    ];
    let generations: Vec<i64> = sqlx::query_scalar(
        "select delivery_generation from runtime_events where id=any($1) order by sequence",
    )
    .bind(ids)
    .fetch_all(state.store.pool())
    .await
    .unwrap();
    assert_eq!(generations, vec![0, 0]);
}

#[tokio::test]
async fn initial_proof_read_failure_with_acked_uncertain_exact_replay_is_explicit() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let old = commit_wait(&state, &run, &[committed_delivery_payload("old")]).await;
    let callback = old.callback_task.as_ref().unwrap().id;
    run.metadata["response_round_id"] = json!(callback);
    let payloads = vec![exact_payload(0), exact_payload(1)];
    let own = commit_wait(&state, &run, &payloads).await;
    settle_exact(&state, &run, callback).await;
    let exact_ids = own
        .tool_delivery_events
        .iter()
        .map(|event| event.id)
        .collect::<Vec<_>>();
    // Isolated fixture schema only. Break the actual initial proof query while
    // leaving legal durable replay payloads present; never fake a successful empty read.
    sqlx::query("alter table flow_run_recovery_history rename to fixture_unavailable_recovery")
        .execute(state.store.pool())
        .await
        .unwrap();
    let replay=own.tool_delivery_events.into_iter().chain(std::iter::once(own.waiting_event))
        .map(crate::routes::application_public_api::stream_terminal_fallback::durable_record_to_runtime_event_envelope).collect();
    let (sender, receiver) = mpsc::channel(8);
    let task = tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            terminal_dependencies: terminal_dependencies(&state),
            initial_run: run.clone(),
            delivery_scope: RuntimeEventDeliveryScope::Resume {
                callback_task_id: callback,
            },
            from_sequence: None,
            durable_round_replay: replay,
            ignored_waiting_callback_task_id: Some(callback),
            subscription: empty_subscription(),
            sender,
        },
    ));
    let (events, claims) = collect_projection(receiver).await;
    task.await.unwrap();
    sqlx::query("alter table fixture_unavailable_recovery rename to flow_run_recovery_history")
        .execute(state.store.pool())
        .await
        .unwrap();
    super::response_scoped_delivery::assert_protocol_failure(
        &events,
        "runtime_event_delivery_proof_unavailable",
    );
    assert_eq!(claims, 0);
    let statuses = delivery_statuses(&state, &exact_ids).await;
    assert_eq!(statuses, vec!["acked", "uncertain"]);
    assert_eq!(
        state
            .store
            .get_flow_run(run.application_id, run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        domain::FlowRunStatus::WaitingCallback,
        "protocol-local failure never commits a business failure"
    );
}

async fn settle_exact(state: &ApiState, run: &NativeRunResult, callback: Uuid) {
    let batch = state
        .store
        .claim_runtime_event_deliveries(&ClaimRuntimeEventDeliveriesInput {
            flow_run_id: run.id,
            scope: RuntimeEventDeliveryScope::Resume {
                callback_task_id: callback,
            },
            include_membership: true,
            limit: 128,
            lease_seconds: 30,
        })
        .await
        .unwrap();
    assert_eq!(batch.claims.len(), 2);
    let first = &batch.claims[0];
    let second = &batch.claims[1];
    state
        .store
        .ack_runtime_event_delivery(&AckRuntimeEventDeliveryInput {
            event_id: first.event.id,
            claim_token: first.claim_token,
            expected_generation: first.generation,
            acknowledged_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    state
        .store
        .mark_runtime_event_delivery_uncertain(&MarkRuntimeEventDeliveryUncertainInput {
            event_id: second.event.id,
            claim_token: second.claim_token,
            expected_generation: second.generation,
            marked_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
}

fn execution_dependencies(
    state: &ApiState,
    stream: Arc<LocalRuntimeEventStream>,
) -> CompatibilityExecutionDependencies {
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
    crate::routes::application_public_api::compatibility_interface::CompatibilityExecutionDependencies {
        provider_transport_store: state.infrastructure.provider_transport_store(),
        native,
    }
}

#[tokio::test]
async fn canonical_callback_cold_and_follower_replay_exact_settled_tools_after_late_diagnostic() {
    assert_canonical_cold_and_follower(true).await;
}

#[tokio::test]
async fn canonical_callback_legacy_without_stream_marker_replays_exact_settled_tools() {
    assert_canonical_cold_and_follower(false).await;
}

async fn assert_canonical_cold_and_follower(with_marker: bool) {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let old = commit_wait(&state, &run, &[committed_delivery_payload("foreign-old")]).await;
    let callback = old.callback_task.as_ref().unwrap().id;
    run.metadata["response_round_id"] = json!(callback);
    let generation = Uuid::now_v7();
    if with_marker {
        append_compat_sse_runtime_event(
            &state,
            run.id,
            "runtime_stream_opened",
            json!({"stream_generation_id":generation}),
        )
        .await;
        append_compat_sse_runtime_event(
            &state,
            run.id,
            "flow_started",
            json!({"stream_generation_id":generation,"response_round_id":callback}),
        )
        .await;
    }
    let mut payloads = vec![exact_payload(0), exact_payload(1)];
    if with_marker {
        for payload in &mut payloads {
            payload["stream_generation_id"] = json!(generation);
            payload["response_round_id"] = json!(callback);
        }
    }
    let own = commit_wait(&state, &run, &payloads).await;
    settle_exact(&state, &run, callback).await;
    let future_round = own.callback_task.as_ref().unwrap().id;
    if with_marker {
        let future_generation = Uuid::now_v7();
        append_compat_sse_runtime_event(
            &state,
            run.id,
            "runtime_stream_opened",
            json!({"stream_generation_id":future_generation}),
        )
        .await;
        append_compat_sse_runtime_event(
            &state,
            run.id,
            "flow_started",
            json!({"stream_generation_id":future_generation,"response_round_id":future_round}),
        )
        .await;
    }
    let future = commit_wait(
        &state,
        &run,
        &[committed_delivery_payload("foreign-future")],
    )
    .await;
    // A diagnostic copy after both canonical stops must not move the lower or upper.
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "waiting_callback",
        json!({"callback_task_id":callback}),
    )
    .await;
    let canonical = state
        .store
        .get_original_callback_waiting_boundary(run.id, callback)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        canonical.original.event_sequence,
        old.waiting_event.sequence
    );
    assert_eq!(canonical.original.recovery_id, old.recovery_history.id);
    assert_eq!(
        canonical.next.unwrap().event_sequence,
        own.waiting_event.sequence
    );
    let stream = Arc::new(LocalRuntimeEventStream::new()); // No open_run: actual cold local lane.
    let dependencies = execution_dependencies(&state, stream.clone());
    assert!(stream.subscribe(run.id, None).await.is_err());
    // Real resolver, including generation/legacy policy; never hand-build a replay Vec.
    let (start, replay) = compatible_round_replay(&dependencies, &run, callback)
        .await
        .unwrap();
    assert!(start.is_none());
    assert_eq!(
        replay
            .iter()
            .filter(|event| event.event_type == "provider_output_item_done")
            .count(),
        2
    );
    let cold = attach_compatible_typed_stream_with_replay(
        terminal_dependencies(&state),
        stream,
        run.clone(),
        None,
        Vec::new(),
    )
    .await
    .unwrap();
    let (events, claims) = collect_projection(cold.events).await;
    assert_exact_response(&events, &payloads, callback);
    assert_eq!(claims, 0, "acked/uncertain exact recovery must not reclaim");
    // Actual production follower probes canonical boundary then invokes the same
    // resolver, bridging durable positions into independent local sequence space.
    let subscription = callback_attachment::follow_callback_subscription(
        dependencies,
        run.clone(),
        callback,
        80000,
        empty_subscription(),
    );
    let follower = attach_compatible_typed_stream_from_subscription(
        terminal_dependencies(&state),
        run.clone(),
        Some(80000),
        Vec::new(),
        RuntimeEventDeliveryScope::Resume {
            callback_task_id: callback,
        },
        subscription,
    );
    let (events, claims) = collect_projection(follower.events).await;
    assert_exact_response(&events, &payloads, callback);
    assert_eq!(claims, 0);
    let ids = vec![
        old.tool_delivery_events[0].id,
        own.tool_delivery_events[0].id,
        own.tool_delivery_events[1].id,
        future.tool_delivery_events[0].id,
    ];
    assert_eq!(
        delivery_statuses(&state, &ids).await,
        vec!["pending", "acked", "uncertain", "pending"]
    );
    let generations: Vec<i64> = sqlx::query_scalar(
        "select delivery_generation from runtime_events where id=any($1) order by sequence",
    )
    .bind(ids)
    .fetch_all(state.store.pool())
    .await
    .unwrap();
    assert_eq!(generations, vec![0, 1, 1, 0]);
}

#[tokio::test]
async fn inference_successor_selected_run_is_start_even_when_prior_callback_authorizes_replay() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut successor = native_run();
    successor.id = Uuid::now_v7();
    successor.metadata =
        json!({"response_round_id":successor.id,"native_inference_recovery_replay":true});
    seed_flow_run_for_compat_sse_test(&state, &successor).await;
    let payloads = vec![exact_payload(0)];
    let committed = commit_wait(&state, &successor, &payloads).await;
    assert_eq!(
        selected_delivery_scope(&successor),
        RuntimeEventDeliveryScope::Start
    );
    let prior_callback_from_failed_flow = Uuid::now_v7();
    let actor = control_plane::application_public_api::api_keys::ApplicationApiKeyActor {
        api_key_id: successor.api_key_id,
        application_id: successor.application_id,
        creator_user_id: Uuid::nil(),
        tenant_id: Uuid::nil(),
        workspace_id: Uuid::nil(),
        actor: domain::ActorContext::root(Uuid::nil(), Uuid::nil(), "root"),
    };
    let command = ResumePublishedCallbackCommand {
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: prior_callback_from_failed_flow,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: json!({"tool_results":[]}),
        response_mode: Some("streaming".into()),
        native_transport: None,
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
    };
    // The normal recovery fastpath attaches before open_compatible_turn. Its
    // command callback belongs to failed F; only S's frozen round selects S scope.
    let stream = Arc::new(LocalRuntimeEventStream::new());
    let dependencies = execution_dependencies(&state, stream);
    let attached = start_compatible_typed_resume_stream_for_actor(
        dependencies,
        successor.clone(),
        command,
        actor,
        None,
    )
    .await
    .unwrap();
    let (events, claims) = collect_projection(attached.events).await;
    assert_exact_response(&events, &payloads, successor.id);
    assert_eq!(claims, 1);
    assert_eq!(
        delivery_statuses(&state, &[committed.tool_delivery_events[0].id]).await,
        vec!["acked"]
    );
}
