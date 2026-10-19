use super::*;
use control_plane::agent_logs::AgentLogsService;
use control_plane_contracts::ports::{
    AgentLogsDeleteJobCreate, AgentLogsDeleteJobStatus, AgentLogsDeleteScope,
};

async fn setup() -> (
    PgControlPlaneStore,
    RuntimeSeedState,
    domain::CompiledPlanRecord,
    domain::ActorContext,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let actor = domain::ActorContext::root_in_scope(
        seeded.actor_user_id,
        root_tenant_id(&store).await,
        seeded.workspace_id,
        "root",
    );
    (store, seeded, compiled, actor)
}

async fn run(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    terminal: bool,
    at: OffsetDateTime,
) -> Uuid {
    let run = seed_flow_run_with_mode(
        store,
        seeded,
        compiled,
        at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    if terminal {
        finish(store, run.id).await;
    }
    run.id
}
async fn finish(store: &PgControlPlaneStore, id: Uuid) {
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({"answer":"retained until deletion"}),
            error_payload: None,
            finished_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
}
async fn group(store: &PgControlPlaneStore, anchor: Uuid, child: Uuid) {
    sqlx::query("delete from application_run_log_tasks where id=$1")
        .bind(child)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("update flow_runs set log_context=jsonb_build_object('log_task_run_id',$2::text) where id=$1").bind(child).bind(anchor.to_string()).execute(store.pool()).await.unwrap();
    sqlx::query("update application_run_log_summaries set log_task_run_id=$2 where flow_run_id=$1")
        .bind(child)
        .bind(anchor)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("select application_run_log_task_refresh($1)")
        .bind(anchor)
        .execute(store.pool())
        .await
        .unwrap();
}
fn input() -> AgentLogsDeleteJobCreate {
    AgentLogsDeleteJobCreate {
        job_id: Uuid::now_v7(),
        scope: AgentLogsDeleteScope::TimeRange {
            started_at_from: "2026-10-07T00:00:00Z".into(),
            started_at_to: "2026-10-08T00:00:00Z".into(),
            batch_size: std::num::NonZeroU32::new(1),
            ingested_at_before: None,
        },
    }
}
async fn exists(store: &PgControlPlaneStore, id: Uuid) -> bool {
    sqlx::query_scalar("select exists(select 1 from flow_runs where id=$1)")
        .bind(id)
        .fetch_one(store.pool())
        .await
        .unwrap()
}
async fn context(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    run_id: Uuid,
    value: Value,
) -> Uuid {
    let body = store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            content: value,
        })
        .await
        .unwrap();
    store
        .append_context_version(&AppendContextVersionInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run_id,
            parent_context_version_id: None,
            sequence: 0,
            transition_kind: ContextTransitionKind::Initial,
            transition_actor: ContextTransitionActor::Host,
            declared_compaction_provenance: None,
            actual_content_id: body.id,
        })
        .await
        .unwrap();
    body.id
}
async fn span(store: &PgControlPlaneStore, run_id: Uuid, parent: Option<Uuid>) -> Uuid {
    store
        .append_runtime_span(&AppendRuntimeSpanInput {
            flow_run_id: run_id,
            node_run_id: None,
            parent_span_id: parent,
            kind: domain::RuntimeSpanKind::Flow,
            name: "deletion boundary fixture".into(),
            status: domain::RuntimeSpanStatus::Succeeded,
            capability_id: None,
            input_ref: None,
            output_ref: None,
            error_payload: None,
            metadata: json!({}),
            started_at: OffsetDateTime::now_utc(),
            finished_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .unwrap()
        .id
}
async fn financial_snapshot(store: &PgControlPlaneStore) -> Value {
    sqlx::query_scalar("select jsonb_build_object(
        'cost',(select coalesce(jsonb_agg(to_jsonb(l) order by id),'[]') from runtime_cost_ledger l),
        'credit',(select coalesce(jsonb_agg(to_jsonb(l) order by id),'[]') from runtime_credit_ledger l),
        'usage',(select coalesce(jsonb_agg(to_jsonb(l)-'node_run_id'-'span_id' order by id),'[]') from runtime_usage_ledger l),
        'sessions',(select coalesce(jsonb_agg(to_jsonb(l) order by id),'[]') from billing_sessions l),
        'accounts',(select coalesce(jsonb_agg(to_jsonb(l) order by id),'[]') from user_credit_accounts l))")
        .fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn native_log_deletion_preserves_financial_history_shared_content_and_fixed_scope() {
    let (store, seeded, compiled, actor) = setup().await;
    let at = datetime!(2026-10-07 12:00 UTC);
    let first = run(&store, &seeded, &compiled, false, at).await;
    let child = run(&store, &seeded, &compiled, true, at).await;
    group(&store, first, child).await;
    let second = run(&store, &seeded, &compiled, true, at).await;
    let active = run(&store, &seeded, &compiled, false, at).await;
    let boundary = run(
        &store,
        &seeded,
        &compiled,
        true,
        datetime!(2026-10-08 00:00 UTC),
    )
    .await;
    let foreign = seed_runtime_base_with_workspace_name(&store, "foreign deletion scope").await;
    let foreign_plan = seed_compiled_plan(&store, &foreign).await;
    let foreign_run = run(&store, &foreign, &foreign_plan, true, at).await;
    let shared = context(&store, &seeded, first, json!({"shared":"keep"})).await;
    assert_eq!(
        context(&store, &seeded, active, json!({"shared":"keep"})).await,
        shared
    );
    let exclusive = context(&store, &seeded, child, json!({"exclusive":"reclaim"})).await;

    let owned_run = store
        .get_flow_run(seeded.application_id, first)
        .await
        .unwrap()
        .unwrap();
    let node = seed_node_run(&store, &owned_run, at).await.id;
    sqlx::query("update node_runs set status='succeeded',finished_at=now() where id=$1")
        .bind(node)
        .execute(store.pool())
        .await
        .unwrap();
    let parent_span = span(&store, first, None).await;
    let retained_span = span(&store, active, Some(parent_span)).await;
    finish(&store, first).await;
    let session = Uuid::now_v7();
    let usage = Uuid::now_v7();
    let cost = Uuid::now_v7();
    let credit = Uuid::now_v7();
    sqlx::query("insert into billing_sessions(id,workspace_id,flow_run_id,idempotency_key,status,actual_amount) values($1,$2,$3,'native-deletion-session','settled',1.25)").bind(session).bind(seeded.workspace_id).bind(first).execute(store.pool()).await.unwrap();
    sqlx::query("insert into runtime_usage_ledger(id,flow_run_id,node_run_id,usage_status,input_tokens,output_tokens,total_tokens) values($1,$2,$3,'recorded',10,20,30)").bind(usage).bind(first).bind(node).execute(store.pool()).await.unwrap();
    sqlx::query("insert into runtime_cost_ledger(id,workspace_id,flow_run_id,usage_ledger_id,billing_session_id,normalized_cost,cost_source,cost_status) values($1,$2,$3,$4,$5,1.25,'fixture','rated')").bind(cost).bind(seeded.workspace_id).bind(first).bind(usage).bind(session).execute(store.pool()).await.unwrap();
    sqlx::query("insert into runtime_credit_ledger(id,transaction_id,workspace_id,user_id,flow_run_id,cost_ledger_id,billing_session_id,transaction_type,amount,balance_after,credit_unit,reason,idempotency_key,status) values($1,$1,$2,$3,$4,$5,$6,'consume',-1.25,98.75,'USD','native log deletion retention fixture','native-deletion-credit','committed')").bind(credit).bind(seeded.workspace_id).bind(seeded.actor_user_id).bind(first).bind(cost).bind(session).execute(store.pool()).await.unwrap();
    let financial = financial_snapshot(&store).await;
    // Controlled negative: reintroduce one destructive FK in a rolled-back
    // transaction. The financial conservation assertion must reject that design.
    let mut negative = store.pool().begin().await.unwrap();
    sqlx::query("alter table runtime_cost_ledger add constraint native_deletion_negative foreign key(flow_run_id) references flow_runs(id) on delete cascade")
        .execute(&mut *negative).await.unwrap();
    sqlx::query("delete from flow_runs where id=$1")
        .bind(first)
        .execute(&mut *negative)
        .await
        .unwrap();
    let lost: i64 = sqlx::query_scalar("select count(*) from runtime_cost_ledger where id=$1")
        .bind(cost)
        .fetch_one(&mut *negative)
        .await
        .unwrap();
    assert_eq!(
        lost, 0,
        "controlled negative must demonstrate the destructive cascade"
    );
    negative.rollback().await.unwrap();
    assert_eq!(financial_snapshot(&store).await, financial);
    let service = AgentLogsService::new(store.for_actor(actor.clone()));
    let request = input();
    assert_eq!(
        service
            .preview_deletion(&actor, seeded.application_id, request.scope.clone())
            .await
            .unwrap()
            .total_records,
        2
    );
    let job = service
        .start_deletion(&actor, seeded.application_id, request.clone())
        .await
        .unwrap();
    assert_eq!(job.total_records, 2);
    assert_eq!(
        service
            .start_deletion(&actor, seeded.application_id, request)
            .await
            .unwrap()
            .job_id,
        job.job_id
    );
    let late = run(&store, &seeded, &compiled, true, at).await;
    // Finish an originally excluded run after the snapshot: still not selected.
    finish(&store, active).await;
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    let progress = service
        .deletion_job(&actor, seeded.application_id, Some(job.job_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!((progress.deleted_records, progress.total_records), (1, 2));
    assert!(!exists(&store, first).await && !exists(&store, child).await);
    assert!(exists(&store, second).await);
    assert_eq!(financial_snapshot(&store).await, financial);
    let retained_parent: Option<Uuid> =
        sqlx::query_scalar("select parent_span_id from runtime_spans where id=$1")
            .bind(retained_span)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        retained_parent.is_none(),
        "retained task span must survive its parent deletion"
    );
    let historical_usage: (Uuid, Option<Uuid>) =
        sqlx::query_as("select flow_run_id,node_run_id from runtime_usage_ledger where id=$1")
            .bind(usage)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(historical_usage, (first, None));
    let content_ids: Vec<Uuid> =
        sqlx::query_scalar("select id from runtime_canonical_contents where id=any($1)")
            .bind(vec![shared, exclusive])
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(content_ids, vec![shared]);
    // Recreate the command owner to represent a server restart.
    AgentLogsService::new(PgControlPlaneStore::new(store.pool().clone()))
        .advance_next_deletion()
        .await
        .unwrap();
    let done = service
        .deletion_job(&actor, seeded.application_id, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.status, AgentLogsDeleteJobStatus::Succeeded);
    assert_eq!(done.deleted_records, 2);
    for id in [active, boundary, foreign_run, late] {
        assert!(exists(&store, id).await);
    }
    assert_eq!(financial_snapshot(&store).await, financial);
    let remaining: i64 =
        sqlx::query_scalar("select count(*) from application_log_deletion_records where job_id=$1")
            .bind(job.job_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn native_log_deletion_changed_members_or_resumed_runs_roll_back_the_whole_batch() {
    let (store, seeded, compiled, actor) = setup().await;
    let at = datetime!(2026-10-07 12:00 UTC);
    let anchor = run(&store, &seeded, &compiled, true, at).await;
    let member = run(&store, &seeded, &compiled, false, at).await;
    group(&store, anchor, member).await;
    let service = AgentLogsService::new(store.for_actor(actor.clone()));
    assert_eq!(
        service
            .preview_deletion(&actor, seeded.application_id, input().scope)
            .await
            .unwrap()
            .total_records,
        0
    );
    finish(&store, member).await;
    let job = service
        .start_deletion(&actor, seeded.application_id, input())
        .await
        .unwrap();
    let late = run(&store, &seeded, &compiled, true, at).await;
    group(&store, anchor, late).await;
    assert!(store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .is_err());
    assert_eq!(
        service
            .deletion_job(&actor, seeded.application_id, None)
            .await
            .unwrap()
            .unwrap()
            .deleted_records,
        0
    );
    for id in [anchor, member, late] {
        assert!(exists(&store, id).await);
    }
    store
        .fail_agent_logs_delete_job(job.job_id, 0)
        .await
        .unwrap();
    let next = service
        .start_deletion(&actor, seeded.application_id, input())
        .await
        .unwrap();
    sqlx::query("update flow_runs set status='running',finished_at=null where id=$1")
        .bind(member)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .advance_agent_logs_delete_job(next.job_id)
        .await
        .is_err());
    assert!(exists(&store, anchor).await && exists(&store, member).await);
}

#[tokio::test]
async fn native_log_deletion_stop_and_authorization_preserve_uncommitted_tasks() {
    let (store, seeded, compiled, actor) = setup().await;
    let at = datetime!(2026-10-07 12:00 UTC);
    for _ in 0..3 {
        run(&store, &seeded, &compiled, true, at).await;
    }
    let service = AgentLogsService::new(store.for_actor(actor.clone()));
    let denied = domain::ActorContext::scoped_in_scope(
        actor.user_id,
        actor.tenant_id,
        seeded.workspace_id,
        "member",
        Vec::<String>::new(),
    );
    assert!(service
        .preview_deletion(&denied, seeded.application_id, input().scope)
        .await
        .is_err());
    assert!(service
        .start_deletion(&denied, seeded.application_id, input())
        .await
        .is_err());
    let job = service
        .start_deletion(&actor, seeded.application_id, input())
        .await
        .unwrap();
    assert!(store
        .get_agent_logs_delete_job(seeded.application_id, Uuid::now_v7(), Some(job.job_id))
        .await
        .unwrap()
        .is_none());
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    service
        .stop_deletion(&actor, seeded.application_id, job.job_id)
        .await
        .unwrap();
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    let stopped = service
        .deletion_job(&actor, seeded.application_id, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            stopped.status,
            stopped.deleted_records,
            stopped.total_records
        ),
        (AgentLogsDeleteJobStatus::Stopped, 1, 3)
    );
    assert_eq!(
        service
            .preview_deletion(&actor, seeded.application_id, input().scope)
            .await
            .unwrap()
            .total_records,
        2
    );
}
