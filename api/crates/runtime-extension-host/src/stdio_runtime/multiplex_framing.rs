use extension_contracts::{MultiplexEnvelope, MultiplexWorkerMessage};
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

pub(super) async fn read_frame<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    bytes: &mut Vec<u8>,
) -> Result<Option<(MultiplexWorkerMessage, usize)>, String> {
    loop {
        let available = reader.fill_buf().await.map_err(|error| error.to_string())?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err("partial multiplex frame".into())
            };
        }
        let count = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |i| i + 1);
        bytes.extend_from_slice(&available[..count]);
        reader.consume(count);
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            // The shared reader keeps partial input across cancelled read futures.
            // A complete frame is owned by its decoded message; do not retain its
            // high-water allocation for the lifetime of the idle worker.
            let complete = std::mem::take(bytes);
            let envelope: MultiplexEnvelope<MultiplexWorkerMessage> =
                serde_json::from_slice(complete.as_slice()).map_err(|error| error.to_string())?;
            if !envelope.is_supported() {
                return Err("unsupported multiplex protocol".into());
            }
            let frame_bytes = complete.len() + 1;
            return Ok(Some((envelope.message, frame_bytes)));
        }
    }
}

#[cfg(test)]
#[path = "../_tests/stdio_runtime/multiplex_framing.rs"]
mod tests;
