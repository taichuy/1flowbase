use crate::{MultiplexEmitter, MultiplexError, MultiplexHostService};
use extension_contracts::{
    PluginExecutionGateError, PluginExecutionGateRequest, PluginExecutionGateResponse,
};
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum PluginExecutionGateClientError {
    #[error(transparent)]
    Transport(#[from] MultiplexError),
    #[error(transparent)]
    Host(#[from] PluginExecutionGateError),
    #[error("invalid execution gate host response")]
    InvalidResponse,
}
#[derive(Clone)]
pub struct PluginExecutionGateClient {
    emitter: MultiplexEmitter,
}
/// Host owns the permit until explicit release or the enclosing call ends.
/// Dropping this value does not schedule asynchronous work.
#[must_use = "release the gate after the handshake; call teardown is the cancellation fallback"]
pub struct PluginExecutionGatePermit {
    client: PluginExecutionGateClient,
    key: String,
    token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    result: Option<PluginExecutionGateResponse>,
    error: Option<PluginExecutionGateError>,
}
impl PluginExecutionGateClient {
    pub fn new(emitter: MultiplexEmitter) -> Self {
        Self { emitter }
    }
    async fn execute(
        &self,
        request: PluginExecutionGateRequest,
    ) -> Result<PluginExecutionGateResponse, PluginExecutionGateClientError> {
        request.validate()?;
        let value = serde_json::to_value(request)
            .map_err(|_| PluginExecutionGateClientError::InvalidResponse)?;
        let value = self
            .emitter
            .callback(MultiplexHostService::PluginExecutionGateV1, value)
            .await?;
        let envelope: Envelope = serde_json::from_value(value)
            .map_err(|_| PluginExecutionGateClientError::InvalidResponse)?;
        match (envelope.result, envelope.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error.into()),
            _ => Err(PluginExecutionGateClientError::InvalidResponse),
        }
    }
    /// Await within the enclosing call. To cancel a queued acquire, end that call;
    /// independently dropping this future does not send a callback-cancel message.
    pub async fn acquire(
        &self,
        key: String,
    ) -> Result<PluginExecutionGatePermit, PluginExecutionGateClientError> {
        match self
            .execute(PluginExecutionGateRequest::Acquire { key: key.clone() })
            .await?
        {
            PluginExecutionGateResponse::Acquire { token } if !token.is_empty() => {
                Ok(PluginExecutionGatePermit {
                    client: self.clone(),
                    key,
                    token,
                })
            }
            _ => Err(PluginExecutionGateClientError::InvalidResponse),
        }
    }
}
impl PluginExecutionGatePermit {
    pub async fn release(self) -> Result<(), PluginExecutionGateClientError> {
        match self
            .client
            .execute(PluginExecutionGateRequest::Release {
                key: self.key,
                token: self.token,
            })
            .await?
        {
            PluginExecutionGateResponse::Release => Ok(()),
            _ => Err(PluginExecutionGateClientError::InvalidResponse),
        }
    }
}
