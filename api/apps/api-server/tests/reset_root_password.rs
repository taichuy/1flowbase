use std::process::Command;

use sqlx::PgPool;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("API_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

#[tokio::test]
async fn reset_root_password_bootstraps_password_local_authenticator_on_empty_database() {
    let database = isolated_database().await;
    let database_url = database.database_url().to_owned();
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
