use super::*;

#[tokio::test]
async fn input_cache_rate_migration_preserves_input_semantics_and_repairs_history() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    for (usage, expected) in [
        (
            json!({"input_tokens":4107,"cache_read_tokens":4096}),
            Some(4107_i64),
        ),
        (
            json!({"input_tokens":4107,"input_cache_miss_tokens":11,"input_cache_hit_tokens":4096}),
            Some(4107),
        ),
        (
            json!({"input_tokens":13,"input_cache_miss_tokens":13,"cache_read_tokens":250,"cache_write_tokens":37}),
            Some(300),
        ),
        (json!({"input_tokens":0,"cache_read_tokens":0}), Some(0)),
        (json!({"cache_read_tokens":0}), None),
    ] {
        let input: Option<i64> =
            sqlx::query_scalar("select application_run_log_usage_input_tokens($1)")
                .bind(usage)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(input, expected);
    }
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-10-02 09:00:00 UTC);
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let node = seed_node_run_for(&store, &run, "llm", "llm", "LLM", json!({}), started_at).await;
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_node_run(&store, &UpdateNodeRunInput {
        node_run_id: node.id, status: NodeRunStatus::Succeeded,
        output_payload: json!({"answer":"OK"}), error_payload: None,
        metrics_payload: json!({"usage":{"input_tokens":4107,"cache_read_tokens":4096,"output_tokens":14,"total_tokens":4121}}),
        debug_payload: json!({}), finished_at: Some(started_at + Duration::seconds(1)),
    }).await.unwrap();
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::update_flow_run(
        &store,
        &UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: json!({"answer":"OK"}),
            error_payload: None,
            finished_at: Some(started_at + Duration::seconds(2)),
        },
    )
    .await
    .unwrap();
    sqlx::query("update application_run_log_summaries set input_cache_hit_rate=4096.0/8203 where flow_run_id=$1")
        .bind(run.id).execute(store.pool()).await.unwrap();
    sqlx::query(
        "update application_run_log_tasks set input_cache_hit_rate=4096.0/8203 where id=$1",
    )
    .bind(run.id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../storage/durable/postgres/migrations/20261002120000_fix_input_cache_hit_rates.sql"
    )))
    .execute(store.pool())
    .await
    .unwrap();
    for query in [
        "select input_cache_hit_rate from application_run_log_summaries where flow_run_id=$1",
        "select input_cache_hit_rate from application_run_log_tasks where id=$1",
    ] {
        let rate: f64 = sqlx::query_scalar(query)
            .bind(run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert!((rate - 4096.0 / 4107.0).abs() < 1e-12);
    }
}
