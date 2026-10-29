use super::*;
use crate::routes::application_public_api::sse::{
    native_sse_payload_for_runtime_event, IncludeWorkflowEvents,
};
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyActor, native::ApplicationNativeRunService,
};
use std::sync::atomic::{AtomicUsize, Ordering};

async fn fixture() -> (Arc<ApiState>, NativeRunResult, ApplicationApiKeyActor) {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let seed = native_run();
    seed_flow_run_for_compat_sse_test(&state, &seed).await;
    sqlx::query("update flow_runs set api_key_id=$2,status='running' where id=$1")
        .bind(seed.id)
        .bind(seed.api_key_id)
        .execute(state.store.pool())
        .await
        .unwrap();
    let (owner,workspace_id,tenant_id):(Uuid,Uuid,Uuid)=sqlx::query_as("select r.created_by,a.workspace_id,w.tenant_id from flow_runs r join applications a on a.id=r.application_id join workspaces w on w.id=a.workspace_id where r.id=$1")
        .bind(seed.id).fetch_one(state.store.pool()).await.unwrap();
    let actor = ApplicationApiKeyActor {
        api_key_id: seed.api_key_id,
        application_id: seed.application_id,
        creator_user_id: owner,
        workspace_id,
        tenant_id,
        actor: domain::ActorContext::root_in_scope(owner, tenant_id, workspace_id, "root"),
    };
    let actual = ApplicationNativeRunService::new(state.store.clone())
        .get_native_run_for_actor(actor.clone(), seed.id)
        .await
        .unwrap();
    assert!(actual.metadata["response_round_id"].is_null());
    assert_eq!(actual.status, NativeRunStatus::Running);
    (state, actual, actor)
}

fn observed_dependencies(
    state: &ApiState,
    stream: Arc<dyn RuntimeEventStream>,
    attempts: Arc<AtomicUsize>,
) -> NativeRunTerminalDependencies {
    let mut deps = NativeRunTerminalDependencies::new(
        state.store.clone(),
        state.runtime_engine.clone(),
        state.provider_runtime.clone(),
        state.provider_secret_master_key.clone(),
        state.model_billing_require_provider_usage,
        state.infrastructure.provider_transport_store(),
        stream,
    );
    deps.delivery_read_attempts = Some(attempts);
    deps
}

fn project_native(snapshot: &NativeRunResult, event: RuntimeEventEnvelope) -> Option<Value> {
    assert!(
        snapshot.metadata["response_round_id"].is_null(),
        "Native attach cannot synthesize Responses identity"
    );
    assert!(
        !event.payload["committed_delivery"]
            .as_bool()
            .unwrap_or(false),
        "provider inbox facts cannot reach a Native consumer"
    );
    native_sse_payload_for_runtime_event(snapshot, IncludeWorkflowEvents::Public, event)
        .map(|(kind, data)| json!({"type":kind,"data":serde_json::to_value(data).unwrap()}))
}

async fn collect_native(mut stream: NativeRunAttachmentStream) -> Vec<Value> {
    let mut frames = Vec::new();
    while let Some((snapshot, event)) =
        tokio::time::timeout(Duration::from_secs(5), stream.next_native())
            .await
            .expect("Native attachment must terminate")
    {
        if let Some(frame) = project_native(&snapshot, event) {
            frames.push(frame);
        }
    }
    frames
}

async fn complete(state: &ApiState, run: &NativeRunResult, answer: &str) {
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: domain::FlowRunStatus::Succeeded,
            output_payload: json!({"answer":answer}),
            error_payload: None,
            finished_at: Some(time::OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
}

async fn assert_untouched(state: &ApiState, ids: &[Uuid], attempts: &AtomicUsize) {
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        0,
        "Native entry must make zero Responses proof attempts, including errors"
    );
    assert_eq!(
        delivery_statuses(state, ids).await,
        vec!["pending"; ids.len()]
    );
    let rows:Vec<(i64,Option<Uuid>)>=sqlx::query_as("select delivery_generation,delivery_claim_token from runtime_events where id=any($1) order by sequence")
        .bind(ids.to_vec()).fetch_all(state.store.pool()).await.unwrap();
    assert_eq!(
        rows,
        vec![(0, None); ids.len()],
        "zero claims/receipts/ACK/release for provider inbox"
    );
}

#[tokio::test]
async fn authenticated_native_attach_without_responses_identity_preserves_replay_live_text_and_terminal(
) {
    let (state, run, _actor) = fixture().await;
    let old = seed_pending_committed_delivery(
        &state,
        &run,
        committed_delivery_payload("native-historical"),
    )
    .await;
    let own =
        seed_pending_committed_delivery(&state, &run, committed_delivery_payload("native-current"))
            .await;
    let stream = Arc::new(LocalRuntimeEventStream::new());
    stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream
        .append(run.id, debug_stream_events::flow_started(run.id))
        .await
        .unwrap();
    stream
        .append(
            run.id,
            RuntimeEventPayload {
                event_type: "provider_output_item_done".into(),
                source: RuntimeEventSource::Provider,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: false,
                trace_visible: true,
                payload: old.payload.clone(),
            },
        )
        .await
        .unwrap();
    let attempts = Arc::new(AtomicUsize::new(0));
    // This is the actual host entry selected only by Native WS attach after the
    // same real get_native_run_for_actor read above, not a fabricated Start scope.
    let mut attached = start_native_run_attachment_stream(
        observed_dependencies(&state, stream.clone(), attempts.clone()),
        stream.clone(),
        run.clone(),
        None,
    )
    .await
    .unwrap();
    let (snapshot, started) = attached.next_native().await.unwrap();
    assert_eq!(started.event_type, "flow_started");
    let mut frames = vec![project_native(&snapshot, started).unwrap()];
    stream
        .append(
            run.id,
            RuntimeEventPayload {
                event_type: "provider_output_item_done".into(),
                source: RuntimeEventSource::Provider,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: false,
                trace_visible: true,
                payload: own.payload.clone(),
            },
        )
        .await
        .unwrap();
    stream
        .append(
            run.id,
            debug_stream_events::answer_text_delta(
                "answer",
                "Native text".into(),
                0,
                None,
                None,
                None,
            ),
        )
        .await
        .unwrap();
    complete(&state, &run, "Native text").await;
    stream
        .append_terminal_if_missing_and_close(
            run.id,
            debug_stream_events::flow_finished(run.id, json!({})),
        )
        .await
        .unwrap();
    frames.extend(collect_native(attached).await);
    assert_eq!(
        frames
            .iter()
            .filter(|f| f["type"] == "message.delta")
            .map(|f| f["data"]["delta"].as_str().unwrap())
            .collect::<String>(),
        "Native text"
    );
    assert_eq!(
        frames
            .iter()
            .filter(|f| f["type"] == "run.completed")
            .count(),
        1
    );
    assert!(
        !frames
            .iter()
            .any(|f| f["type"] == "run.failed"
                || f["type"].as_str().unwrap().starts_with("response."))
    );
    assert_untouched(&state, &[old.id, own.id], &attempts).await;
}

#[tokio::test]
async fn authenticated_native_cold_attach_without_responses_identity_keeps_text_cursor_and_terminal(
) {
    let (state, run, actor) = fixture().await;
    let generation = Uuid::now_v7();
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
        json!({"stream_generation_id":generation}),
    )
    .await;
    let old = seed_pending_committed_delivery(
        &state,
        &run,
        committed_delivery_payload("cold-historical"),
    )
    .await;
    let own =
        seed_pending_committed_delivery(&state, &run, committed_delivery_payload("cold-current"))
            .await;
    let mut delta =
        debug_stream_events::answer_text_delta("answer", "cold Native".into(), 0, None, None, None);
    delta.payload["stream_generation_id"] = json!(generation);
    append_compat_sse_runtime_event(&state, run.id, &delta.event_type, delta.payload).await;
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "flow_finished",
        json!({"stream_generation_id":generation,"status":"succeeded"}),
    )
    .await;
    complete(&state, &run, "cold Native").await;
    let actual = ApplicationNativeRunService::new(state.store.clone())
        .get_native_run_for_actor(actor, run.id)
        .await
        .unwrap();
    assert!(actual.metadata["response_round_id"].is_null());
    let stream = Arc::new(LocalRuntimeEventStream::new());
    assert!(stream.subscribe(run.id, None).await.is_err());
    let attempts = Arc::new(AtomicUsize::new(0));
    let mut attached = start_native_run_attachment_stream(
        observed_dependencies(&state, stream.clone(), attempts.clone()),
        stream,
        actual,
        Some(70000),
    )
    .await
    .unwrap();
    let mut frames = Vec::new();
    while let Some((snapshot, event)) =
        tokio::time::timeout(Duration::from_secs(5), attached.next_native())
            .await
            .unwrap()
    {
        assert!(
            event.sequence > 70000,
            "cold projected cursor keeps its independent local space"
        );
        if let Some(frame) = project_native(&snapshot, event) {
            frames.push(frame);
        }
    }
    assert_eq!(
        frames
            .iter()
            .filter(|f| f["type"] == "message.delta")
            .map(|f| f["data"]["delta"].as_str().unwrap())
            .collect::<String>(),
        "cold Native"
    );
    assert_eq!(
        frames
            .iter()
            .filter(|f| f["type"] == "run.completed")
            .count(),
        1
    );
    assert!(!frames.iter().any(|f| f["type"] == "run.failed"));
    assert_untouched(&state, &[old.id, own.id], &attempts).await;
}

#[tokio::test]
async fn responses_open_empty_refreshes_atomic_tools_when_only_terminal_is_notified() {
    assert_open_empty_terminal_only(2).await;
}

#[tokio::test]
async fn responses_open_empty_terminal_only_projects_385_exact_tools_in_four_batches() {
    assert_open_empty_terminal_only(385).await;
}

async fn assert_open_empty_terminal_only(count: usize) {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let old =
        super::delivery_proof::commit_wait(&state, &run, &[committed_delivery_payload("old")])
            .await;
    let callback = old.callback_task.as_ref().unwrap().id;
    run.metadata["response_round_id"] = json!(callback);
    let (tx, live) = mpsc::unbounded_channel();
    let (sender, mut receiver) = mpsc::channel(8);
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut dependencies = super::delivery_proof::terminal_dependencies(&state);
    dependencies.delivery_read_samples = Some(samples.clone());
    let (_closure_sender, closure) = tokio::sync::watch::channel(None);
    let task = tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            terminal_dependencies: dependencies,
            initial_run: run.clone(),
            delivery_policy: CompatibleDeliveryPolicy::ScopedResponses,
            delivery_scope: RuntimeEventDeliveryScope::Resume {
                callback_task_id: callback,
            },
            from_sequence: Some(50000),
            durable_round_replay: Vec::new(),
            ignored_waiting_callback_task_id: Some(callback),
            subscription: control_plane::ports::RuntimeEventSubscription {
                terminal_writer: Arc::new(UnusedTerminalWriter),
                replay: Vec::new(),
                live_events: control_plane::ports::RuntimeEventReceiver::from_unbounded(live),
                closure,
            },
            sender,
        },
    ));
    tx.send(RuntimeEventEnvelope::new(
        run.id,
        60000,
        debug_stream_events::flow_started(run.id),
    ))
    .unwrap();
    assert_eq!(
        receiver.recv().await.unwrap().envelope.event_type,
        "flow_started",
        "initial successful Open-empty query barrier"
    );
    let payloads = (0..count)
        .map(|index| {
            let mut payload = super::delivery_proof::exact_payload(index);
            payload["output_index"] = json!((count - index) % 3);
            payload
        })
        .collect::<Vec<_>>();
    let own = super::delivery_proof::commit_wait(&state, &run, &payloads).await;
    let future =
        super::delivery_proof::commit_wait(&state, &run, &[committed_delivery_payload("future")])
            .await;
    let mut terminal=crate::routes::application_public_api::stream_terminal_fallback::durable_record_to_runtime_event_envelope(own.waiting_event);
    terminal.sequence = 80000;
    tx.send(terminal).unwrap();
    drop(tx); // No owned tool notifications.
    let (events, claims) = super::delivery_proof::collect_projection(receiver).await;
    task.await.unwrap();
    assert_eq!(claims, count);
    super::delivery_proof::assert_exact_response(&events, &payloads, callback);
    let reads = samples.lock().expect("proof observations").clone();
    eprintln!("Open-empty terminal-only proof/claim/bytes observations: {reads:?}");
    let mut expected = vec![(false, 0, 0)];
    expected.extend((0..count).step_by(128).map(|offset| {
        (
            true,
            if offset == 0 { count } else { 0 },
            (count - offset).min(128),
        )
    }));
    assert_eq!(
        reads.iter().map(|r| (r.0, r.1, r.2)).collect::<Vec<_>>(),
        expected
    );
    assert!(reads[1].3 > 0);
    assert!(reads
        .iter()
        .enumerate()
        .all(|(index, r)| index == 1 || r.3 == 0));
    assert_eq!(
        delivery_statuses(
            &state,
            &[
                old.tool_delivery_events[0].id,
                future.tool_delivery_events[0].id
            ]
        )
        .await,
        vec!["pending", "pending"]
    );
}

#[tokio::test]
async fn native_cold_attachment_preserves_ordinary_callback_and_human_actions() {
    use control_plane::ports::CreateCallbackTaskInput;
    for human in [false, true] {
        let (state, run, actor) = fixture().await;
        let callback = if human {
            None
        } else {
            let node = state
                .store
                .create_node_run(&CreateNodeRunInput {
                    flow_run_id: run.id,
                    node_id: "callback".into(),
                    node_type: "tool".into(),
                    node_alias: "Callback".into(),
                    status: domain::NodeRunStatus::WaitingCallback,
                    input_payload: json!({}),
                    debug_payload: json!({}),
                    started_at: time::OffsetDateTime::now_utc(),
                })
                .await
                .unwrap();
            Some(
                state
                    .store
                    .create_callback_task(&CreateCallbackTaskInput {
                        flow_run_id: run.id,
                        node_run_id: node.id,
                        callback_kind: "tool".into(),
                        request_payload: json!({"opaque":{"preserve":[1,"00"]}}),
                        external_ref_payload: None,
                    })
                    .await
                    .unwrap(),
            )
        };
        state
            .store
            .update_flow_run(&UpdateFlowRunInput {
                flow_run_id: run.id,
                status: if human {
                    domain::FlowRunStatus::WaitingHuman
                } else {
                    domain::FlowRunStatus::WaitingCallback
                },
                output_payload: json!({}),
                error_payload: None,
                finished_at: None,
            })
            .await
            .unwrap();
        let actual = ApplicationNativeRunService::new(state.store.clone())
            .get_native_run_for_actor(actor, run.id)
            .await
            .unwrap();
        let stream = Arc::new(LocalRuntimeEventStream::new());
        let attempts = Arc::new(AtomicUsize::new(0));
        let attached = start_native_run_attachment_stream(
            observed_dependencies(&state, stream.clone(), attempts.clone()),
            stream,
            actual,
            None,
        )
        .await
        .unwrap();
        let frames = collect_native(attached).await;
        let required = &frames
            .iter()
            .find(|f| f["type"] == "required_action")
            .unwrap()["data"]["required_action"];
        if let Some(callback) = callback {
            assert_eq!(required["action_type"], "callback");
            assert_eq!(required["payload"]["callback_task_id"], json!(callback.id));
            assert_eq!(
                required["payload"]["request_payload"],
                callback.request_payload
            );
        } else {
            assert_eq!(required["type"], "waiting_human");
            assert_eq!(required["run_id"], json!(run.id));
        }
        assert_eq!(attempts.load(Ordering::SeqCst), 0);
    }
}
