use super::*;
use control_plane_contracts::ports::ApplicationLogConversationsQuery;
use sqlx::migrate::Migrator;
use std::borrow::Cow;

const MIGRATION_VERSION: i64 = 20261013160000;

fn context(thread: &str) -> ApplicationRunLogContext {
    ApplicationRunLogContext {
        identity_status: "identified".into(),
        protocol: Some("openai_responses".into()),
        call_kind: Some("generate".into()),
        thread_id: Some(thread.into()),
        turn_id: Some(format!("turn-{thread}")),
        prompt: Some(json!({"role":"user","content":thread})),
        ..Default::default()
    }
}

async fn create_task(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    key: Uuid,
    context: ApplicationRunLogContext,
    tokens: i64,
) -> Uuid {
    let thread = context.thread_id.clone().unwrap();
    let mut input = task_fixture_input(seeded, compiled, key, &thread);
    input.idempotency_key = Some(thread.clone());
    input.application_run_log_context = Some(context);
    let id = ApplicationPublishedFlowRunRepository::create_published_flow_run(store, &input)
        .await
        .unwrap()
        .flow_run
        .id;
    append_output_item(store, id, json!({"type":"message","id":format!("answer-{thread}"),"role":"assistant","phase":"final_answer","content":[{"type":"output_text","text":format!("answer {thread}")}]})).await;
    sqlx::query("insert into runtime_cost_ledger(id,flow_run_id,workspace_id,normalized_cost,cost_source,cost_status) select gen_random_uuid(),id,scope_id,$2::bigint,'local_token_pricing','rated' from flow_runs where id=$1")
        .bind(id).bind(tokens).execute(store.pool()).await.unwrap();
    finish(store, id, tokens).await;
    id
}

// Real published-run service supplies the fork provenance and parent resolution.
// Parent-thread-only, subagent-only, both declarations, and a plain child must
// remain children; manual fork is the sole corrected relation.
async fn fixture(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    key: Uuid,
) -> Vec<Uuid> {
    let mut ids = Vec::new();
    ids.push(create_task(store, seeded, compiled, key, context("source"), 10).await);
    for (thread, parent, subagent, fork) in [
        ("manual-fork-needle", false, false, true),
        ("delegated-fork", true, true, true),
        ("parent-only-fork", true, false, true),
        ("subagent-only-fork", false, true, true),
        ("plain-child", true, false, false),
    ] {
        let mut ctx = context(thread);
        ctx.parent_thread_id = parent.then(|| "source".into());
        ctx.subagent_kind = subagent.then(|| "review".into());
        ctx.forked_from_thread_id = fork.then(|| "source".into());
        ids.push(create_task(store, seeded, compiled, key, ctx, 20).await);
    }
    ids
}

async fn assert_relations(store: &PgControlPlaneStore, ids: &[Uuid]) {
    for (index, id) in ids.iter().enumerate() {
        let task: (Option<Uuid>, bool, Option<Uuid>) = sqlx::query_as(
            "select parent_task_run_id,is_root,native_run_id from application_run_log_tasks where id=$1",
        ).bind(id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(task.0, (index >= 2).then_some(ids[0]), "task {index}");
        assert_eq!(task.1, index < 2, "generated root flag {index}");
        assert_eq!(task.2, Some(*id));
        if index > 0 {
            let provenance: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
                "select (f.log_context->>'parent_run_id')::uuid,s.parent_run_id from flow_runs f join application_run_log_summaries s on s.flow_run_id=f.id where f.id=$1",
            ).bind(id).fetch_one(store.pool()).await.unwrap();
            assert_eq!(
                provenance,
                (Some(ids[0]), Some(ids[0])),
                "source provenance {index}"
            );
        }
    }
}

fn query(filter: domain::ResourceFilterExpr) -> ApplicationLogConversationsQuery {
    ApplicationLogConversationsQuery {
        filter,
        sort_field: "started_at".into(),
        descending: false,
        page: 1,
        page_size: 100,
    }
}

fn field(
    name: &str,
    operator: domain::ResourceFilterOperator,
    value: serde_json::Value,
) -> domain::ResourceFilterExpr {
    domain::ResourceFilterExpr::Field {
        field: name.into(),
        operator,
        value,
    }
}

#[tokio::test]
async fn manual_fork_is_visible_as_separate_root_with_source_provenance_and_costs() {
    let database = isolated_database().await;
    let store = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let ids = fixture(&store, &seeded, &compiled, key).await;
    assert_relations(&store, &ids).await;
    let all = query(domain::ResourceFilterExpr::All(vec![]));
    let page = store
        .query_application_log_conversations(seeded.workspace_id, &[seeded.application_id], &all)
        .await
        .unwrap();
    assert_eq!(
        page.total, 2,
        "fork source and manual fork are separate main conversations"
    );
    let source = page.items.iter().find(|r| r.record_id == ids[0]).unwrap();
    let fork = page.items.iter().find(|r| r.record_id == ids[1]).unwrap();
    assert_ne!(source.log_conversation_id, fork.log_conversation_id);
    for (row, tokens) in [(source, 10), (fork, 20)] {
        assert_eq!(row.total_tokens, Some(tokens));
        assert_eq!(
            row.total_cost.as_ref().unwrap().parse::<f64>().unwrap(),
            tokens as f64
        );
    }
    for filter in [
        field(
            "user_input",
            domain::ResourceFilterOperator::Includes,
            json!("needle"),
        ),
        field(
            "client_thread_id",
            domain::ResourceFilterOperator::Eq,
            json!("manual-fork-needle"),
        ),
    ] {
        let matched = store
            .query_application_log_conversations(
                seeded.workspace_id,
                &[seeded.application_id],
                &query(filter),
            )
            .await
            .unwrap();
        assert_eq!(matched.total, 1);
        assert_eq!(matched.items[0].record_id, ids[1]);
    }
    for id in &ids[2..] {
        let matched = store
            .query_application_log_conversations(
                seeded.workspace_id,
                &[seeded.application_id],
                &query(field("id", domain::ResourceFilterOperator::Eq, json!(id))),
            )
            .await
            .unwrap();
        assert_eq!(
            matched.total, 0,
            "declared child stays out of main matching"
        );
    }
    let listed = store
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
    assert_eq!(listed.total, 2);
    assert!(listed.items.iter().any(|r| r.run.id == ids[1]));
    let detail = store
        .get_application_run_detail(seeded.application_id, ids[1])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detail.flow_run.id, ids[1]);
    assert!(detail.child_task_traces.is_empty());
    let source_detail = store
        .get_application_run_detail(seeded.application_id, ids[0])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(source_detail.child_task_traces.len(), 4);
    assert!(source_detail
        .child_task_traces
        .iter()
        .all(|child| child.source_flow_run.id != ids[1]));
    let record = store
        .application_log_record_conversation_page(seeded.application_id, ids[1], None, None, 20)
        .await
        .unwrap();
    assert_eq!(record.items.len(), 1);
    assert_eq!(record.items[0].record_id, ids[1]);
    assert_eq!(record.items[0].parent_task_run_id, None);
    assert!(record.items[0]
        .messages
        .iter()
        .any(|message| message.content == "answer manual-fork-needle"));
    assert_eq!(
        record.items[0].log_conversation_id,
        fork.log_conversation_id
    );
    assert!(store
        .query_application_log_conversations(Uuid::now_v7(), &[seeded.application_id], &all)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(store
        .query_application_log_conversations(seeded.workspace_id, &[Uuid::now_v7()], &all)
        .await
        .unwrap()
        .items
        .is_empty());
}

// Full JSON snapshots protect raw metadata, source/summary facts, messages,
// usage, costs, and every task field except the two intended derived fields.
async fn retained_facts(store: &PgControlPlaneStore, application: Uuid) -> serde_json::Value {
    sqlx::query_scalar(r#"select jsonb_build_object(
        'runs',(select jsonb_agg(to_jsonb(f) order by id) from flow_runs f where application_id=$1),
        'summaries',(select jsonb_agg(to_jsonb(s) order by flow_run_id) from application_run_log_summaries s where application_id=$1),
        'tasks',(select jsonb_agg(to_jsonb(t)-'parent_task_run_id'-'is_root' order by id) from application_run_log_tasks t where application_id=$1),
        'messages',(select jsonb_agg(to_jsonb(m) order by id) from application_run_conversation_message_items m where application_id=$1),
        'events',(select jsonb_agg(to_jsonb(e) order by id) from runtime_events e where flow_run_id in (select id from flow_runs where application_id=$1)),
        'usage',(select jsonb_agg(to_jsonb(u) order by id) from runtime_usage_ledger u where flow_run_id in (select id from flow_runs where application_id=$1)),
        'costs',(select jsonb_agg(to_jsonb(c) order by id) from runtime_cost_ledger c where flow_run_id in (select id from flow_runs where application_id=$1)))"#)
        .bind(application).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn manual_fork_forward_migration_repairs_populated_projection_without_rewriting_facts() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    let prior = Migrator {
        migrations: Cow::Owned(
            sqlx::migrate!("../storage/durable/postgres/migrations")
                .iter()
                .filter(|migration| migration.version < MIGRATION_VERSION)
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
    let ids = fixture(&store, &seeded, &compiled, key).await;
    let old: (Option<Uuid>, bool) = sqlx::query_as(
        "select parent_task_run_id,is_root from application_run_log_tasks where id=$1",
    )
    .bind(ids[1])
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        old,
        (Some(ids[0]), false),
        "fixture reproduces old hidden manual fork"
    );
    let imported = Uuid::now_v7();
    sqlx::query("insert into application_run_log_tasks(id,application_id,scope_id,member_run_ids,parent_task_run_id,source_kind,source_id,source_session_id,source_task_id,run_mode,status,outcome,title,call_kind,started_at,created_at,updated_at) select $1,application_id,scope_id,'{}'::uuid[],id,'imported','fixture','session','child','imported','succeeded','no_final_answer','imported child','generate',started_at,created_at,updated_at from application_run_log_tasks where id=$2")
        .bind(imported).bind(ids[0]).execute(store.pool()).await.unwrap();
    let before = retained_facts(&store, seeded.application_id).await;
    run_migrations(store.pool()).await.unwrap();
    assert_relations(&store, &ids).await;
    assert_eq!(retained_facts(&store, seeded.application_id).await, before);
    let imported_parent: Option<Uuid> =
        sqlx::query_scalar("select parent_task_run_id from application_run_log_tasks where id=$1")
            .bind(imported)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(imported_parent, Some(ids[0]));
    for _ in 0..2 {
        run_migrations(store.pool()).await.unwrap();
        for id in &ids {
            sqlx::query("select application_run_log_task_refresh($1)")
                .bind(id)
                .execute(store.pool())
                .await
                .unwrap();
        }
        assert_relations(&store, &ids).await;
        assert_eq!(retained_facts(&store, seeded.application_id).await, before);
    }
}

#[tokio::test]
async fn manual_fork_parent_resolution_keeps_caller_application_and_scope_boundaries() {
    let database = isolated_database().await;
    let store = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    create_task(&store, &seeded, &compiled, key, context("source"), 10).await;
    let other_key = seed_application_api_key(&store, &seeded).await;
    for (index, key) in [other_key, key, key].into_iter().enumerate() {
        let mut ctx = context(&format!("isolated-fork-{index}"));
        ctx.forked_from_thread_id = Some("source".into());
        if index == 1 {
            ctx.protocol = Some("other_protocol".into());
        }
        let mut input = task_fixture_input(&seeded, &compiled, key, "isolated fork");
        input.idempotency_key = Some(format!("isolated-{index}"));
        input.application_run_log_context = Some(ctx);
        if index == 2 {
            input.external_user = Some("different-user".into());
        }
        let id = ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
            .await
            .unwrap()
            .flow_run
            .id;
        finish(&store, id, 20).await;
        let row: (Option<Uuid>, Option<Uuid>, bool) = sqlx::query_as("select (f.log_context->>'parent_run_id')::uuid,t.parent_task_run_id,t.is_root from flow_runs f join application_run_log_tasks t on t.id=f.id where f.id=$1")
            .bind(id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(
            row,
            (None, None, true),
            "fork must not resolve foreign caller source {index}"
        );
    }
    let foreign = seed_runtime_base_with_workspace_name(&store, "Foreign fork scope").await;
    let foreign_compiled = seed_compiled_plan(&store, &foreign).await;
    let foreign_key = seed_application_api_key(&store, &foreign).await;
    let mut ctx = context("foreign-application-fork");
    ctx.forked_from_thread_id = Some("source".into());
    let foreign_id = create_task(&store, &foreign, &foreign_compiled, foreign_key, ctx, 30).await;
    let row: (Option<Uuid>, Option<Uuid>, bool) = sqlx::query_as("select (f.log_context->>'parent_run_id')::uuid,t.parent_task_run_id,t.is_root from flow_runs f join application_run_log_tasks t on t.id=f.id where f.id=$1")
        .bind(foreign_id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(row, (None, None, true));
    let all = query(domain::ResourceFilterExpr::All(vec![]));
    for (scope, application) in [
        (seeded.workspace_id, foreign.application_id),
        (foreign.workspace_id, seeded.application_id),
    ] {
        assert!(store
            .query_application_log_conversations(scope, &[application], &all)
            .await
            .unwrap()
            .items
            .is_empty());
    }
}
