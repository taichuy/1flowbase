use super::*;

#[tokio::test]
async fn migration_smoke_creates_application_public_run_state() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    let flow_run_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'flow_runs'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let tables: Vec<String> = sqlx::query_scalar(
        r#"
        select table_name
        from information_schema.tables
        where table_schema = $1
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let conversation_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'application_public_conversations'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let run_mode_check: String = sqlx::query_scalar(
        r#"
        select pg_get_constraintdef(c.oid)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'flow_runs'
          and c.conname = 'flow_runs_run_mode_check'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    let conversation_unique_column_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        join unnest(c.conkey) with ordinality as cols(attnum, ord) on true
        join pg_attribute a on a.attrelid = r.oid and a.attnum = cols.attnum
        where n.nspname = $1
          and r.relname = 'application_public_conversations'
          and c.contype = 'u'
          and a.attname in (
              'application_id',
              'api_key_id',
              'external_user',
              'external_conversation_id'
          )
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(flow_run_columns.contains(&"api_key_id".to_string()));
    assert!(flow_run_columns.contains(&"publication_version_id".to_string()));
    assert!(flow_run_columns.contains(&"external_user".to_string()));
    assert!(flow_run_columns.contains(&"external_conversation_id".to_string()));
    assert!(flow_run_columns.contains(&"external_trace_id".to_string()));
    assert!(flow_run_columns.contains(&"compatibility_mode".to_string()));
    assert!(flow_run_columns.contains(&"idempotency_key".to_string()));
    assert!(flow_run_columns.contains(&"updated_at".to_string()));
    assert!(run_mode_check.contains("debug_node_preview"));
    assert!(run_mode_check.contains("debug_flow_run"));
    assert!(run_mode_check.contains("published_api_run"));
    assert!(run_mode_check.contains("workflow_http_run"));
    assert!(run_mode_check.contains("workflow_schedule_run"));
    assert!(tables.contains(&"application_public_conversations".to_string()));
    assert!(conversation_columns.contains(&"id".to_string()));
    assert!(conversation_columns.contains(&"application_id".to_string()));
    assert!(conversation_columns.contains(&"api_key_id".to_string()));
    assert!(conversation_columns.contains(&"external_user".to_string()));
    assert!(conversation_columns.contains(&"external_conversation_id".to_string()));
    assert!(conversation_columns.contains(&"created_at".to_string()));
    assert!(conversation_columns.contains(&"updated_at".to_string()));
    assert_eq!(conversation_unique_column_count, 4);
}

#[tokio::test]
async fn migration_smoke_creates_system_default_upgrade_ledger() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    let tables: Vec<String> = sqlx::query_scalar(
        r#"
        select table_name
        from information_schema.tables
        where table_schema = $1
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let run_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'system_default_upgrade_runs'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let item_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'system_default_upgrade_items'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let run_status_check: String = sqlx::query_scalar(
        r#"
        select pg_get_constraintdef(c.oid)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'system_default_upgrade_runs'
          and c.conname = 'system_default_upgrade_runs_status_check'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    let item_status_check: String = sqlx::query_scalar(
        r#"
        select pg_get_constraintdef(c.oid)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'system_default_upgrade_items'
          and c.conname = 'system_default_upgrade_items_status_check'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(tables.contains(&"system_default_upgrade_runs".to_string()));
    assert!(tables.contains(&"system_default_upgrade_items".to_string()));
    assert!(run_columns.contains(&"system_version".to_string()));
    assert!(run_columns.contains(&"status".to_string()));
    assert!(run_columns.contains(&"requested_by".to_string()));
    assert!(run_columns.contains(&"summary_json".to_string()));
    assert!(run_columns.contains(&"error_message".to_string()));
    assert!(item_columns.contains(&"default_key".to_string()));
    assert!(item_columns.contains(&"target_kind".to_string()));
    assert!(item_columns.contains(&"target_id".to_string()));
    assert!(item_columns.contains(&"status".to_string()));
    assert!(item_columns.contains(&"skip_reason".to_string()));
    assert!(item_columns.contains(&"before_hash".to_string()));
    assert!(item_columns.contains(&"after_hash".to_string()));
    assert!(item_columns.contains(&"patch_json".to_string()));
    assert!(item_columns.contains(&"error_message".to_string()));
    assert!(item_columns.contains(&"created_at".to_string()));
    assert!(item_columns.contains(&"updated_at".to_string()));
    assert!(run_status_check.contains("running"));
    assert!(run_status_check.contains("pending"));
    assert!(run_status_check.contains("succeeded"));
    assert!(run_status_check.contains("failed"));
    assert!(run_status_check.contains("partially_applied"));
    assert!(item_status_check.contains("applied"));
    assert!(item_status_check.contains("skipped"));
    assert!(item_status_check.contains("failed"));
}
