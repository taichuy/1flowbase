use std::{ffi::OsString, path::PathBuf};

use storage_durable_postgres::PostgreSqlToolchain;
use thiserror::Error;

pub const PG_DUMP_PATH_ENV: &str = "API_POSTGRES_PG_DUMP_PATH";
pub const PG_RESTORE_PATH_ENV: &str = "API_POSTGRES_PG_RESTORE_PATH";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PostgreSqlToolchainSource {
    Explicit {
        pg_dump: PathBuf,
        pg_restore: PathBuf,
    },
    Path,
}

#[derive(Debug, Error)]
pub enum PostgreSqlToolchainDiscoveryError {
    #[error("API_POSTGRES_PG_DUMP_PATH and API_POSTGRES_PG_RESTORE_PATH must be configured together and must not be empty")]
    IncompleteExplicitConfiguration,
    #[error("PostgreSQL backup tool discovery failed")]
    Discovery,
}

pub(crate) fn resolve_postgres_toolchain_source(
    pg_dump: Option<OsString>,
    pg_restore: Option<OsString>,
) -> Result<PostgreSqlToolchainSource, PostgreSqlToolchainDiscoveryError> {
    match (pg_dump, pg_restore) {
        (Some(pg_dump), Some(pg_restore)) if !pg_dump.is_empty() && !pg_restore.is_empty() => {
            Ok(PostgreSqlToolchainSource::Explicit {
                pg_dump: PathBuf::from(pg_dump),
                pg_restore: PathBuf::from(pg_restore),
            })
        }
        (None, None) => Ok(PostgreSqlToolchainSource::Path),
        _ => Err(PostgreSqlToolchainDiscoveryError::IncompleteExplicitConfiguration),
    }
}

pub async fn discover_postgres_toolchain(
) -> Result<PostgreSqlToolchain, PostgreSqlToolchainDiscoveryError> {
    match resolve_postgres_toolchain_source(
        std::env::var_os(PG_DUMP_PATH_ENV),
        std::env::var_os(PG_RESTORE_PATH_ENV),
    )? {
        PostgreSqlToolchainSource::Explicit {
            pg_dump,
            pg_restore,
        } => PostgreSqlToolchain::discover(pg_dump, pg_restore)
            .await
            .map_err(|_| PostgreSqlToolchainDiscoveryError::Discovery),
        PostgreSqlToolchainSource::Path => PostgreSqlToolchain::discover_from_path()
            .await
            .map_err(|_| PostgreSqlToolchainDiscoveryError::Discovery),
    }
}
