use super::*;

#[tokio::test]
async fn agent_logs_bulk_reclaim_retains_one_application_lock_and_indexes_ownership_edges() {
    let (store, scope, app) = setup().await;
    let events = (0..256)
        .map(|n| event(&format!("bulk-{n}"), n, AgentLogEventKind::Context, None))
        .collect();
    AgentLogsService::new(store.clone())
        .ingest(app, scope, Uuid::now_v7(), batch(events))
        .await
        .unwrap();
    // Controlled negative: restore the previous official trigger function only inside a
    // rolled-back isolated transaction. The same cascade must expose one lock per body.
    let mut negative = store.pool().begin().await.unwrap();
    let old_function = include_str!(
        "../../../../../storage/durable/postgres/migrations/20260929103000_runtime_content_reference_lifecycle.sql"
    )
    .split("create constraint trigger")
    .next()
    .unwrap();
    sqlx::raw_sql(old_function)
        .execute(&mut *negative)
        .await
        .unwrap();
    sqlx::query("delete from application_run_log_tasks where application_id=$1")
        .bind(app)
        .execute(&mut *negative)
        .await
        .unwrap();
    sqlx::query("set constraints all immediate")
        .execute(&mut *negative)
        .await
        .unwrap();
    let previous_locks: i64 = sqlx::query_scalar(
        "select count(*) from pg_locks where pid=pg_backend_pid() and locktype='advisory'",
    )
    .fetch_one(&mut *negative)
    .await
    .unwrap();
    assert_eq!(
        previous_locks, 256,
        "negative must expose the prior unbounded footprint"
    );
    negative.rollback().await.unwrap();
    let mut tx = store.pool().begin().await.unwrap();
    sqlx::query("delete from application_run_log_tasks where application_id=$1")
        .bind(app)
        .execute(&mut *tx)
        .await
        .unwrap();
    // Flush the real deferred last-reference triggers before observing their lock footprint.
    sqlx::query("set constraints all immediate")
        .execute(&mut *tx)
        .await
        .unwrap();
    let advisory_locks: i64 = sqlx::query_scalar(
        "select count(*) from pg_locks where pid=pg_backend_pid() and locktype='advisory'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(
        advisory_locks, 1,
        "lock count must not grow with content count"
    );
    let remaining: i64 = sqlx::query_scalar(
        "select count(*) from runtime_canonical_contents where application_id=$1",
    )
    .bind(app)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
    tx.rollback().await.unwrap();
    let retained: i64 = sqlx::query_scalar(
        "select count(*) from application_log_upload_receipts where application_id=$1",
    )
    .bind(app)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(retained, 256);

    for (table, column) in [
        ("application_log_upload_receipts", "record_id"),
        ("application_log_upload_receipts", "step_id"),
        ("client_trajectory_captures", "record_id"),
        ("application_run_conversation_message_items", "record_id"),
        ("client_trajectory_steps", "request_id"),
        ("client_trajectory_sections", "request_id"),
        ("client_trajectory_sections", "step_id"),
        ("runtime_context_projections", "actual_content_id"),
        ("flow_run_recovery_history", "recovery_content_id"),
        ("runtime_legacy_shadow_rows", "canonical_content_id"),
    ] {
        let indexed: bool = sqlx::query_scalar(
            "select exists(select 1 from pg_index i join pg_attribute a on a.attrelid=i.indrelid and a.attnum=i.indkey[0] where i.indrelid=to_regclass($1) and a.attname=$2 and i.indisvalid)",
        )
        .bind(table)
        .bind(column)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert!(indexed, "missing leading ownership index: {table}.{column}");
    }
}

#[tokio::test]
async fn agent_logs_canonical_writer_and_reclaim_share_lock_without_blocking_other_application() {
    use std::time::Duration;
    let (store, scope, app) = setup().await;
    let other = seed_runtime_base(&store).await;
    let mut tx = store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("canonical-runtime-application:{app}"))
        .execute(&mut *tx)
        .await
        .unwrap();
    let writer_store = store.clone();
    let mut writer = tokio::spawn(async move {
        writer_store
            .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
                scope_id: scope,
                application_id: app,
                content: json!({"writer":"same application"}),
            })
            .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut writer)
            .await
            .is_err()
    );
    let independent = tokio::time::timeout(
        Duration::from_secs(5),
        store.put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: other.workspace_id,
            application_id: other.application_id,
            content: json!({"writer":"different application"}),
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_ne!(independent.application_id, app);
    tx.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), &mut writer)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
