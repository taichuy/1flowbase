use std::process::Command;

use sqlx::PgPool;
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("API_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();
    admin_pool.close().await;

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

#[tokio::test]
async fn reset_root_password_bootstraps_password_local_authenticator_on_empty_database() {
    let database_url = isolated_database_url().await;
    let output = Command::new(env!("CARGO_BIN_EXE_reset_root_password"))
        .env("API_ENV", "development")
        .env("API_DATABASE_URL", &database_url)
        .env("API_DATABASE_POOL_MAX_CONNECTIONS", "1")
        .env("API_PLUGIN_SET", "default")
        .env("API_SECRET_RESOLVER", "env")
        .env("BOOTSTRAP_WORKSPACE_NAME", "1flowbase")
        .env("BOOTSTRAP_ROOT_ACCOUNT", "root")
        .env("BOOTSTRAP_ROOT_EMAIL", "root@example.com")
        .env("BOOTSTRAP_ROOT_PASSWORD", "test-root-password")
        .env("BOOTSTRAP_ROOT_NAME", "Root")
        .env("BOOTSTRAP_ROOT_NICKNAME", "Root")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "reset_root_password failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let pool = PgPool::connect(&database_url).await.unwrap();
    let authenticator_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from authenticators
        where id = '00000000-0000-0000-0000-000000000001'
          and auth_type = 'password-local'
          and enabled = true
          and is_builtin = true
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(authenticator_count, 1);

    let root_identity_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from user_auth_identities
        where authenticator_id = '00000000-0000-0000-0000-000000000001'
          and subject_type in ('account', 'email')
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(root_identity_count, 2);
}
