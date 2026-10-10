use crate::{MultiplexEmitter, MultiplexError, MultiplexHostService};
use extension_contracts::{
    PluginCredentialError, PluginCredentialRequest, PluginCredentialResponse,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum PluginCredentialClientError {
    #[error(transparent)]
    Transport(#[from] MultiplexError),
    #[error(transparent)]
    Host(#[from] PluginCredentialError),
    #[error("invalid credential host response")]
    InvalidResponse,
}
#[derive(Clone)]
pub struct PluginCredentialClient {
    emitter: MultiplexEmitter,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseEnvelope {
    result: Option<PluginCredentialResponse>,
    error: Option<PluginCredentialError>,
}
impl PluginCredentialClient {
    pub fn new(emitter: MultiplexEmitter) -> Self {
        Self { emitter }
    }
    async fn execute(
        &self,
        request: PluginCredentialRequest,
    ) -> Result<PluginCredentialResponse, PluginCredentialClientError> {
        request.validate()?;
        let request = serde_json::to_value(request)
            .map_err(|_| PluginCredentialClientError::InvalidResponse)?;
        let response = self
            .emitter
            .callback(MultiplexHostService::PluginCredentialV1, request)
            .await?;
        let response: ResponseEnvelope = serde_json::from_value(response)
            .map_err(|_| PluginCredentialClientError::InvalidResponse)?;
        match (response.result, response.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error.into()),
            _ => Err(PluginCredentialClientError::InvalidResponse),
        }
    }
    pub async fn put(
        &self,
        credential_id: String,
        value: Value,
    ) -> Result<(), PluginCredentialClientError> {
        self.execute(PluginCredentialRequest::Put {
            credential_id,
            value,
        })
        .await?;
        Ok(())
    }
    pub async fn get(
        &self,
        credential_id: String,
    ) -> Result<Option<Value>, PluginCredentialClientError> {
        Ok(self
            .execute(PluginCredentialRequest::Get { credential_id })
            .await?
            .value)
    }
    pub async fn delete(&self, credential_id: String) -> Result<(), PluginCredentialClientError> {
        self.execute(PluginCredentialRequest::Delete { credential_id })
            .await?;
        Ok(())
    }
}
