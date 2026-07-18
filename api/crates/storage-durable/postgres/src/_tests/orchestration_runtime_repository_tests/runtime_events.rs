use super::*;

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

#[tokio::test]
async fn orchestration_runtime_repository_persists_compiled_plan_runs_and_events() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
async fn orchestration_runtime_repository_lists_runtime_event_backfill_page_by_stream_sequence() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
            &store, run.id, 50, 1,
        )
        .await
        .unwrap();

    assert_eq!(page.len(), 1);
    assert_eq!(page[0].event_type, "text_delta");
    assert_eq!(page[0].payload["sequence_end"], 100);
}

#[tokio::test]
async fn orchestration_runtime_repository_finds_callback_event_sequence_without_loading_history() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-04-17 09:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let callback_task_id = Uuid::now_v7();

    let events = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_events(
        &store,
        &[
            AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: None,
                span_id: None,
                parent_span_id: None,
                event_type: "debug_blob".into(),
                layer: domain::RuntimeEventLayer::Diagnostic,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({ "history": "x".repeat(2 * 1024 * 1024) }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
            AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: None,
                span_id: None,
                parent_span_id: None,
                event_type: "waiting_callback".into(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({ "callback_task_id": callback_task_id }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
        ],
    )
    .await
    .unwrap();

    let sequence = <PgControlPlaneStore as OrchestrationRuntimeRepository>::
        get_runtime_event_sequence_for_callback_task(&store, run.id, callback_task_id)
        .await
        .unwrap();

    assert_eq!(sequence, Some(events[1].sequence));
}

#[tokio::test]
async fn orchestration_runtime_repository_serializes_concurrent_run_event_sequences() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-04-17 09:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let barrier = Arc::new(Barrier::new(12));
    let mut handles = Vec::new();

    for index in 0..12 {
        let store = store.clone();
        let barrier = Arc::clone(&barrier);
        let flow_run_id = run.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_run_event(
                &store,
                &AppendRunEventInput {
                    flow_run_id,
                    node_run_id: None,
                    event_type: format!("event_{index}"),
                    payload: json!({ "index": index }),
                },
            )
            .await
        }));
    }

    let mut events = Vec::new();
    for handle in handles {
        events.push(handle.await.unwrap().unwrap());
    }
    events.sort_by_key(|event| event.sequence);

    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        (1..=12).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn orchestration_runtime_repository_persists_waiting_human_checkpoint() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-04-17 10:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let node_run = seed_node_run_for(
        &store,
        &run,
        "node-human",
        "human_input",
        "Human Input",
        json!({ "prompt": "请人工审核" }),
        started_at + Duration::seconds(1),
    )
    .await;

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run(
        &store,
        &UpdateNodeRunInput {
            node_run_id: node_run.id,
            status: NodeRunStatus::WaitingHuman,
            output_payload: json!({}),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({ "waiting_reason": "manual_review" }),
            finished_at: None,
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_checkpoint(
        &store,
        &CreateCheckpointInput {
            flow_run_id: run.id,
            node_run_id: Some(node_run.id),
            status: "waiting_human".to_string(),
            reason: "等待人工输入".to_string(),
            locator_payload: json!({ "node_id": "node-human", "next_node_index": 3 }),
            variable_snapshot: json!({ "node-llm": { "text": "草稿回复" } }),
            external_ref_payload: Some(json!({ "prompt": "请人工审核" })),
        },
    )
    .await
    .unwrap();

    let detail =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_detail(
            &store,
            run.application_id,
            run.id,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(detail.flow_run.run_mode.as_str(), "debug_flow_run");
    assert_eq!(
        detail.node_runs[0].debug_payload,
        json!({ "waiting_reason": "manual_review" })
    );
    assert_eq!(detail.checkpoints[0].status, "waiting_human");
    assert_eq!(
        detail.checkpoints[0].external_ref_payload.as_ref().unwrap()["prompt"],
        json!("请人工审核")
    );
}

#[tokio::test]
async fn orchestration_runtime_repository_returns_callback_tasks_with_run_detail() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-04-17 11:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let node_run = seed_node_run_for(
        &store,
        &run,
        "node-tool",
        "tool",
        "Tool",
        json!({ "tool_name": "lookup_order" }),
        started_at + Duration::seconds(1),
    )
    .await;

    let task = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
        &store,
        &CreateCallbackTaskInput {
            flow_run_id: run.id,
            node_run_id: node_run.id,
            callback_kind: "tool".to_string(),
            request_payload: json!({ "tool_name": "lookup_order" }),
            external_ref_payload: Some(json!({ "tool_name": "lookup_order" })),
        },
    )
    .await
    .unwrap();

    let detail =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_application_run_detail(
            &store,
            run.application_id,
            run.id,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(detail.callback_tasks.len(), 1);
    assert_eq!(detail.callback_tasks[0].callback_kind, "tool");
    assert_eq!(detail.callback_tasks[0].status, CallbackTaskStatus::Pending);
    assert_eq!(detail.callback_tasks[0].id, task.id);

    <PgControlPlaneStore as OrchestrationRuntimeRepository>::complete_callback_task(
        &store,
        &control_plane::ports::CompleteCallbackTaskInput {
            callback_task_id: task.id,
            response_payload: json!({ "result": "ok" }),
            completed_at: started_at + Duration::seconds(5),
        },
    )
    .await
    .unwrap();
    let duplicate =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::complete_callback_task(
            &store,
            &control_plane::ports::CompleteCallbackTaskInput {
                callback_task_id: task.id,
                response_payload: json!({ "result": "again" }),
                completed_at: started_at + Duration::seconds(6),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        duplicate.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("callback_task_not_pending"))
    ));
}

#[tokio::test]
async fn published_run_control_cancels_pending_callback_tasks_for_run() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-04 11:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let first_node = seed_node_run_for(
        &store,
        &run,
        "node-tool-1",
        "tool",
        "Tool 1",
        json!({ "tool_name": "lookup_order" }),
        started_at + Duration::seconds(1),
    )
    .await;
    let second_node = seed_node_run_for(
        &store,
        &run,
        "node-tool-2",
        "tool",
        "Tool 2",
        json!({ "tool_name": "lookup_inventory" }),
        started_at + Duration::seconds(2),
    )
    .await;
    let pending = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
        &store,
        &CreateCallbackTaskInput {
            flow_run_id: run.id,
            node_run_id: first_node.id,
            callback_kind: "tool".to_string(),
            request_payload: json!({ "tool_name": "lookup_order" }),
            external_ref_payload: None,
        },
    )
    .await
    .unwrap();
    let completed = <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
        &store,
        &CreateCallbackTaskInput {
            flow_run_id: run.id,
            node_run_id: second_node.id,
            callback_kind: "tool".to_string(),
            request_payload: json!({ "tool_name": "lookup_inventory" }),
            external_ref_payload: None,
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::complete_callback_task(
        &store,
        &control_plane::ports::CompleteCallbackTaskInput {
            callback_task_id: completed.id,
            response_payload: json!({ "result": "ok" }),
            completed_at: started_at + Duration::seconds(5),
        },
    )
    .await
    .unwrap();

    let cancelled =
        <PgControlPlaneStore as control_plane::application_public_api::run_service::ApplicationPublishedRunControlRepository>::cancel_published_pending_callback_tasks_for_run(
            &store,
            run.id,
            started_at + Duration::seconds(6),
        )
        .await
        .unwrap();

    assert_eq!(cancelled.len(), 1);
    assert_eq!(cancelled[0].id, pending.id);
    assert_eq!(cancelled[0].status, CallbackTaskStatus::Cancelled);
    assert_eq!(
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_callback_task(
            &store,
            completed.id,
        )
        .await
        .unwrap()
        .unwrap()
        .status,
        CallbackTaskStatus::Completed
    );
}

#[tokio::test]
async fn published_run_control_lists_waiting_callback_runs_for_conversation() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-04 12:00:00 UTC);
    let matching = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: matching.id,
            status: FlowRunStatus::WaitingCallback,
            output_payload: json!({}),
            error_payload: None,
            finished_at: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        r#"
        update flow_runs
        set api_key_id = $2,
            external_user = $3,
            external_conversation_id = $4,
            compatibility_mode = $5
        where id = $1
        "#,
    )
    .bind(matching.id)
    .bind(api_key_id)
    .bind("claude-user")
    .bind("session-1")
    .bind("anthropic-messages-v1")
    .execute(store.pool())
    .await
    .unwrap();
    let finished = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at + Duration::seconds(1),
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    sqlx::query(
        r#"
        update flow_runs
        set api_key_id = $2,
            external_user = $3,
            external_conversation_id = $4,
            compatibility_mode = $5,
            status = 'succeeded'
        where id = $1
        "#,
    )
    .bind(finished.id)
    .bind(api_key_id)
    .bind("claude-user")
    .bind("session-1")
    .bind("anthropic-messages-v1")
    .execute(store.pool())
    .await
    .unwrap();

    let waiting_run_ids =
        <PgControlPlaneStore as control_plane::application_public_api::run_service::ApplicationPublishedRunControlRepository>::list_waiting_callback_published_flow_run_ids_for_conversation(
            &store,
            &control_plane::application_public_api::run_service::ListWaitingCallbackPublishedRunsInput {
                application_id: matching.application_id,
                api_key_id,
                external_user: "claude-user".to_string(),
                external_conversation_id: "session-1".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(waiting_run_ids, vec![matching.id]);
}

#[tokio::test]
async fn orchestration_runtime_repository_records_callback_resume_attempts_idempotently() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-06-02 10:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::WaitingCallback,
            output_payload: json!({}),
            error_payload: None,
            finished_at: None,
        },
    )
    .await
    .unwrap();
    let node_run = seed_node_run_for(
        &store,
        &run,
        "node-tool",
        "tool",
        "Tool",
        json!({ "tool_name": "lookup_order" }),
        started_at + Duration::seconds(1),
    )
    .await;
    let callback_task =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
            &store,
            &CreateCallbackTaskInput {
                flow_run_id: run.id,
                node_run_id: node_run.id,
                callback_kind: "external_callback".to_string(),
                request_payload: json!({ "prompt": "approve" }),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();

    let first_record =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::record_flow_run_callback_resume_attempt(
            &store,
            &RecordFlowRunCallbackResumeAttemptInput {
                flow_run_id: run.id,
                callback_task_id: callback_task.id,
                source: "native_agent".into(),
                response_payload: json!({ "answer": "approved" }),
                idempotency_key: format!("callback_task:{}", callback_task.id),
            },
        )
        .await
        .unwrap();
    let duplicate_record =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::record_flow_run_callback_resume_attempt(
            &store,
            &RecordFlowRunCallbackResumeAttemptInput {
                flow_run_id: run.id,
                callback_task_id: callback_task.id,
                source: "openai_chat".into(),
                response_payload: json!({ "answer": "ignored" }),
                idempotency_key: format!("callback_task:{}", callback_task.id),
            },
        )
        .await
        .unwrap();

    assert!(first_record.inserted);
    assert!(!duplicate_record.inserted);
    assert_eq!(first_record.attempt.id, duplicate_record.attempt.id);
    assert_eq!(duplicate_record.attempt.source, "native_agent");
    assert_eq!(
        duplicate_record.attempt.response_payload,
        json!({ "answer": "approved" })
    );
    assert_eq!(
        duplicate_record.attempt.status,
        FlowRunCallbackResumeAttemptStatus::Processing
    );

    let loaded = <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_flow_run_callback_resume_attempt_by_callback_task(
        &store,
        callback_task.id,
    )
    .await
    .unwrap()
    .expect("attempt should be queryable by callback task");
    assert_eq!(loaded.id, first_record.attempt.id);

    let finished =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::finish_flow_run_callback_resume_attempt(
            &store,
            &FinishFlowRunCallbackResumeAttemptInput {
                attempt_id: loaded.id,
                status: FlowRunCallbackResumeAttemptStatus::Succeeded,
                error_payload: None,
                completed_at: started_at + Duration::seconds(5),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        finished.status,
        FlowRunCallbackResumeAttemptStatus::Succeeded
    );
    assert!(finished.completed_at.is_some());
}
