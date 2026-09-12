use super::*;
use control_plane_contracts::application_public_runtime::ApplicationPublishedFlowRunRepository;
use control_plane_contracts::ports::ApplicationRunLogContext;
use sqlx::migrate::Migrator;
use std::borrow::Cow;

const CALL_KIND_MIGRATION_VERSION: i64 = 20260912120000;

fn call_kind_fixture_input(
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    key: Uuid,
) -> CreateFlowRunInput {
    CreateFlowRunInput {
        application_run_log_context: None,
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
        title: "refactor login".into(),
        status: FlowRunStatus::Running,
        input_payload: json!({"query":"refactor login","history":[]}),
        started_at: OffsetDateTime::now_utc(),
        api_key_id: Some(key),
        publication_version_id: None,
        assistant_conversation_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: Some("openai-responses".into()),
        idempotency_key: None,
    }
}

async fn record_usage(store: &PgControlPlaneStore, flow_run_id: Uuid, tokens: i64) {
    sqlx::query("insert into runtime_usage_ledger(id,flow_run_id,total_tokens,usage_status) values($1,$2,$3,'recorded')")
        .bind(Uuid::now_v7()).bind(flow_run_id).bind(tokens).execute(store.pool()).await.unwrap();
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({"answer":"done"}),
            error_payload: None,
            finished_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
}

// #2034 AC-001/005/006: a compaction call stays inside its user task, is counted
// separately, keeps its tokens in the task total, and the protocol comes from
// the mapping layer rather than the storage binder.
#[tokio::test]
async fn issue_2034_compaction_calls_stay_in_task_with_separate_count() {
    let database = isolated_database().await;
    let store = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let mut input = call_kind_fixture_input(&seeded, &compiled, key);
    let mut ids = Vec::new();
    for (index, call_kind) in ["generate", "compact", "generate"].iter().enumerate() {
        input.idempotency_key = Some(format!("call-kind-{index}"));
        input.application_run_log_context = Some(ApplicationRunLogContext {
            identity_status: "identified".into(),
            protocol: Some("fixture_protocol".into()),
            call_kind: Some((*call_kind).into()),
            request_kind: Some(
                if *call_kind == "compact" {
                    "compaction"
                } else {
                    "turn"
                }
                .into(),
            ),
            thread_id: Some("thread-K".into()),
            turn_id: Some("turn-K".into()),
            ..Default::default()
        });
        let created =
            ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
                .await
                .unwrap();
        // #2036 AC-009: two generations on the same node run, with two
        // provider attempts each, contribute two invocations rather than four.
        // A compaction span never contributes; the last legacy run has no spans.
        if index < 2 {
            let node = store
                .create_node_run(&CreateNodeRunInput {
                    flow_run_id: created.flow_run.id,
                    node_id: "node-1".into(),
                    node_type: "llm".into(),
                    node_alias: "LLM".into(),
                    status: domain::NodeRunStatus::Running,
                    input_payload: json!({}),
                    debug_payload: json!({}),
                    started_at: OffsetDateTime::now_utc(),
                })
                .await
                .unwrap();
            for _ in 0..(if index == 0 { 2 } else { 1 }) {
                let span = Uuid::now_v7();
                sqlx::query("insert into runtime_spans(id,flow_run_id,node_run_id,kind,name,status,started_at) values($1,$2,$3,'llm_turn','LLM','running',now())")
                    .bind(span).bind(created.flow_run.id).bind(node.id)
                    .execute(store.pool()).await.unwrap();
                for attempt in 0..2_i32 {
                    sqlx::query("insert into model_failover_attempt_ledger(id,flow_run_id,node_run_id,llm_turn_span_id,attempt_index,provider_code,upstream_model_id,protocol,started_at,status) values($1,$2,$3,$4,$5,'fixture','fixture','responses',now(),'succeeded')")
                        .bind(Uuid::now_v7()).bind(created.flow_run.id).bind(node.id)
                        .bind(span).bind(attempt).execute(store.pool()).await.unwrap();
                }
            }
        }
        record_usage(&store, created.flow_run.id, 1000 * (index as i64 + 1)).await;
        // Re-projecting an unchanged outcome cannot increment the count.
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
    // AC-005: the binder persists the declared protocol verbatim.
    let protocol: Option<String> =
        sqlx::query_scalar("select log_context->>'protocol' from flow_runs where id=$1")
            .bind(ids[0])
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(protocol.as_deref(), Some("fixture_protocol"));
    let kinds: Vec<String> = sqlx::query_scalar(
        "select call_kind from application_run_log_summaries where flow_run_id=any($1) order by flow_run_id",
    )
    .bind(&ids)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(kinds, ["generate", "compact", "generate"]);

    // AC-001: one task row, generate calls counted, compaction counted apart, tokens include all.
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
    assert_eq!(page.total, 1);
    let task = &page.items[0];
    assert_eq!(task.run.id, ids[0]);
    assert_eq!(task.invocation_count, 3);
    assert_eq!(task.compaction_count, 1);
    assert_eq!(task.total_tokens, Some(6000));
    assert_eq!(task.call_kind, "generate");
    let metadata = store
        .list_runtime_model_metadata()
        .await
        .unwrap()
        .into_iter()
        .find(|model| model.model_code == "application_run_log_tasks")
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
    assert_eq!(runtime_page.items[0]["invocation_count"], 3);
    assert_eq!(runtime_page.items[0]["compaction_count"], 1);
    assert_eq!(runtime_page.items[0]["call_kind"], "generate");

    // AC-006 (#2034) → #2035: task membership is owned by the task projection;
    // the conversation scope helper remains the single conversation definition.
    let conversation_members: Vec<Uuid> = sqlx::query_scalar(
        "select run_id from application_run_log_conversation_runs($1,$2) order by run_id",
    )
    .bind(seeded.application_id)
    .bind(
        task.log_conversation_id
            .expect("identified calls bind a log conversation"),
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(conversation_members, expected);
    assert_eq!(
        store
            .expand_application_run_log_tasks(seeded.application_id, &[ids[1]])
            .await
            .unwrap(),
        ids
    );
    let messages = store
        .list_application_run_conversation_message_items_page(
            seeded.application_id,
            ids[1],
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
    assert_eq!(detail_ids, std::iter::once(ids[1]).collect());
}

// #2034 AC-007: existing summaries gain call_kind from the retained native
// operation; the constant invocation_count placeholder becomes a generated contribution.
#[tokio::test]
async fn issue_2034_migration_backfills_call_kind_and_retires_placeholder_column() {
    let pool = isolated_database().await.connect().await.unwrap();
    let prior = Migrator {
        migrations: Cow::Owned(
            sqlx::migrate!("../storage/durable/postgres/migrations")
                .iter()
                .filter(|m| m.version < CALL_KIND_MIGRATION_VERSION)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    prior.run(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let mut runs = Vec::new();
    for (index, operation) in [
        json!({"kind":"generate","profile":"standard"}),
        json!({"kind":"compact","profile":"responses_compaction_v2"}),
        json!({"kind":"generate","profile":"local_summary"}),
    ]
    .into_iter()
    .enumerate()
    {
        let run = Uuid::now_v7();
        sqlx::query("insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,input_payload,created_by,api_key_id,finished_at,log_context) values($1,$2,$3,$4,$5,'published_api_run','succeeded',$6,$7,$8,now(),$9)")
            .bind(run).bind(seeded.application_id).bind(seeded.flow_id).bind(seeded.draft_id).bind(compiled.id)
            .bind(json!({"node-1":{"operation":operation}}))
            .bind(seeded.actor_user_id).bind(key)
            .bind(json!({"identity_status":"identified","thread_id":"legacy","turn_id":"legacy-turn","protocol":"openai_responses"}))
            .execute(store.pool()).await.unwrap();
        sqlx::query("insert into application_run_log_summaries(flow_run_id,scope_id,application_id,run_mode,status,title,input_payload,api_key_id,total_tokens,started_at,finished_at,created_at,updated_at) select id,scope_id,application_id,run_mode,status,'legacy',input_payload,api_key_id,$2,started_at,finished_at,created_at,updated_at from flow_runs where id=$1")
            .bind(run).bind(10 * (index as i64 + 1)).execute(store.pool()).await.unwrap();
        // #2036 AC-009 migration: existing formal generations replace the old
        // per-run contribution; rows without facts retain their stored values.
        if index == 0 {
            for _ in 0..2 {
                sqlx::query("insert into runtime_spans(id,flow_run_id,kind,name,status,started_at) values($1,$2,'llm_turn','LLM','running',now())")
                    .bind(Uuid::now_v7()).bind(run).execute(store.pool()).await.unwrap();
            }
        }
        runs.push(run);
    }
    run_migrations(store.pool()).await.unwrap();
    let kinds: Vec<(Uuid, String, Option<i64>)> = sqlx::query_as(
        "select flow_run_id,call_kind,total_tokens from application_run_log_summaries where flow_run_id=any($1) order by flow_run_id",
    )
    .bind(&runs)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(kinds[0], (runs[0], "generate".into(), Some(10)));
    assert_eq!(kinds[1], (runs[1], "compact".into(), Some(20)));
    assert_eq!(kinds[2], (runs[2], "compact".into(), Some(30)));
    // #2036 retains the column as a projection of logical generation facts;
    // the former generated expression and constant check are both gone.
    let generated: Option<String> = sqlx::query_scalar(
        "select is_generated from information_schema.columns where table_schema=current_schema() and table_name='application_run_log_summaries' and column_name='invocation_count'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(generated.as_deref(), Some("NEVER"));
    let constant_check: bool = sqlx::query_scalar(
        "select exists(select 1 from pg_constraint where conrelid='application_run_log_summaries'::regclass and pg_get_constraintdef(oid) like '%invocation_count = 1%')",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(
        !constant_check,
        "constant invocation_count placeholder is retired"
    );
    let contributions: Vec<(i64, i64)> = sqlx::query_as(
        "select invocation_count,compaction_count from application_run_log_summaries where flow_run_id=any($1) order by flow_run_id",
    )
    .bind(&runs)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(contributions, [(2, 0), (0, 1), (0, 1)]);
    let task_count: i64 = sqlx::query_scalar(
        "select sum(invocation_count)::bigint from application_run_log_tasks where application_id=$1",
    )
    .bind(seeded.application_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(task_count, 2);
}
