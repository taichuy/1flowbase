use std::path::{Path, PathBuf};

use sqlx::{PgPool, Row};
use thiserror::Error;
use tokio::{io::AsyncReadExt, process::Command};

const STDERR_LIMIT_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgreSqlToolchain {
    pg_dump: PathBuf,
    pg_restore: PathBuf,
    major_version: u32,
}

#[derive(Debug, Error)]
pub enum PostgreSqlBackupError {
    #[error("PostgreSQL backup tool is unavailable")]
    ToolUnavailable,
    #[error("PostgreSQL backup tool version is invalid")]
    InvalidToolVersion,
    #[error("PostgreSQL backup tool major version is incompatible with the server")]
    IncompatibleToolVersion,
    #[error("PostgreSQL backup command failed: {code}")]
    CommandFailed { code: String },
    #[error("PostgreSQL backup I/O failed")]
    Io(#[from] std::io::Error),
    #[error("PostgreSQL backup metadata query failed")]
    Database(#[from] sqlx::Error),
    #[error("PostgreSQL managed schema backup inventory is inconsistent")]
    ManagedSchemaInventoryInvalid,
}

impl PostgreSqlToolchain {
    pub async fn discover(
        pg_dump: impl AsRef<Path>,
        pg_restore: impl AsRef<Path>,
    ) -> Result<Self, PostgreSqlBackupError> {
        let pg_dump = pg_dump.as_ref().to_path_buf();
        let pg_restore = pg_restore.as_ref().to_path_buf();
        let dump_major = command_major_version(&pg_dump).await?;
        let restore_major = command_major_version(&pg_restore).await?;
        if dump_major != restore_major {
            return Err(PostgreSqlBackupError::IncompatibleToolVersion);
        }
        Ok(Self {
            pg_dump,
            pg_restore,
            major_version: dump_major,
        })
    }

    pub async fn discover_from_path() -> Result<Self, PostgreSqlBackupError> {
        Self::discover("pg_dump", "pg_restore").await
    }

    pub const fn major_version(&self) -> u32 {
        self.major_version
    }

    pub fn pg_dump(&self) -> &Path {
        &self.pg_dump
    }

    pub fn pg_restore(&self) -> &Path {
        &self.pg_restore
    }

    pub async fn verify_server_compatibility(
        &self,
        pool: &PgPool,
    ) -> Result<u32, PostgreSqlBackupError> {
        let version: String = sqlx::query("show server_version_num")
            .fetch_one(pool)
            .await?
            .try_get(0)?;
        let version = version
            .parse::<u32>()
            .map_err(|_| PostgreSqlBackupError::InvalidToolVersion)?;
        let server_major = version / 10_000;
        if server_major != self.major_version {
            return Err(PostgreSqlBackupError::IncompatibleToolVersion);
        }
        Ok(server_major)
    }
}

async fn command_major_version(path: &Path) -> Result<u32, PostgreSqlBackupError> {
    let output = Command::new(path)
        .arg("--version")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("LC_ALL", "C")
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|_| PostgreSqlBackupError::ToolUnavailable)?;
    if !output.status.success() {
        return Err(PostgreSqlBackupError::CommandFailed {
            code: "tool_version_failed".to_owned(),
        });
    }
    parse_major_version(std::str::from_utf8(&output.stdout).unwrap_or_default())
}

pub(crate) fn parse_major_version(value: &str) -> Result<u32, PostgreSqlBackupError> {
    let version = value
        .split_whitespace()
        .find(|part| {
            part.bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_digit())
        })
        .ok_or(PostgreSqlBackupError::InvalidToolVersion)?;
    version
        .split('.')
        .next()
        .and_then(|major| major.parse::<u32>().ok())
        .filter(|major| *major > 0)
        .ok_or(PostgreSqlBackupError::InvalidToolVersion)
}

pub(crate) async fn read_bounded_stderr(
    stderr: impl tokio::io::AsyncRead + Unpin,
) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    stderr
        .take(STDERR_LIMIT_BYTES)
        .read_to_end(&mut bytes)
        .await?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_owned())
}
