use std::time::{Duration, Instant};

use sqlx::{postgres::PgConnectOptions, Connection, PgConnection};

use crate::{metrics, PostgresTestPhase};

pub(crate) fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn template_id(name: &str) -> Option<&str> {
    name.strip_prefix("test_backup_")
        .filter(|suffix| is_hex(suffix, 32))
}

pub(crate) fn is_owned_clone(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|suffix| is_hex(suffix, 16))
}

pub(crate) fn is_owned_database_name(name: &str) -> bool {
    if template_id(name).is_some() {
        return true;
    }
    let Some((template, suffix)) = name.rsplit_once('_') else {
        return false;
    };
    template_id(template).is_some() && is_hex(suffix, 16)
}

/// Reuses the original whole-database cleanup boundary. Template owners may
/// additionally reap orphan clones of their exact UUID after a child exits.
pub(crate) async fn cleanup_owned_databases(
    options: &PgConnectOptions,
    database_name: &str,
    clone_prefix: Option<&str>,
) -> Result<(), sqlx::Error> {
    if !is_owned_database_name(database_name)
        || clone_prefix.is_some_and(|prefix| {
            template_id(database_name).is_none() || prefix != format!("{database_name}_")
        })
    {
        return Err(sqlx::Error::InvalidArgument(
            "refusing cleanup without an exact test database owner".into(),
        ));
    }
    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        let mut connection = PgConnection::connect_with(options).await?;
        let cleanup = async {
            // Stop new clones first. DROP takes the template database lock, so
            // an in-flight CREATE DATABASE finishes before the orphan scan.
            // This closes the child-exit / server-command completion race.
            let mut first_error = drop_database(&mut connection, database_name).await.err();
            if let Some(prefix) = clone_prefix {
                let names: Vec<String> = sqlx::query_scalar(
                    "select datname::text from pg_database where starts_with(datname::text, $1)",
                )
                .bind(prefix)
                .fetch_all(&mut connection)
                .await?;
                for name in names.iter().filter(|name| is_owned_clone(name, prefix)) {
                    if let Err(error) = drop_database(&mut connection, name).await {
                        first_error.get_or_insert(error);
                    }
                }
            }
            first_error.map_or(Ok(()), Err)
        }
        .await;
        let close = connection.close().await;
        cleanup.and(close)
    })
    .await
    .unwrap_or_else(|_| {
        Err(sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "PostgreSQL test database cleanup timed out",
        )))
    });
    metrics::record(
        PostgresTestPhase::Cleanup,
        started.elapsed(),
        result.is_ok(),
    );
    result
}

async fn drop_database(
    connection: &mut PgConnection,
    database_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "select pg_terminate_backend(pid) from pg_stat_activity where datname = $1 and pid <> pg_backend_pid()",
    )
    .bind(database_name)
    .execute(&mut *connection)
    .await?;
    // SQL safety: callers only pass generated or exact-owner-validated UUID names.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"drop database if exists "{database_name}""#
    )))
    .execute(connection)
    .await?;
    Ok(())
}
