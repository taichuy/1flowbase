use super::*;

#[tokio::test]
async fn migration_smoke_creates_auth_and_workspace_tables() {
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

    assert!(tables.contains(&"users".to_string()));
    assert!(tables.contains(&"roles".to_string()));
    assert!(tables.contains(&"permission_definitions".to_string()));
    assert!(tables.contains(&"authenticators".to_string()));
    assert!(tables.contains(&"workspaces".to_string()));
    assert!(tables.contains(&"workspace_memberships".to_string()));
}

#[tokio::test]
async fn migration_smoke_removes_legacy_model_provider_routing_table() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    let legacy_table_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from information_schema.tables
        where table_schema = $1
          and table_name = 'model_provider_routings'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(legacy_table_count, 0);
}

#[tokio::test]
async fn migration_smoke_creates_workspace_tables_and_workspace_scoped_indexes() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
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
    let workspace_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'workspaces'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let role_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'roles'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let audit_log_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'audit_logs'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let root_tenant_code: Option<String> =
        sqlx::query_scalar("select code from tenants where code = 'root-tenant'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    let permission_codes: Vec<String> =
        sqlx::query_scalar("select code from permission_definitions order by code")
            .fetch_all(&pool)
            .await
            .unwrap();

    assert!(tables.contains(&"tenants".to_string()));
    assert!(tables.contains(&"workspaces".to_string()));
    assert!(tables.contains(&"workspace_memberships".to_string()));
    assert!(workspace_columns.contains(&"tenant_id".to_string()));
    assert!(role_columns.contains(&"workspace_id".to_string()));
    assert!(role_columns.contains(&"auto_grant_new_permissions".to_string()));
    assert!(role_columns.contains(&"is_default_member_role".to_string()));
    assert!(audit_log_columns.contains(&"workspace_id".to_string()));
    assert!(permission_codes.contains(&"workspace.configure.all".to_string()));
    assert!(!permission_codes
        .iter()
        .any(|code| code.starts_with("route_page.")));
    assert_eq!(root_tenant_code.as_deref(), Some("root-tenant"));
}

#[tokio::test]
async fn migration_smoke_creates_lifecycle_scoped_readiness_columns_and_indexes() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    for (table, expected_columns) in [
        (
            "application_publication_versions",
            vec![
                "id",
                "scope_id",
                "created_at",
                "created_by",
                "updated_at",
                "updated_by",
            ],
        ),
        (
            "flow_versions",
            vec![
                "id",
                "scope_id",
                "created_at",
                "created_by",
                "updated_at",
                "updated_by",
            ],
        ),
        (
            "model_failover_queue_snapshots",
            vec!["id", "scope_id", "created_at", "created_by", "updated_by"],
        ),
    ] {
        let columns: Vec<String> = sqlx::query_scalar(
            r#"
            select column_name
            from information_schema.columns
            where table_schema = $1
              and table_name = $2
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_all(&pool)
        .await
        .unwrap();
        for expected_column in expected_columns {
            assert!(
                columns.contains(&expected_column.to_string()),
                "missing {table}.{expected_column}"
            );
        }

        let scope_index_count: i64 = sqlx::query_scalar(
            r#"
            select count(*)
            from pg_indexes
            where schemaname = $1
              and tablename = $2
              and indexdef ilike '%(scope_id, created_at, id)%'
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            scope_index_count, 1,
            "missing {table} scope readiness index"
        );
    }

    let publication_unique_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_indexes
        where schemaname = $1
          and tablename = 'application_publication_versions'
          and indexname = 'application_publication_versions_application_id_idx'
          and indexdef ilike '%unique%'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(publication_unique_count, 1);

    let flow_sequence_unique_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from information_schema.table_constraints constraints
        join information_schema.key_column_usage columns
          on columns.constraint_schema = constraints.constraint_schema
         and columns.constraint_name = constraints.constraint_name
        where constraints.table_schema = $1
          and constraints.table_name = 'flow_versions'
          and constraints.constraint_type = 'UNIQUE'
          and columns.column_name in ('flow_id', 'sequence')
        group by constraints.constraint_name
        having count(*) = 2
        "#,
    )
    .bind(&schema)
    .fetch_optional(&pool)
    .await
    .unwrap()
    .unwrap_or(0);
    assert_eq!(flow_sequence_unique_count, 2);

    let snapshot_template_delete_rule: String = sqlx::query_scalar(
        r#"
        select constraints.delete_rule
        from information_schema.referential_constraints constraints
        join information_schema.key_column_usage columns
          on columns.constraint_schema = constraints.constraint_schema
         and columns.constraint_name = constraints.constraint_name
        where columns.table_schema = $1
          and columns.table_name = 'model_failover_queue_snapshots'
          and columns.column_name = 'queue_template_id'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(snapshot_template_delete_rule, "RESTRICT");
}

#[tokio::test]
async fn migration_smoke_creates_system_global_scoped_readiness_columns_and_indexes() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let system_scope_id = "00000000-0000-0000-0000-000000000000";

    for table in [
        "file_storages",
        "frontend_block_catalog",
        "host_extension_migrations",
        "host_infrastructure_provider_configs",
        "js_dependency_registry",
        "node_contribution_registry",
        "permission_definitions",
        "system_default_upgrade_items",
        "system_default_upgrade_runs",
    ] {
        let columns: Vec<String> = sqlx::query_scalar(
            r#"
            select column_name
            from information_schema.columns
            where table_schema = $1
              and table_name = $2
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_all(&pool)
        .await
        .unwrap();
        for expected_column in [
            "id",
            "scope_id",
            "created_at",
            "created_by",
            "updated_at",
            "updated_by",
        ] {
            assert!(
                columns.contains(&expected_column.to_string()),
                "missing {table}.{expected_column}"
            );
        }

        let (scope_nullable, scope_default): (String, Option<String>) = sqlx::query_as(
            r#"
            select is_nullable, column_default
            from information_schema.columns
            where table_schema = $1
              and table_name = $2
              and column_name = 'scope_id'
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(scope_nullable, "NO", "{table}.scope_id must be not null");
        assert!(
            scope_default
                .as_deref()
                .is_some_and(|default| default.contains(system_scope_id)),
            "missing {table}.scope_id SYSTEM_SCOPE_ID default"
        );

        let check_constraint: String = sqlx::query_scalar(
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
        .bind(format!("{table}_system_scope_id_check"))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(check_constraint.contains("scope_id"));
        assert!(check_constraint.contains(system_scope_id));

        let scope_index_count: i64 = sqlx::query_scalar(
            r#"
            select count(*)
            from pg_indexes
            where schemaname = $1
              and tablename = $2
              and indexdef ilike '%(scope_id, created_at, id)%'
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            scope_index_count, 1,
            "missing {table} scope readiness index"
        );
    }

    for excluded_table in ["authenticators", "tenants", "users", "workspaces"] {
        let system_scope_check_count: i64 = sqlx::query_scalar(
            r#"
            select count(*)
            from pg_constraint c
            join pg_class r on r.oid = c.conrelid
            join pg_namespace n on n.oid = r.relnamespace
            where n.nspname = $1
              and r.relname = $2
              and c.conname = $3
            "#,
        )
        .bind(&schema)
        .bind(excluded_table)
        .bind(format!("{excluded_table}_system_scope_id_check"))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            system_scope_check_count, 0,
            "{excluded_table} must remain outside #1075 system/global migration"
        );
    }
}

#[tokio::test]
async fn migration_smoke_creates_identity_join_scoped_readiness_without_replacing_business_keys() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    let legacy_permission_table_exists: bool = sqlx::query_scalar(
        r#"
        select exists(
            select 1
            from information_schema.tables
            where table_schema = $1
              and table_name = 'api_key_data_model_permissions'
        )
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!legacy_permission_table_exists);

    for (table, expected_primary_key_columns) in [
        ("application_api_mappings", vec!["application_id"]),
        (
            "application_environment_variables",
            vec!["application_id", "name"],
        ),
        ("application_tag_bindings", vec!["application_id", "tag_id"]),
        ("frontstage_page_schemas", vec!["tab_id"]),
        ("main_source_defaults", vec!["workspace_id"]),
        (
            "model_provider_instance_secrets",
            vec!["provider_instance_id"],
        ),
        (
            "model_provider_main_instances",
            vec!["workspace_id", "provider_code"],
        ),
        (
            "provider_instance_model_catalog_cache",
            vec!["provider_instance_id"],
        ),
    ] {
        let columns: Vec<String> = sqlx::query_scalar(
            r#"
            select column_name
            from information_schema.columns
            where table_schema = $1
              and table_name = $2
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_all(&pool)
        .await
        .unwrap();
        for expected_column in [
            "id",
            "scope_id",
            "created_at",
            "created_by",
            "updated_at",
            "updated_by",
        ] {
            assert!(
                columns.contains(&expected_column.to_string()),
                "missing {table}.{expected_column}"
            );
        }

        let scope_nullable: String = sqlx::query_scalar(
            r#"
            select is_nullable
            from information_schema.columns
            where table_schema = $1
              and table_name = $2
              and column_name = 'scope_id'
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(scope_nullable, "NO", "{table}.scope_id must be not null");

        let primary_key_columns: Vec<String> = sqlx::query_scalar(
            r#"
            select a.attname
            from pg_constraint c
            join pg_class r on r.oid = c.conrelid
            join pg_namespace n on n.oid = r.relnamespace
            join unnest(c.conkey) with ordinality as cols(attnum, ord) on true
            join pg_attribute a on a.attrelid = r.oid and a.attnum = cols.attnum
            where n.nspname = $1
              and r.relname = $2
              and c.contype = 'p'
            order by cols.ord
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            primary_key_columns,
            expected_primary_key_columns
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>(),
            "{table} primary key must remain the business identity"
        );
        assert!(
            !primary_key_columns.contains(&"id".to_string()),
            "{table}.id must remain a platform tie-breaker, not the functional identity"
        );

        let scope_index_count: i64 = sqlx::query_scalar(
            r#"
            select count(*)
            from pg_indexes
            where schemaname = $1
              and tablename = $2
              and indexdef ilike '%(scope_id, created_at, id)%'
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            scope_index_count, 1,
            "missing {table} scope readiness index"
        );
    }

    let role_join_tables = ["role_permissions", "user_role_bindings"];
    for table in role_join_tables {
        let has_scope_id: bool = sqlx::query_scalar(
            r#"
            select exists(
                select 1
                from information_schema.columns
                where table_schema = $1
                  and table_name = $2
                  and column_name = 'scope_id'
            )
            "#,
        )
        .bind(&schema)
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            has_scope_id,
            "{table} should be covered by the later role owner-chain migration"
        );
    }
}

#[tokio::test]
async fn migration_smoke_creates_remaining_owner_review_scoped_readiness() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    for table in [
        "roles",
        "role_permissions",
        "user_role_bindings",
        "extension_installations",
        "plugin_tasks",
        "plugin_worker_leases",
        "data_source_secrets",
        "data_source_catalog_caches",
        "audit_logs",
        "flow_drafts",
    ] {
        assert_scoped_readiness_columns_and_index(&pool, &schema, table).await;
    }

    assert_primary_key_columns(
        &pool,
        &schema,
        "data_source_secrets",
        &["data_source_instance_id"],
    )
    .await;
    assert_primary_key_columns(
        &pool,
        &schema,
        "data_source_catalog_caches",
        &["data_source_instance_id"],
    )
    .await;
    assert_unique_constraint_columns(
        &pool,
        &schema,
        "role_permissions",
        &["role_id", "permission_id"],
    )
    .await;
    assert_unique_constraint_columns(
        &pool,
        &schema,
        "user_role_bindings",
        &["user_id", "role_id"],
    )
    .await;
    assert_unique_constraint_columns(&pool, &schema, "flow_drafts", &["flow_id"]).await;

    let plugin_task_scope_check_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = 'plugin_tasks'
          and c.contype = 'c'
          and pg_get_constraintdef(c.oid) ilike '%scope_kind%'
          and pg_get_constraintdef(c.oid) ilike '%workspace_id%'
        "#,
    )
    .bind(&schema)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        plugin_task_scope_check_count, 1,
        "plugin_tasks should enforce task kind and workspace scope consistency"
    );
}
