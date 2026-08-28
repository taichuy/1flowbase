use std::io::{BufRead, Write};

use extension_contracts::{
    PluginDataRequest, PluginDataResponse, RuntimeHostFrame, RuntimeHostWorkerFrame,
    PLUGIN_DATA_SERVICE_V1, RUNTIME_HOST_CALL_PROTOCOL_V1,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeExtensionSdkError {
    #[error("plugin data request is invalid: {0}")]
    InvalidRequest(String),
    #[error("runtime host I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("runtime host frame is malformed: {0}")]
    MalformedFrame(#[from] serde_json::Error),
    #[error("runtime host returned an uncorrelated frame")]
    UncorrelatedFrame,
    #[error("runtime host rejected plugin data operation: {0}")]
    Host(extension_contracts::PluginDataError),
}

pub struct PluginDataClient<R, W> {
    reader: R,
    writer: W,
    next_call_sequence: u64,
}

impl<R, W> PluginDataClient<R, W>
where
    R: BufRead,
    W: Write,
{
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            next_call_sequence: 1,
        }
    }

    pub fn execute(
        &mut self,
        request: PluginDataRequest,
    ) -> Result<PluginDataResponse, RuntimeExtensionSdkError> {
        request
            .validate()
            .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.code))?;
        let call_id = format!("sdk-{}", self.next_call_sequence);
        self.next_call_sequence = self.next_call_sequence.saturating_add(1);
        let frame = RuntimeHostWorkerFrame::HostCall {
            protocol: RUNTIME_HOST_CALL_PROTOCOL_V1.to_string(),
            call_id: call_id.clone(),
            service: PLUGIN_DATA_SERVICE_V1.to_string(),
            request,
        };
        serde_json::to_writer(&mut self.writer, &frame)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;

        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Err(RuntimeExtensionSdkError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "runtime host closed before host_result",
            )));
        }
        let response: RuntimeHostFrame = serde_json::from_str(line.trim())?;
        match response {
            RuntimeHostFrame::HostResult {
                protocol,
                call_id: response_call_id,
                result,
                error,
            } if protocol == RUNTIME_HOST_CALL_PROTOCOL_V1 && response_call_id == call_id => {
                match (result, error) {
                    (Some(result), None) => Ok(result),
                    (None, Some(error)) => Err(RuntimeExtensionSdkError::Host(error)),
                    _ => Err(RuntimeExtensionSdkError::UncorrelatedFrame),
                }
            }
            _ => Err(RuntimeExtensionSdkError::UncorrelatedFrame),
        }
    }

    pub fn into_inner(self) -> (R, W) {
        (self.reader, self.writer)
    }
}
