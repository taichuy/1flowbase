use std::{str::FromStr, time::Duration};

use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, Connection, PgConnection, PgPool,
};
use uuid::Uuid;

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
        sqlx::query(&format!(r#"create schema "{schema_name}""#))
            .execute(&mut connection)
            .await?;
        connection.close().await?;
        // PgConnectOptions::to_url_lossy does not serialize startup options.
        // Keep the explicit encoded option in the caller-visible connection URL.
        let query_separator = if database.database_url().contains('?') {
            '&'
        } else {
            '?'
        };
        let database_url = format!(
            "{}{query_separator}options=-csearch_path%3D{schema_name}%2Cpublic",
            database.database_url()
        );
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
        sqlx::query(&format!(r#"create database "{database_name}""#))
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
                        sqlx::query(
                            "select pg_terminate_backend(pid) from pg_stat_activity where datname = $1 and pid <> pg_backend_pid()",
                        )
                        .bind(&database_name)
                        .execute(&mut connection)
                        .await?;
                        sqlx::query(&format!(r#"drop database if exists "{database_name}""#))
                            .execute(&mut connection)
                            .await?;
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
                if cleanup.join().is_err() {
                    eprintln!("PostgreSQL test database cleanup thread panicked");
                }
            }
            Err(error) => eprintln!("failed to spawn PostgreSQL test database cleanup: {error}"),
        }
    }
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
