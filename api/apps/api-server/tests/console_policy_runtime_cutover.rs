use std::{borrow::Cow, time::Duration};

use api_server::{
    app_from_config, config::ApiConfig,
    console_policy_migration::require_runtime_console_policy_cutover,
};
use control_plane::ports::{
    RoleConsolePolicyMigrationCutoverMarker, RoleConsolePolicyMigrationRepository,
};
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let base_url = base_database_url();
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(30))
        .connect(&base_url)
        .await
        .expect("the PostgreSQL test server must be reachable");
    let schema = format!("test_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("create schema {schema}"))
        .execute(&admin_pool)
        .await
        .expect("the isolated schema must be created");
    admin_pool.close().await;
    format!("{base_url}?options=-csearch_path%3D{schema}")
}

fn test_config(database_url: &str) -> ApiConfig {
    ApiConfig::from_env_map(&[
        ("API_DATABASE_URL", database_url),
        ("API_DATABASE_POOL_MAX_CONNECTIONS", "1"),
        ("BOOTSTRAP_WORKSPACE_NAME", "cutover-fixture"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "change-me"),
    ])
    .expect("the isolated API config must be valid")
}

#[tokio::test]
async fn ac_011_runtime_rejects_legacy_console_policy_marker_for_existing_roles() {
    let database_url = isolated_database_url().await;
    let pool = PgPool::connect(&database_url)
        .await
        .expect("the isolated PostgreSQL schema must be reachable");
    let migrations = sqlx::migrate!("../../crates/storage-durable/postgres/migrations");
    let before_fresh_cutover = Migrator {
        migrations: Cow::Owned(
            migrations
                .iter()
                .filter(|migration| migration.version < 20260715110000)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    before_fresh_cutover
        .run(&pool)
        .await
        .expect("the historical schema must migrate to the pre-cutover boundary");
    let store = storage_durable::MainDurableStore::new(pool.clone());
    let tenant = store
        .upsert_root_tenant()
        .await
        .expect("the fixture tenant must exist");
    let workspace = store
        .upsert_workspace(tenant.id, "legacy-cutover-fixture")
        .await
        .expect("the fixture workspace must exist");
    sqlx::query(
        r#"
        insert into roles (
            id, scope_id, scope_kind, workspace_id, code, name, introduction,
            is_builtin, is_editable, auto_grant_new_permissions, is_default_member_role
        )
        values ($1, $2, 'workspace', $2, 'legacy_operator', 'Legacy operator', '',
                false, true, false, false)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace.id)
    .execute(&pool)
    .await
    .expect("the historical fixture role must exist");
    migrations
        .run(&pool)
        .await
        .expect("the historical fixture must upgrade through the cutover migration");
    let marker: String = sqlx::query_scalar(
        "select marker from role_console_policy_migration_cutover_state where singleton",
    )
    .fetch_one(&pool)
    .await
    .expect("the runtime cutover marker must exist");
    assert_eq!(marker, "legacy");
    pool.close().await;

    let error = app_from_config(&test_config(&database_url))
        .await
        .expect_err("the new runtime must not start before the legacy role migration finalizes");

    assert!(
        error
            .to_string()
            .contains("console policy migration is still legacy"),
        "unexpected startup error: {error:#}"
    );
}

#[tokio::test]
async fn ac_011_fresh_install_starts_on_console_policy_without_a_migration_run() {
    let database_url = isolated_database_url().await;
    let pool = PgPool::connect(&database_url)
        .await
        .expect("the isolated PostgreSQL schema must be reachable");
    sqlx::migrate!("../../crates/storage-durable/postgres/migrations")
        .run(&pool)
        .await
        .expect("the fresh schema must migrate");
    let store = storage_durable::MainDurableStore::new(pool.clone());

    let state = store
        .role_console_policy_migration_cutover_state()
        .await
        .expect("the fresh cutover marker must exist");
    assert_eq!(
        state.marker,
        RoleConsolePolicyMigrationCutoverMarker::ConsolePolicy
    );
    assert_eq!(state.run_id, None);
    require_runtime_console_policy_cutover(&store)
        .await
        .expect("a fresh install has no legacy grants to migrate");
    pool.close().await;
}
