use std::{str::FromStr, time::Instant};

use sqlx::{postgres::PgConnectOptions, ConnectOptions, Connection, PgConnection};
use uuid::Uuid;

use crate::{cleanup, metrics, PostgresTestDatabase, PostgresTestPhase, PostgresTestSchema};

static FORMAL_MIGRATIONS: sqlx::migrate::Migrator =
    sqlx::migrate!("../storage/durable/postgres/migrations");

/// A command-owned, formally migrated database with no application bootstrap.
/// Preparation closes every connection before callers can clone it. Clones keep
/// the same schema name and migration ledger but own separate databases.
/// Call `cleanup` after the child has exited; Drop remains a panic fallback.
pub struct PostgresTestTemplate {
    schema: PostgresTestSchema,
}

impl PostgresTestTemplate {
    pub async fn prepare(base_database_url: &str) -> Result<Self, sqlx::Error> {
        let started = Instant::now();
        let result = Self::prepare_inner(base_database_url).await;
        metrics::record(
            PostgresTestPhase::Prepare,
            started.elapsed(),
            result.is_ok(),
        );
        result
    }

    async fn prepare_inner(base_database_url: &str) -> Result<Self, sqlx::Error> {
        let mut schema = PostgresTestSchema::create(base_database_url).await?;
        let connection = PgConnection::connect(schema.database_url()).await?;
        migrate_and_close(connection, &FORMAL_MIGRATIONS).await?;
        schema.database.clone_prefix = Some(format!("{}_", schema.database_name()));
        Ok(Self { schema })
    }

    pub fn database_name(&self) -> &str {
        self.schema.database_name()
    }

    pub fn schema_name(&self) -> &str {
        self.schema.schema_name()
    }

    /// Remove this template, then reap its exact-owner orphan clones. Never
    /// deletes unrelated `test_*` databases. The child must already be waited.
    pub async fn cleanup(mut self) -> Result<(), sqlx::Error> {
        self.schema.database.cleanup().await
    }
}

pub(crate) async fn migrate_and_close(
    mut connection: PgConnection,
    migrations: &sqlx::migrate::Migrator,
) -> Result<(), sqlx::Error> {
    let migration = migrations.run(&mut connection).await;
    // Always close before returning a migration error: failed migrations may
    // retain a PostgreSQL advisory lock until their connection is closed.
    let close = connection.close().await;
    migration.map_err(|error| sqlx::Error::Migrate(Box::new(error)))?;
    close
}

impl PostgresTestSchema {
    /// Clone an explicitly supplied prepared template. This does not migrate or
    /// bootstrap the clone: its normal production Migrator must still validate
    /// the inherited ledger. Invalid names/schema/ledger never fall back to fresh.
    pub async fn create_from_template(
        base_database_url: &str,
        database_name: &str,
        schema_name: &str,
    ) -> Result<Self, sqlx::Error> {
        let started = Instant::now();
        let result = Self::clone_template(base_database_url, database_name, schema_name).await;
        metrics::record(PostgresTestPhase::Clone, started.elapsed(), result.is_ok());
        result
    }

    async fn clone_template(
        base_database_url: &str,
        template_name: &str,
        schema_name: &str,
    ) -> Result<Self, sqlx::Error> {
        let database_name = clone_database_name(template_name)?;
        if !schema_name
            .strip_prefix("test_")
            .is_some_and(|suffix| cleanup::is_hex(suffix, 32))
        {
            return Err(sqlx::Error::InvalidArgument(
                "template schema must be an explicit generated test schema".into(),
            ));
        }
        let admin_options = PgConnectOptions::from_str(base_database_url)?;
        let mut connection = PgConnection::connect_with(&admin_options).await?;
        // SQL safety: both database names are fixed-prefix lower-case UUID names.
        let create = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"create database "{database_name}" with template "{template_name}""#
        )))
        .execute(&mut connection)
        .await;
        if let Err(error) = create {
            let _ = connection.close().await;
            return Err(error);
        }
        // Establish the Drop owner before another fallible operation.
        let database = PostgresTestDatabase {
            cleanup_options: admin_options.clone().database("postgres"),
            database_url: admin_options
                .database(&database_name)
                .to_url_lossy()
                .to_string(),
            database_name,
            clone_prefix: None,
            cleaned: false,
        };
        connection.close().await?;
        let separator = if database.database_url().contains('?') {
            '&'
        } else {
            '?'
        };
        let database_url = format!(
            "{}{separator}options=-csearch_path%3D{schema_name}%2Cpublic",
            database.database_url()
        );
        let mut connection = PgConnection::connect(&database_url).await?;
        let valid: Result<bool, sqlx::Error> =
            sqlx::query_scalar("select current_schema() = $1 and to_regclass($2) is not null")
                .bind(schema_name)
                .bind(format!("{schema_name}._sqlx_migrations"))
                .fetch_one(&mut connection)
                .await;
        let close = connection.close().await;
        if !valid? {
            return Err(sqlx::Error::InvalidArgument(
                "template schema or formal migration ledger is absent".into(),
            ));
        }
        close?;
        Ok(Self {
            database,
            database_url,
            schema_name: schema_name.into(),
        })
    }
}

pub(crate) fn clone_database_name(template_name: &str) -> Result<String, sqlx::Error> {
    if cleanup::template_id(template_name).is_none() {
        return Err(sqlx::Error::InvalidArgument(
            "template database must be an explicit generated test database".into(),
        ));
    }
    let id = Uuid::now_v7().simple().to_string();
    // Full template identity plus the random UUID suffix: 12 + 32 + 1 + 16 = 61
    // bytes, below PostgreSQL's 63-byte identifier limit without truncation.
    Ok(format!("{template_name}_{}", &id[16..]))
}
