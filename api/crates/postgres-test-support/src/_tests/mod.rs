use super::{PostgresTestDatabase, PostgresTestSchema};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::str::FromStr;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("API_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn database_exists(pool: &PgPool, name: &str) -> bool {
    sqlx::query_scalar("select exists(select 1 from pg_database where datname=$1)")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn ac_001_drop_removes_the_test_schema() {
    let base_url = base_database_url();
    let database = PostgresTestSchema::create(&base_url).await.unwrap();
    let database_name = database.database_name().to_owned();
    let schema_name = database.schema_name().to_owned();
    let test_pool = database.connect().await.unwrap();
    let actual_database: String = sqlx::query_scalar("select current_database()")
        .fetch_one(&test_pool)
        .await
        .unwrap();
    assert_eq!(actual_database, database_name);
    let actual_schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(&test_pool)
        .await
        .unwrap();
    assert_eq!(actual_schema, schema_name);
    let exists: bool =
        sqlx::query_scalar("select exists(select 1 from pg_namespace where nspname=$1)")
            .bind(&schema_name)
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert!(exists);
    let admin = PgPool::connect(&base_url).await.unwrap();
    assert!(database_exists(&admin, &database_name).await);
    test_pool.close().await;
    drop(test_pool);
    assert!(
        !database_exists(&admin, &database_name).await,
        "owned database and schema must be removed when the pool guard drops"
    );
    admin.close().await;
}

#[tokio::test]
async fn ac_002_panic_drops_the_test_schema() {
    let base_url = base_database_url();
    let database = PostgresTestSchema::create(&base_url).await.unwrap();
    let database_name = database.database_name().to_owned();
    let test_pool = database.connect().await.unwrap();
    let panic = tokio::spawn(async move {
        let _test_pool = test_pool;
        panic!("intentional test panic");
    })
    .await;
    assert!(panic.unwrap_err().is_panic());
    let admin = PgPool::connect(&base_url).await.unwrap();
    assert!(
        !database_exists(&admin, &database_name).await,
        "panic must not leak the owned database/schema"
    );
    admin.close().await;
}

#[tokio::test]
async fn historical_migrations_preserve_parent_public_metadata_and_cleanup_owned_database() {
    let base_url = base_database_url();
    // All sentinel writes occur in a proof-owned parent, never in the supplied database.
    let parent = PostgresTestDatabase::create(&base_url).await.unwrap();
    let parent_name = parent.database_name().to_owned();
    let parent_pool = parent.connect().await.unwrap();
    let parent_ltree_before: bool =
        sqlx::query_scalar("select exists(select 1 from pg_extension where extname='ltree')")
            .fetch_one(&parent_pool)
            .await
            .unwrap();
    sqlx::raw_sql(
        "create table public.model_fields(id bigint primary key, code text not null); \
         insert into public.model_fields values(41,'private_nondefault_field'); \
         create table public.model_change_logs(id bigint primary key, action text not null); \
         insert into public.model_change_logs values(73,'preserve_private_history');",
    )
    .execute(&parent_pool)
    .await
    .unwrap();
    let before: (i64, i64, String, String) = sqlx::query_as(
        "select 'public.model_fields'::regclass::oid::bigint, 'public.model_change_logs'::regclass::oid::bigint, \
         (select code from public.model_fields where id=41), (select action from public.model_change_logs where id=73)"
    ).fetch_one(&parent_pool).await.unwrap();
    let inner = PostgresTestSchema::create(parent.database_url())
        .await
        .unwrap();
    let inner_name = inner.database_name().to_owned();
    let schema_name = inner.schema_name().to_owned();
    let inner_pool = inner.connect().await.unwrap();
    let before_migration: (String, String) =
        sqlx::query_as("select current_database(), current_schema()")
            .fetch_one(&inner_pool)
            .await
            .unwrap();
    assert_eq!(before_migration, (inner_name.clone(), schema_name.clone()));
    // Executes the actual 20260413103000 unqualified DROP statements and all
    // subsequent migrations, including the AgentLogs model-field registration.
    sqlx::migrate!("../storage/durable/postgres/migrations")
        .run(&inner_pool)
        .await
        .unwrap();
    let after: (i64, i64, String, String) = sqlx::query_as(
        "select 'public.model_fields'::regclass::oid::bigint, 'public.model_change_logs'::regclass::oid::bigint, \
         (select code from public.model_fields where id=41), (select action from public.model_change_logs where id=73)"
    ).fetch_one(&parent_pool).await.unwrap();
    assert_eq!(
        after, before,
        "parent metadata identities and contents must stay untouched"
    );
    let actual: (String, String) = sqlx::query_as("select current_database(), current_schema()")
        .fetch_one(&inner_pool)
        .await
        .unwrap();
    assert_eq!(actual, (inner_name.clone(), schema_name));
    assert_ne!(inner_name, parent_name);
    let parent_ltree: bool =
        sqlx::query_scalar("select exists(select 1 from pg_extension where extname='ltree')")
            .fetch_one(&parent_pool)
            .await
            .unwrap();
    assert_eq!(
        parent_ltree, parent_ltree_before,
        "extension installation must stay in the owned inner database"
    );
    let inner_ltree: bool =
        sqlx::query_scalar("select exists(select 1 from pg_extension where extname='ltree')")
            .fetch_one(&inner_pool)
            .await
            .unwrap();
    assert!(inner_ltree);
    assert!(database_exists(&parent_pool, &inner_name).await);
    inner_pool.close().await;
    drop(inner_pool);
    assert!(!database_exists(&parent_pool, &inner_name).await);
    parent_pool.close().await;
    drop(parent_pool);
    drop(parent);
    let admin = PgPool::connect(&base_url).await.unwrap();
    assert!(!database_exists(&admin, &parent_name).await);
    admin.close().await;
}

#[tokio::test]
async fn inner_panic_cleanup_survives_disappearance_of_owned_parent_database() {
    let base_url = base_database_url();
    let stable_options = PgConnectOptions::from_str(&base_url)
        .unwrap()
        .database("postgres");
    let admin = PgPoolOptions::new()
        .connect_with(stable_options)
        .await
        .unwrap();
    let parent = PostgresTestDatabase::create(&base_url).await.unwrap();
    let parent_name = parent.database_name().to_owned();
    let inner = PostgresTestSchema::create(parent.database_url())
        .await
        .unwrap();
    let inner_name = inner.database_name().to_owned();
    let inner_schema = inner.schema_name().to_owned();
    let inner_pool = inner.connect().await.unwrap();
    assert!(database_exists(&admin, &parent_name).await);
    assert!(database_exists(&admin, &inner_name).await);

    // Drop the parent's owner while the inner pool still owns live connections.
    // An inner cleanup tied to that URL would now fail to connect.
    drop(parent);
    assert!(!database_exists(&admin, &parent_name).await);
    let actual: (String, String) = sqlx::query_as("select current_database(), current_schema()")
        .fetch_one(&inner_pool)
        .await
        .unwrap();
    assert_eq!(actual, (inner_name.clone(), inner_schema));
    assert!(database_exists(&admin, &inner_name).await);
    let panic = tokio::spawn(async move {
        let _inner_pool = inner_pool;
        panic!("intentional inner panic after parent database removal");
    })
    .await;
    assert!(panic.unwrap_err().is_panic());
    assert!(
        !database_exists(&admin, &inner_name).await,
        "inner panic must clean up through the stable maintenance database"
    );
    admin.close().await;
}
