//! Offline PostgreSQL staged-restore adapter.
//!
//! The implementation is intentionally independent from the API server pool; recovery connects
//! through an operator-provided target URL and promotes a verified staging database by rename.

use std::process::Stdio;

use async_trait::async_trait;
use control_plane::{
    ports::BackupComponentReader,
    system_recovery::{RecoveryStepContext, RecoveryStepTarget, RecoveryStepTargetError},
};
use domain::{BackupComponent, BackupComponentKind, BackupComponentRestoreTarget, RecoveryJobId};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tokio::{io::AsyncWriteExt, process::Command};
use url::Url;

use crate::system_backup::{
    migration_head, read_bounded_stderr, PostgreSqlCommandConnection, PostgreSqlToolchain,
};

const POSTGRES_IDENTIFIER_LIMIT: usize = 63;

pub struct PostgreSqlRecoveryTarget {
    target_url: Url,
    target_database: String,
    admin_database: &'static str,
    toolchain: PostgreSqlToolchain,
}

impl PostgreSqlRecoveryTarget {
    pub fn try_new(
        target_database_url: &str,
        toolchain: PostgreSqlToolchain,
    ) -> Result<Self, RecoveryStepTargetError> {
        let target_url =
            Url::parse(target_database_url).map_err(|_| RecoveryStepTargetError::InvalidTarget)?;
        if !matches!(target_url.scheme(), "postgres" | "postgresql")
            || target_url.fragment().is_some()
        {
            return Err(RecoveryStepTargetError::InvalidTarget);
        }
        let target_database = target_url
            .path()
            .strip_prefix('/')
            .filter(|value| valid_database_name(value))
            .ok_or(RecoveryStepTargetError::InvalidTarget)?
            .to_owned();
        let admin_database = match target_database.as_str() {
            "postgres" => "template1",
            "template0" | "template1" => return Err(RecoveryStepTargetError::InvalidTarget),
            _ => "postgres",
        };
        Ok(Self {
            target_url,
            target_database,
            admin_database,
            toolchain,
        })
    }

    fn names(&self, recovery_job_id: RecoveryJobId) -> RecoveryDatabaseNames {
        let job = recovery_job_id.as_uuid().simple();
        RecoveryDatabaseNames {
            staging: format!("_1flowbase_restore_{job}"),
            rollback: format!("_1flowbase_rollback_{job}"),
        }
    }

    fn database_url(&self, database: &str) -> String {
        let mut url = self.target_url.clone();
        url.set_path(&format!("/{database}"));
        url.to_string()
    }

    async fn admin_pool(&self) -> Result<PgPool, RecoveryStepTargetError> {
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.database_url(self.admin_database))
            .await
            .map_err(|_| RecoveryStepTargetError::Unavailable)
    }

    async fn reset_staging(
        &self,
        names: &RecoveryDatabaseNames,
    ) -> Result<(), RecoveryStepTargetError> {
        let pool = self.admin_pool().await?;
        let target_exists = database_exists(&pool, &self.target_database).await?;
        let rollback_exists = database_exists(&pool, &names.rollback).await?;
        if !target_exists && !rollback_exists {
            pool.close().await;
            return Err(RecoveryStepTargetError::InvalidTarget);
        }
        drop_database_if_exists(&pool, &names.staging).await?;
        execute_database_statement(
            &pool,
            &format!("create database {}", quote_identifier(&names.staging)),
        )
        .await?;
        pool.close().await;
        Ok(())
    }

    async fn restore_staging(
        &self,
        names: &RecoveryDatabaseNames,
        mut source: BackupComponentReader,
    ) -> Result<(), RecoveryStepTargetError> {
        let connection = PostgreSqlCommandConnection::parse(&self.database_url(&names.staging))
            .map_err(|_| RecoveryStepTargetError::InvalidTarget)?;
        let mut command = Command::new(self.toolchain.pg_restore());
        command
            .args(["--exit-on-error", "--no-owner", "--no-privileges"])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LC_ALL", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        connection.apply(&mut command);
        let mut child = command
            .spawn()
            .map_err(|_| RecoveryStepTargetError::Unavailable)?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or(RecoveryStepTargetError::Unavailable)?;
        let stderr = child
            .stderr
            .take()
            .ok_or(RecoveryStepTargetError::Unavailable)?;
        let stderr_task = tokio::spawn(read_bounded_stderr(stderr));
        if tokio::io::copy(&mut source, &mut stdin).await.is_err()
            || stdin.shutdown().await.is_err()
        {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(RecoveryStepTargetError::Staging);
        }
        drop(stdin);
        let status = child
            .wait()
            .await
            .map_err(|_| RecoveryStepTargetError::Staging)?;
        let stderr_bytes = stderr_task
            .await
            .ok()
            .and_then(Result::ok)
            .map_or(0, |stderr| stderr.len());
        if !status.success() {
            tracing::warn!(
                exit_status = ?status.code(),
                stderr_bytes,
                "PostgreSQL staged recovery failed"
            );
            return Err(RecoveryStepTargetError::Staging);
        }
        Ok(())
    }

    async fn verify_staging(
        &self,
        names: &RecoveryDatabaseNames,
        context: &RecoveryStepContext,
    ) -> Result<(), RecoveryStepTargetError> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.database_url(&names.staging))
            .await
            .map_err(|_| RecoveryStepTargetError::Integrity)?;
        let actual = migration_head(&pool)
            .await
            .map_err(|_| RecoveryStepTargetError::Integrity)?;
        pool.close().await;
        if actual != context.migration_head {
            return Err(RecoveryStepTargetError::Integrity);
        }
        Ok(())
    }

    async fn promote_staging(
        &self,
        names: &RecoveryDatabaseNames,
    ) -> Result<(), RecoveryStepTargetError> {
        let pool = self.admin_pool().await?;
        if !database_exists(&pool, &names.staging).await? {
            pool.close().await;
            return Err(RecoveryStepTargetError::Promotion);
        }
        let rollback_exists = database_exists(&pool, &names.rollback).await?;
        let target_exists = database_exists(&pool, &self.target_database).await?;
        if rollback_exists {
            if target_exists {
                drop_database_if_exists(&pool, &self.target_database).await?;
            }
        } else if target_exists {
            terminate_database_connections(&pool, &self.target_database).await?;
            rename_database(&pool, &self.target_database, &names.rollback).await?;
        } else {
            pool.close().await;
            return Err(RecoveryStepTargetError::Promotion);
        }
        terminate_database_connections(&pool, &names.staging).await?;
        if rename_database(&pool, &names.staging, &self.target_database)
            .await
            .is_err()
        {
            if !database_exists(&pool, &self.target_database).await?
                && database_exists(&pool, &names.rollback).await?
            {
                let _ = rename_database(&pool, &names.rollback, &self.target_database).await;
            }
            pool.close().await;
            return Err(RecoveryStepTargetError::Promotion);
        }
        pool.close().await;
        Ok(())
    }

    async fn rollback_databases(
        &self,
        names: &RecoveryDatabaseNames,
    ) -> Result<(), RecoveryStepTargetError> {
        let pool = self.admin_pool().await?;
        drop_database_if_exists(&pool, &names.staging).await?;
        if database_exists(&pool, &names.rollback).await? {
            drop_database_if_exists(&pool, &self.target_database).await?;
            rename_database(&pool, &names.rollback, &self.target_database)
                .await
                .map_err(|_| RecoveryStepTargetError::Compensation)?;
        }
        pool.close().await;
        Ok(())
    }

    async fn finalize_databases(
        &self,
        names: &RecoveryDatabaseNames,
    ) -> Result<(), RecoveryStepTargetError> {
        let pool = self.admin_pool().await?;
        drop_database_if_exists(&pool, &names.staging).await?;
        drop_database_if_exists(&pool, &names.rollback).await?;
        pool.close().await;
        Ok(())
    }
}

#[async_trait]
impl RecoveryStepTarget for PostgreSqlRecoveryTarget {
    async fn begin(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        validate_postgres_components(components)?;
        self.reset_staging(&self.names(context.recovery_job_id))
            .await
    }

    async fn stage_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
        plaintext: BackupComponentReader,
    ) -> Result<(), RecoveryStepTargetError> {
        validate_postgres_components(std::slice::from_ref(component))?;
        let names = self.names(context.recovery_job_id);
        self.restore_staging(&names, plaintext).await?;
        self.verify_staging(&names, context).await
    }

    async fn stage_identity(
        &self,
        _context: &RecoveryStepContext,
        _component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        Err(RecoveryStepTargetError::InvalidTarget)
    }

    async fn promote(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        validate_postgres_components(components)?;
        let names = self.names(context.recovery_job_id);
        self.promote_staging(&names).await?;
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.database_url(&self.target_database))
            .await
            .map_err(|_| RecoveryStepTargetError::Integrity)?;
        let actual = migration_head(&pool)
            .await
            .map_err(|_| RecoveryStepTargetError::Integrity)?;
        pool.close().await;
        if actual != context.migration_head {
            self.rollback_databases(&names).await?;
            return Err(RecoveryStepTargetError::Integrity);
        }
        Ok(())
    }

    async fn rollback(
        &self,
        context: &RecoveryStepContext,
        _components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        self.rollback_databases(&self.names(context.recovery_job_id))
            .await
    }

    async fn finalize(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        validate_postgres_components(components)?;
        self.finalize_databases(&self.names(context.recovery_job_id))
            .await
    }
}

struct RecoveryDatabaseNames {
    staging: String,
    rollback: String,
}

fn validate_postgres_components(
    components: &[BackupComponent],
) -> Result<(), RecoveryStepTargetError> {
    if components.len() == 1
        && components[0].kind == BackupComponentKind::PostgreSql
        && components[0].restore_target == BackupComponentRestoreTarget::PostgreSql
    {
        Ok(())
    } else {
        Err(RecoveryStepTargetError::InvalidTarget)
    }
}

fn valid_database_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= POSTGRES_IDENTIFIER_LIMIT
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

async fn database_exists(pool: &PgPool, database: &str) -> Result<bool, RecoveryStepTargetError> {
    sqlx::query("select exists(select 1 from pg_database where datname = $1)")
        .bind(database)
        .fetch_one(pool)
        .await
        .map_err(|_| RecoveryStepTargetError::Unavailable)?
        .try_get(0)
        .map_err(|_| RecoveryStepTargetError::Unavailable)
}

async fn terminate_database_connections(
    pool: &PgPool,
    database: &str,
) -> Result<(), RecoveryStepTargetError> {
    sqlx::query(
        "select pg_terminate_backend(pid) from pg_stat_activity where datname = $1 and pid <> pg_backend_pid()",
    )
    .bind(database)
    .execute(pool)
    .await
    .map_err(|_| RecoveryStepTargetError::Unavailable)?;
    Ok(())
}

async fn drop_database_if_exists(
    pool: &PgPool,
    database: &str,
) -> Result<(), RecoveryStepTargetError> {
    if !database_exists(pool, database).await? {
        return Ok(());
    }
    terminate_database_connections(pool, database).await?;
    execute_database_statement(
        pool,
        &format!("drop database {}", quote_identifier(database)),
    )
    .await
}

async fn rename_database(
    pool: &PgPool,
    from: &str,
    to: &str,
) -> Result<(), RecoveryStepTargetError> {
    execute_database_statement(
        pool,
        &format!(
            "alter database {} rename to {}",
            quote_identifier(from),
            quote_identifier(to)
        ),
    )
    .await
}

async fn execute_database_statement(
    pool: &PgPool,
    statement: &str,
) -> Result<(), RecoveryStepTargetError> {
    sqlx::query(statement)
        .execute(pool)
        .await
        .map_err(|_| RecoveryStepTargetError::Unavailable)?;
    Ok(())
}
