use std::sync::Arc;

use axum::{body::Bytes, response::IntoResponse};
use interface_runtime::InterfaceStreamCompletion;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;

use super::{projector::ResponsesWebSocketProjector, ResponsesWebSocketAuthorization};
use crate::{
    app_state::ApiState,
    routes::application_public_api::{
        compatibility_interface::{
            CompatibilityBlockingOutput, CompatibilityBlockingTargetError, CompatibilityStreamEvent,
        },
        openai,
    },
};

#[derive(Debug, Error)]
pub(crate) enum ResponsesTurnBridgeError {
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
        // The authenticated actor is retained for the entire socket lifetime.
        // Do not reinterpret any client frame as authentication context.
        let body = serde_json::to_vec(&response)
            .map(Bytes::from)
            .map_err(|_| ResponsesTurnBridgeError::IngressRejected)?;
        let prepared = match openai::prepare_typed_response_turn(
            self.state.clone(),
            self.authorization.principal.clone(),
            self.authorization.handshake_headers.clone(),
            body,
        )
        .await {
            Ok(prepared) => prepared,
            Err(error) => {
                let response=error.into_response();
                let status=response.status().as_u16();
                let bytes=axum::body::to_bytes(response.into_body(),usize::MAX).await
                    .map_err(|_|ResponsesTurnBridgeError::IngressRejected)?;
                let mut event:Value=serde_json::from_slice(&bytes)
                    .map_err(|_|ResponsesTurnBridgeError::IngressRejected)?;
                event["type"]=Value::String("error".into());
                event["status"]=Value::from(status);
                frames.send(event.to_string()).await.map_err(|_|ResponsesTurnBridgeError::SocketWriterClosed)?;
                return Err(ResponsesTurnBridgeError::IngressRejected);
            }
        };
        let (model, previous_response_id, runtime) = prepared.into_parts();
        let (events, completion) = runtime.into_parts();
        project_turn(
            events,
            completion,
            ResponsesWebSocketProjector::new(model, previous_response_id),
            frames,
        )
        .await
    }
}

pub(super) async fn project_turn(
    mut events: mpsc::Receiver<CompatibilityStreamEvent>,
    completion: InterfaceStreamCompletion<
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
    >,
    mut projector: ResponsesWebSocketProjector,
    frames: mpsc::Sender<String>,
) -> Result<(), ResponsesTurnBridgeError> {
    // The socket actor may abort this task or projection/writing may fail.
    // Detaching this handle preserves the sole Kernel finalization owner;
    // transport shutdown must not drop hooks/receipt or cancel the business run.
    let completion = tokio::spawn(completion.complete());
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
            let terminal = completion
                .await
                .map_err(|_| ResponsesTurnBridgeError::TypedStreamRejected)?
                .map_err(|_| ResponsesTurnBridgeError::TypedStreamRejected)?;
            let _receipt = terminal.receipt().clone().projected();
            return Ok(());
        }
    }
    Err(ResponsesTurnBridgeError::MissingTerminal)
}
