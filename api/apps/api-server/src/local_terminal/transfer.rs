use crate::local_terminal::{
    dto::{DownloadInput, DownloadOutput},
    path, Budget,
};
fn invalid(message: &'static str) -> ApiError {
    super::file_error(crate::local_terminal::invalid(message))
}
use crate::{app_state::ApiState, error_response::ApiError};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(crate) async fn download(
    state: &Arc<ApiState>,
    actor: domain::ActorContext,
    input: DownloadInput,
) -> Result<DownloadOutput, ApiError> {
    let budget = Budget::new(input.timeout_ms).map_err(super::file_error)?;
    path(&input.target_path).map_err(super::file_error)?;
    let duration = std::time::Duration::from_millis(input.timeout_ms);
    tokio::time::timeout(duration, async {
        let source = control_plane::file_management::FileAccessService::new(
            state.store.clone(),
            state.file_storage_registry.clone(),
            state.runtime_engine.clone(),
            state.infrastructure.ephemeral_kv_store(),
        )
        .read(actor, input.file_table_id, input.record_id)
        .await?;
        let reader = source
            .reader
            .into_inner()
            .map_err(|_| invalid("terminal_file_reader_unavailable"))?;
        transfer(reader, source.content_length, input, budget).await
    })
    .await
    .map_err(|_| invalid("terminal_timeout"))?
}
/// Hash the complete authorized stream, then independently read the staged bytes before commit.
/// This proves transfer integrity, not authenticity of an absent stored source digest.
pub(super) async fn transfer<R: AsyncRead + Unpin>(
    mut reader: R,
    content_length: u64,
    input: DownloadInput,
    budget: Budget,
) -> Result<DownloadOutput, ApiError> {
    let target = path(&input.target_path).map_err(super::file_error)?;
    let parent = target
        .parent()
        .ok_or_else(|| invalid("terminal_target_invalid"))?;
    if let Some(expected) = &input.expected_sha256 {
        if expected.len() != 64 || !expected.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(invalid("terminal_sha256_invalid"));
        }
    }
    let temporary_path = parent.join(format!(".terminal-{}.part", uuid::Uuid::now_v7()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // Create and acquire cleanup ownership without an await cancellation gap.
    let file = options.open(&temporary_path)?;
    let temporary = Temporary(temporary_path);
    let mut output = tokio::fs::File::from_std(file);
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = vec![0_u8; 65536];
    loop {
        budget.check().map_err(super::file_error)?;
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(read as u64)
            .ok_or_else(|| invalid("terminal_file_length_invalid"))?;
        if bytes > content_length {
            return Err(invalid("terminal_file_length_invalid"));
        }
        hasher.update(&buffer[..read]);
        output.write_all(&buffer[..read]).await?;
    }
    if bytes != content_length {
        return Err(invalid("terminal_file_length_invalid"));
    }
    output.flush().await?;
    output.sync_all().await?;
    drop(output);
    let sha256 = format!("{:x}", base16ct::HexDisplay(&hasher.finalize()));
    if input
        .expected_sha256
        .as_ref()
        .is_some_and(|expected| !expected.eq_ignore_ascii_case(&sha256))
    {
        return Err(invalid("terminal_file_hash_mismatch"));
    }
    let mut staged = tokio::fs::File::open(&temporary.0).await?;
    let mut staged_hash = Sha256::new();
    let mut staged_bytes = 0_u64;
    loop {
        budget.check().map_err(super::file_error)?;
        let read = staged.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        staged_hash.update(&buffer[..read]);
        staged_bytes += read as u64;
    }
    drop(staged);
    if staged_bytes != bytes
        || format!("{:x}", base16ct::HexDisplay(&staged_hash.finalize())) != sha256
    {
        return Err(invalid("terminal_file_hash_mismatch"));
    }
    budget.check().map_err(super::file_error)?;
    // Synchronous final publication cannot be detached by timeout cancellation.
    if input.overwrite {
        std::fs::rename(&temporary.0, &target)?;
    } else {
        std::fs::hard_link(&temporary.0, &target).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                control_plane::errors::ControlPlaneError::Conflict("terminal_target_exists").into()
            } else {
                ApiError::from(error)
            }
        })?;
    }
    Ok(DownloadOutput {
        file_table_id: input.file_table_id,
        record_id: input.record_id,
        target_path: input.target_path,
        byte_count: bytes,
        sha256,
    })
}
