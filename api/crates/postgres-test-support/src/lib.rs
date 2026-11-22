use std::{
    str::FromStr,
    sync::OnceLock,
    time::{Duration, Instant},
};

use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, Connection, PgConnection, PgPool,
};
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../storage/durable/postgres/migrations");

const TEMPLATE_SEED_SCHEMA: &str = "test_seed";

fn migration_template_identity() -> (&'static str, i64) {
    static IDENTITY: OnceLock<(String, i64)> = OnceLock::new();
    let (name, lock_key) = IDENTITY.get_or_init(|| {
        let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
        fn fnv1a_update(hash: &mut u64, bytes: &[u8]) {
            for &byte in bytes {
                *hash ^= u64::from(byte);
                *hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
        }

        fnv1a_update(
            &mut digest,
            b"1flowbase:postgres-test-support:template:v1\0",
        );
        fnv1a_update(
            &mut digest,
            &(MIGRATOR.migrations.len() as u64).to_le_bytes(),
        );
        for migration in MIGRATOR.iter() {
            fnv1a_update(&mut digest, &migration.version.to_le_bytes());
            fnv1a_update(
                &mut digest,
                &[u8::from(migration.migration_type.is_down_migration())],
            );
            fnv1a_update(
                &mut digest,
                &(migration.checksum.len() as u64).to_le_bytes(),
            );
            fnv1a_update(&mut digest, &migration.checksum);
        }

        let template_name = format!("test_tpl_{digest:016x}");
        let lock_key = digest as i64;
        (template_name, lock_key)
    });
    (name.as_str(), *lock_key)
}

fn with_search_path_option(database_url: &str, schema_name: &str) -> String {
    // PgConnectOptions::to_url_lossy does not serialize startup options.
    // Keep the explicit encoded option in the caller-visible connection URL.
    let query_separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{query_separator}options=-csearch_path%3D{schema_name}%2Cpublic")
}

async fn drop_database_with_connection(
    connection: &mut PgConnection,
    database_name: &str,
) -> Result<(), sqlx::Error> {
    let force_drop = sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"drop database if exists "{database_name}" with (force)"#
    )))
    .execute(&mut *connection)
    .await;

    match force_drop {
        Ok(_) => Ok(()),
        Err(_) => {
            sqlx::query(
                "select pg_terminate_backend(pid) from pg_stat_activity where datname = $1 and pid <> pg_backend_pid()",
            )
            .bind(database_name.to_owned())
            .execute(&mut *connection)
            .await?;
            sqlx::query(sqlx::AssertSqlSafe(format!(
                r#"drop database if exists "{database_name}""#
            )))
            .execute(&mut *connection)
            .await?;
            Ok(())
        }
    }
}

async fn is_ready_template_database(
    maintenance_conn: &mut PgConnection,
    template_name: &'static str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "select exists(select 1 from pg_database where datname = $1 and datistemplate = true and datallowconn = false)",
    )
    .bind(template_name)
    .fetch_one(&mut *maintenance_conn)
    .await
}

async fn wait_for_zero_backends(
    maintenance_conn: &mut PgConnection,
    database_name: &'static str,
) -> Result<(), sqlx::Error> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let active: bool = sqlx::query_scalar(
            "select exists(select 1 from pg_stat_activity where datname = $1 and pid <> pg_backend_pid())",
        )
        .bind(database_name)
        .fetch_one(&mut *maintenance_conn)
        .await?;
        if !active || Instant::now() >= deadline {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn populate_template_database(
    maintenance_conn: &mut PgConnection,
    admin_options: &PgConnectOptions,
    template_name: &'static str,
) -> Result<(), sqlx::Error> {
    let template_base_url = admin_options
        .clone()
        .database(template_name)
        .to_url_lossy()
        .to_string();

    let mut setup_conn = PgConnection::connect(&template_base_url).await?;
    sqlx::query("create extension if not exists ltree with schema public")
        .execute(&mut setup_conn)
        .await?;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"create schema "{TEMPLATE_SEED_SCHEMA}""#
    )))
    .execute(&mut setup_conn)
    .await?;
    setup_conn.close().await?;

    let seed_url = with_search_path_option(&template_base_url, TEMPLATE_SEED_SCHEMA);
    let seed_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&seed_url)
        .await?;
    let migrate_result = MIGRATOR.run(&seed_pool).await;
    seed_pool.close().await;
    migrate_result?;

    sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"alter database "{template_name}" with is_template = true allow_connections = false"#
    )))
    .execute(&mut *maintenance_conn)
    .await?;

    wait_for_zero_backends(maintenance_conn, template_name).await?;
    Ok(())
}

async fn build_template_database_locked(
    maintenance_conn: &mut PgConnection,
    admin_options: &PgConnectOptions,
    template_name: &'static str,
) -> Result<(), sqlx::Error> {
    if is_ready_template_database(maintenance_conn, template_name).await? {
        return Ok(());
    }

    let existing: Option<(bool, bool)> =
        sqlx::query_as("select datistemplate, datallowconn from pg_database where datname = $1")
            .bind(template_name)
            .fetch_optional(&mut *maintenance_conn)
            .await?;

    if let Some((is_template, _allow_conn)) = existing {
        if is_template {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                r#"alter database "{template_name}" with is_template = false"#
            )))
            .execute(&mut *maintenance_conn)
            .await?;
        }
        drop_database_with_connection(maintenance_conn, template_name).await?;
    }

    sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"create database "{template_name}""#
    )))
    .execute(&mut *maintenance_conn)
    .await?;

    if let Err(error) =
        populate_template_database(maintenance_conn, admin_options, template_name).await
    {
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"alter database "{template_name}" with is_template = false"#
        )))
        .execute(&mut *maintenance_conn)
        .await;
        let _ = drop_database_with_connection(maintenance_conn, template_name).await;
        return Err(error);
    }

    Ok(())
}

async fn ensure_migrated_template_database(
    maintenance_conn: &mut PgConnection,
    admin_options: &PgConnectOptions,
    template_name: &'static str,
    lock_key: i64,
) -> Result<(), sqlx::Error> {
    if is_ready_template_database(maintenance_conn, template_name).await? {
        return Ok(());
    }

    sqlx::query("select pg_advisory_lock($1)")
        .bind(lock_key)
        .execute(&mut *maintenance_conn)
        .await?;

    let build_result =
        build_template_database_locked(maintenance_conn, admin_options, template_name).await;

    let unlock_result = sqlx::query("select pg_advisory_unlock($1)")
        .bind(lock_key)
        .execute(&mut *maintenance_conn)
        .await;

    build_result?;
    unlock_result?;
    Ok(())
}

/// A test schema inside a wholly owned temporary database.
///
/// Historical migrations use unqualified destructive statements. Schema-only
/// search paths can fall through to a parent's `public` objects, so both the
/// schema and its extension-accessible `public` namespace must belong to the test.
pub struct PostgresTestSchema {
    database: PostgresTestDatabase,
    database_url: String,
    schema_name: String,
}

impl PostgresTestSchema {
    pub async fn create(base_database_url: &str) -> Result<Self, sqlx::Error> {
        let database = PostgresTestDatabase::create(base_database_url).await?;
        let schema_name = format!("test_{}", Uuid::now_v7().simple());
        let mut connection = PgConnection::connect(database.database_url()).await?;
        sqlx::query("create extension if not exists ltree with schema public")
            .execute(&mut connection)
            .await?;
        // SQL safety: temporary schema/database names are generated from fixed prefixes and UUID hex.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"create schema "{schema_name}""#
        )))
        .execute(&mut connection)
        .await?;
        connection.close().await?;
        let database_url = with_search_path_option(database.database_url(), &schema_name);
        Ok(Self {
            database,
            database_url,
            schema_name,
        })
    }

    pub async fn create_migrated(base_database_url: &str) -> Result<Self, sqlx::Error> {
        let (template_name, lock_key) = migration_template_identity();
        let admin_options = PgConnectOptions::from_str(base_database_url)?;
        let maintenance_options = admin_options.clone().database("postgres");
        let mut maintenance_conn = PgConnection::connect_with(&maintenance_options).await?;

        ensure_migrated_template_database(
            &mut maintenance_conn,
            &admin_options,
            template_name,
            lock_key,
        )
        .await?;

        let database_name = format!("test_backup_{}", Uuid::now_v7().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"create database "{database_name}" template "{template_name}""#
        )))
        .execute(&mut maintenance_conn)
        .await?;
        maintenance_conn.close().await?;

        let base_url = admin_options
            .database(&database_name)
            .to_url_lossy()
            .to_string();
        let database = PostgresTestDatabase {
            cleanup_options: maintenance_options,
            database_url: base_url,
            database_name,
        };

        let schema_name = format!("test_{}", Uuid::now_v7().simple());
        let mut connection = PgConnection::connect(database.database_url()).await?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"alter schema "{TEMPLATE_SEED_SCHEMA}" rename to "{schema_name}""#
        )))
        .execute(&mut connection)
        .await?;
        connection.close().await?;

        let database_url = with_search_path_option(database.database_url(), &schema_name);
        Ok(Self {
            database,
            database_url,
            schema_name,
        })
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub fn database_name(&self) -> &str {
        self.database.database_name()
    }

    pub async fn connect(self) -> Result<PgPool, sqlx::Error> {
        let database_url = self.database_url.clone();
        let schema_guard = std::sync::Arc::new(self);
        let pool_guard = schema_guard.clone();

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .after_release(move |_connection, _metadata| {
                let _schema_guard = pool_guard.clone();
                Box::pin(async { Ok(true) })
            })
            .connect(&database_url)
            .await?;
        drop(schema_guard);
        Ok(pool)
    }
}

/// Owns a whole temporary database for destructive migrations and backup/restore
/// fixtures. `PostgresTestSchema` reuses this database boundary and adds its own
/// schema/search path; logical restore fixtures use the database owner directly.
/// The test role requires CREATEDB and CONNECT to the existing `postgres`
/// maintenance database, which remains available if a proof-owned parent is dropped.
pub struct PostgresTestDatabase {
    cleanup_options: PgConnectOptions,
    database_url: String,
    database_name: String,
}

impl PostgresTestDatabase {
    pub async fn create(admin_database_url: &str) -> Result<Self, sqlx::Error> {
        let database_name = format!("test_backup_{}", Uuid::now_v7().simple());
        let admin_options = PgConnectOptions::from_str(admin_database_url)?;
        let mut connection = PgConnection::connect_with(&admin_options).await?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"create database "{database_name}""#
        )))
        .execute(&mut connection)
        .await?;
        connection.close().await?;
        let database_url = admin_options
            .clone()
            .database(&database_name)
            .to_url_lossy()
            .to_string();
        Ok(Self {
            // Keep typed startup options and credentials: a lossy URL round-trip
            // would discard options, and the supplied database may be temporary.
            cleanup_options: admin_options.database("postgres"),
            database_url,
            database_name,
        })
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn database_name(&self) -> &str {
        &self.database_name
    }

    pub async fn connect(&self) -> Result<PgPool, sqlx::Error> {
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&self.database_url)
            .await
    }
}

impl Drop for PostgresTestDatabase {
    fn drop(&mut self) {
        let cleanup_options = self.cleanup_options.clone();
        let database_name = self.database_name.clone();
        if !database_name.starts_with("test_backup_") {
            eprintln!("refusing to drop non-temporary PostgreSQL database {database_name}");
            return;
        }
        let cleanup = std::thread::Builder::new()
            .name(format!("drop-{database_name}"))
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        eprintln!("failed to start PostgreSQL test database cleanup: {error}");
                        return;
                    }
                };
                let result = runtime.block_on(async {
                    tokio::time::timeout(Duration::from_secs(30), async {
                        let mut connection = PgConnection::connect_with(&cleanup_options).await?;
                        drop_database_with_connection(&mut connection, &database_name).await?;
                        connection.close().await
                    })
                    .await
                });
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => eprintln!(
                        "failed to drop PostgreSQL test database {database_name}: {error}"
                    ),
                    Err(_) => eprintln!(
                        "timed out while dropping PostgreSQL test database {database_name}"
                    ),
                }
            });
        match cleanup {
            Ok(cleanup) => {
                let join_cleanup = move || {
                    if cleanup.join().is_err() {
                        eprintln!("PostgreSQL test database cleanup thread panicked");
                    }
                };
                if tokio::runtime::Handle::try_current().is_ok_and(|handle| {
                    handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread
                }) {
                    tokio::task::block_in_place(join_cleanup);
                } else {
                    join_cleanup();
                }
            }
            Err(error) => eprintln!("failed to spawn PostgreSQL test database cleanup: {error}"),
        }
    }
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;

/// Seed only the workspace row contract available in historical schemas.
/// Current bootstrap also writes newer feature tables, so it is inappropriate
/// for a fixture intentionally frozen before those migrations.
pub async fn insert_historical_workspace(
    pool: &PgPool,
    workspace_id: Uuid,
    tenant_id: Uuid,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("insert into workspaces (id, tenant_id, name) values ($1, $2, $3)")
        .bind(workspace_id)
        .bind(tenant_id)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}
