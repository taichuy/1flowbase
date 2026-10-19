use super::super::protocol_mappers::{
    anthropic_delta_payload, openai_delta_chunk_payload, openai_finish_chunk_payload,
    openai_response_function_call_output_items, openai_tool_call_chunk_payload,
    AnthropicStreamMapper, OpenAiChatStreamMapper, OpenAiResponseStreamMapper,
};
use super::super::*;
use super::support::*;
use crate::{
    app_state::ApiState,
    host_infrastructure::LocalRuntimeEventStream,
    routes::application_public_api::{
        sse::{
            send_native_runtime_event_stream_with_dependencies, IncludeWorkflowEvents,
            NativeRunSseDependencies,
        },
        stream_terminal_fallback::NativeRunTerminalDependencies,
    },
};
use axum::response::IntoResponse;
use control_plane::{
    application_public_api::native::{AnswerProjectionSegment, NativeError, NativeRequiredAction},
    ports::{
        CreateNodeRunInput, OrchestrationRuntimeRepository, RuntimeEventCloseReason,
        RuntimeEventDurability, RuntimeEventPayload, RuntimeEventSource, RuntimeEventStream,
        RuntimeEventStreamPolicy, UpdateFlowRunInput,
    },
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
use uuid::Uuid;

fn native_run_sse_dependencies(state: &ApiState) -> NativeRunSseDependencies {
    NativeRunSseDependencies::new(
        state.runtime_event_stream.clone(),
        NativeRunTerminalDependencies::new(
            state.store.clone(),
            state.runtime_engine.clone(),
            state.provider_runtime.clone(),
            state.provider_secret_master_key.clone(),
            state.model_billing_require_provider_usage,
            state.infrastructure.provider_transport_store(),
            state.runtime_event_stream.clone(),
        ),
    )
}

async fn seed_pending_committed_delivery(
    state: &ApiState,
    run: &NativeRunResult,
    payload: serde_json::Value,
) -> domain::RuntimeEventRecord {
    use control_plane::ports::AppendRuntimeEventInput;
    let record = state
        .store
        .append_runtime_event(&AppendRuntimeEventInput {
            flow_run_id: run.id,
            node_run_id: None,
            span_id: None,
            parent_span_id: None,
            event_type: "provider_output_item_done".to_string(),
            layer: domain::RuntimeEventLayer::ProviderRaw,
            source: domain::RuntimeEventSource::ProviderPlugin,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload,
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
        })
        .await
        .unwrap();
    sqlx::query("update runtime_events set delivery_status = 'pending' where id = $1")
        .bind(record.id)
        .execute(state.store.pool())
        .await
        .unwrap();
    record
}

fn committed_delivery_payload(call_id: &str) -> serde_json::Value {
    json!({
        "committed_delivery": true,
        "item": {"type":"function_call","call_id":call_id,"name":call_id}
    })
}

fn committed_delivery_envelope(
    run_id: Uuid,
    sequence: i64,
    payload: serde_json::Value,
) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run_id,
        sequence,
        RuntimeEventPayload {
            event_type: "provider_output_item_done".to_string(),
            source: RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: false,
            trace_visible: true,
            payload,
        },
    )
}

async fn complete_attached_callback_round(
    state: &ApiState,
    run_id: Uuid,
    first_item: &serde_json::Value,
    second_item: &serde_json::Value,
) {
    append_attached_callback_round_events(state, run_id, first_item, second_item).await;
    seal_attached_callback_round(state, run_id).await;
}

async fn append_attached_callback_round_events(
    state: &ApiState,
    run_id: Uuid,
    first_item: &serde_json::Value,
    second_item: &serde_json::Value,
) {
    for (index, item) in [(1, second_item), (0, first_item)] {
        append_compat_sse_runtime_event(
            state,
            run_id,
            "provider_output_item_done",
            json!({"output_index": index, "item": item}),
        )
        .await;
    }
    append_compat_sse_runtime_event(
        state,
        run_id,
        "flow_finished",
        json!({"type": "flow_finished", "run_id": run_id, "status": "succeeded"}),
    )
    .await;
}

async fn seal_attached_callback_round(state: &ApiState, run_id: Uuid) {
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run_id,
            status: domain::FlowRunStatus::Succeeded,
            output_payload: json!({}),
            error_payload: None,
            finished_at: Some(time::OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
}

fn spawn_typed_stream(
    state: &Arc<ApiState>,
    run: &NativeRunResult,
    replay: Vec<RuntimeEventEnvelope>,
    live_events: tokio::sync::mpsc::UnboundedReceiver<RuntimeEventEnvelope>,
    sender: mpsc::Sender<CompatibleProjectionInput>,
) -> tokio::task::JoinHandle<()> {
    spawn_typed_stream_with_durable(state, run, Vec::new(), replay, live_events, sender)
}

fn spawn_typed_stream_with_durable(
    state: &Arc<ApiState>,
    run: &NativeRunResult,
    durable_round_replay: Vec<RuntimeEventEnvelope>,
    replay: Vec<RuntimeEventEnvelope>,
    live_events: tokio::sync::mpsc::UnboundedReceiver<RuntimeEventEnvelope>,
    sender: mpsc::Sender<CompatibleProjectionInput>,
) -> tokio::task::JoinHandle<()> {
    use control_plane::ports::RuntimeEventSubscription;
    let (closure_sender, closure) = tokio::sync::watch::channel(None);
    let state = Arc::clone(state);
    let run = run.clone();
    tokio::spawn(async move {
        let _closure_sender = closure_sender;
        send_subscribed_compatible_typed_event_stream(SubscribedCompatibleTypedEventStream {
            terminal_dependencies: NativeRunTerminalDependencies::new(
                state.store.clone(),
                state.runtime_engine.clone(),
                state.provider_runtime.clone(),
                state.provider_secret_master_key.clone(),
                state.model_billing_require_provider_usage,
                state.infrastructure.provider_transport_store(),
                state.runtime_event_stream.clone(),
            ),
            initial_run: run.clone(),
            from_sequence: None,
            durable_round_replay,
            ignored_waiting_callback_task_id: None,
            subscription: RuntimeEventSubscription {
                terminal_writer: Arc::new(UnusedTerminalWriter),
                replay,
                live_events: control_plane::ports::RuntimeEventReceiver::from_unbounded(
                    live_events,
                ),
                closure,
            },
            sender,
        })
        .await
    })
}

async fn delivery_statuses(state: &ApiState, ids: &[Uuid]) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "select delivery_status from runtime_events where id = any($1) order by sequence",
    )
    .bind(ids.to_vec())
    .fetch_all(state.store.pool())
    .await
    .unwrap()
}

async fn claimable_delivery_count(state: &ApiState, run_id: Uuid) -> usize {
    state
        .store
        .claim_runtime_event_deliveries(&control_plane::ports::ClaimRuntimeEventDeliveriesInput {
            flow_run_id: run_id,
            limit: 8,
            lease_seconds: 30,
        })
        .await
        .unwrap()
        .len()
}

// AC-002: the durable ACK is settled by the protocol writer's receipt, not
// by the internal enqueue. A projected receipt ACKs; a receipt dropped before
// any write releases the claim; a receipt dropped mid-write is uncertain and
// never replays automatically.
#[tokio::test]
async fn typed_responses_stream_claims_orders_deduplicates_and_acks_projected_tool_deliveries() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let payloads = [
        committed_delivery_payload("call-1"),
        committed_delivery_payload("call-2"),
    ];
    let mut records = Vec::new();
    for payload in &payloads {
        records.push(seed_pending_committed_delivery(&state, &run, payload.clone()).await);
    }
    let replay = vec![committed_delivery_envelope(run.id, 1, payloads[0].clone())];
    let (live_sender, live_events) = tokio::sync::mpsc::unbounded_channel();
    drop(live_sender);
    let (sender, mut receiver) = mpsc::channel(8);
    let forwarding = spawn_typed_stream(&state, &run, replay, live_events, sender);
    let mut forwarded = Vec::new();
    while let Some(input) = receiver.recv().await {
        let (_, envelope, receipt) = input.into_parts();
        let receipt = receipt.expect("committed delivery carries a receipt");
        assert_eq!(receipt.event_id(), Some(records[forwarded.len()].id));
        receipt.projected();
        forwarded.push(envelope);
    }
    forwarding.await.unwrap();
    assert_eq!(forwarded.len(), 2);
    assert_eq!(forwarded[0].payload, payloads[0]);
    assert_eq!(forwarded[1].payload, payloads[1]);

    let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    assert_eq!(
        delivery_statuses(&state, &ids).await,
        vec!["acked", "acked"]
    );
    assert_eq!(
        claimable_delivery_count(&state, run.id).await,
        0,
        "acked deliveries must not replay"
    );
}

#[tokio::test]
async fn durable_round_replay_keeps_exact_native_item_and_does_not_replay_acked_tool() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    run.metadata["response_round_id"] = json!(Uuid::now_v7());
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let reasoning = json!({
        "id": "rs_provider_original",
        "type": "reasoning",
        "summary": [{"type":"summary_text","text":"exact summary"}],
        "encrypted_content": "opaque-provider-bytes"
    });
    let mut tool = committed_delivery_payload("call-replay-once");
    tool["output_index"] = json!(1);
    let record = seed_pending_committed_delivery(&state, &run, tool.clone()).await;
    let durable_replay = vec![
        RuntimeEventEnvelope::new(run.id, 100, debug_stream_events::flow_started(run.id)),
        RuntimeEventEnvelope::new(
            run.id,
            101,
            debug_stream_events::provider_output_item_done(
                "llm",
                Uuid::now_v7(),
                0,
                reasoning.clone(),
            ),
        ),
        committed_delivery_envelope(run.id, 102, tool),
    ];
    for expected_tool_count in [1, 0] {
        let (live_sender, live_events) = tokio::sync::mpsc::unbounded_channel();
        drop(live_sender);
        let (sender, mut receiver) = mpsc::channel(8);
        let forwarding = spawn_typed_stream_with_durable(
            &state,
            &run,
            durable_replay.clone(),
            Vec::new(),
            live_events,
            sender,
        );
        let mut mapper = OpenAiResponseStreamMapper::with_mode(
            "fixture".into(),
            None,
            ResponsesProjectionMode::TransparentProviderResponses,
        );
        let mut projected = Vec::new();
        let mut tool_count = 0;
        while let Some(input) = receiver.recv().await {
            let (snapshot, envelope, receipt) = input.into_parts();
            if let Some(receipt) = receipt {
                tool_count += 1;
                receipt.projected();
            }
            projected.extend(mapper.runtime_event_to_sse(&snapshot, envelope));
        }
        forwarding.await.unwrap();
        assert_eq!(tool_count, expected_tool_count);
        let response = axum::response::sse::Sse::new(tokio_stream::iter(projected)).into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("rs_provider_original"), "{body}");
        assert!(body.contains("opaque-provider-bytes"), "{body}");
        assert_eq!(body.contains("call-replay-once"), expected_tool_count == 1);
    }
    assert_eq!(delivery_statuses(&state, &[record.id]).await, vec!["acked"]);
}

#[tokio::test]
async fn durable_callback_boundary_excludes_the_next_response_round() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let node_run = state
        .store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run.id,
            node_id: "llm".into(),
            node_type: "llm".into(),
            node_alias: "LLM".into(),
            status: domain::NodeRunStatus::Running,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let first_callback = Uuid::now_v7();
    let next_callback = Uuid::now_v7();
    let waiting = |callback_task_id| RuntimeEventPayload {
        event_type: "waiting_callback".into(),
        source: RuntimeEventSource::Runtime,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: true,
        payload: json!({"callback_task_id":callback_task_id}),
    };
    let original = json!({
        "id":"rs_original",
        "type":"reasoning",
        "encrypted_content":"opaque-original"
    });
    let tool = json!({
        "id":"call_original",
        "type":"custom_tool_call",
        "call_id":"call_original"
    });
    let later = json!({"id":"rs_later","type":"reasoning"});
    for (sequence, payload) in [
        waiting(first_callback),
        debug_stream_events::flow_started(run.id),
        debug_stream_events::provider_output_item_done("llm", node_run.id, 1, original.clone()),
        debug_stream_events::provider_output_item_done("llm", node_run.id, 0, tool.clone()),
        waiting(next_callback),
        debug_stream_events::flow_started(run.id),
        debug_stream_events::provider_output_item_done("llm", node_run.id, 0, later),
    ]
    .into_iter()
    .enumerate()
    {
        control_plane::orchestration_runtime::persist_runtime_debug_stream_events(
            &state.store,
            vec![RuntimeEventEnvelope::new(
                run.id,
                sequence as i64 + 1,
                payload,
            )],
        )
        .await
        .unwrap();
    }
    let boundary = state
        .store
        .get_runtime_event_sequence_for_callback_task(run.id, first_callback)
        .await
        .unwrap()
        .unwrap();
    let prefix = event_forwarding::durable_round_prefix(
        state
            .store
            .list_runtime_events(run.id, boundary)
            .await
            .unwrap(),
    );
    let items = prefix
        .iter()
        .filter(|event| event.event_type == "provider_output_item_done")
        .map(|event| event.payload["item"].clone())
        .collect::<Vec<_>>();
    assert_eq!(items, vec![tool, original]);
    assert_eq!(
        prefix.last().unwrap().payload["callback_task_id"],
        json!(next_callback)
    );
}

#[tokio::test]
async fn typed_responses_stream_releases_a_tool_delivery_dropped_before_any_write() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let record =
        seed_pending_committed_delivery(&state, &run, committed_delivery_payload("call-drop"))
            .await;
    let (live_sender, live_events) = tokio::sync::mpsc::unbounded_channel();
    drop(live_sender);
    let (sender, mut receiver) = mpsc::channel(8);
    let forwarding = spawn_typed_stream(&state, &run, Vec::new(), live_events, sender);
    let input = receiver
        .recv()
        .await
        .expect("pending delivery is forwarded");
    let (_, _, receipt) = input.into_parts();
    drop(receipt);
    assert!(receiver.recv().await.is_none());
    forwarding.await.unwrap();

    assert_eq!(
        delivery_statuses(&state, &[record.id]).await,
        vec!["pending"]
    );
    assert_eq!(
        claimable_delivery_count(&state, run.id).await,
        1,
        "an unprojected delivery replays on the next stream"
    );
}

#[tokio::test]
async fn typed_responses_stream_marks_a_tool_delivery_lost_mid_write_uncertain() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let record =
        seed_pending_committed_delivery(&state, &run, committed_delivery_payload("call-uncertain"))
            .await;
    let (live_sender, live_events) = tokio::sync::mpsc::unbounded_channel();
    drop(live_sender);
    let (sender, mut receiver) = mpsc::channel(8);
    let forwarding = spawn_typed_stream(&state, &run, Vec::new(), live_events, sender);
    let input = receiver
        .recv()
        .await
        .expect("pending delivery is forwarded");
    let (_, _, receipt) = input.into_parts();
    let mut receipt = receipt.expect("committed delivery carries a receipt");
    receipt.begin_write();
    drop(receipt);
    assert!(receiver.recv().await.is_none());
    forwarding.await.unwrap();

    assert_eq!(
        delivery_statuses(&state, &[record.id]).await,
        vec!["uncertain"]
    );
    assert_eq!(
        claimable_delivery_count(&state, run.id).await,
        0,
        "an uncertain delivery must not replay automatically"
    );
}

#[tokio::test]
async fn typed_responses_stream_claims_and_acks_a_live_committed_tool_delivery() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    let payload = committed_delivery_payload("call-live");
    let (live_sender, live_events) = tokio::sync::mpsc::unbounded_channel();
    let (sender, mut receiver) = mpsc::channel(8);
    let forwarding = spawn_typed_stream(&state, &run, Vec::new(), live_events, sender);
    tokio::task::yield_now().await;

    let record = seed_pending_committed_delivery(&state, &run, payload.clone()).await;
    live_sender
        .send(committed_delivery_envelope(
            run.id,
            record.sequence,
            payload.clone(),
        ))
        .unwrap();
    drop(live_sender);

    let forwarded = receiver.recv().await.expect("live committed delivery");
    let (_, envelope, receipt) = forwarded.into_parts();
    assert_eq!(envelope.payload, payload);
    receipt
        .expect("live committed delivery carries a receipt")
        .projected();
    forwarding.await.unwrap();
    assert_eq!(delivery_statuses(&state, &[record.id]).await, vec!["acked"]);
}

async fn native_sse_body_from_replay(
    base_state: &ApiState,
    run: NativeRunResult,
    replay: Vec<RuntimeEventEnvelope>,
    from_sequence: Option<i64>,
) -> String {
    let runtime_event_stream = Arc::new(
        ReplayBeforeFallbackRuntimeEventStream::with_closed_subscription_replay(
            replay,
            Vec::new(),
            RuntimeEventCloseReason::Incomplete,
        ),
    );
    let mut state = (*base_state).clone();
    state.runtime_event_stream = runtime_event_stream;
    let (sender, mut receiver) = mpsc::channel(32);

    tokio::time::timeout(
        Duration::from_secs(2),
        send_native_runtime_event_stream_with_dependencies(
            native_run_sse_dependencies(&state),
            run,
            IncludeWorkflowEvents::None,
            from_sequence,
            None,
            sender,
        ),
    )
    .await
    .expect("Native SSE replay should stop at an incomplete terminal");

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let response = axum::response::sse::Sse::new(tokio_stream::iter(events)).into_response();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Native SSE response body should be readable");
    String::from_utf8(body.to_vec()).expect("Native SSE response body should be UTF-8")
}

async fn running_run_with_closed_stream() -> (NativeRunResult, Arc<ApiState>) {
    let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&base_state, &run).await;
    base_state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: domain::FlowRunStatus::Running,
            output_payload: json!({
                "answer": "partial output",
                "__canonical_answer_presentation": true
            }),
            error_payload: None,
            finished_at: None,
        })
        .await
        .expect("seed a published running run");

    let runtime_event_stream = Arc::new(LocalRuntimeEventStream::new());
    runtime_event_stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .expect("open the live stream");
    runtime_event_stream
        .close_run(run.id, RuntimeEventCloseReason::Failed)
        .await
        .expect("close the live stream without a terminal");
    let mut state = (*base_state).clone();
    state.runtime_event_stream = runtime_event_stream;
    (run, Arc::new(state))
}

async fn sse_body(
    events: Vec<Result<axum::response::sse::Event, std::convert::Infallible>>,
) -> String {
    let response = axum::response::sse::Sse::new(tokio_stream::iter(events)).into_response();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("SSE response body should be readable");
    String::from_utf8(body.to_vec()).expect("SSE response body should be UTF-8")
}

#[test]
fn compatible_runtime_event_view_classifies_answer_and_terminal_semantics_once() {
    let run = native_run();
    let delta = CompatibleRuntimeEventView::from(RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "partial".to_string(),
            0,
            Some("node-llm"),
            Some(Uuid::now_v7()),
            Some("text"),
        ),
    ));
    let terminal = CompatibleRuntimeEventView::from(RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::flow_finished(run.id, json!({})),
    ));

    assert_eq!(delta.answer_delta(), Some(CompatibleAnswerDeltaKind::Text));
    assert_eq!(delta.terminal(), None);
    assert_eq!(terminal.answer_delta(), None);
    assert_eq!(terminal.terminal(), Some(CompatibleTerminalKind::Finished));
}

#[tokio::test]
async fn ac_011_client_disconnect_bounds_projection_without_failing_the_durable_run() {
    let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let run = native_run();
    seed_flow_run_for_compat_sse_test(&base_state, &run).await;
    base_state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: domain::FlowRunStatus::Running,
            output_payload: json!({}),
            error_payload: None,
            finished_at: None,
        })
        .await
        .expect("seed an accepted durable run");

    let runtime_event_stream = Arc::new(
        ReplayBeforeFallbackRuntimeEventStream::with_subscription_replay(Vec::new(), Vec::new()),
    );
    let mut state = (*base_state).clone();
    state.runtime_event_stream = runtime_event_stream.clone();
    let state = Arc::new(state);
    let (sender, receiver) = mpsc::channel(1);
    drop(receiver);

    let projection = tokio::spawn(send_compatible_runtime_event_stream(
        state.clone(),
        run.clone(),
        OPENAI_CHAT_SSE_PROJECTION,
        None,
        None,
        sender,
        |_winner, _envelope| vec![Ok(axum::response::sse::Event::default().data("delta"))],
    ));
    tokio::time::timeout(Duration::from_secs(2), async {
        while runtime_event_stream.active_subscriber_count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("projection should establish its runtime subscription");
    runtime_event_stream.emit_live(RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "delta".to_string(),
            0,
            Some("node-llm"),
            Some(Uuid::now_v7()),
            Some("text"),
        ),
    ));

    tokio::time::timeout(Duration::from_secs(2), projection)
        .await
        .expect("client disconnect should bound the projection task")
        .expect("projection task should not panic");
    assert_eq!(runtime_event_stream.active_subscriber_count(), 0);
    let durable = state
        .store
        .get_flow_run(run.application_id, run.id)
        .await
        .expect("read accepted durable run")
        .expect("accepted durable run should remain present");
    assert_eq!(durable.status, domain::FlowRunStatus::Running);
    assert!(durable.error_payload.is_none());
}

#[tokio::test]
async fn d2_ac_008_native_eof_fallback_finalizes_running_winner_before_projection() {
    let (run, state) = running_run_with_closed_stream().await;
    let (sender, mut receiver) = mpsc::channel(32);

    send_native_runtime_event_stream_with_dependencies(
        native_run_sse_dependencies(state.as_ref()),
        run.clone(),
        IncludeWorkflowEvents::None,
        None,
        None,
        sender,
    )
    .await;

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    let body = sse_body(events).await;
    let recovered = state
        .store
        .get_flow_run(run.application_id, run.id)
        .await
        .expect("read durable run")
        .expect("seeded run remains present");
    let runtime_events = state
        .store
        .list_runtime_events(run.id, 0)
        .await
        .expect("read durable runtime events");

    assert_eq!(recovered.status, domain::FlowRunStatus::Failed);
    assert_eq!(
        recovered.error_payload,
        Some(json!({
            "code": "stream_terminal_missing",
            "message": "runtime event stream ended without a terminal event"
        }))
    );
    assert_eq!(
        runtime_events
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1,
        "EOF recovery must write one durable canonical failure"
    );
    assert!(body.contains("event: run.failed"), "{body}");
    assert!(body.contains("partial output"), "{body}");
    assert!(!body.contains("event: run.completed"), "{body}");
}

#[tokio::test]
async fn d2_ac_008_compatible_eof_fallback_projects_recovered_failed_winner() {
    let (run, state) = running_run_with_closed_stream().await;
    let (sender, _receiver) = mpsc::channel(32);
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mapper_seen = seen.clone();

    send_compatible_runtime_event_stream(
        state.clone(),
        run.clone(),
        OPENAI_CHAT_SSE_PROJECTION,
        None,
        None,
        sender,
        move |winner, envelope| {
            mapper_seen
                .lock()
                .expect("mapper observation lock should be available")
                .push((format!("{:?}", winner.status), envelope.event_type));
            Vec::new()
        },
    )
    .await;

    let recovered = state
        .store
        .get_flow_run(run.application_id, run.id)
        .await
        .expect("read durable run")
        .expect("seeded run remains present");
    let runtime_events = state
        .store
        .list_runtime_events(run.id, 0)
        .await
        .expect("read durable runtime events");

    assert_eq!(recovered.status, domain::FlowRunStatus::Failed);
    assert_eq!(
        runtime_events
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1,
        "EOF recovery must write one durable canonical failure"
    );
    assert!(
        seen.lock()
            .expect("mapper observation lock should be available")
            .iter()
            .any(|(status, event_type)| status == "Failed" && event_type == "flow_failed"),
        "compatible mapper must receive the reloaded durable failed winner"
    );
    let seen = seen
        .lock()
        .expect("mapper observation lock should be available");
    let partial_position = seen
        .iter()
        .position(|(_, event_type)| event_type == "text_delta")
        .expect("canonical partial should be recovered before failure");
    let terminal_position = seen
        .iter()
        .position(|(_, event_type)| event_type == "flow_failed")
        .expect("failed terminal should be projected");
    assert!(partial_position < terminal_position);
}

#[test]
fn equal_answer_chunks_are_ordered_facts_and_are_all_claimed() {
    let run = native_run();
    let node_run_id = Uuid::from_u128(0x55555555555555555555555555555555);
    let first = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::answer_text_delta(
            "node-answer",
            " ".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    let second = RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::answer_text_delta(
            "node-answer",
            " ".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    let mut durable = RuntimeEventEnvelope::new(
        run.id,
        3,
        debug_stream_events::answer_text_delta(
            "node-answer",
            " ".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    durable.payload["event_ids"] = json!([format!("{}:1", run.id), format!("{}:2", run.id)]);
    let mut stats = CompatibleStreamStats::default();

    assert!(stats.claim_runtime_event(&first));
    assert!(stats.claim_runtime_event(&second));
    assert!(stats.claim_runtime_event(&durable));
}

#[test]
fn live_answer_chunks_are_not_treated_as_cumulative_snapshots() {
    let run = native_run();
    let node_run_id = Uuid::from_u128(0x55555555555555555555555555555555);
    let first = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "a".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    let second = RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "ab".to_string(),
            0,
            Some("node-llm"),
            Some(node_run_id),
            Some("text"),
        ),
    );
    let mut stats = CompatibleStreamStats::default();

    assert!(stats.claim_runtime_event(&first));
    assert!(stats.claim_runtime_event(&second));
    assert_eq!(second.text.as_deref(), Some("ab"));
}

#[test]
fn typed_turn_ordering_preserves_equal_text_and_filters_only_cursor_history() {
    let run = native_run();
    let node_run_id = Uuid::from_u128(0x77777777777777777777777777777777);
    let event = |sequence| {
        RuntimeEventEnvelope::new(
            run.id,
            sequence,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "same  文本".to_string(),
                0,
                Some("node-llm"),
                Some(node_run_id),
                Some("text"),
            ),
        )
    };
    let mut cursor = 0;

    let first = take_ordered_compatible_event(event(1), &mut cursor, None)
        .expect("first ordered fact must pass");
    let second = take_ordered_compatible_event(event(2), &mut cursor, None)
        .expect("equal text at a later sequence remains a distinct fact");
    assert_eq!(first.text, second.text);
    assert_eq!(cursor, 2);
    assert!(take_ordered_compatible_event(event(2), &mut cursor, None).is_none());
    assert!(take_ordered_compatible_event(event(1), &mut cursor, None).is_none());
}

#[test]
fn typed_projection_input_keeps_runtime_identity_and_canonical_envelope() {
    let run = native_run();
    let envelope = RuntimeEventEnvelope::new(
        run.id,
        9,
        debug_stream_events::flow_finished(run.id, json!({ "answer": "done" })),
    );
    let input = CompatibleProjectionInput {
        run_snapshot: Arc::new(run.clone()),
        envelope: envelope.clone(),
        delivery: None,
    };
    let (run_snapshot, projected_envelope, delivery) = input.into_parts();
    assert!(delivery.is_none());

    assert_eq!(run_snapshot.id, run.id);
    assert_eq!(projected_envelope, envelope);
    assert_eq!(projected_envelope.event_id, format!("{}:9", run.id));
}

#[tokio::test]
async fn d1_ac_007_native_sse_initial_and_durable_replay_keep_incomplete_distinct_from_completed() {
    let (base_state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut run = native_run();
    run.status = NativeRunStatus::Incomplete;
    run.answer = Some("partial output at the limit".to_string());
    seed_flow_run_for_compat_sse_test(&base_state, &run).await;
    base_state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: domain::FlowRunStatus::Incomplete,
            output_payload: json!({
                "answer": "partial output at the limit",
                "__canonical_answer_presentation": true
            }),
            error_payload: None,
            finished_at: Some(time::OffsetDateTime::now_utc()),
        })
        .await
        .expect("seed the durable incomplete winner before terminal projection");

    let initial_stream = native_sse_body_from_replay(
        &base_state,
        run.clone(),
        vec![
            RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
            RuntimeEventEnvelope::new(
                run.id,
                2,
                debug_stream_events::flow_incomplete(
                    run.id,
                    json!({ "answer": "partial output at the limit" }),
                ),
            ),
        ],
        None,
    )
    .await;
    let replay_stream = native_sse_body_from_replay(
        &base_state,
        run.clone(),
        vec![RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::flow_incomplete(
                run.id,
                json!({ "answer": "partial output at the limit" }),
            ),
        )],
        Some(1),
    )
    .await;

    for body in [initial_stream, replay_stream] {
        assert_eq!(body.matches("event: run.incomplete").count(), 1, "{body}");
        assert!(body.contains("\"status\":\"incomplete\""), "{body}");
        assert!(body.contains("partial output at the limit"), "{body}");
        assert!(!body.contains("event: run.completed"), "{body}");
        assert!(!body.contains("\"status\":\"succeeded\""), "{body}");
    }
}

#[test]
fn openai_delta_chunk_maps_reasoning_to_reasoning_content() {
    let chat_completion_id = "chatcmpl-test";
    let payload = openai_delta_chunk_payload(
        &native_run(),
        "deepseek-v4-pro",
        chat_completion_id,
        "reasoning_delta",
        "先分析用户问题".to_string(),
    )
    .expect("reasoning delta should map to an OpenAI-compatible chunk");

    assert_eq!(payload["id"], json!(chat_completion_id));
    assert_eq!(
        payload["choices"][0]["delta"]["reasoning_content"],
        json!("先分析用户问题")
    );
    assert_eq!(payload["choices"][0]["delta"].get("content"), None);
}

#[tokio::test]
async fn openai_terminal_does_not_reconstruct_structured_answer_segments() {
    let mut run = native_run();
    run.answer = Some("<think>旧思考</think>旧回答".to_string());
    run.answer_segments = Some(vec![
        AnswerProjectionSegment::reasoning("结构化思考"),
        AnswerProjectionSegment::message("结构化回答"),
    ]);
    let mut mapper =
        OpenAiChatStreamMapper::new("deepseek-v4-pro".to_string(), "chatcmpl-test".to_string());

    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({})),
        ),
    );
    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(!body.contains("结构化思考"), "{body}");
    assert!(!body.contains("结构化回答"), "{body}");
    assert!(!body.contains("旧思考"), "{body}");
    assert!(!body.contains("旧回答"), "{body}");
}

#[test]
fn openai_waiting_callback_maps_to_tool_call_chunk() {
    let callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let chat_completion_id = "chatcmpl-tool-test";
    let payload = openai_tool_call_chunk_payload(
        &native_run(),
        "1flowbase",
        chat_completion_id,
        &json!({
            "callback_kind": "llm_tool_calls",
            "callback_task_id": callback_task_id,
            "tool_calls": [
                {
                    "id": "call_weather",
                    "name": "lookup_weather",
                    "arguments": {"city": "Hangzhou"}
                }
            ]
        }),
    )
    .expect("LLM callback should map to OpenAI tool call chunk");

    assert_eq!(payload["id"], json!(chat_completion_id));
    assert_eq!(
        payload["choices"][0]["delta"]["tool_calls"][0]["function"]["name"],
        json!("lookup_weather")
    );
    assert_eq!(
        payload["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
        json!("{\"city\":\"Hangzhou\"}")
    );
    let call_id = payload["choices"][0]["delta"]["tool_calls"][0]["id"]
        .as_str()
        .expect("tool call id should be encoded");
    assert!(call_id.contains("call_weather"));
}

#[test]
fn openai_chat_completion_id_changes_for_callback_resume() {
    let run_id = Uuid::from_u128(0x11111111111111111111111111111111);
    let callback_task_id = Uuid::from_u128(0x22222222222222222222222222222222);

    assert_ne!(
        openai_chat_completion_id_from_run_id(run_id),
        openai_chat_completion_id_from_callback_task(run_id, callback_task_id)
    );
}

#[test]
fn openai_responses_waiting_callback_maps_to_function_call_item() {
    let callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let output = openai_response_function_call_output_items(&json!({
        "callback_kind": "llm_tool_calls",
        "callback_task_id": callback_task_id,
        "tool_calls": [
            {
                "id": "call_inventory",
                "name": "lookup_inventory",
                "arguments": {"sku": "sku_123"}
            }
        ]
    }))
    .expect("LLM callback should map to Responses function_call output");

    assert_eq!(output[0]["type"], json!("function_call"));
    assert_eq!(output[0]["name"], json!("lookup_inventory"));
    assert_eq!(output[0]["arguments"], json!("{\"sku\":\"sku_123\"}"));
    assert_eq!(output[0]["call_id"], "call_inventory");
}

#[test]
fn openai_finish_chunk_uses_deepseek_compatible_terminal_shape() {
    let payload = openai_finish_chunk_payload(&native_run(), "1flowbase", "chatcmpl-test", "stop");

    assert_eq!(payload["choices"][0]["delta"]["content"], json!(""));
    assert_eq!(payload["choices"][0]["delta"]["role"], Value::Null);
    assert_eq!(payload["choices"][0]["finish_reason"], json!("stop"));
    assert_eq!(payload["usage"]["prompt_tokens"], json!(0));
    assert_eq!(payload["usage"]["completion_tokens"], json!(0));
    assert_eq!(payload["usage"]["total_tokens"], json!(0));
}

#[test]
fn openai_chat_terminal_emits_only_finish_without_live_content() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
        ),
    );

    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn openai_chat_terminal_does_not_reconstruct_thinking_or_text() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());

    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({ "answer": "<think>先分析</think>\n最终回答" }),
            ),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(!body.contains("先分析"), "{body}");
    assert!(!body.contains("最终回答"), "{body}");
    assert!(!body.contains("<think>"), "{body}");
    assert!(body.contains("[DONE]"), "{body}");
}

#[tokio::test]
async fn openai_responses_terminal_does_not_reconstruct_thinking_or_text() {
    let run = native_run();
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({ "answer": "<think>先分析</think>\n最终回答" }),
            ),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(!body.contains("response.reasoning_text.delta"), "{body}");
    assert!(!body.contains("response.output_text.delta"), "{body}");
    assert!(!body.contains("先分析"), "{body}");
    assert!(!body.contains("最终回答"), "{body}");
    assert!(!body.contains("<think>"), "{body}");
    assert!(body.contains("event: response.completed"), "{body}");
}

#[tokio::test]
async fn openai_response_completed_event_includes_usage() {
    let mut run = native_run();
    run.usage = Some(NativeUsage {
        prompt_tokens: Some(11),
        completion_tokens: Some(7),
        total_tokens: Some(18),
        ..Default::default()
    });
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "Final answer" })),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: response.completed"), "{body}");
    assert!(body.contains("\"usage\""), "{body}");
    assert!(body.contains("\"input_tokens\":11"), "{body}");
    assert!(body.contains("\"output_tokens\":7"), "{body}");
    assert!(body.contains("\"total_tokens\":18"), "{body}");
}

#[tokio::test]
async fn openai_chat_terminal_does_not_decode_artifact_preview_answer() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({
                    "answer": {
                        "__runtime_debug_artifact": true,
                        "artifact_ref": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                        "is_truncated": false,
                        "preview": "\"最终回答\""
                    }
                }),
            ),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(!body.contains("最终回答"), "{body}");
    assert!(body.contains("\"finish_reason\":\"stop\""), "{body}");
    assert!(body.contains("[DONE]"), "{body}");
}

#[test]
fn openai_chat_terminal_ignores_provider_raw_delta_without_reconstruction() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
    let text_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::text_delta("node-llm", run.id, "已流式输出".to_string()),
        ),
    );
    let terminal_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
        ),
    );

    assert!(text_events.is_empty());
    assert_eq!(terminal_events.len(), 2);
}

#[test]
fn compatible_stream_stats_count_only_emitted_answer_deltas() {
    let run = native_run();
    let mut stats = CompatibleStreamStats::default();
    let answer_delta = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "已输出".to_string(),
            0,
            Some("node-llm"),
            None,
            Some("text"),
        ),
    );
    stats.record_sent_runtime_event(&run, &answer_delta, true);

    let terminal_event = RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
    );
    stats.record_sent_runtime_event(&run, &terminal_event, true);

    assert!(stats.emitted_content());
    assert_eq!(stats.emitted_content_bytes, "已输出".len());
}

#[test]
fn openai_chat_projects_answer_presentation_delta_not_provider_raw_delta() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
    let provider_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::text_delta(
                "node-llm",
                Uuid::from_u128(0x55555555555555555555555555555555),
                "provider raw".to_string(),
            ),
        ),
    );
    let presentation_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            RuntimeEventPayload {
                event_type: "text_delta".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: false,
                payload: json!({
                    "type": "text_delta",
                    "node_run_id": Uuid::from_u128(0x66666666666666666666666666666666),
                    "node_id": "node-answer",
                    "text": "answer presentation",
                    "presentation": { "kind": "answer" }
                }),
            },
        ),
    );

    assert!(provider_events.is_empty());
    assert_eq!(presentation_events.len(), 1);
}

#[test]
fn callback_diagnostic_tail_and_synthetic_start_do_not_establish_a_round() {
    let run_id = Uuid::now_v7();
    let round_id = Uuid::now_v7();
    let mut replay = vec![RuntimeEventEnvelope::new(
        run_id,
        0,
        debug_stream_events::flow_started(run_id),
    )];
    for sequence in [41, 45] {
        replay.push(RuntimeEventEnvelope::new(
            run_id,
            sequence,
            RuntimeEventPayload {
                event_type: "client_protocol_trajectory".into(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({"type": "client_protocol_trajectory"}),
            },
        ));
    }
    assert!(
        !callback_round_replay_established(&replay, round_id),
        "nonempty diagnostic replay must not suppress the real callback marker"
    );
    replay[0].payload["response_round_id"] = json!(Uuid::now_v7());
    assert!(
        !callback_round_replay_established(&replay, round_id),
        "a foreign round marker must not establish the requested callback"
    );
    replay[0].payload["response_round_id"] = json!(round_id);
    assert!(callback_round_replay_established(&replay, round_id));
}

#[test]
fn legacy_completed_callback_replay_remains_established_without_a_tagged_marker() {
    let run_id = Uuid::now_v7();
    let replay = vec![RuntimeEventEnvelope::new(
        run_id,
        7,
        debug_stream_events::flow_finished(run_id, json!({})),
    )];
    assert!(
        callback_round_replay_established(&replay, Uuid::now_v7()),
        "generation-filtered completed legacy rounds must stay replayable"
    );
}

#[path = "forwarding/_tests/live_delivery_batch.rs"]
mod live_delivery_batch;

#[path = "forwarding/_tests/terminal_generation.rs"]
mod terminal_generation;

#[path = "forwarding/_tests/snapshot_ownership.rs"]
mod snapshot_ownership;

#[path = "forwarding/_tests/cold_replay.rs"]
mod cold_replay;

#[path = "forwarding/_tests/reservation_lifecycle.rs"]
mod reservation_lifecycle;

#[path = "forwarding/_tests/callback_attach.rs"]
mod callback_attach;

#[path = "forwarding/_tests/anthropic_projection.rs"]
mod anthropic_projection;

#[path = "forwarding/_tests/executor_support.rs"]
mod executor_support;
use executor_support::RejectAttachExecutor;
