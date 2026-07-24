use std::borrow::Cow;

use sqlx::migrate::Migrator;
use storage_postgres::PgControlPlaneStore;
use uuid::Uuid;

const MIGRATION_VERSION: i64 = 20260714203000;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

fn before_member_role_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn historical_store() -> (PgControlPlaneStore, Uuid, Uuid, Uuid) {
    let pool = isolated_database().await.connect().await.unwrap();
    before_member_role_migrator().run(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "historical-workspace")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    sqlx::query(
        "update roles set code = 'manager', system_kind = 'manager' where workspace_id = $1 and code = 'member'",
    )
    .bind(workspace.id)
    .execute(store.pool())
    .await
    .unwrap();

    let manager_role_id: Uuid =
        sqlx::query_scalar("select id from roles where workspace_id = $1 and code = 'manager'")
            .bind(workspace.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let user_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into users (
            id, account, email, password_hash, name, nickname, introduction,
            default_display_role, status
        )
        values ($1, $2, $3, 'hash', 'Historical member', 'Historical member', '', 'manager', 'active')
        "#,
    )
    .bind(user_id)
    .bind(format!("historical-{user_id}"))
    .bind(format!("historical-{user_id}@example.com"))
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into user_role_bindings (id, user_id, role_id, scope_id) values ($1, $2, $3, $4)",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(manager_role_id)
    .bind(workspace.id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into frontstage_page_visibility_rules (
            id, workspace_id, page_id, role_id, visibility
        )
        values ($1, $2, null, $3, 'hidden')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace.id)
    .bind(manager_role_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into api_keys (
            id, name, token_hash, token_prefix, creator_user_id, tenant_id,
            scope_kind, scope_id, key_kind, role_code
        )
        values ($1, 'historical key', $2, 'pat_historical', $3, $4,
                'workspace', $5, 'user_api_key', 'manager')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(format!("hash-{user_id}"))
    .bind(user_id)
    .bind(tenant.id)
    .bind(workspace.id)
    .execute(store.pool())
    .await
    .unwrap();

    (store, workspace.id, manager_role_id, user_id)
}

#[tokio::test]
async fn historical_manager_role_migrates_in_place_with_all_bindings() {
    let (store, workspace_id, historical_role_id, user_id) = historical_store().await;
    let permission_count_before: i64 =
        sqlx::query_scalar("select count(*) from role_permissions where role_id = $1")
            .bind(historical_role_id)
            .fetch_one(store.pool())
            .await
            .unwrap();

    sqlx::migrate!("./migrations")
        .run(store.pool())
        .await
        .unwrap();

    let migrated_role: (Uuid, String, Option<String>, bool) = sqlx::query_as(
        "select id, code, system_kind, is_default_member_role from roles where workspace_id = $1 and code = 'member'",
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        migrated_role,
        (
            historical_role_id,
            "member".into(),
            Some("member".into()),
            true
        )
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from role_permissions where role_id = $1")
            .bind(historical_role_id)
            .fetch_one(store.pool())
            .await
            .unwrap(),
        permission_count_before
    );
    assert!(sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from role_data_policies where role_id = $1)",
    )
    .bind(historical_role_id)
    .fetch_one(store.pool())
    .await
    .unwrap());
    assert!(sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from user_role_bindings where user_id = $1 and role_id = $2)",
    )
    .bind(user_id)
    .bind(historical_role_id)
    .fetch_one(store.pool())
    .await
    .unwrap());
    assert!(sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from frontstage_page_visibility_rules where role_id = $1)",
    )
    .bind(historical_role_id)
    .fetch_one(store.pool())
    .await
    .unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, String>("select default_display_role from users where id = $1")
            .bind(user_id)
            .fetch_one(store.pool())
            .await
            .unwrap(),
        "member"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select role_code from api_keys where creator_user_id = $1 and key_kind = 'user_api_key'",
        )
        .bind(user_id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        "member"
    );
    assert!(!sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from roles where workspace_id = $1 and code = 'manager')",
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap());
}

#[tokio::test]
async fn member_code_collision_aborts_without_partial_changes() {
    let (store, workspace_id, historical_role_id, user_id) = historical_store().await;
    sqlx::query(
        r#"
        insert into roles (
            id, scope_id, scope_kind, workspace_id, code, name, introduction,
            is_builtin, is_editable, auto_grant_new_permissions, is_default_member_role
        )
        values ($1, $2, 'workspace', $2, 'member', 'Custom member', '', false, true, false, false)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .execute(store.pool())
    .await
    .unwrap();

    let error = sqlx::migrate!("./migrations")
        .run(store.pool())
        .await
        .expect_err("a historical manager/member collision must stop the migration");
    assert!(error
        .to_string()
        .contains("manager/member role code collision"));
    assert!(error.to_string().contains(&workspace_id.to_string()));
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "select id from roles where workspace_id = $1 and code = 'manager'",
        )
        .bind(workspace_id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        historical_role_id
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("select default_display_role from users where id = $1")
            .bind(user_id)
            .fetch_one(store.pool())
            .await
            .unwrap(),
        "manager"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select role_code from api_keys where creator_user_id = $1 and key_kind = 'user_api_key'",
        )
        .bind(user_id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        "manager"
    );
}
