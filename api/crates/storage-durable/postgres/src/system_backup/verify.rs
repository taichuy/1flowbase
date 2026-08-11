use std::process::Stdio;

use tokio::{
    io::{AsyncRead, AsyncWriteExt},
    process::Command,
};

use super::{read_bounded_stderr, PostgreSqlBackupError, PostgreSqlToolchain};

pub async fn verify_custom_dump(
    toolchain: &PostgreSqlToolchain,
    mut source: impl AsyncRead + Unpin,
) -> Result<(), PostgreSqlBackupError> {
    let mut child = Command::new(toolchain.pg_restore())
        .arg("--list")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| PostgreSqlBackupError::ToolUnavailable)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or(PostgreSqlBackupError::ToolUnavailable)?;
    tokio::io::copy(&mut source, &mut stdin).await?;
    stdin.shutdown().await?;
    drop(stdin);
    let stderr = child
        .stderr
        .take()
        .ok_or(PostgreSqlBackupError::ToolUnavailable)?;
    let stderr_task = tokio::spawn(read_bounded_stderr(stderr));
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
            "PostgreSQL custom dump verification failed"
        );
        return Err(PostgreSqlBackupError::CommandFailed {
            code: "pg_restore_verify_failed".to_owned(),
        });
    }
    Ok(())
}
