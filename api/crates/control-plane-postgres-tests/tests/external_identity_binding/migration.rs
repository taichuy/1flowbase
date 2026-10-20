use super::*;
use sqlx::migrate::Migrator;
use std::borrow::Cow;

const LIFECYCLE_MIGRATION: i64 = 20261020020000;

async fn old_schema() -> sqlx::PgPool {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    let migrations = sqlx::migrate!("../storage/durable/postgres/migrations")
        .iter()
        .filter(|migration| migration.version < LIFECYCLE_MIGRATION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .unwrap();
    pool
}

async fn legacy_owner(pool: &sqlx::PgPool, suffix: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("insert into users (id,account,email,password_hash,name,nickname,status) values ($1,$2,$3,'!external-identity-no-password','','','active')")
        .bind(id).bind(format!("legacy-{suffix}")).bind(format!("{suffix}@example.com")).execute(pool).await.unwrap();
    id
}

async fn installation(pool: &sqlx::PgPool, owner: Uuid, publisher: &str, artifact: &str) -> Uuid {
    let id = Uuid::now_v7();
    // Valid Host verification with an explicitly allowed unsigned development package.
    sqlx::query("insert into extension_installations (id,category,organization,artifact_id,artifact_version,plugin_id,contract_version,protocol,display_name,source_kind,trust_level,verification_status,desired_state,signature_status,metadata_json,created_by) values ($1,'runtime-extensions',$2,$3,'1.0.0','manifest.canonical-id','1flowbase.extension-bus/v1','stdio_json_multiplex_v1','Fixture','uploaded','unverified','valid','active_requested','missing',$4,$5)")
        .bind(id).bind(publisher).bind(artifact).bind(serde_json::json!({"authentication_providers":[{"auth_type":"github"}]})).bind(owner)
        .execute(pool).await.unwrap();
    id
}

async fn legacy_identity(pool: &sqlx::PgPool, owner: Uuid, install: Option<Uuid>) -> Uuid {
    let connection = Uuid::now_v7();
    sqlx::query("insert into authentication_connections (id,auth_type,is_builtin,config) values ($1,'github',false,'{}')")
        .bind(connection).execute(pool).await.unwrap();
    let metadata = install
        .map(|id| serde_json::json!({"installation_id":id}))
        .unwrap_or_else(|| serde_json::json!({}));
    sqlx::query("insert into user_auth_identities (id,user_id,connection_id,subject_type,subject_value,issuer,profile,verified,metadata) values ($1,$2,$3,'github','42','https://github.com','{}',true,$4)")
        .bind(Uuid::now_v7()).bind(owner).bind(connection).bind(metadata).execute(pool).await.unwrap();
    connection
}

#[tokio::test]
async fn migration_uses_verified_host_installation_and_merges_only_same_owner_subject() {
    let pool = old_schema().await;
    let owner = legacy_owner(&pool, "one").await;
    let install = installation(&pool, owner, "publisher", "github-provider").await;
    legacy_identity(&pool, owner, Some(install)).await;
    legacy_identity(&pool, owner, Some(install)).await;
    let isolated_connection = legacy_identity(&pool, owner, None).await;
    storage_durable_postgres::run_migrations(&pool)
        .await
        .unwrap();
    let namespaces: Vec<String> = sqlx::query_scalar(
        "select authority_namespace from user_auth_identities order by authority_namespace",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        namespaces,
        vec![
            format!("connection:{isolated_connection}"),
            "managed:publisher/github-provider:github".into()
        ]
    );
    let retained: i64 = sqlx::query_scalar(
        "select count(*) from user_auth_identities where state='bound' and unlinked_at is null",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained, 2);
}

#[tokio::test]
async fn migration_stops_cross_connection_canonical_multi_owner_conflict_without_selecting_owner() {
    let pool = old_schema().await;
    let one = legacy_owner(&pool, "one").await;
    let two = legacy_owner(&pool, "two").await;
    let install = installation(&pool, one, "publisher", "github-provider").await;
    legacy_identity(&pool, one, Some(install)).await;
    legacy_identity(&pool, two, Some(install)).await;
    let error = storage_durable_postgres::run_migrations(&pool)
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("external_identity_canonical_owner_conflict"),
        "{error:#}"
    );
    let owners: i64 =
        sqlx::query_scalar("select count(distinct user_id) from user_auth_identities")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        owners, 2,
        "failed migration must not merge or discard an owner"
    );
}

#[tokio::test]
async fn migration_does_not_trust_unverified_installation_or_provider_subject_metadata() {
    let pool = old_schema().await;
    let owner = legacy_owner(&pool, "unverified").await;
    let install = installation(&pool, owner, "publisher", "github-provider").await;
    sqlx::query("update extension_installations set verification_status='invalid' where id=$1")
        .bind(install)
        .execute(&pool)
        .await
        .unwrap();
    let connection = legacy_identity(&pool, owner, Some(install)).await;
    sqlx::query("update user_auth_identities set metadata=metadata || '{\"authority_namespace\":\"managed:forged/github:github\"}'::jsonb where connection_id=$1")
        .bind(connection).execute(&pool).await.unwrap();
    storage_durable_postgres::run_migrations(&pool)
        .await
        .unwrap();
    let namespace: String = sqlx::query_scalar(
        "select authority_namespace from user_auth_identities where connection_id=$1",
    )
    .bind(connection)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(namespace, format!("connection:{connection}"));
}
