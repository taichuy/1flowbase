use super::*;
use control_plane_contracts::application_public_runtime::ApplicationPublishedFlowRunRepository;
use control_plane_contracts::ports::ApplicationRunLogContext;

// Rework AC-001/002/006/008: the ORIGINAL list contracts own grouping.
#[tokio::test]
async fn issue_2032_rework_original_logs_collect_calls_without_merging_user_tasks() {
    let database = isolated_database().await;
    let store = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let other_key = seed_application_api_key(&store, &seeded).await;
    let mut input = CreateFlowRunInput {
        application_run_log_context: Some(ApplicationRunLogContext {
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
        status: FlowRunStatus::Running,
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

    let mut ids = Vec::new();
    for i in 0..6 {
        input.idempotency_key = Some(format!("rework-{i}"));
        if i == 4 {
            input.application_run_log_context.as_mut().unwrap().turn_id = Some("turn-2".into());
        }
        if i == 5 {
            input.api_key_id = Some(other_key);
        }
        if let Some(id) = ids.last() {
            input
                .application_run_log_context
                .as_mut()
                .unwrap()
                .previous_response_id = Some(format!("resp_{id}"));
        }
        input.application_run_log_context.as_mut().unwrap().prompt =
            Some(json!({"role":"user","content":"same question"}));
        input.application_run_log_context.as_mut().unwrap().tool_results=(0..i.min(3)).map(|n|json!({"type":"custom_tool_call_output","call_id":format!("call-{n}"),"output":format!("result-{n}")})).collect();
        let created =
            ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
                .await
                .unwrap();
        assert_eq!(created.flow_run.input_payload, input.input_payload);
        let item = if i < 3 {
            json!({"type":"custom_tool_call","id":format!("tool-{i}"),"call_id":format!("call-{i}"),"name":"exec","input":format!("console.log({i})")})
        } else {
            json!({"type":"message","id":format!("message-{i}"),"role":"assistant","phase":"final_answer","content":[{"type":"output_text","text":"done"}]})
        };
        for _ in 0..2 {
            store
                .append_runtime_event(&AppendRuntimeEventInput {
                    flow_run_id: created.flow_run.id,
                    node_run_id: None,
                    span_id: None,
                    parent_span_id: None,
                    event_type: "provider_output_item_done".into(),
                    layer: domain::RuntimeEventLayer::AgentTransition,
                    source: domain::RuntimeEventSource::Host,
                    trust_level: domain::RuntimeTrustLevel::HostFact,
                    item_id: None,
                    ledger_ref: None,
                    visibility: domain::RuntimeEventVisibility::Workspace,
                    durability: domain::RuntimeEventDurability::Durable,
                    payload: json!({"item":item}),
                })
                .await
                .unwrap();
        }
        // AC-003/009: a billed failed attempt followed by success stays inside
        // this invocation; task reads sum the original usage leaves once.
        if i == 0 {
            for (attempt_index, status, tokens, cost) in [
                (1, "failed", 40_i64, "0.001"),
                (2, "succeeded", 60_i64, "0.002"),
            ] {
                let attempt = Uuid::now_v7();
                sqlx::query("insert into model_provider_request_logs(id,scope_id,attempt_id,flow_run_id,application_name,attempt_index,provider_code,protocol,upstream_model_id,status,total_tokens,started_at,finished_at,pricing_provider_code,pricing_model_id,total_cost,currency_code,billing_status) select $1,scope_id,$2,id,'rework retry fixture',$3,'fixture','openai_compatible','fixture',$4,$5,started_at,now(),'fixture','fixture',$6::text::numeric,'USD','settled' from flow_runs where id=$7")
                    .bind(Uuid::now_v7()).bind(attempt).bind(attempt_index).bind(status).bind(tokens).bind(cost).bind(created.flow_run.id).execute(store.pool()).await.unwrap();
                sqlx::query("insert into model_failover_attempt_ledger(id,flow_run_id,attempt_index,provider_code,upstream_model_id,protocol,started_at,finished_at,status) select $1,id,$2,'fixture','fixture','openai_compatible',started_at,now(),$3 from flow_runs where id=$4")
                    .bind(attempt).bind(attempt_index).bind(status).bind(created.flow_run.id).execute(store.pool()).await.unwrap();
                sqlx::query("insert into runtime_usage_ledger(id,flow_run_id,failover_attempt_id,total_tokens,usage_status) values($1,$2,$3,$4,'recorded')")
                    .bind(Uuid::now_v7()).bind(created.flow_run.id).bind(attempt).bind(tokens).execute(store.pool()).await.unwrap();
            }
        }
        store
            .update_flow_run(&UpdateFlowRunInput {
                flow_run_id: created.flow_run.id,
                status: FlowRunStatus::Succeeded,
                output_payload: json!({"answer":"done"}),
                error_payload: None,
                finished_at: Some(OffsetDateTime::now_utc()),
            })
            .await
            .unwrap();
        ids.push(created.flow_run.id);
    }
    // AC-011: capture the actual shared query plan on this isolated six-call fixture.
    let summary_sql = include_str!("../../../../storage/durable/postgres/src/orchestration_runtime_repository/application_run_logs/task_summary.sql")
        .replace("/* log_scope */", &format!("application_id='{}'::uuid", seeded.application_id));
    let plan: serde_json::Value = sqlx::query_scalar(&format!(
        "explain (format json) select flow_run_id,invocation_count from ({summary_sql}) summaries order by created_at desc,flow_run_id desc limit 20"
    )).fetch_one(store.pool()).await.unwrap();
    eprintln!("original_log_task_query_plan={plan}");
    // AC-001/011: the original page consumes Runtime Data Model records.
    let metadata = store
        .list_runtime_model_metadata()
        .await
        .unwrap()
        .into_iter()
        .find(|model| model.model_code == "application_run_log_summaries")
        .unwrap();
    let runtime_page =
        storage_durable::runtime_record_repository::RuntimeRecordRepository::list_records(
            &store,
            &metadata,
            storage_durable::runtime_record_repository::RuntimeListQuery {
                scope_id: None,
                owner_user_id: None,
                filter: domain::ResourceFilterExpr::Field {
                    field: "application_id".into(),
                    operator: domain::ResourceFilterOperator::Eq,
                    value: json!(seeded.application_id),
                },
                sorts: vec![],
                expand_relations: vec![],
                page: 1,
                page_size: 100,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        runtime_page.total, 3,
        "original Runtime Data Model list must collect explicit tasks"
    );
    assert_eq!(
        runtime_page
            .items
            .iter()
            .find(|row| row["id"] == ids[0].to_string())
            .unwrap()["invocation_count"],
        4
    );
    assert_eq!(
        runtime_page
            .items
            .iter()
            .find(|row| row["id"] == ids[0].to_string())
            .unwrap()["total_tokens"],
        100
    );
    let attempts: (i64, String) = sqlx::query_as("select count(*),sum(total_cost)::text from model_provider_request_logs where flow_run_id=$1")
        .bind(ids[0]).fetch_one(store.pool()).await.unwrap();
    assert_eq!(attempts, (2, "0.003000000000000000".to_owned()));
    let hidden = storage_durable::runtime_record_repository::RuntimeRecordRepository::list_records(
        &store,
        &metadata,
        storage_durable::runtime_record_repository::RuntimeListQuery {
            scope_id: Some(Uuid::now_v7()),
            owner_user_id: None,
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 100,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        hidden.total, 0,
        "task aggregation must preserve runtime scope filtering"
    );

    let page = store
        .list_application_run_logs_page(
            seeded.application_id,
            ListApplicationRunsPageInput {
                page: 1,
                page_size: 100,
                created_after: None,
                sort_by: None,
                sort_order: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        page.total, 3,
        "four calls in one explicit task, second task and other credential remain separate"
    );
    assert!(page
        .items
        .iter()
        .all(|row| row.run.status == FlowRunStatus::Succeeded));
    // Original detail must retain a route into every real call in this task.
    let messages = store
        .list_application_run_conversation_message_items_page(
            seeded.application_id,
            ids[0],
            ListApplicationRunConversationMessageItemsPageInput {
                limit: 50,
                before_sequence: None,
                after_sequence: None,
            },
        )
        .await
        .unwrap();
    let detail_ids = messages
        .items
        .iter()
        .filter_map(|item| item.detail_run_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        detail_ids,
        ids[..4].iter().copied().collect(),
        "original detail contains all task calls only"
    );
    let projected=sqlx::query_scalar::<_,serde_json::Value>("select native_message->'_source_item' from application_run_conversation_message_items where flow_run_id=any($1) and source_item_key is not null order by flow_run_id,display_sequence").bind(&ids[..4]).fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        projected
            .iter()
            .filter(|item| item["type"] == "custom_tool_call")
            .count(),
        3,
        "formal native calls appear exactly once through original projection"
    );
    assert_eq!(
        projected
            .iter()
            .filter(|item| item["type"] == "custom_tool_call_output")
            .count(),
        3,
        "full history resends do not duplicate tool results"
    );
    assert!(projected.iter().any(|item| item["phase"] == "final_answer"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from flow_runs where application_id=$1")
            .bind(seeded.application_id)
            .fetch_one(store.pool())
            .await
            .unwrap(),
        6
    );
    assert_eq!(
        page.items
            .iter()
            .find(|item| item.run.id == ids[0])
            .unwrap()
            .tool_callback_count,
        3
    );
    let detail = store
        .get_application_run_detail(seeded.application_id, ids[0])
        .await
        .unwrap()
        .unwrap();
    let trace=control_plane::orchestration_runtime::trace_projection::build_application_run_trace_projection(&detail).unwrap();
    let tool = trace
        .contents
        .iter()
        .find(|node| node.content_kind == "tool_callback")
        .unwrap();
    assert_eq!(tool.payload["request_payload"]["type"], "custom_tool_call");
    assert_eq!(tool.payload["request_payload"]["input"], "console.log(0)");
    assert_eq!(
        tool.payload["callback_payload"]["type"],
        "custom_tool_call_output"
    );
    assert_eq!(tool.payload["callback_payload"]["output"], "result-0");
    assert_eq!(
        store
            .get_application_run_trace_projection_source_watermark(seeded.application_id, ids[0])
            .await
            .unwrap()
            .unwrap(),
        trace.source_watermark
    );
    let wrong = store
        .list_application_run_conversation_message_items_page(
            Uuid::now_v7(),
            ids[0],
            ListApplicationRunConversationMessageItemsPageInput {
                limit: 50,
                before_sequence: None,
                after_sequence: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(wrong.total_count, 0);
    let conversation_id = page
        .items
        .iter()
        .find(|item| item.run.id == ids[0])
        .unwrap()
        .log_conversation_id
        .unwrap();
    let history = store
        .list_application_conversation_runs_page(
            seeded.application_id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: conversation_id.to_string(),
                around_run_id: Some(ids[0]),
                before_run_id: None,
                after_run_id: None,
                limit: 50,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        history.items.len(),
        5,
        "original conversation includes both tasks, excludes other credential"
    );
    assert_eq!(
        store
            .expand_application_run_log_tasks(seeded.application_id, &[ids[0]])
            .await
            .unwrap(),
        ids[..4]
    );
    assert!(store
        .expand_application_run_log_tasks(Uuid::now_v7(), &[ids[0]])
        .await
        .is_err());

    // Explicit parentage is scoped; cursor causality never supplies a turn ID.
    input.api_key_id = Some(key);
    input.status = FlowRunStatus::Failed;
    input.idempotency_key = Some("child-task".into());
    input.application_run_log_context = Some(ApplicationRunLogContext {
        identity_status: "identified".into(),
        thread_id: Some("child-thread".into()),
        turn_id: Some("child-turn".into()),
        parent_thread_id: Some("thread-1".into()),
        parent_turn_id: Some("turn-1".into()),
        ..Default::default()
    });
    let child = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    let duplicate =
        ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
            .await
            .unwrap();
    assert_eq!(child.flow_run.id, duplicate.flow_run.id);
    let scope_cause: Option<String> =
        sqlx::query_scalar("select log_context->>'caused_by_run_id' from flow_runs where id=$1")
            .bind(ids[5])
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(scope_cause, None);
    for n in 0..2 {
        input.idempotency_key = Some(format!("unknown-{n}"));
        input.status = FlowRunStatus::Succeeded;
        input.application_run_log_context = Some(ApplicationRunLogContext {
            identity_status: "conflicting_identity".into(),
            thread_id: Some("thread-1".into()),
            turn_id: Some("turn-1".into()),
            ..Default::default()
        });
        let unknown =
            ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
                .await
                .unwrap();
        assert_eq!(unknown.flow_run.input_payload, input.input_payload);
        let detail = store
            .list_application_run_conversation_message_items_page(
                seeded.application_id,
                unknown.flow_run.id,
                ListApplicationRunConversationMessageItemsPageInput {
                    limit: 50,
                    before_sequence: None,
                    after_sequence: None,
                },
            )
            .await
            .unwrap();
        assert!(detail
            .items
            .iter()
            .any(|item| item.content.as_deref() == Some("same question")));
    }
    let final_page = store
        .list_application_run_logs_page(
            seeded.application_id,
            ListApplicationRunsPageInput {
                page: 1,
                page_size: 100,
                created_after: None,
                sort_by: None,
                sort_order: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(final_page.total, 6);
    let child_row = final_page
        .items
        .iter()
        .find(|row| row.run.id == child.flow_run.id)
        .unwrap();
    assert_eq!(child_row.parent_run_id, Some(ids[0]));
    assert_eq!(child_row.run.status, FlowRunStatus::Failed);
    assert_eq!(
        final_page
            .items
            .iter()
            .filter(|row| row.log_task_run_id.is_none())
            .count(),
        2
    );
    // AC-008/011: a live task member must remain visible beside terminal calls,
    // without writing a terminal message projection or duplicating its prompt.
    input.api_key_id = Some(key);
    input.idempotency_key = Some("rework-active-member".into());
    input.status = FlowRunStatus::Running;
    input.application_run_log_context = Some(ApplicationRunLogContext {
        identity_status: "identified".into(),
        thread_id: Some("thread-1".into()),
        turn_id: Some("turn-1".into()),
        ..Default::default()
    });
    let active = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    let live_page = store
        .list_application_run_conversation_message_items_page(
            seeded.application_id,
            ids[0],
            ListApplicationRunConversationMessageItemsPageInput {
                limit: 50,
                before_sequence: None,
                after_sequence: None,
            },
        )
        .await
        .unwrap();
    assert!(
        live_page
            .items
            .iter()
            .any(|item| item.detail_run_id == Some(active.flow_run.id) && item.status == "running"),
        "active member must be visible in original task detail"
    );
    let persisted: i64 = sqlx::query_scalar(
        "select count(*) from application_run_conversation_message_items where flow_run_id=$1",
    )
    .bind(active.flow_run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        persisted, 0,
        "reading a live member must not persist a terminal projection"
    );
    input.external_user = Some(String::new());
    input.idempotency_key = Some("rework-empty-external-user".into());
    let blank = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
        .await
        .unwrap();
    let grouped = store
        .list_application_run_logs_page(
            seeded.application_id,
            ListApplicationRunsPageInput {
                page: 1,
                page_size: 100,
                created_after: None,
                sort_by: None,
                sort_order: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        grouped.total, 6,
        "empty and absent external user retain the existing identity scope semantics"
    );
    assert_eq!(
        grouped
            .items
            .iter()
            .find(|row| row.run.id == ids[0])
            .unwrap()
            .invocation_count,
        6
    );
    let live = store
        .list_application_run_conversation_message_items_page(
            seeded.application_id,
            ids[0],
            ListApplicationRunConversationMessageItemsPageInput {
                limit: 50,
                before_sequence: None,
                after_sequence: None,
            },
        )
        .await
        .unwrap();
    assert!(live
        .items
        .iter()
        .any(|item| item.detail_run_id == Some(blank.flow_run.id)));
    // AC-007: a later call can contradict an already projected canonical item.
    // Reading the original owner must expose that conflict without a manual rebuild.
    store.append_runtime_event(&AppendRuntimeEventInput {
        flow_run_id: active.flow_run.id, node_run_id: None, span_id: None, parent_span_id: None,
        event_type: "provider_output_item_done".into(),
        layer: domain::RuntimeEventLayer::AgentTransition,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None, ledger_ref: None,
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
        payload: json!({"item":{"type":"custom_tool_call","id":"tool-0","call_id":"call-0","name":"exec","input":"different payload"}}),
    }).await.unwrap();
    store
        .list_application_run_conversation_message_items_page(
            seeded.application_id,
            ids[0],
            ListApplicationRunConversationMessageItemsPageInput {
                limit: 50,
                before_sequence: None,
                after_sequence: None,
            },
        )
        .await
        .unwrap();
    let conflicting: bool = sqlx::query_scalar("select (native_message->>'_log_conflicting')::boolean from application_run_conversation_message_items where flow_run_id=$1 and source_item_key='output:tool:call-0'")
        .bind(ids[0]).fetch_one(store.pool()).await.unwrap();
    assert!(
        conflicting,
        "later formal conflict must invalidate the original message projection"
    );
}
