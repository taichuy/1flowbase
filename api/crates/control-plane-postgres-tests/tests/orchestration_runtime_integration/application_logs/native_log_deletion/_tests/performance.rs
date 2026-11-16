use super::*;
use std::time::Instant;

const INDEX_MIGRATION: &str = include_str!("../../../../../../storage/durable/postgres/migrations/20261116093000_runtime_log_deletion_probe_indexes.sql");

async fn populate(store: &PgControlPlaneStore, run_id: Uuid, population: i32) {
    sqlx::query("insert into node_runs(id,flow_run_id,scope_id,node_id,node_type,node_alias,status) select gen_random_uuid(),$1,(select scope_id from flow_runs where id=$1),'probe-'||n,'code','Probe','succeeded' from generate_series(1,8) n")
        .bind(run_id).execute(store.pool()).await.unwrap();
    sqlx::query("insert into runtime_spans(id,flow_run_id,node_run_id,kind,name,status,started_at) select gen_random_uuid(),$1,(select id from node_runs where flow_run_id=$1 limit 1),'node','probe','succeeded',now() from generate_series(1,$2)")
        .bind(run_id).bind(population).execute(store.pool()).await.unwrap();
    sqlx::query("with nodes as materialized (select gen_random_uuid() id,n from generate_series(1,$2) n) insert into application_run_trace_nodes(trace_node_id,id,flow_run_id,scope_id,stable_locator,node_kind,order_key,node_alias,status,started_at,projection_version,source_watermark) select id,id,$1,(select scope_id from flow_runs where id=$1),'probe-'||n,'node_run',n::text,'Probe','succeeded',now(),1,'fixture' from nodes n")
        .bind(run_id).bind(population).execute(store.pool()).await.unwrap();
    // Retained graphs have actual parent links, not just empty partial indexes.
    sqlx::query("update application_run_trace_nodes set parent_trace_node_id=(select trace_node_id from application_run_trace_nodes where flow_run_id=$1 and stable_locator='probe-1') where flow_run_id=$1 and stable_locator<>'probe-1'")
        .bind(run_id).execute(store.pool()).await.unwrap();
}

async fn probe(pool: &sqlx::PgPool, sql: &str, key: Uuid) -> Value {
    // Only compiled-in probe queries enter this helper; all identities are bound.
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "explain (analyze, buffers, format json) {sql}"
    )))
    .bind(key)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn sample(population: i32, indexed: bool) -> Value {
    let (store, seeded, compiled, actor) = setup().await;
    if !indexed {
        // Controlled old-design fixture: remove ONLY this candidate's indexes,
        // in a wholly owned temporary database with all formal migrations.
        for line in INDEX_MIGRATION.lines() {
            if let Some(index) = line.strip_prefix("create index ") {
                sqlx::query(sqlx::AssertSqlSafe(format!("drop index {index}")))
                    .execute(store.pool())
                    .await
                    .unwrap();
            }
        }
    }
    let mut selected = Vec::new();
    for _ in 0..2 {
        let anchor = run(
            &store,
            &seeded,
            &compiled,
            true,
            datetime!(2026-10-07 12:00 UTC),
        )
        .await;
        let child = run(
            &store,
            &seeded,
            &compiled,
            true,
            datetime!(2026-10-07 12:00 UTC),
        )
        .await;
        group(&store, anchor, child).await;
        for id in [anchor, child] {
            populate(&store, id, 128).await;
            selected.push(id);
        }
    }
    // Many distinct leading run IDs prevent an unrealistically cheap skip scan
    // of the existing (flow_run_id, parent_id, ...) index on a one-run fixture.
    let mut retained = Vec::new();
    for _ in 0..256 {
        let id = run(
            &store,
            &seeded,
            &compiled,
            true,
            datetime!(2026-10-08 12:00 UTC),
        )
        .await;
        populate(&store, id, population / 256).await;
        retained.push(id);
    }
    for table in ["application_run_trace_nodes", "runtime_spans", "node_runs"] {
        sqlx::query(sqlx::AssertSqlSafe(format!("analyze {table}")))
            .execute(store.pool())
            .await
            .unwrap();
    }
    let absent = Uuid::now_v7();
    let mut plans = Vec::new();
    for query in [
        "select 1 from application_run_trace_nodes where parent_trace_node_id=$1",
        "select 1 from runtime_spans where parent_span_id=$1",
        "select 1 from runtime_spans where node_run_id=$1",
    ] {
        let plan = probe(store.pool(), query, absent).await;
        if indexed {
            assert!(!plan.to_string().contains("Seq Scan"), "{query}: {plan}");
        }
        plans.push(plan);
    }
    let retained_before: Value = sqlx::query_scalar("select jsonb_build_object('traces',(select count(*) from application_run_trace_nodes where flow_run_id=any($1)),'spans',(select count(*) from runtime_spans where flow_run_id=any($1)),'nodes',(select count(*) from node_runs where flow_run_id=any($1)))")
        .bind(&retained).fetch_one(store.pool()).await.unwrap();
    let service = AgentLogsService::new(store.for_actor(actor.clone()));
    let mut request = input();
    if let AgentLogsDeleteScope::TimeRange { batch_size, .. } = &mut request.scope {
        *batch_size = std::num::NonZeroU32::new(2);
    }
    let job = service
        .start_deletion(&actor, seeded.application_id, request)
        .await
        .unwrap();
    assert_eq!(
        job.total_records, 2,
        "two logical tasks containing four native runs"
    );
    let started = Instant::now();
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let receipt = service
        .deletion_job(&actor, seeded.application_id, Some(job.job_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.status, AgentLogsDeleteJobStatus::Succeeded);
    assert_eq!((receipt.deleted_records, receipt.total_records), (2, 2));
    for id in selected {
        assert!(!exists(&store, id).await);
    }
    let retained_runs: i64 = sqlx::query_scalar("select count(*) from flow_runs where id=any($1)")
        .bind(&retained)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(retained_runs, 256);
    let retained_after: Value = sqlx::query_scalar("select jsonb_build_object('traces',(select count(*) from application_run_trace_nodes where flow_run_id=any($1)),'spans',(select count(*) from runtime_spans where flow_run_id=any($1)),'nodes',(select count(*) from node_runs where flow_run_id=any($1)))")
        .bind(&retained).fetch_one(store.pool()).await.unwrap();
    assert_eq!(retained_before, retained_after);
    json!({"retained_population":population,"indexed":indexed,"batch_commit_ms":elapsed_ms,"probe_plans":plans,"retained":retained_after,"deleted_records":receipt.deleted_records})
}

#[tokio::test]
async fn native_log_deletion_probe_indexes_remove_retained_table_scan_amplification() {
    let mut samples = Vec::new();
    for population in [4096, 16384] {
        samples.push(sample(population, false).await);
        samples.push(sample(population, true).await);
    }
    // Timing is measured, not a flaky absolute-time gate. Probe plans and exact
    // service results are assertions; receipts retain raw timing/buffer evidence.
    let receipt = json!({"samples":samples});
    println!("cleanup_performance={receipt}");
    if let Ok(path) = std::env::var("ONEFLOWBASE_CLEANUP_PERFORMANCE_RECEIPT") {
        std::fs::write(path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    }
}

#[tokio::test]
async fn native_log_deletion_probe_indexes_cover_single_column_references() {
    let (store, _, _, _) = setup().await;
    let missing: Vec<String> = sqlx::query_scalar(
        "with recursive targets(oid) as (
            select 'flow_runs'::regclass::oid union
            select c.conrelid from pg_constraint c join targets p on c.confrelid=p.oid
            where c.contype='f' and c.confdeltype='c'
         ) select c.conname from pg_constraint c join targets p on c.confrelid=p.oid
         where c.contype='f' and cardinality(c.conkey)=1
           and not exists(select 1 from pg_index i where i.indrelid=c.conrelid
               and i.indisvalid and i.indisready and i.indkey[0]=c.conkey[1])",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert!(
        missing.is_empty(),
        "unbounded per-parent probes: {missing:?}"
    );
}
