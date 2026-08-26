use sqlx::migrate::Migrator;
use std::borrow::Cow;
use storage_durable_postgres::run_migrations;
use uuid::Uuid;

const REMOVE_BUILTIN_BLOCK_MIGRATION_VERSION: i64 = 20260825000000;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

fn before_cleanup_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < REMOVE_BUILTIN_BLOCK_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn insert_installation(
    pool: &sqlx::PgPool,
    installation_id: Uuid,
    actor_id: Uuid,
    organization: &str,
    artifact_id: &str,
    plugin_id: &str,
    source_kind: &str,
) {
    sqlx::query(
        r#"
        insert into extension_installations (
            id, category, organization, artifact_id, artifact_version, plugin_id,
            contract_version, protocol, display_name, source_kind, trust_level,
            verification_status, desired_state, signature_status, created_by, updated_by
        ) values (
            $1, 'capability-plugins', $2, $3, '1.0.0', $4,
            '1flowbase.capability/v1', 'stdio_json', 'Migration fixture', $5,
            'verified_official', 'valid', 'active_requested', 'verified', $6, $6
        )
        "#,
    )
    .bind(installation_id)
    .bind(organization)
    .bind(artifact_id)
    .bind(plugin_id)
    .bind(source_kind)
    .bind(actor_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into frontend_block_catalog (
            id, installation_id, provider_code, plugin_id, plugin_version,
            contribution_code, title, runtime, entry, context_contract,
            permission_network, permission_storage, permission_secrets
        ) values (
            $1, $2, $3, $4, '1.0.0', 'frontstage.js-ui-block', 'Fixture',
            'native_react', 'index.js', '{}'::jsonb, 'none', 'none', 'none'
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(installation_id)
    .bind(artifact_id)
    .bind(plugin_id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn cleanup_removes_only_the_legacy_builtin_frontstage_block() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    before_cleanup_migrator().run(&pool).await.unwrap();

    let actor_id = Uuid::now_v7();
    let account = format!("builtin-block-cleanup-{}", actor_id.simple());
    sqlx::query(
        r#"
        insert into users (
            id, account, email, password_hash, name, nickname, status
        ) values ($1, $2, $3, 'fixture', 'Migration fixture', 'Migration fixture', 'active')
        "#,
    )
    .bind(actor_id)
    .bind(&account)
    .bind(format!("{account}@example.test"))
    .execute(&pool)
    .await
    .unwrap();

    let legacy_id = Uuid::now_v7();
    insert_installation(
        &pool,
        legacy_id,
        actor_id,
        "1flowbase",
        "1flowbase",
        "1flowbase@1.0.0",
        "builtin",
    )
    .await;

    let unrelated_id = Uuid::now_v7();
    insert_installation(
        &pool,
        unrelated_id,
        actor_id,
        "example",
        "1flowbase",
        "example/1flowbase@1.0.0",
        "official_registry",
    )
    .await;

    run_migrations(&pool).await.unwrap();

    let legacy_installations: i64 =
        sqlx::query_scalar("select count(*) from extension_installations where id = $1")
            .bind(legacy_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let legacy_blocks: i64 = sqlx::query_scalar(
        "select count(*) from frontend_block_catalog where installation_id = $1",
    )
    .bind(legacy_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let unrelated_installations: i64 =
        sqlx::query_scalar("select count(*) from extension_installations where id = $1")
            .bind(unrelated_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let unrelated_blocks: i64 = sqlx::query_scalar(
        "select count(*) from frontend_block_catalog where installation_id = $1",
    )
    .bind(unrelated_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(legacy_installations, 0);
    assert_eq!(legacy_blocks, 0);
    assert_eq!(unrelated_installations, 1);
    assert_eq!(unrelated_blocks, 1);
}
