use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

#[tokio::test]
async fn migration_smoke_creates_auth_and_workspace_tables() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    assert_eq!(root_tenant_code.as_deref(), Some("root-tenant"));
}

#[tokio::test]
async fn migration_smoke_creates_lifecycle_scoped_readiness_columns_and_indexes() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
        ("frontstage_page_schemas", vec!["page_id"]),
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();

    for table in [
        "roles",
        "role_permissions",
        "user_role_bindings",
        "plugin_installations",
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

async fn assert_scoped_readiness_columns_and_index(pool: &PgPool, schema: &str, table: &str) {
    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = $2
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
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
    .bind(schema)
    .bind(table)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(scope_nullable, "NO", "{table}.scope_id must be not null");

    let scope_index_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_indexes
        where schemaname = $1
          and tablename = $2
          and indexdef ilike '%(scope_id, created_at, id)%'
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        scope_index_count, 1,
        "missing {table} scope readiness index"
    );
}

async fn assert_primary_key_columns(
    pool: &PgPool,
    schema: &str,
    table: &str,
    expected_columns: &[&str],
) {
    let columns: Vec<String> = sqlx::query_scalar(
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
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(
        columns,
        expected_columns
            .iter()
            .map(|column| (*column).to_string())
            .collect::<Vec<_>>(),
        "{table} primary key should keep its business identity"
    );
}

async fn assert_unique_constraint_columns(
    pool: &PgPool,
    schema: &str,
    table: &str,
    expected_columns: &[&str],
) {
    let count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = $2
          and c.contype in ('u', 'p')
          and (
            select array_agg(a.attname::text order by cols.ord)
            from unnest(c.conkey) with ordinality as cols(attnum, ord)
            join pg_attribute a on a.attrelid = r.oid and a.attnum = cols.attnum
          ) = $3::text[]
        "#,
    )
    .bind(schema)
    .bind(table)
    .bind(expected_columns)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "{table} should keep unique identity on {expected_columns:?}"
    );
}

#[tokio::test]
async fn bootstrap_repository_upserts_password_local_and_root_user() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let root = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();

    assert_eq!(root.account, "root");
    assert!(store
        .list_permissions()
        .await
        .unwrap()
        .iter()
        .any(|permission| permission.code == "workspace.configure.all"));
    assert_eq!(
        store
            .find_authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
            .await
            .unwrap()
            .unwrap()
            .id,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
    );
    let root_identities: Vec<(String, String)> = sqlx::query_as(
        r#"
        select subject_type, subject_value
        from user_auth_identities
        where user_id = $1
        order by subject_type asc
        "#,
    )
    .bind(root.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        root_identities,
        vec![
            ("account".to_string(), "root".to_string()),
            ("email".to_string(), "root@example.com".to_string()),
        ]
    );
}

#[tokio::test]
async fn bootstrap_repository_preserves_password_local_saved_config_on_conflict() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            options: serde_json::json!({
                "description": "Local password authentication",
                "config_form_schema": [
                    {
                        "key": "description",
                        "label": "Description",
                        "type": "string",
                        "control": "textarea",
                        "read_only": false,
                        "required": false
                    }
                ],
                "extension_config": {}
            }),
        })
        .await
        .unwrap();

    let mut saved = store
        .find_authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
        .await
        .unwrap()
        .unwrap();
    saved.title = "Custom Password".into();
    saved.enabled = false;
    saved.options = serde_json::json!({
        "description": "Custom local password",
        "config_form_schema": [
            {
                "key": "description",
                "label": "Custom Description",
                "type": "string",
                "control": "textarea",
                "read_only": false,
                "required": false
            }
        ],
        "extension_config": {
            "lockout_after_attempts": 5
        }
    });
    store.update_authenticator_config(&saved).await.unwrap();

    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            options: serde_json::json!({
                "description": "Local password authentication",
                "config_form_schema": [
                    {
                        "key": "description",
                        "label": "Description",
                        "type": "string",
                        "control": "textarea",
                        "read_only": false,
                        "required": false
                    }
                ],
                "extension_config": {}
            }),
        })
        .await
        .unwrap();

    let after_bootstrap = store
        .find_authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_bootstrap.title, "Custom Password");
    assert!(!after_bootstrap.enabled);
    assert_eq!(
        after_bootstrap.options["description"],
        serde_json::json!("Custom local password")
    );
    assert_eq!(
        after_bootstrap.options["config_form_schema"][0]["label"],
        serde_json::json!("Custom Description")
    );
    assert_eq!(
        after_bootstrap.options["extension_config"],
        serde_json::json!({
            "lockout_after_attempts": 5
        })
    );
}

#[tokio::test]
async fn bootstrap_repository_overwrites_non_builtin_authenticator_on_conflict() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let oidc_id = Uuid::now_v7();

    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: oidc_id,
            auth_type: "oidc".into(),
            title: "OIDC".into(),
            enabled: false,
            is_builtin: false,
            sort_order: 10,
            options: serde_json::json!({
                "description": "Old OIDC",
                "extension_config": {
                    "issuer_url": "https://old.example.com"
                }
            }),
        })
        .await
        .unwrap();

    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: oidc_id,
            auth_type: "oidc".into(),
            title: "OIDC Login".into(),
            enabled: true,
            is_builtin: false,
            sort_order: 20,
            options: serde_json::json!({
                "description": "New OIDC",
                "extension_config": {
                    "issuer_url": "https://new.example.com"
                }
            }),
        })
        .await
        .unwrap();

    let oidc = store.find_authenticator(oidc_id).await.unwrap().unwrap();
    assert_eq!(oidc.title, "OIDC Login");
    assert!(oidc.enabled);
    assert_eq!(oidc.options["description"], serde_json::json!("New OIDC"));
    assert_eq!(
        oidc.options["extension_config"]["issuer_url"],
        serde_json::json!("https://new.example.com")
    );
}

#[tokio::test]
async fn migration_smoke_creates_plugin_trust_columns_and_constraints() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
          and table_name = 'plugin_installations'
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
    assert!(columns.contains(&"artifact_status".to_string()));
    assert!(columns.contains(&"runtime_status".to_string()));
    assert!(columns.contains(&"availability_status".to_string()));
    assert!(columns.contains(&"package_path".to_string()));
    assert!(columns.contains(&"installed_path".to_string()));
    assert!(columns.contains(&"manifest_fingerprint".to_string()));
    assert!(columns.contains(&"last_load_error".to_string()));
    assert!(!columns.contains(&"enabled".to_string()));
    assert!(!columns.contains(&"install_path".to_string()));
    assert!(task_columns.contains(&"status".to_string()));
    assert!(task_status_check.contains("queued"));
    assert!(task_status_check.contains("succeeded"));
}

#[tokio::test]
async fn migration_smoke_creates_plugin_artifact_instances_table() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
          and table_name = 'plugin_artifact_instances'
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
          and r.relname = 'plugin_artifact_instances'
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
          and r.relname = 'plugin_artifact_instances'
          and c.conname = 'plugin_artifact_instances_artifact_status_check'
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
    assert!(columns.contains(&"installed_path".to_string()));
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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

#[tokio::test]
async fn migration_smoke_creates_application_public_run_state() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
    assert!(run_mode_check.contains("published_api_run"));
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
    let pool = connect(&isolated_database_url().await).await.unwrap();
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
