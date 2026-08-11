use std::process::Stdio;

use async_trait::async_trait;
use control_plane::{
    ports::BackupComponentWriter,
    system_backup::{BackupComponentDescriptor, BackupComponentSource, BackupSourceError},
};
use domain::ContentDigest;
use domain::{
    ArtifactRebuildability, BackupComponentDisposition, BackupComponentId, BackupComponentKind,
    BackupComponentRestoreTarget, BackupSourceIdentity,
};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    process::Command,
};

use super::{read_bounded_stderr, PostgreSqlBackupError, PostgreSqlToolchain};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgreSqlDumpReceipt {
    pub size_bytes: u64,
    pub content_digest: ContentDigest,
}

pub struct PostgreSqlLogicalBackup {
    database_url: String,
    toolchain: PostgreSqlToolchain,
}

impl PostgreSqlLogicalBackup {
    pub fn new(database_url: impl Into<String>, toolchain: PostgreSqlToolchain) -> Self {
        Self {
            database_url: database_url.into(),
            toolchain,
        }
    }

    pub async fn dump_to(
        &self,
        mut destination: impl AsyncWrite + Unpin,
    ) -> Result<PostgreSqlDumpReceipt, PostgreSqlBackupError> {
        let mut child = Command::new(self.toolchain.pg_dump())
            .args([
                "--format=custom",
                "--compress=0",
                "--no-owner",
                "--no-privileges",
                "--strict-names",
            ])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LC_ALL", "C")
            .env("PGDATABASE", &self.database_url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| PostgreSqlBackupError::ToolUnavailable)?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or(PostgreSqlBackupError::ToolUnavailable)?;
        let stderr = child
            .stderr
            .take()
            .ok_or(PostgreSqlBackupError::ToolUnavailable)?;
        let stderr_task = tokio::spawn(read_bounded_stderr(stderr));
        let mut hasher = Sha256::new();
        let mut size_bytes = 0_u64;
        let mut buffer = vec![0_u8; 256 * 1024];
        loop {
            let read = stdout.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            size_bytes = size_bytes.checked_add(read as u64).ok_or(
                PostgreSqlBackupError::CommandFailed {
                    code: "dump_size_overflow".to_owned(),
                },
            )?;
            destination.write_all(&buffer[..read]).await?;
        }
        let status = child.wait().await?;
        let stderr = stderr_task
            .await
            .map_err(|_| PostgreSqlBackupError::CommandFailed {
                code: "stderr_reader_failed".to_owned(),
            })??;
        if !status.success() {
            tracing::warn!(
                exit_status = ?status.code(),
                stderr_bytes = stderr.len(),
                "PostgreSQL logical backup failed"
            );
            return Err(PostgreSqlBackupError::CommandFailed {
                code: "pg_dump_failed".to_owned(),
            });
        }
        destination.flush().await?;
        let digest = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(PostgreSqlDumpReceipt {
            size_bytes,
            content_digest: ContentDigest::try_from(digest)
                .map_err(|_| PostgreSqlBackupError::InvalidToolVersion)?,
        })
    }
}

#[async_trait]
impl BackupComponentSource for PostgreSqlLogicalBackup {
    fn descriptor(&self) -> BackupComponentDescriptor {
        BackupComponentDescriptor {
            component_id: BackupComponentId::try_from("postgresql").expect("static backup id"),
            kind: BackupComponentKind::PostgreSql,
            source_identity: BackupSourceIdentity::try_from("postgresql/durable")
                .expect("static backup source identity"),
            content_type: "application/vnd.postgresql.custom-dump".to_string(),
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
            restore_target: BackupComponentRestoreTarget::PostgreSql,
        }
    }

    async fn write_to(&self, destination: BackupComponentWriter) -> Result<(), BackupSourceError> {
        self.dump_to(destination)
            .await
            .map(|_| ())
            .map_err(|_| BackupSourceError::Unavailable)
    }
}
