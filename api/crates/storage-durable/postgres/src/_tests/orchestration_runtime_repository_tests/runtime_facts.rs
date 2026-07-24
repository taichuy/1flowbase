use super::*;

#[tokio::test]
async fn latest_node_run_returns_most_recent_run_for_node() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let first_started_at = datetime!(2026-04-17 09:00:00 UTC);
    let second_started_at = first_started_at + Duration::minutes(5);
    let first_run = seed_flow_run(&store, &seeded, &compiled, first_started_at).await;
    let _ = seed_node_run(&store, &first_run, first_started_at + Duration::seconds(1)).await;
    let second_run = seed_flow_run(&store, &seeded, &compiled, second_started_at).await;
    let second_node_run = seed_node_run(
        &store,
        &second_run,
        second_started_at + Duration::seconds(1),
    )
    .await;

    let node_last_run =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_latest_node_run(
            &store,
            seeded.application_id,
            "node-llm",
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(node_last_run.node_run.id, second_node_run.id);
}

#[tokio::test]
async fn runtime_fact_spine_preserves_span_sequence_and_trust_level() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-04-27 09:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;

    let span = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_span(
        &store,
        &AppendRuntimeSpanInput {
            flow_run_id: run.id,
            node_run_id: None,
            parent_span_id: None,
            kind: domain::RuntimeSpanKind::Flow,
            name: "debug flow".into(),
            status: domain::RuntimeSpanStatus::Running,
            capability_id: None,
            input_ref: None,
            output_ref: None,
            error_payload: None,
            metadata: json!({ "mode": "debug_flow_run" }),
            started_at,
            finished_at: None,
        },
    )
    .await
    .unwrap();

    let event = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_event(
        &store,
        &AppendRuntimeEventInput {
            flow_run_id: run.id,
            node_run_id: None,
            span_id: Some(span.id),
            parent_span_id: None,
            event_type: "run_started".into(),
            layer: domain::RuntimeEventLayer::RuntimeItem,
            source: domain::RuntimeEventSource::Host,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload: json!({ "run_id": run.id }),
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
        },
    )
    .await
    .unwrap();

    let spans =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_runtime_spans(&store, run.id)
            .await
            .unwrap();
    let events = <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_runtime_events(
        &store, run.id, 0,
    )
    .await
    .unwrap();

    assert_eq!(spans[0].id, span.id);
    assert_eq!(events[0].id, event.id);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[0].trust_level, domain::RuntimeTrustLevel::HostFact);
}

#[tokio::test]
async fn orchestration_runtime_repository_persists_model_failover_attempt_and_input_cache_usage_ledger(
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-04-27 09:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let node_run = seed_node_run(&store, &run, started_at).await;
    let span = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_runtime_span(
        &store,
        &AppendRuntimeSpanInput {
            flow_run_id: run.id,
            node_run_id: Some(node_run.id),
            parent_span_id: None,
            kind: domain::RuntimeSpanKind::LlmTurn,
            name: "LLM".into(),
            status: domain::RuntimeSpanStatus::Running,
            capability_id: None,
            input_ref: None,
            output_ref: None,
            error_payload: None,
            metadata: json!({}),
            started_at,
            finished_at: None,
        },
    )
    .await
    .unwrap();
    let attempt =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_model_failover_attempt_ledger(
            &store,
            &AppendModelFailoverAttemptLedgerInput {
                flow_run_id: run.id,
                node_run_id: Some(node_run.id),
                llm_turn_span_id: Some(span.id),
                queue_snapshot_id: None,
                attempt_index: 0,
                provider_instance_id: None,
                provider_code: "fixture_provider".into(),
                upstream_model_id: "gpt-5.4-mini".into(),
                protocol: "openai_compatible".into(),
                request_ref: Some("runtime_artifact:inline:req".into()),
                request_hash: Some("sha256:req".into()),
                started_at,
                first_token_at: None,
                finished_at: Some(started_at + Duration::seconds(1)),
                status: "succeeded".into(),
                failed_after_first_token: false,
                upstream_request_id: Some("req-1".into()),
                error_code: None,
                error_message_ref: None,
                usage_ledger_id: None,
                cost_ledger_id: None,
                response_ref: Some("runtime_artifact:inline:res".into()),
            },
        )
        .await
        .unwrap();
    let usage = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_usage_ledger(
        &store,
        &AppendUsageLedgerInput {
            flow_run_id: run.id,
            node_run_id: Some(node_run.id),
            span_id: Some(span.id),
            failover_attempt_id: Some(attempt.id),
            provider_instance_id: None,
            gateway_route_id: None,
            model_id: Some("gpt-5.4-mini".into()),
            upstream_model_id: Some("gpt-5.4-mini".into()),
            upstream_request_id: Some("req-1".into()),
            input_tokens: Some(1),
            cached_input_tokens: None,
            output_tokens: Some(2),
            reasoning_output_tokens: None,
            total_tokens: Some(3),
            input_cache_hit_tokens: Some(40),
            input_cache_miss_tokens: Some(60),
            cache_read_tokens: None,
            cache_write_tokens: None,
            price_snapshot: None,
            cost_snapshot: None,
            usage_status: domain::UsageLedgerStatus::Recorded,
            raw_usage: json!({ "total_tokens": 3 }),
            normalized_usage: json!({ "total_tokens": 3 }),
        },
    )
    .await
    .unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::link_usage_ledger_to_model_failover_attempt(
        &store,
        &LinkUsageLedgerToModelFailoverAttemptInput {
            failover_attempt_id: attempt.id,
            usage_ledger_id: usage.id,
        },
    )
    .await
    .unwrap();

    let attempts =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_model_failover_attempt_ledger(
            &store,
            run.id,
        )
        .await
        .unwrap();
    let usage_records =
        <PgControlPlaneStore as OrchestrationRuntimeRepository>::list_usage_ledger(&store, run.id)
            .await
            .unwrap();

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].id, attempt.id);
    assert_eq!(attempts[0].usage_ledger_id, Some(usage.id));
    assert_eq!(usage_records[0].failover_attempt_id, Some(attempt.id));
    assert_eq!(usage_records[0].input_cache_hit_tokens, Some(40));
    assert_eq!(usage_records[0].input_cache_miss_tokens, Some(60));

    let cache_usage = sqlx::query_as::<_, (Option<i64>, Option<i64>)>(
        "select input_cache_hit_tokens, input_cache_miss_tokens from runtime_usage_ledger where id = $1",
    )
    .bind(usage.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(cache_usage.0, Some(40));
    assert_eq!(cache_usage.1, Some(60));
}

#[tokio::test]
async fn credit_ledger_idempotency_prevents_double_debit() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = seed_workspace(&store, "Billing").await;
    let credit_ledger_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'runtime_credit_ledger'
        "#,
    )
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert!(credit_ledger_columns.contains(&"application_id".to_string()));
    assert!(!credit_ledger_columns.contains(&"app_id".to_string()));

    let first = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_credit_ledger(
        &store,
        &AppendCreditLedgerInput {
            workspace_id,
            user_id: None,
            application_id: None,
            agent_id: None,
            flow_run_id: None,
            span_id: None,
            cost_ledger_id: None,
            transaction_type: "debit".into(),
            amount: "3.50".into(),
            balance_after: Some("96.50".into()),
            credit_unit: "credit".into(),
            reason: "gateway_settle".into(),
            idempotency_key: "idem-1".into(),
            status: "posted".into(),
        },
    )
    .await
    .unwrap();

    let replay = <PgControlPlaneStore as OrchestrationRuntimeRepository>::append_credit_ledger(
        &store,
        &AppendCreditLedgerInput {
            workspace_id,
            idempotency_key: "idem-1".into(),
            amount: "3.50".into(),
            transaction_type: "debit".into(),
            credit_unit: "credit".into(),
            reason: "gateway_settle".into(),
            status: "posted".into(),
            user_id: None,
            application_id: None,
            agent_id: None,
            flow_run_id: None,
            span_id: None,
            cost_ledger_id: None,
            balance_after: Some("96.50".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(first.id, replay.id);
}

#[tokio::test]
async fn audit_hash_chain_links_runtime_facts() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(
        &store,
        &seeded,
        &compiled,
        datetime!(2026-04-27 12:00:00 UTC),
    )
    .await;

    let first = store
        .append_audit_hash(
            run.id,
            "runtime_events",
            Uuid::now_v7(),
            serde_json::json!({"a":1}),
        )
        .await
        .unwrap();
    let second = store
        .append_audit_hash(
            run.id,
            "runtime_events",
            Uuid::now_v7(),
            serde_json::json!({"a":2}),
        )
        .await
        .unwrap();

    assert_eq!(second.prev_hash.as_deref(), Some(first.row_hash.as_str()));
}
