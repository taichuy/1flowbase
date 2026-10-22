use std::time::Instant;

use sqlx::{Connection, PgConnection};

use super::{base_database_url, database_exists, wait_for_database_removed, PgPool};
use crate::{
    cleanup, record_phase_duration, template, PostgresTestDatabase, PostgresTestPhase,
    PostgresTestSchema, PostgresTestTemplate,
};

static MIGRATIONS: sqlx::migrate::Migrator =
    sqlx::migrate!("../storage/durable/postgres/migrations");

#[test]
fn clone_names_fit_postgres_and_keep_exact_command_ownership() {
    let owner = "test_backup_0123456789abcdef0123456789abcdef";
    let first = template::clone_database_name(owner).unwrap();
    let second = template::clone_database_name(owner).unwrap();
    assert_eq!(first.len(), 61);
    assert_ne!(first, second);
    let prefix = format!("{owner}_");
    assert!(cleanup::is_owned_clone(&first, &prefix));
    assert!(cleanup::is_owned_database_name(&first));
    assert!(!cleanup::is_owned_clone(owner, &prefix));
    assert!(!cleanup::is_owned_clone(
        &format!("{prefix}sentinel"),
        &prefix
    ));
    assert!(!cleanup::is_owned_clone(
        "test_backup_ffffffffffffffffffffffffffffffff_0123456789abcdef",
        &prefix
    ));
    assert!(template::clone_database_name("postgres").is_err());
    assert!(template::clone_database_name(&format!("{owner}\";drop database postgres;")).is_err());
}

#[tokio::test]
async fn formal_template_closes_connections_and_clones_isolate_data_ddl_and_sequences() {
    let base_url = base_database_url();
    let parent = PostgresTestDatabase::create(&base_url).await.unwrap();
    let parent_pool = parent.connect().await.unwrap();
    sqlx::raw_sql("create table public.template_parent_sentinel(value text); insert into public.template_parent_sentinel values ('parent')")
        .execute(&parent_pool)
        .await
        .unwrap();
    let template = PostgresTestTemplate::prepare(parent.database_url())
        .await
        .unwrap();
    let template_name = template.database_name().to_owned();
    let template_connections: i64 =
        sqlx::query_scalar("select count(*) from pg_stat_activity where datname = $1")
            .bind(&template_name)
            .fetch_one(&parent_pool)
            .await
            .unwrap();
    assert_eq!(template_connections, 0);

    let left = PostgresTestSchema::create_from_template(
        parent.database_url(),
        template.database_name(),
        template.schema_name(),
    )
    .await
    .unwrap();
    let right = PostgresTestSchema::create_from_template(
        parent.database_url(),
        template.database_name(),
        template.schema_name(),
    )
    .await
    .unwrap();
    let left_name = left.database_name().to_owned();
    let right_name = right.database_name().to_owned();
    assert_ne!(left_name, right_name);
    assert_ne!(left_name, template_name);
    assert_eq!(left.schema_name(), template.schema_name());
    assert_eq!(right.schema_name(), template.schema_name());
    let left = left.connect().await.unwrap();
    let right = right.connect().await.unwrap();
    for pool in [&left, &right] {
        MIGRATIONS.run(pool).await.unwrap();
        let current_schema: String = sqlx::query_scalar("select current_schema()")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(current_schema, template.schema_name());
        let unseeded: (i64, i64, i64, i64, i64) = sqlx::query_as(
            "select (select count(*) from users), (select count(*) from workspaces), \
             (select count(*) from applications), (select count(*) from file_storages), \
             (select count(*) from file_tables)",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(unseeded, (0, 0, 0, 0, 0));
        let ledger_count: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(
            ledger_count as usize,
            MIGRATIONS
                .iter()
                .filter(|migration| migration.migration_type.is_up_migration())
                .count()
        );
        // No formal migration currently owns a sequence. Same-name sequences
        // created in each database must nevertheless have independent state.
        sqlx::raw_sql("create sequence fixture_sequence; create table fixture_rows(value text)")
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::raw_sql(
        "insert into fixture_rows values ('left'); \
         create table public.template_child_only(value text); \
         alter table fixture_rows add column left_only text; \
         select nextval('fixture_sequence'); select nextval('fixture_sequence');",
    )
    .execute(&left)
    .await
    .unwrap();
    let right_next: i64 = sqlx::query_scalar("select nextval('fixture_sequence')")
        .fetch_one(&right)
        .await
        .unwrap();
    assert_eq!(right_next, 1);
    let right_rows: i64 = sqlx::query_scalar("select count(*) from fixture_rows")
        .fetch_one(&right)
        .await
        .unwrap();
    assert_eq!(right_rows, 0);
    let right_public: bool =
        sqlx::query_scalar("select to_regclass('public.template_child_only') is not null")
            .fetch_one(&right)
            .await
            .unwrap();
    assert!(!right_public);
    let right_column: bool = sqlx::query_scalar(
        "select exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = 'fixture_rows' and column_name = 'left_only')",
    )
    .fetch_one(&right)
    .await
    .unwrap();
    assert!(!right_column);
    let parent_value: String =
        sqlx::query_scalar("select value from public.template_parent_sentinel")
            .fetch_one(&parent_pool)
            .await
            .unwrap();
    assert_eq!(parent_value, "parent");

    left.close().await;
    right.close().await;
    drop(left);
    drop(right);
    assert!(!database_exists(&parent_pool, &left_name).await);
    assert!(!database_exists(&parent_pool, &right_name).await);
    template.cleanup().await.unwrap();
    assert!(!database_exists(&parent_pool, &template_name).await);
    parent_pool.close().await;
}

#[tokio::test]
async fn clone_checksum_mismatch_fails_and_migration_error_closes_its_connection() {
    let base_url = base_database_url();
    let template = PostgresTestTemplate::prepare(&base_url).await.unwrap();
    let clone = PostgresTestSchema::create_from_template(
        &base_url,
        template.database_name(),
        template.schema_name(),
    )
    .await
    .unwrap();
    let clone_name = clone.database_name().to_owned();
    let mut connection = PgConnection::connect(clone.database_url()).await.unwrap();
    sqlx::query("update _sqlx_migrations set checksum = decode('00', 'hex') where version = (select min(version) from _sqlx_migrations)")
        .execute(&mut connection)
        .await
        .unwrap();
    let error = template::migrate_and_close(connection, &MIGRATIONS)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        sqlx::Error::Migrate(error) if matches!(error.as_ref(), sqlx::migrate::MigrateError::VersionMismatch(_))
    ));
    let admin = PgPool::connect(&base_url).await.unwrap();
    let connections: i64 =
        sqlx::query_scalar("select count(*) from pg_stat_activity where datname = $1")
            .bind(&clone_name)
            .fetch_one(&admin)
            .await
            .unwrap();
    assert_eq!(
        connections, 0,
        "failed checksum validation must release its connection and advisory lock"
    );
    drop(clone);
    assert!(!database_exists(&admin, &clone_name).await);
    template.cleanup().await.unwrap();
    admin.close().await;
}

#[tokio::test]
async fn explicit_template_rejects_missing_ledger_and_schema_without_fresh_fallback() {
    let base_url = base_database_url();
    let empty = PostgresTestSchema::create(&base_url).await.unwrap();
    let empty_name = empty.database_name().to_owned();
    let result = PostgresTestSchema::create_from_template(
        &base_url,
        empty.database_name(),
        empty.schema_name(),
    )
    .await;
    assert!(matches!(result, Err(sqlx::Error::InvalidArgument(_))));
    let admin = PgPool::connect(&base_url).await.unwrap();
    let leaked: i64 =
        sqlx::query_scalar("select count(*) from pg_database where starts_with(datname::text, $1)")
            .bind(format!("{empty_name}_"))
            .fetch_one(&admin)
            .await
            .unwrap();
    assert_eq!(leaked, 0);
    let absent_schema = format!("test_{}", uuid::Uuid::now_v7().simple());
    let result =
        PostgresTestSchema::create_from_template(&base_url, &empty_name, &absent_schema).await;
    assert!(matches!(result, Err(sqlx::Error::InvalidArgument(_))));
    let result = PostgresTestSchema::create_from_template(&base_url, "postgres", "public").await;
    assert!(matches!(result, Err(sqlx::Error::InvalidArgument(_))));
    drop(empty);
    assert!(!database_exists(&admin, &empty_name).await);
    admin.close().await;
}

#[tokio::test]
async fn panic_releases_clone_and_template_owner_reaps_only_its_orphans() {
    let base_url = base_database_url();
    let template = PostgresTestTemplate::prepare(&base_url).await.unwrap();
    let template_name = template.database_name().to_owned();
    let unrelated = PostgresTestDatabase::create(&base_url).await.unwrap();
    let unrelated_name = unrelated.database_name().to_owned();
    let clone = PostgresTestSchema::create_from_template(
        &base_url,
        template.database_name(),
        template.schema_name(),
    )
    .await
    .unwrap();
    let clone_name = clone.database_name().to_owned();
    let pool = clone.connect().await.unwrap();
    let panic = tokio::spawn(async move {
        let _pool = pool;
        panic!("intentional cloned fixture panic");
    })
    .await;
    assert!(panic.unwrap_err().is_panic());
    let admin = PgPool::connect(&base_url).await.unwrap();
    wait_for_database_removed(&admin, &clone_name).await;
    let orphan = PostgresTestSchema::create_from_template(
        &base_url,
        template.database_name(),
        template.schema_name(),
    )
    .await
    .unwrap();
    let orphan_name = orphan.database_name().to_owned();
    // Model abrupt child process exit: no per-test Rust destructor can run.
    std::mem::forget(orphan);
    assert!(database_exists(&admin, &orphan_name).await);
    let panic = tokio::spawn(async move {
        let _template = template;
        panic!("intentional command template owner panic");
    })
    .await;
    assert!(panic.unwrap_err().is_panic());
    assert!(!database_exists(&admin, &orphan_name).await);
    assert!(!database_exists(&admin, &template_name).await);
    assert!(database_exists(&admin, &unrelated_name).await);
    drop(unrelated);
    admin.close().await;
}

#[tokio::test]
async fn fresh_versus_clone_records_separate_create_and_migrator_phases() {
    let base_url = base_database_url();
    let fresh_started = Instant::now();
    let fresh = PostgresTestSchema::create(&base_url).await.unwrap();
    let fresh_create = fresh_started.elapsed();
    record_phase_duration(PostgresTestPhase::Fresh, fresh_create, true).unwrap();
    let migration_started = Instant::now();
    let fresh_pool = fresh.connect().await.unwrap();
    MIGRATIONS.run(&fresh_pool).await.unwrap();
    let fresh_migrate = migration_started.elapsed();
    record_phase_duration(PostgresTestPhase::Migration, fresh_migrate, true).unwrap();
    fresh_pool.close().await;
    drop(fresh_pool);

    let template = PostgresTestTemplate::prepare(&base_url).await.unwrap();
    let clone_started = Instant::now();
    let clone = PostgresTestSchema::create_from_template(
        &base_url,
        template.database_name(),
        template.schema_name(),
    )
    .await
    .unwrap();
    let clone_create = clone_started.elapsed();
    let validation_started = Instant::now();
    let clone_pool = clone.connect().await.unwrap();
    MIGRATIONS.run(&clone_pool).await.unwrap();
    let clone_validate = validation_started.elapsed();
    record_phase_duration(PostgresTestPhase::Migration, clone_validate, true).unwrap();
    eprintln!(
        "postgres fixture phases: fresh_create_ns={} fresh_migrate_ns={} fresh_total_ns={} clone_create_ns={} clone_validate_ns={} clone_total_ns={}",
        fresh_create.as_nanos(), fresh_migrate.as_nanos(), (fresh_create + fresh_migrate).as_nanos(),
        clone_create.as_nanos(), clone_validate.as_nanos(), (clone_create + clone_validate).as_nanos(),
    );
    // No wall-clock speed assertion: CI records the comparison under its actual
    // resource policy; both paths must execute the same formal migrator checks.
    clone_pool.close().await;
    drop(clone_pool);
    template.cleanup().await.unwrap();
    if let Some(directory) = std::env::var_os("POSTGRES_TEST_METRICS_DIR") {
        let directory = std::path::PathBuf::from(directory);
        let summary = format!(
            concat!(
                "{{\n  \"sampleCount\": 1,\n",
                "  \"fresh_create_ns\": {},\n",
                "  \"fresh_migrate_ns\": {},\n",
                "  \"fresh_total_ns\": {},\n",
                "  \"clone_create_ns\": {},\n",
                "  \"clone_validate_ns\": {},\n",
                "  \"clone_total_ns\": {}\n}}\n"
            ),
            fresh_create.as_nanos(),
            fresh_migrate.as_nanos(),
            (fresh_create + fresh_migrate).as_nanos(),
            clone_create.as_nanos(),
            clone_validate.as_nanos(),
            (clone_create + clone_validate).as_nanos(),
        );
        // One paired observation, not evidence of statistical significance.
        std::fs::write(
            directory.join(format!("fresh-versus-clone-{}.json", std::process::id())),
            summary,
        )
        .unwrap();
    }
}
