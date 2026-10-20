use super::*;

#[tokio::test]
async fn replay_gc_durable_event_pages_preserve_order_across_stream_cursor_resets() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-04-17 09:00:00 UTC),
    )
    .await;
    let mut committed = Vec::new();
    for local_sequence in [100_i64, 101, 1, 2, 3, 4] {
        let mut input = runtime_event_input(run.id, "node_checkpoint");
        input.payload = json!({"stream_sequence": local_sequence, "opaque": "00"});
        committed.push(
            OrchestrationRuntimeRepository::append_runtime_event(&store, &input)
                .await
                .unwrap(),
        );
    }
    let through = committed[3].sequence;
    let mut after = 0;
    let mut observed = Vec::new();
    loop {
        let page = OrchestrationRuntimeRepository::list_runtime_event_durable_page(
            &store,
            run.id,
            after,
            Some(through),
            2,
        )
        .await
        .unwrap();
        assert!(page.len() <= 2);
        if page.is_empty() {
            break;
        }
        for event in &page {
            assert!(event.sequence > after && event.sequence <= through);
            assert_eq!(event.payload["opaque"], "00");
        }
        after = page.last().unwrap().sequence;
        observed.extend(page.into_iter().map(|event| event.id));
    }
    assert_eq!(
        observed,
        committed[..4]
            .iter()
            .map(|event| event.id)
            .collect::<Vec<_>>()
    );
    let empty = OrchestrationRuntimeRepository::list_runtime_event_durable_page(
        &store,
        Uuid::now_v7(),
        0,
        None,
        2,
    )
    .await
    .unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn replay_gc_window_separates_reopened_generation_and_freezes_durable_tail() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-04-17 09:00:00 UTC),
    )
    .await;
    assert!(
        OrchestrationRuntimeRepository::get_runtime_event_replay_window(&store, run.id)
            .await
            .unwrap()
            .is_none()
    );
    let mut old = runtime_event_input(run.id, "flow_finished");
    old.payload = json!({"stream_sequence": 100});
    OrchestrationRuntimeRepository::append_runtime_event(&store, &old)
        .await
        .unwrap();
    let generation = Uuid::now_v7();
    let mut anchor = runtime_event_input(run.id, "runtime_stream_opened");
    anchor.payload = json!({"stream_generation_id": generation});
    let anchor = OrchestrationRuntimeRepository::append_runtime_event(&store, &anchor)
        .await
        .unwrap();
    assert!(
        !OrchestrationRuntimeRepository::has_runtime_event_terminal_after(
            &store,
            run.id,
            anchor.sequence,
            "flow_finished",
            generation,
        )
        .await
        .unwrap()
    );
    let mut item = runtime_event_input(run.id, "provider_output_item_done");
    item.payload =
        json!({"stream_sequence": 1, "stream_generation_id": generation, "opaque": "00"});
    let first = OrchestrationRuntimeRepository::append_runtime_event(&store, &item)
        .await
        .unwrap();
    let window = OrchestrationRuntimeRepository::get_runtime_event_replay_window(&store, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            window.after_sequence,
            window.through_sequence,
            window.generation_id
        ),
        (anchor.sequence, first.sequence, Some(generation))
    );
    let terminal = runtime_event_input(run.id, "flow_finished");
    OrchestrationRuntimeRepository::append_runtime_event(&store, &terminal)
        .await
        .unwrap();
    let frozen = OrchestrationRuntimeRepository::list_runtime_event_durable_page(
        &store,
        run.id,
        window.after_sequence,
        Some(window.through_sequence),
        1,
    )
    .await
    .unwrap();
    assert_eq!(frozen.len(), 1);
    assert_eq!(frozen[0].id, first.id);
    assert_eq!(frozen[0].payload["opaque"], "00");
    assert!(
        OrchestrationRuntimeRepository::has_runtime_event_terminal_after(
            &store,
            run.id,
            anchor.sequence,
            "flow_finished",
            generation,
        )
        .await
        .unwrap()
    );
    let generation2 = Uuid::now_v7();
    let mut anchor2 = runtime_event_input(run.id, "runtime_stream_opened");
    anchor2.payload = json!({"stream_generation_id": generation2});
    let anchor2 = OrchestrationRuntimeRepository::append_runtime_event(&store, &anchor2)
        .await
        .unwrap();
    let window2 = OrchestrationRuntimeRepository::get_runtime_event_replay_window(&store, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(window2.generation_id, Some(generation2));
    assert_eq!(window2.after_sequence, anchor2.sequence);
    assert!(
        !OrchestrationRuntimeRepository::has_runtime_event_terminal_after(
            &store,
            run.id,
            window2.after_sequence,
            "flow_finished",
            generation2,
        )
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn replay_gc_terminal_proof_rejects_foreign_generation_written_after_anchor() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-10-10 00:00:00 UTC),
    )
    .await;
    let current = Uuid::now_v7();
    let mut anchor = runtime_event_input(run.id, "runtime_stream_opened");
    anchor.payload = json!({"stream_generation_id": current});
    let anchor = OrchestrationRuntimeRepository::append_runtime_event(&store, &anchor)
        .await
        .unwrap();
    let mut terminal = runtime_event_input(run.id, "flow_finished");
    terminal.payload["stream_generation_id"] = json!(Uuid::now_v7());
    OrchestrationRuntimeRepository::append_runtime_event(&store, &terminal)
        .await
        .unwrap();
    assert!(
        !OrchestrationRuntimeRepository::has_runtime_event_terminal_after(
            &store,
            run.id,
            anchor.sequence,
            "flow_finished",
            current
        )
        .await
        .unwrap()
    );
    terminal.payload["stream_generation_id"] = json!(current);
    OrchestrationRuntimeRepository::append_runtime_event(&store, &terminal)
        .await
        .unwrap();
    assert!(
        OrchestrationRuntimeRepository::has_runtime_event_terminal_after(
            &store,
            run.id,
            anchor.sequence,
            "flow_finished",
            current
        )
        .await
        .unwrap()
    );
}

async fn append_event(
    store: &PgControlPlaneStore,
    flow_run: &domain::FlowRunRecord,
    node_run: Option<&domain::NodeRunRecord>,
    event_type: &str,
) -> domain::RunEventRecord {
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
        store,
        &AppendRunEventInput {
            flow_run_id: flow_run.id,
            node_run_id: node_run.map(|value| value.id),
            event_type: event_type.into(),
            payload: json!({ "event_type": event_type }),
        },
    )
    .await
    .unwrap()
}

fn runtime_event_input(flow_run_id: Uuid, event_type: &str) -> AppendRuntimeEventInput {
    AppendRuntimeEventInput {
        flow_run_id,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: event_type.to_string(),
        layer: domain::RuntimeEventLayer::AgentTransition,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload: json!({ "type": event_type }),
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
    }
}

#[tokio::test]
async fn orchestration_runtime_repository_persists_compiled_plan_runs_and_events() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-04-17 09:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run(&store, &run, started_at + Duration::seconds(1)).await;
    append_event(&store, &run, Some(&node_run), "node_run_completed").await;

    let detail =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_detail(
            &store,
            run.application_id,
            run.id,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(detail.flow_run.id, run.id);
    assert_eq!(detail.node_runs.len(), 1);
    assert_eq!(detail.events[0].event_type, "node_run_completed");
}

#[tokio::test]
async fn orchestration_runtime_repository_batch_appends_run_and_runtime_events() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-04-17 09:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;

    let run_events = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_events(
        &store,
        &[
            AppendRunEventInput {
                flow_run_id: run.id,
                node_run_id: None,
                event_type: "text_delta".into(),
                payload: json!({ "delta": "hello" }),
            },
            AppendRunEventInput {
                flow_run_id: run.id,
                node_run_id: None,
                event_type: "finish".into(),
                payload: json!({ "reason": "stop" }),
            },
        ],
    )
    .await
    .unwrap();
    let runtime_events =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_events(
            &store,
            &[
                AppendRuntimeEventInput {
                    flow_run_id: run.id,
                    node_run_id: None,
                    span_id: None,
                    parent_span_id: None,
                    event_type: "text_delta".into(),
                    layer: domain::RuntimeEventLayer::ProviderRaw,
                    source: domain::RuntimeEventSource::Host,
                    trust_level: domain::RuntimeTrustLevel::HostFact,
                    item_id: None,
                    ledger_ref: None,
                    payload: json!({ "delta": "hello" }),
                    visibility: domain::RuntimeEventVisibility::Workspace,
                    durability: domain::RuntimeEventDurability::Durable,
                },
                AppendRuntimeEventInput {
                    flow_run_id: run.id,
                    node_run_id: None,
                    span_id: None,
                    parent_span_id: None,
                    event_type: "finish".into(),
                    layer: domain::RuntimeEventLayer::ProviderRaw,
                    source: domain::RuntimeEventSource::Host,
                    trust_level: domain::RuntimeTrustLevel::HostFact,
                    item_id: None,
                    ledger_ref: None,
                    payload: json!({ "reason": "stop" }),
                    visibility: domain::RuntimeEventVisibility::Workspace,
                    durability: domain::RuntimeEventDurability::Durable,
                },
            ],
        )
        .await
        .unwrap();

    assert_eq!(
        run_events
            .iter()
            .map(|event| (event.sequence, event.event_type.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "text_delta"), (2, "finish")]
    );
    assert_eq!(
        runtime_events
            .iter()
            .map(|event| (event.sequence, event.event_type.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "text_delta"), (2, "finish")]
    );
}

#[tokio::test]
async fn event_batches_respect_postgres_bind_limit_and_preserve_order() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-04-17 09:00:00 UTC),
    )
    .await;

    let mut expected_runtime = Vec::new();
    // 4784 reproduces the observed 71760-bind failure; the other sizes straddle one batch.
    for count in [4369, 4370, 4784] {
        let offset = expected_runtime.len();
        let inputs = (0..count)
            .map(|index| {
                let mut input = runtime_event_input(run.id, "text_delta");
                input.payload = json!({"index": offset + index});
                input
            })
            .collect::<Vec<_>>();
        let records =
            <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_events(
                &store, &inputs,
            )
            .await
            .unwrap();
        assert_eq!(records.len(), count);
        for (index, record) in records.iter().enumerate() {
            assert_eq!(record.sequence, (offset + index + 1) as i64);
            assert_eq!(record.payload, inputs[index].payload);
            expected_runtime.push((record.id, record.sequence, record.payload.clone()));
        }
    }
    let stored_runtime = sqlx::query_as::<_, (Uuid, i64, Value)>(
        "select id, sequence, payload from runtime_events where flow_run_id = $1 order by sequence",
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(stored_runtime, expected_runtime);

    let inputs = (0..7282)
        .map(|index| AppendRunEventInput {
            flow_run_id: run.id,
            node_run_id: None,
            event_type: "text_delta".into(),
            payload: json!({"index": index}),
        })
        .collect::<Vec<_>>();
    let records =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_events(&store, &inputs)
            .await
            .unwrap();
    assert_eq!(records.len(), inputs.len());
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.sequence, (index + 1) as i64);
        assert_eq!(record.payload, inputs[index].payload);
    }
    let stored_run = sqlx::query_as::<_, (Uuid, i64, Value)>(
        "select id, sequence, payload from flow_run_events where flow_run_id = $1 order by sequence",
    ).bind(run.id).fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        stored_run,
        records
            .into_iter()
            .map(|record| (record.id, record.sequence, record.payload))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn event_batches_roll_back_all_chunks_when_second_chunk_has_invalid_node() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-04-17 09:00:00 UTC),
    )
    .await;
    let existing_run = append_event(&store, &run, None, "existing").await;
    let existing_runtime =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_event(
            &store,
            &runtime_event_input(run.id, "existing"),
        )
        .await
        .unwrap();

    let mut inputs = (0..4370)
        .map(|_| runtime_event_input(run.id, "text_delta"))
        .collect::<Vec<_>>();
    inputs.last_mut().unwrap().node_run_id = Some(Uuid::now_v7());
    let error = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_events(
        &store, &inputs,
    )
    .await
    .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<sqlx::Error>()
            .unwrap()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23503")
    );
    let retained_runtime = sqlx::query_scalar::<_, Uuid>(
        "select id from runtime_events where flow_run_id = $1 order by sequence",
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(retained_runtime, vec![existing_runtime.id]);
    let next_runtime =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_event(
            &store,
            &runtime_event_input(run.id, "after_rollback"),
        )
        .await
        .unwrap();
    assert_eq!(next_runtime.sequence, existing_runtime.sequence + 1);

    let mut inputs = (0..7282)
        .map(|_| AppendRunEventInput {
            flow_run_id: run.id,
            node_run_id: None,
            event_type: "text_delta".into(),
            payload: json!({}),
        })
        .collect::<Vec<_>>();
    inputs.last_mut().unwrap().node_run_id = Some(Uuid::now_v7());
    let error =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_events(&store, &inputs)
            .await
            .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<sqlx::Error>()
            .unwrap()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23503")
    );
    let retained_run = sqlx::query_scalar::<_, Uuid>(
        "select id from flow_run_events where flow_run_id = $1 order by sequence",
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(retained_run, vec![existing_run.id]);
    let next_run = append_event(&store, &run, None, "after_rollback").await;
    assert_eq!(next_run.sequence, existing_run.sequence + 1);
}

/// FIX-BE-002: ordinary event writers retain pre-terminal tool evidence but cannot append after
/// the terminal CAS has committed its canonical flow and runtime terminal events.
#[tokio::test]
async fn terminal_flow_run_fences_late_run_and_runtime_event_appends() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-08-16 09:00:00 UTC),
    )
    .await;

    append_event(&store, &run, None, "tool_call_completed").await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_event(
        &store,
        &runtime_event_input(run.id, "tool_call_commit"),
    )
    .await
    .unwrap();

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::commit_flow_run_terminal(
        &store,
        &CommitFlowRunTerminalInput {
            flow_run_id: run.id,
            expected_status: FlowRunStatus::Running,
            result: CommitFlowRunTerminalResult::Cancelled {
                output_payload: json!({}),
                error_payload: None,
            },
            flow_run_event_payload: json!({ "reason": "manual_stop" }),
            terminal_event_payload: json!({
                "type": "flow_cancelled",
                "run_id": run.id,
                "status": "cancelled"
            }),
            finished_at: datetime!(2026-08-16 09:00:01 UTC),
        },
    )
    .await
    .unwrap();

    let late_run_event = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
        &store,
        &AppendRunEventInput {
            flow_run_id: run.id,
            node_run_id: None,
            event_type: "late_run_event".to_string(),
            payload: json!({}),
        },
    )
    .await
    .unwrap_err();
    let late_runtime_event =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_event(
            &store,
            &runtime_event_input(run.id, "late_runtime_event"),
        )
        .await
        .unwrap_err();
    let late_runtime_batch =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_events(
            &store,
            &[
                runtime_event_input(run.id, "late_runtime_batch_1"),
                runtime_event_input(run.id, "late_runtime_batch_2"),
            ],
        )
        .await
        .unwrap_err();
    for error in [late_run_event, late_runtime_event, late_runtime_batch] {
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::Conflict("flow_run_terminal"))
        ));
    }

    let detail =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_detail(
            &store,
            run.application_id,
            run.id,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(detail
        .events
        .iter()
        .any(|event| event.event_type == "tool_call_completed"));
    assert!(detail
        .events
        .iter()
        .any(|event| event.event_type == "flow_run_cancelled"));
    assert!(!detail
        .events
        .iter()
        .any(|event| event.event_type == "late_run_event"));

    let runtime_events =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_runtime_events(
            &store, run.id, 0,
        )
        .await
        .unwrap();
    assert!(runtime_events
        .iter()
        .any(|event| event.event_type == "tool_call_commit"));
    assert!(runtime_events
        .iter()
        .any(|event| event.event_type == "flow_cancelled"));
    assert!(!runtime_events
        .iter()
        .any(|event| event.event_type.starts_with("late_runtime")));
}

#[tokio::test]
async fn assistant_activity_sequence_orders_runtime_event_backfill_by_interval_start() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-04-17 09:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_events(
        &store,
        &[
            AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: None,
                span_id: None,
                parent_span_id: None,
                event_type: "assistant_tool_call_started".into(),
                layer: domain::RuntimeEventLayer::RuntimeItem,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "stream_sequence": 50,
                    "sequence_start": 50,
                    "sequence_end": 50
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
            AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: None,
                span_id: None,
                parent_span_id: None,
                event_type: "text_delta".into(),
                layer: domain::RuntimeEventLayer::ProviderRaw,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "text": "hello",
                    "sequence_start": 10,
                    "sequence_end": 100
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
            AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: None,
                span_id: None,
                parent_span_id: None,
                event_type: "flow_finished".into(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "status": "succeeded",
                    "stream_sequence": 101
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
        ],
    )
    .await
    .unwrap();

    let page =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_runtime_event_backfill_page(
            &store, run.id, -1, 3,
        )
        .await
        .unwrap();

    assert_eq!(page.len(), 3);
    assert_eq!(page[0].event_type, "text_delta");
    assert_eq!(page[0].payload["sequence_end"], 100);
    assert_eq!(page[1].event_type, "assistant_tool_call_started");
    assert_eq!(page[2].event_type, "flow_finished");
}
mod callback_control;
