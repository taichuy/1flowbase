use super::*;
use control_plane_contracts::ports::ApplicationRunTraceNodeProjectionInput;
use std::time::Duration as StdDuration;
use tokio::time::timeout;

const WAIT_LIMIT: StdDuration = StdDuration::from_secs(10);

fn projection(
    run: Uuid,
    source: Uuid,
    watermark: &str,
) -> ReplaceApplicationRunTraceProjectionInput {
    ReplaceApplicationRunTraceProjectionInput {
        flow_run_id: run,
        projection_version: 1,
        source_watermark: watermark.into(),
        nodes: vec![ApplicationRunTraceNodeProjectionInput {
            trace_node_id: Uuid::now_v7(),
            parent_trace_node_id: None,
            stable_locator: "locking-fixture".into(),
            node_kind: "node_run".into(),
            owner_kind: Some("node_run".into()),
            owner_id: Some(source.to_string()),
            order_key: "1".into(),
            node_id: None,
            node_type: Some("llm".into()),
            node_mode: None,
            node_alias: "Locking fixture".into(),
            status: "succeeded".into(),
            started_at: OffsetDateTime::now_utc(),
            finished_at: None,
            duration_ms: None,
            metrics_payload: json!({}),
            has_children: false,
            child_count: 0,
            has_content: false,
            content_ref: None,
            source_flow_run_id: Some(source),
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        }],
        contents: vec![],
    }
}

// Inspect a real lock dependency, rather than assuming a sleep schedules the writer.
async fn wait_for_blocked_query(pool: &sqlx::PgPool, blocker: i32, query_part: &str) {
    timeout(WAIT_LIMIT, async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where $1=any(pg_blocking_pids(pid)) and position($2 in query)>0)",
            )
            .bind(blocker)
            .bind(query_part)
            .fetch_one(pool)
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("production writer must reach the expected database lock wait");
}

async fn assert_projection(
    store: &PgControlPlaneStore,
    input: &ReplaceApplicationRunTraceProjectionInput,
) {
    let rows: Vec<(Uuid, Option<Uuid>, String)> = sqlx::query_as(
        "select trace_node_id, source_flow_run_id, source_watermark from application_run_trace_nodes where flow_run_id=$1",
    )
    .bind(input.flow_run_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![(
            input.nodes[0].trace_node_id,
            input.nodes[0].source_flow_run_id,
            input.source_watermark.clone()
        )]
    );
    let status: (String, String) = sqlx::query_as(
        "select status,source_watermark from application_run_trace_projection_statuses where flow_run_id=$1 and projection_version=$2",
    )
    .bind(input.flow_run_id)
    .bind(input.projection_version)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(status, ("succeeded".into(), input.source_watermark.clone()));
}

#[tokio::test]
async fn trace_projection_cross_run_summary_fk_does_not_deadlock() {
    let (pool, anchor) =
        crate::_tests::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let member = Uuid::now_v7();
    let application: Uuid = sqlx::query_scalar(
        "insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by,log_context) select $1,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by,jsonb_build_object('log_task_run_id',$2::text) from flow_runs where id=$2 returning application_id",
    )
    .bind(member)
    .bind(anchor)
    .fetch_one(&pool)
    .await
    .unwrap();
    let member_record = store
        .get_flow_run(application, member)
        .await
        .unwrap()
        .unwrap();
    let mut writer = pool.begin().await.unwrap();
    let writer_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    // The real sequencing helper represents the runtime writer's held member lock.
    flow_run_scope_id_for_update(&mut writer, member)
        .await
        .unwrap();
    let input = projection(anchor, member, "cross-run");
    let trace = store.replace_application_run_trace_projection(&input);
    tokio::pin!(trace);
    tokio::select! {
        result = &mut trace => panic!("trace must first wait on the member FK: {result:?}"),
        () = wait_for_blocked_query(&pool, writer_pid, "insert into application_run_trace_nodes") => {}
    }
    // This production helper writes the summary's FK to anchor while the trace
    // holds anchor and waits for member. FOR UPDATE creates the historic cycle;
    // NO KEY UPDATE allows this FK check to complete before member is released.
    timeout(
        WAIT_LIMIT,
        PgControlPlaneStore::upsert_application_run_log_summary_projection_for_flow_run(
            &mut writer,
            &member_record,
        ),
    )
    .await
    .expect("summary FK must not wait on trace's anchor lock")
    .expect("summary projection must succeed without a database deadlock");
    writer.commit().await.unwrap();
    timeout(WAIT_LIMIT, &mut trace).await.unwrap().unwrap();
    let summary_anchor: Uuid = sqlx::query_scalar(
        "select log_task_run_id from application_run_log_summaries where flow_run_id=$1",
    )
    .bind(member)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(summary_anchor, anchor);
    assert_projection(&store, &input).await;
}

#[tokio::test]
async fn trace_projection_same_run_writers_remain_serialized() {
    let (pool, run) = crate::_tests::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let initial = projection(run, run, "before");
    store
        .replace_application_run_trace_projection(&initial)
        .await
        .unwrap();
    let mut first = pool.begin().await.unwrap();
    let first_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *first)
        .await
        .unwrap();
    trace_projection_flow_run_scope_id_for_update(&mut first, run)
        .await
        .unwrap();
    let input = projection(run, run, "after");
    let second = store.replace_application_run_trace_projection(&input);
    tokio::pin!(second);
    tokio::select! {
        result = &mut second => panic!("same-run writer bypassed serialization: {result:?}"),
        () = wait_for_blocked_query(&pool, first_pid, "select flow_runs.scope_id") => {}
    }
    assert_projection(&store, &initial).await;
    first.commit().await.unwrap();
    timeout(WAIT_LIMIT, &mut second).await.unwrap().unwrap();
    assert_projection(&store, &input).await;
}
