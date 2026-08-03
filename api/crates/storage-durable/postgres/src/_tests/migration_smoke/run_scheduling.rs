use super::*;

#[tokio::test]
async fn migration_smoke_keeps_active_run_statuses_when_adding_incomplete() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    for (table, constraint) in [
        ("flow_runs", "flow_runs_status_check"),
        (
            "application_run_log_summaries",
            "application_run_log_summaries_status_check",
        ),
    ] {
        let status_check: String = sqlx::query_scalar(
            r#"
            select pg_get_constraintdef(c.oid)
            from pg_constraint c
            join pg_class r on r.oid = c.conrelid
            join pg_namespace n on n.oid = r.relnamespace
            where n.nspname = $1
              and r.relname = $2
              and c.conname = $3
            "#,
        )
        .bind(&schema)
        .bind(table)
        .bind(constraint)
        .fetch_one(&pool)
        .await
        .unwrap();

        for expected_status in [
            "queued",
            "running",
            "waiting_callback",
            "waiting_human",
            "paused",
            "succeeded",
            "incomplete",
            "failed",
            "cancelled",
        ] {
            assert!(
                status_check.contains(expected_status),
                "{table}.{constraint} must allow {expected_status}"
            );
        }
    }
}

#[test]
fn workflow_schedule_idempotency_index_migration_only_adds_the_partial_unique_index() {
    let normalized = WORKFLOW_SCHEDULE_IDEMPOTENCY_INDEX_MIGRATION
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    assert!(normalized.contains(
        "create unique index if not exists flow_runs_workflow_schedule_idempotency_unique_idx on flow_runs (application_id, idempotency_key) where run_mode = 'workflow_schedule_run' and idempotency_key is not null"
    ));
    for forbidden_statement in ["alter table", "insert into", "update ", "delete from"] {
        assert!(
            !normalized.contains(forbidden_statement),
            "schedule idempotency migration must not contain {forbidden_statement}"
        );
    }
}

#[tokio::test]
async fn migration_smoke_creates_workflow_schedule_idempotency_partial_unique_index() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let index_definition: String = sqlx::query_scalar(
        r#"
        select indexdef
        from pg_indexes
        where schemaname = $1
          and tablename = 'flow_runs'
          and indexname = 'flow_runs_workflow_schedule_idempotency_unique_idx'
        "#,
    )
    .bind(schema)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(index_definition.starts_with("CREATE UNIQUE INDEX"));
    assert!(index_definition.contains("(application_id, idempotency_key)"));
    assert!(index_definition.contains("run_mode = 'workflow_schedule_run'::text"));
    assert!(index_definition.contains("idempotency_key IS NOT NULL"));
    assert!(!index_definition.contains("api_key_id"));
}
