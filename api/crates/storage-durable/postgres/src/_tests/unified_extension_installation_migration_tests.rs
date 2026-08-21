use sqlx::migrate::Migrator;
use std::borrow::Cow;
use storage_postgres::run_migrations;
use uuid::Uuid;

const UNIFIED_MIGRATION_VERSION: i64 = 20260803010000;

fn before_unified_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < UNIFIED_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

fn through_unified_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version <= UNIFIED_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

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
        "network_egress_providers",
        "node_contribution_registry",
        "plugin_assignments",
        "plugin_package_catalog_projection",
        "plugin_tasks",
        "plugin_worker_leases",
        "retained_frontend_module_assets",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(referencing_tables, expected);
}

#[tokio::test]
async fn root_ac_005_pre_034_upgrade_keeps_current_assignment_enabled_and_history_dormant() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    before_unified_migrator().run(&pool).await.unwrap();
    let user_id = Uuid::now_v7();
    sqlx::query(
        r#"insert into users (
            id, account, email, password_hash, name, nickname, status
        ) values ($1, $2, $3, 'fixture', 'Upgrade fixture', 'Upgrade fixture', 'active')"#,
    )
    .bind(user_id)
    .bind(format!("upgrade-fixture-{}", user_id.simple()))
    .bind(format!("upgrade-fixture-{}@example.com", user_id.simple()))
    .execute(&pool)
    .await
    .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name) values ($1, '00000000-0000-0000-0000-000000000001', 'Upgrade fixture')",
    )
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    let current_id = Uuid::now_v7();
    let historical_id = Uuid::now_v7();
    for (installation_id, version) in [(current_id, "0.3.4"), (historical_id, "0.3.1")] {
        sqlx::query(
            r#"insert into plugin_installations (
                id, provider_code, plugin_id, plugin_version, contract_version,
                protocol, display_name, source_kind, trust_level,
                verification_status, desired_state, artifact_status,
                runtime_status, availability_status, installed_path,
                signature_status, metadata_json, created_by, updated_by
            ) values (
                $1, 'upgrade_fixture', $2, $3, '1flowbase.provider/v2',
                'stdio_json', 'Upgrade fixture', 'official_registry',
                'verified_official', 'valid', 'disabled', 'ready', 'inactive',
                'disabled', $4, 'verified', '{"vendor":"taichuy"}'::jsonb, $5, $5
            )"#,
        )
        .bind(installation_id)
        .bind(format!("taichuy/upgrade_fixture@{version}"))
        .bind(version)
        .bind(format!("/tmp/upgrade_fixture/{version}"))
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"insert into extension_installations (
                id, category, organization, artifact_id, artifact_version,
                node_id, source, trust, local_path, checksum, signature_status,
                status, installed_by, is_current, application_action
            ) values (
                $1, 'runtime-extensions', 'taichuy', 'upgrade_fixture', $2,
                'node-upgrade', 'official', 'official', $3, 'sha256:fixture',
                'verified', 'installed', $4, $5, 'configure_model_provider'
            )"#,
        )
        .bind(Uuid::now_v7())
        .bind(version)
        .bind(format!("/tmp/upgrade_fixture/{version}"))
        .bind(user_id)
        .bind(installation_id == current_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"insert into plugin_artifact_instances (
                node_id, installation_id, local_version, local_checksum,
                installed_path, artifact_status, runtime_status
            ) values ('node-upgrade', $1, $2, 'sha256:fixture', $3, 'ready', 'inactive')"#,
        )
        .bind(installation_id)
        .bind(version)
        .bind(format!("/tmp/upgrade_fixture/{version}"))
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "insert into plugin_assignments (id, installation_id, workspace_id, provider_code, assigned_by) values ($1, $2, $3, 'upgrade_fixture', $4)",
    )
    .bind(Uuid::now_v7())
    .bind(current_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    through_unified_migrator().run(&pool).await.unwrap();

    let pre_lifecycle_upgrade_state: String =
        sqlx::query_scalar("select desired_state from extension_installations where id = $1")
            .bind(current_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pre_lifecycle_upgrade_state, "disabled");

    run_migrations(&pool).await.unwrap();

    let installations: Vec<(Uuid, String, String)> = sqlx::query_as(
        r#"select id, artifact_version, desired_state
           from extension_installations
           where artifact_id = 'upgrade_fixture'
           order by artifact_version"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        installations,
        vec![
            (historical_id, "0.3.1".into(), "disabled".into()),
            (current_id, "0.3.4".into(), "active_requested".into()),
        ]
    );
    let artifact: (String, bool) = sqlx::query_as(
        "select availability_status, is_current from extension_artifact_instances where installation_id = $1",
    )
    .bind(current_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(artifact, ("install_incomplete".into(), false));
    let assigned_id: Uuid = sqlx::query_scalar(
        "select installation_id from plugin_assignments where workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(assigned_id, current_id);
}
