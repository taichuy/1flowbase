use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, HeaderValue},
};
use futures_util::StreamExt;
use serde_json::Value;
use thiserror::Error;

use super::ResponsesWebSocketAuthorization;
use crate::{app_state::ApiState, routes::application_public_api::openai};

#[derive(Debug, Error)]
pub(crate) enum ResponsesTurnBridgeError {
    #[error("authenticated credential cannot be represented as an Authorization header")]
    InvalidAuthenticatedCredential,
    #[error("Responses ingress rejected the turn")]
    IngressRejected,
    #[error("Responses ingress stream failed")]
    IngressStreamFailed,
}

/// Bridges a generated WebSocket turn into the existing OpenAI Responses
/// ingress. That ingress remains responsible for translation to AI Native and
/// creation/execution of the published AgentFlow run.
pub(crate) struct ResponsesTurnBridge {
    state: Arc<ApiState>,
    authorization: Arc<ResponsesWebSocketAuthorization>,
}

impl ResponsesTurnBridge {
    pub(crate) fn new(
        state: Arc<ApiState>,
        authorization: Arc<ResponsesWebSocketAuthorization>,
    ) -> Self {
        Self {
            state,
            authorization,
        }
    }

    pub(crate) async fn execute(&self, response: Value) -> Result<(), ResponsesTurnBridgeError> {
        let mut headers = HeaderMap::new();
        let bearer = HeaderValue::from_str(&format!("Bearer {}", self.authorization.bearer_token))
            .map_err(|_| ResponsesTurnBridgeError::InvalidAuthenticatedCredential)?;
        headers.insert(AUTHORIZATION, bearer);

        // The authenticated actor is retained for the entire socket lifetime.
        // Do not reinterpret any client frame as authentication context.
        let _authenticated_actor = &self.authorization.actor;
        let body = serde_json::to_vec(&response)
            .map(Bytes::from)
            .map_err(|_| ResponsesTurnBridgeError::IngressRejected)?;
        let response = openai::create_response(State(self.state.clone()), headers, body)
            .await
            .map_err(|_| ResponsesTurnBridgeError::IngressRejected)?;

        // Consuming the existing ingress response keeps Active held until the
        // run reaches its terminal stream boundary. WP-08 owns mapping these
        // bytes to WebSocket server events; this packet intentionally does not
        // establish a second canonical projector.
        let mut body = response.into_body().into_data_stream();
        while let Some(chunk) = body.next().await {
            chunk.map_err(|_| ResponsesTurnBridgeError::IngressStreamFailed)?;
        }
        Ok(())
    }
}
