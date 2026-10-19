//! Binary readers are owned by one active multiplex call, never by a worker or ticket registry.
use base64::{engine::general_purpose::STANDARD, Engine};
use extension_contracts::{
    FileReadError, FileReadRequest, FileReadResponse, FILE_READ_CHUNK_BYTES,
};
use runtime_core::runtime_backend::{FileReadBinding, FileReadPort, FileReadSource};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::io::AsyncReadExt;

struct OpenRead {
    ticket: String,
    source: FileReadSource,
    offset: u64,
    hash: Sha256,
}
#[derive(Clone)]
pub(crate) struct FileReadCall {
    binding: FileReadBinding,
    port: Arc<dyn FileReadPort>,
    state: Arc<tokio::sync::Mutex<Option<OpenRead>>>,
}
impl FileReadCall {
    pub(crate) fn new(binding: FileReadBinding, port: Arc<dyn FileReadPort>) -> Self {
        Self {
            binding,
            port,
            state: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
    pub(crate) async fn execute(
        &self,
        value: serde_json::Value,
    ) -> Result<FileReadResponse, FileReadError> {
        let mut state = self.state.lock().await;
        // Removing before awaiting means abort/timeout also drops the reader.
        let previous = state.take();
        let future = async {
            let request: FileReadRequest = serde_json::from_value(value)
                .map_err(|_| FileReadError::new("file_read_request_invalid"))?;
            request.validate()?;
            self.port.authorize(&self.binding).await?;
            match request {
                FileReadRequest::Open {
                    file_table_id,
                    record_id,
                } => {
                    if previous.is_some() {
                        return Err(FileReadError::new("file_read_already_open"));
                    }
                    let source = self
                        .port
                        .open(&self.binding, &file_table_id, &record_id)
                        .await?;
                    let ticket = uuid::Uuid::now_v7().to_string();
                    let response = FileReadResponse::Open {
                        ticket: ticket.clone(),
                        metadata: source.metadata.clone(),
                    };
                    Ok((
                        response,
                        Some(OpenRead {
                            ticket,
                            source,
                            offset: 0,
                            hash: Sha256::new(),
                        }),
                    ))
                }
                FileReadRequest::Chunk { ticket, offset } => {
                    let mut read = previous
                        .filter(|r| r.ticket == ticket && r.offset == offset)
                        .ok_or_else(|| FileReadError::new("file_read_ticket_invalid"))?;
                    let remaining = read
                        .source
                        .metadata
                        .content_length
                        .checked_sub(offset)
                        .ok_or_else(|| FileReadError::new("file_read_length_invalid"))?;
                    let mut data = vec![0; remaining.min(FILE_READ_CHUNK_BYTES as u64) as usize];
                    read.source
                        .reader
                        .read_exact(&mut data)
                        .await
                        .map_err(|_| FileReadError::new("file_read_truncated"))?;
                    read.offset += data.len() as u64;
                    read.hash.update(&data);
                    let eof = read.offset == read.source.metadata.content_length;
                    let sha256 = if eof {
                        let mut extra = [0u8; 1];
                        if read
                            .source
                            .reader
                            .read(&mut extra)
                            .await
                            .map_err(|_| FileReadError::new("file_read_io_failed"))?
                            != 0
                        {
                            return Err(FileReadError::new("file_read_length_invalid"));
                        }
                        Some(format!(
                            "{:x}",
                            base16ct::HexDisplay(&read.hash.clone().finalize())
                        ))
                    } else {
                        None
                    };
                    Ok((
                        FileReadResponse::Chunk {
                            ticket,
                            offset,
                            data_base64: STANDARD.encode(data),
                            eof,
                            sha256,
                        },
                        Some(read),
                    ))
                }
                FileReadRequest::Close { ticket } => {
                    if previous.is_none_or(|r| r.ticket != ticket) {
                        return Err(FileReadError::new("file_read_ticket_invalid"));
                    }
                    Ok((FileReadResponse::Close { ticket }, None))
                }
            }
        };
        let remaining = self.binding.deadline_unix_ms.saturating_sub(
            i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
                .unwrap_or(i64::MAX),
        );
        if remaining <= 0 {
            return Err(FileReadError::new("file_read_deadline_exceeded"));
        }
        let (response, reader) = if self.binding.deadline_unix_ms == i64::MAX {
            future.await?
        } else {
            tokio::time::timeout(std::time::Duration::from_millis(remaining as u64), future)
                .await
                .map_err(|_| FileReadError::new("file_read_deadline_exceeded"))??
        };
        *state = reader;
        Ok(response)
    }
}

#[cfg(test)]
#[path = "_tests/host_services/file_read_callbacks.rs"]
mod tests;
