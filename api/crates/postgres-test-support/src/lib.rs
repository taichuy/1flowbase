use std::{str::FromStr, time::Duration};

use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, Connection, PgConnection, PgPool,
};
use uuid::Uuid;

pub struct PostgresTestSchema {
    base_database_url: String,
    database_url: String,
    schema_name: String,
}

impl PostgresTestSchema {
    pub async fn create(base_database_url: &str) -> Result<Self, sqlx::Error> {
        let schema_name = format!("test_{}", Uuid::now_v7().simple());
        let mut connection = PgConnection::connect(base_database_url).await?;
        sqlx::query(&format!(r#"create schema "{schema_name}""#))
            .execute(&mut connection)
            .await?;
        connection.close().await?;

        let query_separator = if base_database_url.contains('?') {
            '&'
        } else {
            '?'
        };
        let database_url =
            format!("{base_database_url}{query_separator}options=-csearch_path%3D{schema_name}");

        Ok(Self {
            base_database_url: base_database_url.to_owned(),
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

impl Drop for PostgresTestSchema {
    fn drop(&mut self) {
        let base_database_url = self.base_database_url.clone();
        let schema_name = self.schema_name.clone();
        let cleanup = std::thread::Builder::new()
            .name(format!("drop-{schema_name}"))
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        eprintln!("failed to start PostgreSQL test schema cleanup: {error}");
                        return;
                    }
                };

                let result = runtime.block_on(async {
                    tokio::time::timeout(Duration::from_secs(30), async {
                        let mut connection = PgConnection::connect(&base_database_url).await?;
                        sqlx::query(&format!(r#"drop schema if exists "{schema_name}" cascade"#))
                            .execute(&mut connection)
                            .await?;
                        connection.close().await
                    })
                    .await
                });

                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        eprintln!("failed to drop PostgreSQL test schema {schema_name}: {error}")
                    }
                    Err(_) => {
                        eprintln!("timed out while dropping PostgreSQL test schema {schema_name}")
                    }
                }
            });

        match cleanup {
            Ok(cleanup) => {
                if cleanup.join().is_err() {
                    eprintln!("PostgreSQL test schema cleanup thread panicked");
                }
            }
            Err(error) => eprintln!("failed to spawn PostgreSQL test schema cleanup: {error}"),
        }
    }
}

/// Owns a whole temporary database for destructive backup/restore fixtures.
///
/// This is intentionally separate from `PostgresTestSchema`: a logical restore
/// can replace schemas and migration metadata, so schema-only isolation is not
/// a sufficient safety boundary.
pub struct PostgresTestDatabase {
    admin_database_url: String,
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
            admin_database_url: admin_database_url.to_owned(),
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
        let admin_database_url = self.admin_database_url.clone();
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
                        let mut connection = PgConnection::connect(&admin_database_url).await?;
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
mod tests {
    use sqlx::PgPool;

    fn base_database_url() -> String {
        std::env::var("DATABASE_URL")
            .or_else(|_| std::env::var("API_DATABASE_URL"))
            .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
    }

    #[tokio::test]
    async fn ac_001_drop_removes_the_test_schema() {
        let base_url = base_database_url();
        let database = super::PostgresTestSchema::create(&base_url).await.unwrap();
        let schema_name = database.schema_name().to_owned();
        let test_pool = database.connect().await.unwrap();

        let pool = PgPool::connect(&base_url).await.unwrap();
        let exists: bool =
            sqlx::query_scalar("select exists(select 1 from pg_namespace where nspname = $1)")
                .bind(&schema_name)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(exists);

        test_pool.close().await;
        drop(test_pool);

        let pool = PgPool::connect(&base_url).await.unwrap();
        let exists: bool =
            sqlx::query_scalar("select exists(select 1 from pg_namespace where nspname = $1)")
                .bind(&schema_name)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!exists, "test schema must be removed when its guard drops");
    }

    #[tokio::test]
    async fn ac_002_panic_drops_the_test_schema() {
        let base_url = base_database_url();
        let database = super::PostgresTestSchema::create(&base_url).await.unwrap();
        let schema_name = database.schema_name().to_owned();
        let test_pool = database.connect().await.unwrap();

        let panic = tokio::spawn(async move {
            let _test_pool = test_pool;
            panic!("intentional test panic");
        })
        .await;
        assert!(panic.unwrap_err().is_panic());

        let pool = PgPool::connect(&base_url).await.unwrap();
        let exists: bool =
            sqlx::query_scalar("select exists(select 1 from pg_namespace where nspname = $1)")
                .bind(&schema_name)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!exists, "panic must not leak the test schema");
    }
}
