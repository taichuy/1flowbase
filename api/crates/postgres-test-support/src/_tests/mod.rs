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

async fn wait_for_database_removed(pool: &PgPool, name: &str) {
    // SQLx background tasks can retain the pool guard after the panic task exits.
    // Observe cleanup without closing the pool or deleting the database for it.
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while database_exists(pool, name).await {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!("owned database {name} was not removed within the cleanup deadline")
    });
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
    wait_for_database_removed(&admin, &database_name).await;
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
    wait_for_database_removed(&admin, &inner_name).await;
    assert!(
        !database_exists(&admin, &inner_name).await,
        "inner panic must clean up through the stable maintenance database"
    );
    admin.close().await;
}

#[tokio::test]
async fn ac_001_create_migrated_clones_template_schema_and_preserves_template_on_drop() {
    let base_url = base_database_url();
    let (template_name, _) = super::migration_template_identity();
    let expected_migrations = super::MIGRATOR
        .iter()
        .filter(|migration| !migration.migration_type.is_down_migration())
        .count() as i64;

    let database = PostgresTestSchema::create_migrated(&base_url)
        .await
        .unwrap();
    let database_name = database.database_name().to_owned();
    let schema_name = database.schema_name().to_owned();
    let test_pool = database.connect().await.unwrap();

    let (actual_database, actual_schema): (String, String) =
        sqlx::query_as("select current_database(), current_schema()")
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert_eq!(actual_database, database_name);
    assert_eq!(actual_schema, schema_name);

    let seed_schema_exists: bool =
        sqlx::query_scalar("select exists(select 1 from pg_namespace where nspname = 'test_seed')")
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert!(
        !seed_schema_exists,
        "template seed schema must be renamed to the per-test schema"
    );

    let applied_before: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations")
        .fetch_one(&test_pool)
        .await
        .unwrap();
    assert_eq!(applied_before, expected_migrations);

    let rerun_started = std::time::Instant::now();
    super::MIGRATOR.run(&test_pool).await.unwrap();
    let rerun_elapsed = rerun_started.elapsed();
    assert!(
        rerun_elapsed < std::time::Duration::from_secs(5),
        "re-running MIGRATOR on a template-cloned schema should be a fast no-op, took {rerun_elapsed:?}"
    );

    let applied_after: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations")
        .fetch_one(&test_pool)
        .await
        .unwrap();
    assert_eq!(applied_after, expected_migrations);

    let admin_options = PgConnectOptions::from_str(&base_url)
        .unwrap()
        .database("postgres");
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(admin_options)
        .await
        .unwrap();

    let template_flags: (bool, bool) =
        sqlx::query_as("select datistemplate, datallowconn from pg_database where datname = $1")
            .bind(template_name)
            .fetch_one(&admin)
            .await
            .unwrap();
    assert_eq!(
        template_flags,
        (true, false),
        "migration template database must be sealed with IS_TEMPLATE=true ALLOW_CONNECTIONS=false"
    );
    assert!(database_exists(&admin, &database_name).await);

    test_pool.close().await;
    drop(test_pool);

    assert!(
        !database_exists(&admin, &database_name).await,
        "dropping the test pool must remove test_backup_<uuid>"
    );
    assert!(
        database_exists(&admin, template_name).await,
        "dropping the test pool must preserve test_tpl_<hash>"
    );
    admin.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ac_001_concurrent_create_migrated_produces_isolated_clones() {
    let base_url = base_database_url();
    let (template_name, _) = super::migration_template_identity();

    let mut tasks = Vec::new();
    for idx in 0..4_u32 {
        let url = base_url.clone();
        tasks.push(tokio::spawn(async move {
            let schema = PostgresTestSchema::create_migrated(&url).await.unwrap();
            let db_name = schema.database_name().to_owned();
            let schema_name = schema.schema_name().to_owned();
            let pool = schema.connect().await.unwrap();
            let code = format!("tenant-{idx}");
            let mut conn = pool.acquire().await.unwrap();
            sqlx::query("insert into tenants (id, code, name) values ($1, $2, $3)")
                .bind(uuid::Uuid::now_v7())
                .bind(code.clone())
                .bind(code.clone())
                .execute(&mut *conn)
                .await
                .unwrap();
            let count: i64 = sqlx::query_scalar("select count(*) from tenants where code = $1")
                .bind(code)
                .fetch_one(&mut *conn)
                .await
                .unwrap();
            drop(conn);
            assert_eq!(count, 1);
            pool.close().await;
            drop(pool);
            (db_name, schema_name)
        }));
    }

    let mut db_names = std::collections::BTreeSet::new();
    let mut schema_names = std::collections::BTreeSet::new();
    for task in tasks {
        let (db_name, schema_name) = task.await.unwrap();
        assert!(db_names.insert(db_name));
        assert!(schema_names.insert(schema_name));
    }

    let admin_options = PgConnectOptions::from_str(&base_url)
        .unwrap()
        .database("postgres");
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(admin_options)
        .await
        .unwrap();
    for db_name in &db_names {
        wait_for_database_removed(&admin, db_name).await;
    }
    assert!(database_exists(&admin, template_name).await);
    admin.close().await;
}
