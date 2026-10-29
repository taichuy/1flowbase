use super::*;
use control_plane::ports::{
    AppendContextVersionInput, AppendRecoveryHistoryInput, ClaimRuntimeEventDeliveriesInput,
    PutCanonicalRuntimeContentInput,
};

/// Real repository anchors, including human/non-native stops. Unlike a diagnostic
/// waiting copy these rows are the immutable business history used by production.
pub(super) async fn seed_scope_boundary(
    state: &ApiState,
    run: &NativeRunResult,
    callback: Option<Uuid>,
) -> domain::RuntimeEventRecord {
    let scope_id: Uuid = sqlx::query_scalar("select scope_id from flow_runs where id=$1")
        .bind(run.id)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let content = state
        .store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id,
            application_id: run.application_id,
            content: json!({"fixture":"delivery-scope"}),
        })
        .await
        .unwrap();
    let sequence: i64 = sqlx::query_scalar(
        "select coalesce(max(sequence),-1)+1 from flow_run_recovery_history where flow_run_id=$1",
    )
    .bind(run.id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    let context = state
        .store
        .append_context_version(&AppendContextVersionInput {
            scope_id,
            application_id: run.application_id,
            flow_run_id: run.id,
            parent_context_version_id: None,
            sequence,
            transition_kind: domain::ContextTransitionKind::Append,
            transition_actor: domain::ContextTransitionActor::Host,
            declared_compaction_provenance: None,
            actual_content_id: content.id,
        })
        .await
        .unwrap();
    let event = state
        .store
        .append_runtime_event(&control_plane::ports::AppendRuntimeEventInput {
            flow_run_id: run.id,
            node_run_id: None,
            span_id: None,
            parent_span_id: None,
            event_type: if callback.is_some() {
                "waiting_callback"
            } else {
                "waiting_human"
            }
            .into(),
            layer: domain::RuntimeEventLayer::AgentTransition,
            source: domain::RuntimeEventSource::Host,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload: json!({"callback_task_id":callback}),
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
        })
        .await
        .unwrap();
    state
        .store
        .append_recovery_history(&AppendRecoveryHistoryInput {
            scope_id,
            application_id: run.application_id,
            flow_run_id: run.id,
            node_run_id: None,
            sequence,
            state_code: if callback.is_some() {
                domain::RecoveryStateCode::WaitingCallback
            } else {
                domain::RecoveryStateCode::WaitingHuman
            },
            coordinate: domain::RecoveryCoordinate {
                node_sequence: 0,
                iteration_index: 0,
                attempt_index: 0,
                resume_sequence: sequence,
                event_sequence: event.sequence,
            },
            context_version_id: context.id,
            recovery_content_id: Some(content.id),
            idempotency_key: callback.map_or_else(
                || format!("waiting_human:{}", event.id),
                |id| format!("waiting_callback:{id}"),
            ),
        })
        .await
        .unwrap();
    event
}

async fn project_round(
    state: &Arc<ApiState>,
    run: &NativeRunResult,
    scope: RuntimeEventDeliveryScope,
    lane: usize,
    payloads: &[Value],
) -> Vec<Value> {
    // Local positions are deliberately unrelated to durable sequences.
    let envelopes = || {
        payloads
            .iter()
            .enumerate()
            .map(|(i, p)| committed_delivery_envelope(run.id, 70000 + i as i64, p.clone()))
            .collect::<Vec<_>>()
    };
    let durable = if lane == 0 { envelopes() } else { Vec::new() };
    let replay = if lane == 1 { envelopes() } else { Vec::new() };
    let (tx, live) = mpsc::unbounded_channel();
    if lane == 2 {
        for event in envelopes() {
            tx.send(event).unwrap();
        }
    }
    tx.send(RuntimeEventEnvelope::new(
        run.id,
        80000,
        RuntimeEventPayload {
            event_type: "waiting_callback".into(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: false,
            trace_visible: true,
            payload: json!({}),
        },
    ))
    .unwrap();
    drop(tx);
    let (mut receiver, forwarding) = if lane == 3 {
        // Production generic attach has no ignored callback ID; its frozen selected
        // response metadata still determines ownership after descendants exist.
        let stream = Arc::new(LocalRuntimeEventStream::new());
        stream
            .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        for payload in payloads {
            stream
                .append(
                    run.id,
                    RuntimeEventPayload {
                        event_type: "provider_output_item_done".into(),
                        source: RuntimeEventSource::Provider,
                        durability: RuntimeEventDurability::DurableRequired,
                        persist_required: false,
                        trace_visible: true,
                        payload: payload.clone(),
                    },
                )
                .await
                .unwrap();
        }
        stream
            .append(
                run.id,
                RuntimeEventPayload {
                    event_type: "waiting_callback".into(),
                    source: RuntimeEventSource::Runtime,
                    durability: RuntimeEventDurability::DurableRequired,
                    persist_required: false,
                    trace_visible: true,
                    payload: json!({}),
                },
            )
            .await
            .unwrap();
        let dependencies = NativeRunTerminalDependencies::new(
            state.store.clone(),
            state.runtime_engine.clone(),
            state.provider_runtime.clone(),
            state.provider_secret_master_key.clone(),
            state.model_billing_require_provider_usage,
            state.infrastructure.provider_transport_store(),
            stream.clone(),
        );
        let attached = attach_compatible_typed_stream_with_replay(
            dependencies,
            stream,
            run.clone(),
            None,
            Vec::new(),
        )
        .await
        .unwrap();
        (attached.events, None)
    } else {
        let (sender, receiver) = mpsc::channel(8);
        let forwarding =
            spawn_scoped_typed_stream(state, run, scope, durable, replay, live, sender);
        (receiver, Some(forwarding))
    };
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::TransparentProviderResponses,
    );
    let mut frames = Vec::new();
    while let Some(input) = receiver.recv().await {
        let (snapshot, event, receipt) = input.into_parts();
        frames.extend(mapper.runtime_event_to_sse(&snapshot, event));
        if let Some(receipt) = receipt {
            receipt.projected();
        }
    }
    if let Some(forwarding) = forwarding {
        forwarding.await.unwrap();
    }
    let response = axum::response::sse::Sse::new(tokio_stream::iter(frames)).into_response();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test]
async fn failed_projected_ack_cannot_inject_old_or_future_tools_into_selected_response() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let old_payload = json!({"output_index":3,"committed_delivery":true,
        "item":{"id":"ctc_old","type":"custom_tool_call","call_id":"old","name":"exec","input":"old"}});
    let old = seed_pending_committed_delivery(&state, &run, old_payload.clone()).await;
    // Exercise the real writer receipt's failed persistence, not a mocked ACK count.
    sqlx::raw_sql("create function fail_projected_ack() returns trigger language plpgsql as $$ begin raise exception 'injected ACK storage failure'; end $$; create trigger fail_projected_ack before update on runtime_events for each row when (new.delivery_status='acked') execute function fail_projected_ack()")
        .execute(state.store.pool()).await.unwrap();
    let (tx, live) = mpsc::unbounded_channel();
    drop(tx);
    let (sender, mut receiver) = mpsc::channel(8);
    let forward = spawn_typed_stream(&state, &run, Vec::new(), live, sender);
    let input = receiver.recv().await.unwrap();
    assert_eq!(input.envelope.payload, old_payload);
    input.into_parts().2.unwrap().projected();
    while receiver.recv().await.is_some() {}
    forward.await.unwrap();
    assert_eq!(delivery_statuses(&state, &[old.id]).await, vec!["claimed"]);
    sqlx::raw_sql(
        "drop trigger fail_projected_ack on runtime_events; drop function fail_projected_ack()",
    )
    .execute(state.store.pool())
    .await
    .unwrap();
    sqlx::query(
        "update runtime_events set delivery_lease_expires_at=now()-interval '1 second' where id=$1",
    )
    .bind(old.id)
    .execute(state.store.pool())
    .await
    .unwrap();
    let predecessor = Uuid::now_v7();
    seed_scope_boundary(&state, &run, Some(predecessor)).await;
    run.metadata["response_round_id"] = json!(predecessor);
    let own_payload = json!({"output_index":2,"committed_delivery":true,
        "item":{"id":"ctc_own","type":"custom_tool_call","call_id":"own","name":"exec",
            "input":"exact\narguments","opaque":{"retain":[1,2]}}});
    let own = seed_pending_committed_delivery(&state, &run, own_payload.clone()).await;
    let next = Uuid::now_v7();
    seed_scope_boundary(&state, &run, Some(next)).await;
    let future_payload = json!({"output_index":0,"committed_delivery":true,
        "item":{"id":"ctc_future","type":"function_call","call_id":"future","name":"exec","arguments":"{}"}});
    let future = seed_pending_committed_delivery(&state, &run, future_payload.clone()).await;
    // A later diagnostic copy of A must not replace its original lower anchor.
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "waiting_callback",
        json!({"callback_task_id":predecessor}),
    )
    .await;
    for lane in 0..4 {
        let events = project_round(
            &state,
            &run,
            RuntimeEventDeliveryScope::Resume {
                callback_task_id: predecessor,
            },
            lane,
            &[
                old_payload.clone(),
                own_payload.clone(),
                future_payload.clone(),
            ],
        )
        .await;
        let done = events
            .iter()
            .filter(|v| v["type"] == "response.output_item.done")
            .map(|v| v["item"].clone())
            .collect::<Vec<_>>();
        assert_eq!(done, vec![own_payload["item"].clone()], "lane {lane}");
        let completed = events
            .iter()
            .find(|v| v["type"] == "response.completed")
            .unwrap();
        assert_eq!(
            completed["response"]["output"],
            json!([own_payload["item"]])
        );
        assert_eq!(completed["response"]["id"], format!("resp_{predecessor}"));
    }
    assert_eq!(
        delivery_statuses(&state, &[old.id, own.id, future.id]).await,
        vec!["claimed", "acked", "pending"]
    );
    let generations: Vec<i64> = sqlx::query_scalar(
        "select delivery_generation from runtime_events where id=any($1) order by sequence",
    )
    .bind(vec![old.id, own.id, future.id])
    .fetch_all(state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        generations,
        vec![1, 1, 0],
        "foreign events must not be claimed then released"
    );
}

#[tokio::test]
async fn scope_identity_is_explicit_and_human_wait_closes_initial_delivery_interval() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    assert_eq!(
        selected_delivery_scope(&run),
        RuntimeEventDeliveryScope::Unresolved
    );
    let first =
        seed_pending_committed_delivery(&state, &run, committed_delivery_payload("first")).await;
    seed_scope_boundary(&state, &run, None).await;
    let later =
        seed_pending_committed_delivery(&state, &run, committed_delivery_payload("later")).await;
    let run_id = run.id;
    let input = |scope| ClaimRuntimeEventDeliveriesInput {
        include_membership: true,
        flow_run_id: run_id,
        scope,
        limit: 128,
        lease_seconds: 30,
    };
    let unresolved = state
        .store
        .claim_runtime_event_deliveries(&input(RuntimeEventDeliveryScope::Unresolved))
        .await;
    assert!(unresolved.is_err());
    let missing = state
        .store
        .claim_runtime_event_deliveries(&input(RuntimeEventDeliveryScope::Resume {
            callback_task_id: Uuid::now_v7(),
        }))
        .await;
    assert!(
        missing.is_err(),
        "missing legacy anchor cannot turn into Start"
    );
    let unresolved_projection = project_round(
        &state,
        &run,
        RuntimeEventDeliveryScope::Unresolved,
        3,
        &[first.payload.clone(), later.payload.clone()],
    )
    .await;
    assert!(
        !unresolved_projection
            .iter()
            .any(|v| v["type"] == "response.output_item.done"),
        "generic attach without identity cannot bypass ownership through claim-less replay"
    );
    assert_protocol_failure(
        &unresolved_projection,
        "runtime_event_delivery_scope_unresolved",
    );
    run.metadata["response_round_id"] = json!(Uuid::now_v7());
    let missing_projection = project_round(
        &state,
        &run,
        selected_delivery_scope(&run),
        3,
        &[first.payload.clone()],
    )
    .await;
    assert_protocol_failure(
        &missing_projection,
        "runtime_event_delivery_scope_unresolved",
    );
    assert_eq!(
        delivery_statuses(&state, &[first.id, later.id]).await,
        vec!["pending", "pending"]
    );
    freeze_delivery_scope(&mut run, RuntimeEventDeliveryScope::Start);
    assert_eq!(
        selected_delivery_scope(&run),
        RuntimeEventDeliveryScope::Start
    );
    let batch = state
        .store
        .claim_runtime_event_deliveries(&input(selected_delivery_scope(&run)))
        .await
        .unwrap();
    assert_eq!(
        batch.events.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![first.id]
    );
    assert_eq!(
        batch.claims.iter().map(|c| c.event.id).collect::<Vec<_>>(),
        vec![first.id]
    );
    assert_eq!(
        delivery_statuses(&state, &[later.id]).await,
        vec!["pending"]
    );
}

pub(super) fn assert_protocol_failure(events: &[Value], code: &str) {
    assert!(
        !events.iter().any(|event| matches!(
            event["type"].as_str(),
            Some("response.completed" | "response.incomplete")
        )),
        "no successful/waiting terminal may hide a proof failure: {events:?}"
    );
    let failed = events
        .iter()
        .find(|event| event["type"] == "response.failed")
        .expect("stable failed terminal");
    assert_eq!(failed["response"]["error"]["code"], code);
}
