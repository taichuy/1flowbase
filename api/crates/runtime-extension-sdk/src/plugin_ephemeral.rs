use crate::{MultiplexEmitter, MultiplexError, MultiplexHostService};
use extension_contracts::{PluginEphemeralError, PluginEphemeralRequest, PluginEphemeralResponse};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum PluginEphemeralClientError {
    #[error(transparent)]
    Transport(#[from] MultiplexError),
    #[error(transparent)]
    Host(#[from] PluginEphemeralError),
    #[error("invalid ephemeral host response")]
    InvalidResponse,
}
#[derive(Clone)]
pub struct PluginEphemeralClient {
    emitter: MultiplexEmitter,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseEnvelope {
    result: Option<PluginEphemeralResponse>,
    error: Option<PluginEphemeralError>,
}
impl PluginEphemeralClient {
    pub fn new(emitter: MultiplexEmitter) -> Self {
        Self { emitter }
    }
    async fn execute(
        &self,
        request: PluginEphemeralRequest,
    ) -> Result<PluginEphemeralResponse, PluginEphemeralClientError> {
        request.validate()?;
        let request = serde_json::to_value(request)
            .map_err(|_| PluginEphemeralClientError::InvalidResponse)?;
        let response = self
            .emitter
            .callback(MultiplexHostService::PluginEphemeralV1, request)
            .await?;
        let response: ResponseEnvelope = serde_json::from_value(response)
            .map_err(|_| PluginEphemeralClientError::InvalidResponse)?;
        match (response.result, response.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error.into()),
            _ => Err(PluginEphemeralClientError::InvalidResponse),
        }
    }
    pub async fn put(
        &self,
        key: String,
        value: Value,
        ttl_ms: u64,
    ) -> Result<(), PluginEphemeralClientError> {
        self.execute(PluginEphemeralRequest::Put { key, value, ttl_ms })
            .await?;
        Ok(())
    }
    pub async fn get(&self, key: String) -> Result<Option<Value>, PluginEphemeralClientError> {
        Ok(self
            .execute(PluginEphemeralRequest::Get { key })
            .await?
            .value)
    }
    pub async fn delete(&self, key: String) -> Result<(), PluginEphemeralClientError> {
        self.execute(PluginEphemeralRequest::Delete { key }).await?;
        Ok(())
    }
}
