use storage_postgres::run_migrations;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

#[tokio::test]
async fn issue_1566_ac_001_schema_has_one_installation_root_and_node_artifacts() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    let tables: Vec<String> = sqlx::query_scalar(
        "select table_name from information_schema.tables where table_schema = $1",
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(tables.contains(&"extension_installations".to_string()));
    assert!(tables.contains(&"extension_artifact_instances".to_string()));
    assert!(!tables.contains(&"plugin_installations".to_string()));
    assert!(!tables.contains(&"plugin_artifact_instances".to_string()));

    let installation_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1 and table_name = 'extension_installations'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    for column in [
        "category",
        "organization",
        "artifact_id",
        "artifact_version",
        "plugin_id",
        "contract_version",
        "protocol",
        "desired_state",
        "expected_checksum",
        "is_system_reserved",
    ] {
        assert!(
            installation_columns.contains(&column.to_string()),
            "missing logical installation column {column}"
        );
    }
    for node_column in [
        "node_id",
        "local_path",
        "checksum",
        "status",
        "is_current",
        "installed_path",
        "artifact_status",
        "runtime_status",
        "availability_status",
        "last_load_error",
    ] {
        assert!(
            !installation_columns.contains(&node_column.to_string()),
            "node artifact column leaked into installation root: {node_column}"
        );
    }

    let artifact_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1 and table_name = 'extension_artifact_instances'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    for column in [
        "installation_id",
        "node_id",
        "local_path",
        "local_checksum",
        "artifact_status",
        "runtime_status",
        "availability_status",
        "is_current",
    ] {
        assert!(
            artifact_columns.contains(&column.to_string()),
            "missing node artifact column {column}"
        );
    }
}

#[tokio::test]
async fn issue_1566_ac_003_all_installation_references_target_the_unified_root() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    let mut referencing_tables: Vec<String> = sqlx::query_scalar(
        r#"
        select distinct child.relname
        from pg_constraint relation
        join pg_class child on child.oid = relation.conrelid
        join pg_namespace child_namespace on child_namespace.oid = child.relnamespace
        join pg_class parent on parent.oid = relation.confrelid
        join pg_namespace parent_namespace on parent_namespace.oid = parent.relnamespace
        where relation.contype = 'f'
          and child_namespace.nspname = $1
          and parent_namespace.nspname = $1
          and parent.relname = 'extension_installations'
        order by child.relname
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    referencing_tables.sort();

    let mut expected = vec![
        "application_extension_sources",
        "application_js_dependency_selections",
        "data_source_instances",
        "extension_artifact_instances",
        "frontend_block_catalog",
        "host_infrastructure_provider_configs",
        "js_dependency_registry",
        "mcp_extension_bundle_imports",
        "model_provider_instances",
        "model_provider_preview_sessions",
        "node_contribution_registry",
        "plugin_assignments",
        "plugin_package_catalog_projection",
        "plugin_tasks",
        "plugin_worker_leases",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(referencing_tables, expected);
}
