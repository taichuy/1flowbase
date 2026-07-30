use std::sync::Arc;

use axum::{
    body::Bytes,
    http::{header::AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;

use super::{projector::ResponsesWebSocketProjector, ResponsesWebSocketAuthorization};
use crate::{
    app_state::ApiState,
    routes::application_public_api::{compat_sse, openai},
};

#[derive(Debug, Error)]
pub(crate) enum ResponsesTurnBridgeError {
    #[error("authenticated credential cannot be represented as an Authorization header")]
    InvalidAuthenticatedCredential,
    #[error("Responses ingress rejected the turn")]
    IngressRejected,
    #[error("Responses typed runtime stream could not be opened")]
    TypedStreamRejected,
    #[error("Responses typed runtime event could not be projected")]
    ProjectionFailed,
    #[error("Responses WebSocket writer closed before the turn completed")]
    SocketWriterClosed,
    #[error("Responses typed runtime stream ended without one terminal event")]
    MissingTerminal,
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

    pub(crate) async fn execute(
        &self,
        response: Value,
        frames: mpsc::Sender<String>,
    ) -> Result<(), ResponsesTurnBridgeError> {
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
        let prepared = openai::prepare_typed_response_turn(self.state.clone(), headers, body)
            .await
            .map_err(|_| ResponsesTurnBridgeError::IngressRejected)?;
        let (model, previous_response_id, runtime) = prepared.into_parts();
        let stream = compat_sse::start_compatible_typed_turn_stream(self.state.clone(), runtime)
            .await
            .map_err(|_| ResponsesTurnBridgeError::TypedStreamRejected)?;
        let (_initial_run, mut events) = stream.into_parts();
        let mut projector = ResponsesWebSocketProjector::new(model, previous_response_id);
        while let Some(input) = events.recv().await {
            let (run_snapshot, envelope) = input.into_parts();
            for frame in projector
                .project(&run_snapshot, envelope)
                .map_err(|_| ResponsesTurnBridgeError::ProjectionFailed)?
            {
                frames
                    .send(frame)
                    .await
                    .map_err(|_| ResponsesTurnBridgeError::SocketWriterClosed)?;
            }
            if projector.has_terminal() {
                return Ok(());
            }
        }
        Err(ResponsesTurnBridgeError::MissingTerminal)
    }
}
