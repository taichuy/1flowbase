use super::*;
use control_plane_contracts::application_public_runtime::ApplicationPublishedFlowRunRepository;
use control_plane_contracts::gateway_logs::GatewayLogContext;

#[tokio::test]
async fn issue_2032_gateway_binding_is_durable_scoped_and_does_not_merge_turns_by_cursor() {
    let database = isolated_database().await;
    let store = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let other_key = seed_application_api_key(&store, &seeded).await;
    let mut input = CreateFlowRunInput {
        gateway_log_context: Some(GatewayLogContext {
            identity_status: "identified".into(),
            thread_id: Some("thread-1".into()),
            turn_id: Some("turn-1".into()),
            ..Default::default()
        }),
        actor_user_id: seeded.actor_user_id,
        application_id: seeded.application_id,
        flow_id: seeded.flow_id,
        flow_draft_id: seeded.draft_id,
        compiled_plan_id: compiled.id,
        debug_session_id: String::new(),
        flow_schema_version: compiled.schema_version.clone(),
        document_hash: compiled.document_hash.clone(),
        run_mode: FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: "same question".into(),
        status: FlowRunStatus::Queued,
        input_payload: json!({"query":"same question","history":[]}),
        started_at: OffsetDateTime::now_utc(),
        api_key_id: Some(key),
        publication_version_id: None,
        assistant_conversation_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: Some("openai-responses".into()),
        idempotency_key: Some("request-1".into()),
    };
    let first = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    let replay = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    assert_eq!(first.flow_run.id, replay.flow_run.id);
    assert!(!replay.created);
    input.idempotency_key = Some("request-2".into());
    input
        .gateway_log_context
        .as_mut()
        .unwrap()
        .previous_response_id = Some(format!("resp_{}", first.flow_run.id));
    let second = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    input.idempotency_key = Some("request-3".into());
    input.gateway_log_context.as_mut().unwrap().turn_id = Some("turn-2".into());
    let third = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    input.api_key_id = Some(other_key);
    input.idempotency_key = Some("other-key".into());
    input.gateway_log_context.as_mut().unwrap().turn_id = Some("turn-1".into());
    let other = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    let mut rows = Vec::new();
    for run in [&first, &second, &third, &other] {
        assert_eq!(run.flow_run.input_payload, input.input_payload);
        rows.push(sqlx::query_as::<_,(Uuid,Uuid,Option<Uuid>)>("select conversation_id,turn_id,caused_by_run_id from gateway_log_invocations where flow_run_id=$1").bind(run.flow_run.id).fetch_one(store.pool()).await.unwrap());
    }
    assert_eq!(rows[0].0, rows[1].0);
    assert_eq!(rows[0].1, rows[1].1);
    assert_eq!(rows[0].0, rows[2].0);
    assert_ne!(rows[0].1, rows[2].1);
    assert_ne!(rows[0].0, rows[3].0);
    assert!(rows[3].2.is_none());
    assert_eq!(rows[1].2, Some(first.flow_run.id));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from gateway_log_invocations")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        4
    );
    // Association has no dependency on ephemeral provider state or semantic history binding.
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from application_conversations where application_id=$1"
        )
        .bind(seeded.application_id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        0
    );
    use control_plane_contracts::gateway_logs::{
        GatewayLogQuery, GatewayLogRepository, GatewayLogScope,
    };
    let scope = GatewayLogScope {
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        api_key_id: Some(key),
    };
    let page = store
        .list_gateway_log_page(&scope, &GatewayLogQuery::default())
        .await
        .unwrap();
    assert_eq!(
        page.total, 1,
        "one exact conversation, not one row per invocation"
    );
    let turns = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                conversation_id: Some(rows[0].0),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(turns.total, 2);
    assert!(turns.items.iter().all(|t| t.completion_status == "unknown"));
    let wrong = GatewayLogScope {
        api_key_id: Some(other_key),
        ..scope.clone()
    };
    assert_eq!(
        store
            .list_gateway_log_page(
                &wrong,
                &GatewayLogQuery {
                    conversation_id: Some(rows[0].0),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .total,
        0
    );
    let wrong_workspace = GatewayLogScope {
        scope_id: Uuid::now_v7(),
        ..scope.clone()
    };
    assert_eq!(
        store
            .list_gateway_log_page(&wrong_workspace, &GatewayLogQuery::default())
            .await
            .unwrap()
            .total,
        0
    );
    // AC-003/006/009: authoritative billed leaves, retry and a foreign-scope
    // leaf using the same run UUID must not contaminate counts or details.
    let mut billed = super::super::request_logs::request_log(
        seeded.workspace_id,
        Uuid::now_v7(),
        first.flow_run.started_at,
        "failed",
        Some(2),
    );
    billed.flow_run_id = first.flow_run.id;
    billed.application_id = Some(seeded.application_id);
    let mut retry = billed.clone();
    retry.attempt_id = Uuid::now_v7();
    retry.attempt_index = 2;
    retry.is_retry = true;
    retry.status = "succeeded".into();
    let mut foreign = retry.clone();
    foreign.attempt_id = Uuid::now_v7();
    foreign.scope_id = Uuid::now_v7();
    store
        .insert_model_provider_request_logs_batch(&[billed.clone(), billed, retry, foreign])
        .await
        .unwrap();
    let attempts = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                flow_run_id: Some(first.flow_run.id),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        attempts.total, 2,
        "attempt leaves must be scoped independently of run IDs"
    );
    let summary = store
        .list_gateway_log_page(&scope, &GatewayLogQuery::default())
        .await
        .unwrap();
    assert_eq!(summary.items[0].metrics.attempt_count, 2);
    assert_eq!(summary.items[0].metrics.total_tokens, Some(244));
    assert_eq!(summary.items[0].metrics.failed_attempt_count, 1);
    assert_eq!(
        summary.items[0].metrics.costs[0]
            .amount
            .trim_end_matches('0'),
        "0.0025"
    );
    assert_eq!(
        store
            .list_gateway_log_page(
                &wrong,
                &GatewayLogQuery {
                    flow_run_id: Some(first.flow_run.id),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .total,
        0
    );
    // AC-007: formal facts survive projection deletion and deterministic rebuild.
    let tool = json!({"id":"tool-item","type":"custom_tool_call","call_id":"call-1","name":"exec","input":"return 7;"});
    append_formal_event(&store, &AppendRunEventInput {
            flow_run_id: second.flow_run.id,
            node_run_id: None,
            event_type: "provider_output_item_done".into(),
            payload: json!({"item":tool}),
        })
        .await
        .unwrap();
    for _ in 0..2 {
        append_formal_event(&store, &AppendRunEventInput {
                flow_run_id: first.flow_run.id,
                node_run_id: None,
                event_type: "provider_output_item_done".into(),
                payload: json!({"item":tool}),
            })
            .await
            .unwrap();
    }
    input.api_key_id = Some(key);
    input.idempotency_key = Some("full-history-results".into());
    input.started_at = OffsetDateTime::now_utc() + Duration::seconds(1);
    input.gateway_log_context.as_mut().unwrap().tool_results =
        vec![json!({"type":"custom_tool_call_output","call_id":"call-1","output":"7"})];
    ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    input.idempotency_key = Some("full-history-results-repeated".into());
    ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    let invocations = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                turn_id: Some(rows[0].1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let original = invocations
        .items
        .iter()
        .find(|item| item.id == first.flow_run.id)
        .unwrap();
    assert_eq!(original.messages.len(), 1);
    assert!(original.messages[0].result_received);
    assert!(!original.messages[0].execution_verified);
    assert_eq!(original.messages[0].tool_result.as_deref(), Some("7"));
    assert_eq!(
        invocations
            .items
            .iter()
            .map(|item| item.messages.len())
            .sum::<usize>(),
        1,
        "duplicate full output belongs to one invocation"
    );
    assert_eq!(
        original.messages[0].tool_input.as_deref(),
        Some("return 7;")
    );
    let expected = serde_json::to_value(&original.messages).unwrap();
    sqlx::query("delete from gateway_log_output_items where conversation_id=$1")
        .bind(rows[0].0)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(
        !store
            .rebuild_gateway_log_conversation(&wrong, rows[0].0)
            .await
            .unwrap()
    );
    assert!(
        store
            .rebuild_gateway_log_conversation(&scope, rows[0].0)
            .await
            .unwrap()
    );
    let rebuilt = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                turn_id: Some(rows[0].1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(
            &rebuilt
                .items
                .iter()
                .find(|item| item.id == first.flow_run.id)
                .unwrap()
                .messages
        )
        .unwrap(),
        expected
    );
    // AC-004: declared parent references resolve only inside the key domain.
    input.gateway_log_context = Some(GatewayLogContext {
        identity_status: "identified".into(),
        thread_id: Some("child-thread".into()),
        turn_id: Some("child-turn".into()),
        parent_thread_id: Some("thread-1".into()),
        parent_turn_id: Some("turn-1".into()),
        ..Default::default()
    });
    input.idempotency_key = Some("child".into());
    let child = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    let child_conversation: Uuid = sqlx::query_scalar(
        "select conversation_id from gateway_log_invocations where flow_run_id=$1",
    )
    .bind(child.flow_run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let child_turns = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                conversation_id: Some(child_conversation),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(child_turns.items[0].relation_status, "resolved_parent");
    assert_eq!(
        serde_json::to_value(&child_turns.items[0]).unwrap()["parent_task_id"],
        rows[0].1.to_string()
    );
    // AC-009/011/012 remaining fixed matrix: unknown costs, parallel interval
    // union, scoped pagination/index access, conflicting tool identities, cleanup.
    let mut unknown = super::super::request_logs::request_log(
        seeded.workspace_id,
        Uuid::now_v7(),
        first.flow_run.started_at,
        "succeeded",
        None,
    );
    unknown.flow_run_id = first.flow_run.id;
    unknown.total_cost = None;
    unknown.currency_code = None;
    unknown.pricing_provider_code = None;
    unknown.pricing_model_id = None;
    store
        .insert_model_provider_request_logs_batch(&[unknown])
        .await
        .unwrap();
    let run_page = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                turn_id: Some(rows[0].1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let first_entry = run_page
        .items
        .iter()
        .find(|item| item.id == first.flow_run.id)
        .unwrap();
    assert_eq!(first_entry.metrics.unknown_cost_attempts, 1);
    assert_eq!(first_entry.metrics.total_tokens, None);
    let next = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                turn_id: Some(rows[0].1),
                page: Some(2),
                page_size: Some(1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(next.items.len(), 1);
    assert_ne!(next.items[0].id, run_page.items[0].id);
    let plan:serde_json::Value=sqlx::query_scalar("explain (format json) select flow_run_id from gateway_log_invocations where turn_id=$1 order by created_at,flow_run_id limit 20")
        .bind(rows[0].1).fetch_one(store.pool()).await.unwrap();
    println!("issue_2032_turn_query_plan={plan}");
    assert!(plan.to_string().contains("gateway_log_invocations_turn"));
    let second_tool = json!({"id":"parallel-item","type":"function_call","call_id":"call-2","name":"lookup","arguments":"{}"});
    append_formal_event(&store, &AppendRunEventInput {
            flow_run_id: first.flow_run.id,
            node_run_id: None,
            event_type: "provider_output_item_done".into(),
            payload: json!({"item":second_tool}),
        })
        .await
        .unwrap();
    // Controlled retained timestamps establish overlapping observed intervals:
    // [0,2000] and [1000,3000] => union 3000, not sum 4000.
    let at = first.flow_run.started_at;
    sqlx::query("update runtime_events set created_at=$2 + case when payload->'item'->>'call_id'='call-2' then interval '1 second' else interval '0 second' end where flow_run_id=$1")
        .bind(first.flow_run.id).bind(at).execute(store.pool()).await.unwrap();
    input.gateway_log_context = Some(GatewayLogContext {
        identity_status: "identified".into(),
        thread_id: Some("thread-1".into()),
        turn_id: Some("turn-1".into()),
        tool_results: vec![json!({"type":"function_call_output","call_id":"call-2","output":"ok"})],
        ..Default::default()
    });
    input.started_at = at + Duration::seconds(3);
    input.idempotency_key = Some("parallel-result".into());
    ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    sqlx::query("update flow_runs set started_at=$2 where id=(select flow_run_id from gateway_log_tool_results where conversation_id=$1 and call_id='call-1')")
        .bind(rows[0].0).bind(at+Duration::seconds(2)).execute(store.pool()).await.unwrap();
    store
        .rebuild_gateway_log_conversation(&scope, rows[0].0)
        .await
        .unwrap();
    let parallel = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                turn_id: Some(rows[0].1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        parallel
            .items
            .iter()
            .find(|item| item.id == first.flow_run.id)
            .unwrap()
            .metrics
            .tool_result_wait_ms,
        Some(3000)
    );
    let mut changed_tool = tool.clone();
    changed_tool["input"] = json!("different payload");
    append_formal_event(&store, &AppendRunEventInput {
            flow_run_id: first.flow_run.id,
            node_run_id: None,
            event_type: "provider_output_item_done".into(),
            payload: json!({"item":changed_tool}),
        })
        .await
        .unwrap();
    let conflict = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                turn_id: Some(rows[0].1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let conflict = conflict
        .items
        .iter()
        .find(|item| item.id == first.flow_run.id)
        .unwrap()
        .messages
        .iter()
        .find(|item| item.call_id.as_deref() == Some("call-1"))
        .unwrap();
    assert_eq!(conflict.identity_status, "conflicting_item");
    assert!(
        !conflict.result_received,
        "ambiguous calls cannot claim a trustworthy paired result"
    );
    sqlx::query("update flow_runs set status='succeeded',finished_at=started_at+interval '4 seconds' where id=$1").bind(first.flow_run.id).execute(store.pool()).await.unwrap();
    let elapsed = store
        .list_gateway_log_page(
            &scope,
            &GatewayLogQuery {
                turn_id: Some(rows[0].1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let elapsed = elapsed
        .items
        .iter()
        .find(|item| item.id == first.flow_run.id)
        .unwrap();
    assert_eq!(elapsed.metrics.elapsed_ms, Some(4000));
    assert_eq!(elapsed.metrics.model_duration_ms, Some(750));
    assert_eq!(elapsed.completion_status, "unknown");
    input.gateway_log_context = None;
    input.idempotency_key = Some("legacy".into());
    let legacy = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from gateway_log_invocations where flow_run_id=$1"
        )
        .bind(legacy.flow_run.id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        0
    );
    sqlx::query("delete from applications where id=$1")
        .bind(seeded.application_id)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from gateway_log_invocations")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        0
    );
}

// Exercise the production persistence entry, not a hand-inserted legacy event.
async fn append_formal_event(store:&PgControlPlaneStore,input:&AppendRunEventInput)->anyhow::Result<()> {
    use control_plane::ports::{RuntimeEventPayload,RuntimeEventSource,RuntimeEventDurability,RuntimeEventAfterCommitLane};
    control_plane::orchestration_runtime::persist_runtime_event_payload_with_after_commit(
        store,input.flow_run_id,&RuntimeEventPayload {
            event_type:input.event_type.clone(),source:RuntimeEventSource::Provider,
            durability:RuntimeEventDurability::DurableRequired,persist_required:true,trace_visible:true,payload:input.payload.clone(),
        },&RuntimeEventAfterCommitLane::empty(),
    ).await?;
    Ok(())
}
