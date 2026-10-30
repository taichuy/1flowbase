use super::*;
use crate::routes::application_public_api::stream_terminal_fallback::durable_record_to_runtime_event_envelope;
use control_plane::ports::{
    AckRuntimeEventDeliveryInput, AppendRuntimeEventInput, ClaimRuntimeEventDeliveriesInput,
    MarkRuntimeEventDeliveryUncertainInput, PersistWaitingCallbackTaskInput, PersistWaitingKind,
    PersistWaitingStateInput, PersistedWaitingState, RuntimeEventSubscription,
};

// Reuse the actual inbox owner for both execution-before-output and the next
// waiting commit. An exact second call observes the same processing claim.
pub(in super::super) async fn resume_waiting_callback(
    state: &ApiState,
    run: &NativeRunResult,
) -> (
    Option<Uuid>,
    Option<control_plane::ports::ResumeClaimRecord>,
) {
    let scope_id: Uuid = sqlx::query_scalar("select scope_id from flow_runs where id=$1")
        .bind(run.id)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let previous: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "select h.context_version_id,c.id,t.id from flow_run_recovery_history h \
         join flow_run_checkpoints c on c.flow_run_id=h.flow_run_id \
           and c.locator_payload->>'context_version_id'=h.context_version_id::text \
         join flow_run_callback_tasks t on t.flow_run_id=h.flow_run_id and t.node_run_id=h.node_run_id \
         where h.flow_run_id=$1 order by h.sequence desc,t.created_at desc,t.id desc limit 1",
    ).bind(run.id).fetch_optional(state.store.pool()).await.unwrap();
    if let Some((context, checkpoint, callback)) = previous {
        let task = state
            .store
            .get_callback_task(callback)
            .await
            .unwrap()
            .unwrap();
        let results = if task.status == domain::CallbackTaskStatus::Completed {
            task.response_payload.as_ref().unwrap()["tool_results"]
                .as_array()
                .unwrap()
                .iter()
                .cloned()
                .map(control_plane::ports::ToolCallbackResultInput::from_payload)
                .collect::<anyhow::Result<Vec<_>>>()
                .unwrap()
        } else {
            task.request_payload["tool_calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|call| {
                    control_plane::ports::ToolCallbackResultInput::from_payload(
                        json!({"tool_call_id":call["id"],"content":"fixture-result"}),
                    )
                    .unwrap()
                })
                .collect()
        };
        let resumed = state
            .store
            .commit_tool_callback_results(&control_plane::ports::CommitToolCallbackResultsInput {
                responses_continuation: None,
                scope_id,
                application_id: run.application_id,
                flow_run_id: run.id,
                checkpoint_id: checkpoint,
                callback_task_id: callback,
                results,
            })
            .await
            .unwrap();
        let claim = resumed
            .claim
            .expect("successor wait must have its real execution owner");
        assert_eq!(
            claim.status,
            control_plane::ports::ResumeClaimStatus::Processing
        );
        (Some(context), Some(claim))
    } else {
        let has_history: bool = sqlx::query_scalar(
            "select exists(select 1 from flow_run_recovery_history where flow_run_id=$1)",
        )
        .bind(run.id)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
        assert!(
            !has_history,
            "a successor fixture cannot invent initial-execution authority"
        );
        (None, None)
    }
}

// Use the actual atomic waiting writer: tool facts, original wait, context and
// recovery anchor commit together, unlike asynchronous diagnostic append copies.
pub(super) async fn commit_wait(
    state: &ApiState,
    run: &NativeRunResult,
    payloads: &[Value],
) -> PersistedWaitingState {
    commit_wait_with_tool_calls(state, run, payloads, None).await
}

// Semantic protocol fixtures also need the callback tools on the atomic waiting
// event. Exact-native fixtures keep their existing minimal waiting payload.
pub(in super::super) async fn commit_wait_with_tool_calls(
    state: &ApiState,
    run: &NativeRunResult,
    payloads: &[Value],
    tool_calls: Option<&Value>,
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
    let (parent_context, resume_claim) = resume_waiting_callback(state, run).await;
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
            parent_context_version_id: parent_context,
            context_transition_kind: if parent_context.is_some() {
                domain::ContextTransitionKind::Callback
            } else {
                domain::ContextTransitionKind::Append
            },
            recovery_idempotency_key: format!("waiting_callback:{callback}"),
            resume_claim_id: resume_claim.as_ref().map(|claim| claim.id),
            resume_claim_token: resume_claim.as_ref().map(|claim| claim.claim_token),
            waiting_event: event(
                "waiting_callback",
                match tool_calls {
                    Some(tool_calls) => json!({
                        "type": "waiting_callback",
                        "run_id": run.id,
                        "status": "waiting_callback",
                        "callback_task_id": callback,
                        "callback_kind": "llm_tool_calls",
                        "node_run_id": node.id,
                        "tool_calls": tool_calls
                    }),
                    None => json!({"callback_task_id":callback}),
                },
            ),
            tool_delivery_events: payloads
                .iter()
                .map(|payload| event("provider_output_item_done", payload.clone()))
                .collect(),
            kind: PersistWaitingKind::Callback(PersistWaitingCallbackTaskInput {
                id: callback,
                callback_kind: "llm_tool_calls".into(),
                external_ref_payload: None,
                request_payload: json!({"tool_calls": tool_calls.cloned().unwrap_or_else(|| {
                    json!(payloads.iter().map(|payload| json!({
                        "id":payload["item"]["call_id"],"name":"exec","arguments":{}
                    })).collect::<Vec<_>>())
                })}),
            }),
        })
        .await
        .unwrap()
        .unwrap()
}

pub(super) fn exact_payload(index: usize) -> Value {
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

pub(super) fn terminal_dependencies(state: &ApiState) -> NativeRunTerminalDependencies {
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

pub(super) async fn collect_projection(
    receiver: mpsc::Receiver<CompatibleProjectionInput>,
) -> (Vec<Value>, usize) {
    collect_projection_with_prefix(Vec::new(), receiver).await
}

async fn collect_projection_with_prefix(
    prefix: Vec<CompatibleProjectionInput>,
    mut receiver: mpsc::Receiver<CompatibleProjectionInput>,
) -> (Vec<Value>, usize) {
    let mut prefix = prefix.into_iter();
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::TransparentProviderResponses,
    );
    let mut frames = Vec::new();
    let mut claims = 0;
    while let Some(input) = match prefix.next() {
        Some(input) => Some(input),
        None => tokio::time::timeout(Duration::from_secs(10), receiver.recv())
            .await
            .expect("forwarder must terminate"),
    } {
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

pub(super) fn assert_exact_response(events: &[Value], expected: &[Value], round: Uuid) {
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
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "response.output_item.done")
            .map(|event| event["output_index"].clone())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|payload| payload["output_index"].clone())
            .collect::<Vec<_>>(),
        "provider-local output indices are preserved, never used as durable ordering"
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
            delivery_policy: CompatibleDeliveryPolicy::ScopedResponses,
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

// Provider output_index deliberately repeats and moves backwards. Original
// durable sequence, rather than that provider-local index, orders all assertions.
fn terminal_payloads(count: usize) -> Vec<Value> {
    (0..count)
        .map(|index| {
            let mut payload = exact_payload(index);
            payload["output_index"] = json!((count - index) % 3);
            payload
        })
        .collect()
}

async fn delivery_snapshot(state: &ApiState, ids: &[Uuid]) -> Vec<Value> {
    sqlx::query_scalar(
        "select jsonb_build_object('id',id,'status',delivery_status,'token',delivery_claim_token,\
         'generation',delivery_generation,'lease',delivery_lease_expires_at) \
         from runtime_events where id=any($1) order by sequence,id",
    )
    .bind(ids)
    .fetch_all(state.store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn text_only_open_scope_preserves_durable_final_terminal() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    run.status = NativeRunStatus::Succeeded;
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: domain::FlowRunStatus::Succeeded,
            output_payload: json!({"answer":"done"}),
            error_payload: None,
            finished_at: Some(time::OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
    let mut subscription = empty_subscription();
    subscription.replay.push(RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::flow_finished(run.id, json!({"answer":"done"})),
    ));
    let (sender, receiver) = mpsc::channel(8);
    let task = tokio::spawn(send_subscribed_compatible_typed_event_stream(
        SubscribedCompatibleTypedEventStream {
            terminal_dependencies: terminal_dependencies(&state),
            initial_run: run,
            delivery_policy: CompatibleDeliveryPolicy::ScopedResponses,
            delivery_scope: RuntimeEventDeliveryScope::Start,
            from_sequence: None,
            durable_round_replay: Vec::new(),
            ignored_waiting_callback_task_id: None,
            subscription,
            sender,
        },
    ));
    let (events, claims) = collect_projection(receiver).await;
    task.await.unwrap();
    assert_eq!(claims, 0);
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "response.completed")
            .count(),
        1
    );
    assert!(!events
        .iter()
        .any(|event| event["type"] == "response.failed"));
}

#[tokio::test]
async fn closed_terminal_only_385_completes_durable_subscription_and_live_lanes() {
    for lane in ["durable", "subscription", "live"] {
        let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
        let mut run = native_run();
        seed_flow_run_for_compat_sse_test(&state, &run).await;
        let old = commit_wait(&state, &run, &[committed_delivery_payload("old")]).await;
        let callback = old.callback_task.as_ref().unwrap().id;
        run.metadata["response_round_id"] = json!(callback);
        let payloads = terminal_payloads(385);
        let own = commit_wait(&state, &run, &payloads).await;
        let future = commit_wait(&state, &run, &[committed_delivery_payload("future")]).await;
        let foreign_ids = [
            old.tool_delivery_events[0].id,
            future.tool_delivery_events[0].id,
        ];
        let before = delivery_snapshot(&state, &foreign_ids).await;
        let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut dependencies = terminal_dependencies(&state);
        dependencies.delivery_read_samples = Some(samples.clone());
        let mut terminal = durable_record_to_runtime_event_envelope(own.waiting_event);
        terminal.sequence = 90000;
        let (tx, live) = mpsc::unbounded_channel();
        let mut subscription = empty_subscription();
        subscription.live_events = control_plane::ports::RuntimeEventReceiver::from_unbounded(live);
        let selection = RuntimeEventEnvelope::new(
            run.id,
            60000,
            control_plane::ports::LlmInputSelection {
                node_id: "llm".into(),
                node_run_id: Uuid::now_v7(),
                representation: control_plane::ports::LlmInputRepresentation::NativeOpaque,
            }
            .runtime_event(),
        );
        // Selection is replayed before the provider inbox is drained. The live
        // terminal case has selection in subscription replay, as a reconnect does.
        subscription.replay.push(selection);
        let mut durable = Vec::new();
        match lane {
            "durable" => {
                durable.append(&mut subscription.replay);
                durable.push(terminal);
            }
            "subscription" => subscription.replay.push(terminal),
            "live" => tx.send(terminal).unwrap(),
            _ => unreachable!(),
        }
        drop(tx);
        let (sender, mut receiver) = mpsc::channel(8);
        let task = tokio::spawn(send_subscribed_compatible_typed_event_stream(
            SubscribedCompatibleTypedEventStream {
                terminal_dependencies: dependencies,
                initial_run: run.clone(),
                delivery_policy: CompatibleDeliveryPolicy::ScopedResponses,
                delivery_scope: RuntimeEventDeliveryScope::Resume {
                    callback_task_id: callback,
                },
                from_sequence: Some(50000),
                durable_round_replay: durable,
                ignored_waiting_callback_task_id: Some(callback),
                subscription,
                sender,
            },
        ));
        let selection = tokio::time::timeout(Duration::from_secs(10), receiver.recv())
            .await
            .expect("selection before tools")
            .expect("host selection input");
        assert_eq!(
            selection.envelope.event_type,
            control_plane::ports::LlmInputSelection::EVENT_TYPE
        );
        assert!(selection.delivery.is_none());
        let (events, claims) = collect_projection(receiver).await;
        task.await.unwrap();
        assert_exact_response(&events, &payloads, callback);
        assert_eq!(claims, 385, "{lane}");
        let reads = samples.lock().expect("proof observations").clone();
        eprintln!("Closed terminal-only {lane} proof/claim/bytes observations: {reads:?}");
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
        assert_eq!(delivery_snapshot(&state, &foreign_ids).await, before);
    }
}

#[tokio::test]
async fn closed_mixed_status_prefix_preserves_peer_authority_and_exact_terminal() {
    use control_plane::ports::ReleaseRuntimeEventDeliveryInput;
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let old = commit_wait(&state, &run, &[committed_delivery_payload("old")]).await;
    let callback = old.callback_task.as_ref().unwrap().id;
    run.metadata["response_round_id"] = json!(callback);
    let payloads = terminal_payloads(5);
    let own = commit_wait(&state, &run, &payloads).await;
    let future = commit_wait(&state, &run, &[committed_delivery_payload("future")]).await;
    let peer = state
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
    assert_eq!(peer.claims.len(), 5);
    let claim = &peer.claims[1];
    assert_eq!(
        state
            .store
            .ack_runtime_event_delivery(&AckRuntimeEventDeliveryInput {
                event_id: claim.event.id,
                claim_token: claim.claim_token,
                expected_generation: claim.generation,
                acknowledged_at: time::OffsetDateTime::now_utc(),
            })
            .await
            .unwrap()
            .id,
        claim.event.id
    );
    let claim = &peer.claims[2];
    assert_eq!(
        state
            .store
            .mark_runtime_event_delivery_uncertain(&MarkRuntimeEventDeliveryUncertainInput {
                event_id: claim.event.id,
                claim_token: claim.claim_token,
                expected_generation: claim.generation,
                marked_at: time::OffsetDateTime::now_utc(),
            })
            .await
            .unwrap()
            .id,
        claim.event.id
    );
    let claim = &peer.claims[3];
    assert_eq!(
        state
            .store
            .release_runtime_event_delivery(&ReleaseRuntimeEventDeliveryInput {
                event_id: claim.event.id,
                claim_token: claim.claim_token,
                expected_generation: claim.generation,
            })
            .await
            .unwrap()
            .id,
        claim.event.id
    );
    let mut nonowned = vec![
        old.tool_delivery_events[0].id,
        future.tool_delivery_events[0].id,
    ];
    nonowned.extend([0, 1, 2, 4].map(|index| own.tool_delivery_events[index].id));
    let before = delivery_snapshot(&state, &nonowned).await;
    let (tx, live) = mpsc::unbounded_channel();
    let (sender, mut receiver) = mpsc::channel(1);
    let task = spawn_scoped_typed_stream(
        &state,
        &run,
        RuntimeEventDeliveryScope::Resume {
            callback_task_id: callback,
        },
        Vec::new(),
        Vec::new(),
        live,
        sender,
    );
    // Observe the actual first Closed batch BEFORE any terminal. Its one local
    // claim is index3; indices0..2 must be exact receipt-less prefix replays.
    let mut prefix = Vec::new();
    for (index, payload) in payloads[..4].iter().enumerate() {
        let input = tokio::time::timeout(Duration::from_secs(10), receiver.recv())
            .await
            .expect("Closed prefix must progress")
            .expect("prefix input");
        assert_eq!(input.envelope.payload, *payload);
        assert_eq!(input.delivery.is_some(), index == 3);
        prefix.push(input);
    }
    let mut terminal = durable_record_to_runtime_event_envelope(own.waiting_event);
    terminal.sequence = 90000;
    tx.send(terminal).unwrap();
    drop(tx);
    let (events, claims) = collect_projection_with_prefix(prefix, receiver).await;
    task.await.unwrap();
    assert_exact_response(&events, &payloads, callback);
    assert_eq!(claims, 1, "no receipt for acked/uncertain/peer-held facts");
    assert_eq!(delivery_snapshot(&state, &nonowned).await, before);
    assert_eq!(
        delivery_statuses(&state, &[own.tool_delivery_events[3].id]).await,
        vec!["acked"]
    );
}

#[tokio::test]
async fn closed_claim_read_fault_finishes_exact_remainder_without_receipts() {
    assert_closed_no_progress(true).await;
}

#[tokio::test]
async fn closed_reclaimed_seen_prefix_finishes_without_ack_or_lease_progress() {
    assert_closed_no_progress(false).await;
}

async fn assert_closed_no_progress(read_fault: bool) {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let old = commit_wait(&state, &run, &[committed_delivery_payload("old")]).await;
    let callback = old.callback_task.as_ref().unwrap().id;
    run.metadata["response_round_id"] = json!(callback);
    let payloads = terminal_payloads(385);
    let own = commit_wait(&state, &run, &payloads).await;
    let future = commit_wait(&state, &run, &[committed_delivery_payload("future")]).await;
    let foreign_ids = [
        old.tool_delivery_events[0].id,
        future.tool_delivery_events[0].id,
    ];
    let before = delivery_snapshot(&state, &foreign_ids).await;
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut dependencies = terminal_dependencies(&state);
    dependencies.delivery_read_samples = Some(samples.clone());
    dependencies.delivery_read_attempts = Some(attempts.clone());
    let (tx, live) = mpsc::unbounded_channel();
    let (sender, mut receiver) = mpsc::channel(1);
    let mut subscription = empty_subscription();
    subscription.live_events = control_plane::ports::RuntimeEventReceiver::from_unbounded(live);
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
            subscription,
            sender,
        },
    ));
    let mut prefix = Vec::new();
    for payload in &payloads[..128] {
        let input = tokio::time::timeout(Duration::from_secs(10), receiver.recv())
            .await
            .expect("initial batch must progress")
            .expect("initial claimed input");
        assert_eq!(input.envelope.payload, *payload);
        assert!(input.delivery.is_some());
        prefix.push(input); // Deliberately hold receipts; no ACK is needed for termination.
    }
    if read_fault {
        sqlx::query("alter table flow_run_recovery_history rename to fixture_unavailable_recovery")
            .execute(state.store.pool())
            .await
            .unwrap();
    } else {
        let ids = own.tool_delivery_events[..128]
            .iter()
            .map(|event| event.id)
            .collect::<Vec<_>>();
        let claims: Vec<(Uuid,Uuid,i64)> = sqlx::query_as(
            "select id,delivery_claim_token,delivery_generation from runtime_events where id=any($1) order by sequence",
        ).bind(&ids).fetch_all(state.store.pool()).await.unwrap();
        let tokens = claims.iter().map(|row| row.1).collect::<Vec<_>>();
        let generations = claims.iter().map(|row| row.2).collect::<Vec<_>>();
        let expired = sqlx::query(
            "update runtime_events e set delivery_lease_expires_at=now()-interval '1 second' \
             from unnest($1::uuid[],$2::uuid[],$3::bigint[]) c(id,token,generation) \
             where e.id=c.id and e.delivery_status='claimed' and e.delivery_claim_token=c.token \
             and e.delivery_generation=c.generation",
        )
        .bind(&ids)
        .bind(tokens)
        .bind(generations)
        .execute(state.store.pool())
        .await
        .unwrap();
        assert_eq!(
            expired.rows_affected(),
            128,
            "fixture expires only exact owned claims"
        );
    }
    let mut terminal = durable_record_to_runtime_event_envelope(own.waiting_event);
    terminal.sequence = 90000;
    tx.send(terminal).unwrap();
    drop(tx);
    // A real receipt-less remainder input is the query/no-progress barrier.
    // Do not settle held prefix receipts before the reclaim query executes.
    let next = tokio::time::timeout(Duration::from_secs(10), receiver.recv())
        .await
        .expect("finite exact replay after failed/no-progress claim")
        .expect("first remainder input");
    assert_eq!(next.envelope.payload, payloads[128]);
    assert!(next.delivery.is_none());
    prefix.push(next);
    let (events, claims) = collect_projection_with_prefix(prefix, receiver).await;
    task.await.unwrap();
    if read_fault {
        sqlx::query("alter table fixture_unavailable_recovery rename to flow_run_recovery_history")
            .execute(state.store.pool())
            .await
            .unwrap();
    }
    assert_exact_response(&events, &payloads, callback);
    assert_eq!(
        claims, 128,
        "remaining257 are exact replay without fabricated receipts"
    );
    assert_eq!(
        attempts.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "finite: first batch plus one no-progress/fault query"
    );
    let reads = samples.lock().expect("proof observations").clone();
    eprintln!("Closed no-progress read_fault={read_fault} observations: {reads:?}");
    let expected = if read_fault {
        vec![(true, 385, 128)]
    } else {
        vec![(true, 385, 128), (true, 0, 128)]
    };
    assert_eq!(
        reads.iter().map(|r| (r.0, r.1, r.2)).collect::<Vec<_>>(),
        expected
    );
    assert!(reads[0].3 > 0);
    assert!(reads[1..].iter().all(|r| r.3 == 0));
    let prefix_ids = own.tool_delivery_events[..128]
        .iter()
        .map(|event| event.id)
        .collect::<Vec<_>>();
    let prefix_generations: Vec<i64> = sqlx::query_scalar(
        "select delivery_generation from runtime_events where id=any($1) order by sequence",
    )
    .bind(&prefix_ids)
    .fetch_all(state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        prefix_generations,
        vec![if read_fault { 1 } else { 2 }; 128]
    );
    assert_eq!(
        delivery_statuses(&state, &prefix_ids).await,
        vec![if read_fault { "acked" } else { "pending" }; 128],
        "stale original receipts cannot settle newly reclaimed generations"
    );
    assert_eq!(delivery_snapshot(&state, &foreign_ids).await, before);
    let remainder_ids = own.tool_delivery_events[128..]
        .iter()
        .map(|event| event.id)
        .collect::<Vec<_>>();
    let generations: Vec<i64> = sqlx::query_scalar(
        "select delivery_generation from runtime_events where id=any($1) order by sequence",
    )
    .bind(&remainder_ids)
    .fetch_all(state.store.pool())
    .await
    .unwrap();
    assert_eq!(generations, vec![0; 257]);
    assert_eq!(
        delivery_statuses(&state, &remainder_ids).await,
        vec!["pending"; 257]
    );
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
            delivery_policy: CompatibleDeliveryPolicy::ScopedResponses,
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
