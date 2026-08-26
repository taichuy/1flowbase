use super::*;

#[tokio::test]
async fn publisher_cutover_migration_marks_only_unified_legacy_runtime_receipts_idempotently() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let user_id = uuid::Uuid::now_v7();
    sqlx::query(
        "insert into users (id, account, email, password_hash, name, nickname, status) values ($1, $2, $3, 'x', 'Publisher Cutover', 'Publisher Cutover', 'active')",
    )
    .bind(user_id)
    .bind(format!("publisher-cutover-{user_id}"))
    .bind(format!("publisher-cutover-{user_id}@example.com"))
    .execute(&pool)
    .await
    .unwrap();
    let eligible_id = uuid::Uuid::now_v7();
    let strict_id = uuid::Uuid::now_v7();
    for (id, artifact_id, receipt) in [
        (
            eligible_id,
            "publisher_cutover_legacy",
            serde_json::json!({
                "migration": "unified_extension_installation_lifecycle",
                "legacy_plugin_installation_id": eligible_id,
            }),
        ),
        (strict_id, "publisher_cutover_strict", serde_json::json!({})),
    ] {
        sqlx::query(
            r#"insert into extension_installations (
                id, category, organization, artifact_id, artifact_version, plugin_id,
                contract_version, protocol, display_name, source_kind, trust_level,
                verification_status, desired_state, signature_status, receipt, created_by
            ) values ($1, 'runtime-extensions', '1flowbase', $2, '0.1.0', $3,
                '1flowbase.provider/v1', 'stdio_json', $2, 'official_registry',
                'verified_official', 'valid', 'active_requested', 'verified', $4, $5)"#,
        )
        .bind(id)
        .bind(artifact_id)
        .bind(format!("{artifact_id}@0.1.0"))
        .bind(receipt)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let migration_sql = include_str!(
        "../../../migrations/20260804010000_mark_publisher_cutover_legacy_manifest_compatibility.up.sql"
    );
    sqlx::raw_sql(migration_sql).execute(&pool).await.unwrap();
    sqlx::raw_sql(migration_sql).execute(&pool).await.unwrap();

    let rows: Vec<(uuid::Uuid, Option<String>)> = sqlx::query_as(
        "select id, receipt ->> 'legacy_manifest_compatibility' from extension_installations where id = any($1) order by id",
    )
    .bind(vec![eligible_id, strict_id])
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.iter()
            .find(|(id, _)| *id == eligible_id)
            .and_then(|(_, value)| value.as_deref()),
        Some("missing_publisher_namespace_v1")
    );
    assert_eq!(
        rows.iter()
            .find(|(id, _)| *id == strict_id)
            .and_then(|(_, value)| value.as_deref()),
        None
    );
}

#[tokio::test]
async fn migration_smoke_creates_plugin_trust_columns_and_constraints() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'extension_installations'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let task_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'plugin_tasks'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let task_status_check: String = sqlx::query_scalar(
        r#"
        select pg_get_constraintdef(c.oid)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'plugin_tasks'
          and c.conname = 'plugin_tasks_status_check'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(columns.contains(&"trust_level".to_string()));
    assert!(columns.contains(&"signature_algorithm".to_string()));
    assert!(columns.contains(&"signing_key_id".to_string()));
    assert!(columns.contains(&"desired_state".to_string()));
    assert!(columns.contains(&"expected_checksum".to_string()));
    assert!(columns.contains(&"is_system_reserved".to_string()));
    assert!(!columns.contains(&"artifact_status".to_string()));
    assert!(!columns.contains(&"runtime_status".to_string()));
    assert!(!columns.contains(&"availability_status".to_string()));
    assert!(!columns.contains(&"local_path".to_string()));
    assert!(!columns.contains(&"enabled".to_string()));
    assert!(!columns.contains(&"install_path".to_string()));
    assert!(task_columns.contains(&"status".to_string()));
    assert!(task_status_check.contains("queued"));
    assert!(task_status_check.contains("succeeded"));
}

#[tokio::test]
async fn migration_smoke_creates_extension_artifact_instances_table() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'extension_artifact_instances'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let primary_key_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select a.attname
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        join unnest(c.conkey) with ordinality as cols(attnum, ord) on true
        join pg_attribute a on a.attrelid = r.oid and a.attnum = cols.attnum
        where n.nspname = $1
          and r.relname = 'extension_artifact_instances'
          and c.contype = 'p'
        order by cols.ord
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let artifact_status_check: String = sqlx::query_scalar(
        r#"
        select pg_get_constraintdef(c.oid)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'extension_artifact_instances'
          and c.conname = 'extension_artifact_instances_artifact_status_check'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(columns.contains(&"node_id".to_string()));
    assert!(columns.contains(&"installation_id".to_string()));
    assert!(columns.contains(&"local_version".to_string()));
    assert!(columns.contains(&"local_checksum".to_string()));
    assert!(columns.contains(&"local_path".to_string()));
    assert!(columns.contains(&"package_path".to_string()));
    assert!(columns.contains(&"manifest_fingerprint".to_string()));
    assert!(columns.contains(&"availability_status".to_string()));
    assert!(columns.contains(&"is_current".to_string()));
    assert!(columns.contains(&"artifact_status".to_string()));
    assert!(columns.contains(&"runtime_status".to_string()));
    assert!(columns.contains(&"checked_at".to_string()));
    assert!(columns.contains(&"last_error".to_string()));
    assert_eq!(
        primary_key_columns,
        vec!["node_id".to_string(), "installation_id".to_string()]
    );
    assert!(artifact_status_check.contains("missing"));
    assert!(artifact_status_check.contains("ready"));
    assert!(artifact_status_check.contains("outdated"));
    assert!(artifact_status_check.contains("mismatched"));
    assert!(artifact_status_check.contains("corrupted"));
    assert!(artifact_status_check.contains("load_failed"));
}

#[tokio::test]
async fn migration_smoke_creates_external_bridge_tables() {
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
    let session_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'external_agent_sessions'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let telemetry_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'external_agent_telemetry_events'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(tables.contains(&"external_agent_sessions".to_string()));
    assert!(tables.contains(&"external_agent_telemetry_events".to_string()));
    assert!(session_columns.contains(&"workspace_id".to_string()));
    assert!(session_columns.contains(&"flow_run_id".to_string()));
    assert!(session_columns.contains(&"external_agent_kind".to_string()));
    assert!(session_columns.contains(&"external_session_id".to_string()));
    assert!(session_columns.contains(&"trust_level".to_string()));
    assert!(session_columns.contains(&"opaque_boundary_marked".to_string()));
    assert!(telemetry_columns.contains(&"external_agent_session_id".to_string()));
    assert!(telemetry_columns.contains(&"runtime_event_id".to_string()));
    assert!(telemetry_columns.contains(&"schema_version".to_string()));
    assert!(telemetry_columns.contains(&"signature_status".to_string()));
}
