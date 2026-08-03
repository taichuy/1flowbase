use super::*;

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
