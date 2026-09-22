use control_plane::client_trajectory::ClientTrajectoryRecorder;
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
        recorder: Option<ClientTrajectoryRecorder>,
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
            self.authorization.transport_scope.id().to_string(),
            recorder.clone(),
        )
        .await
        {
            Ok(prepared) => prepared,
            Err(error) => {
                let response = error.into_response();
                let status = response.status().as_u16();
                let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .map_err(|_| ResponsesTurnBridgeError::IngressRejected)?;
                let mut event: Value = serde_json::from_slice(&bytes)
                    .map_err(|_| ResponsesTurnBridgeError::IngressRejected)?;
                event["type"] = Value::String("error".into());
                event["status"] = Value::from(status);
                frames
                    .send(event.to_string())
                    .await
                    .map_err(|_| ResponsesTurnBridgeError::SocketWriterClosed)?;
                return Err(ResponsesTurnBridgeError::IngressRejected);
            }
        };
        let (model, previous_response_id, runtime, projection_mode) = prepared.into_parts();
        let (events, completion) = runtime.into_parts();
        project_observed_turn(
            events,
            completion,
            ResponsesWebSocketProjector::with_mode(model, previous_response_id, projection_mode),
            frames,
            recorder,
        )
        .await
    }
}

#[cfg(test)]
pub(super) async fn project_turn(
    events: mpsc::Receiver<CompatibilityStreamEvent>,
    completion: InterfaceStreamCompletion<
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
    >,
    projector: ResponsesWebSocketProjector,
    frames: mpsc::Sender<String>,
) -> Result<(), ResponsesTurnBridgeError> {
    project_observed_turn(events, completion, projector, frames, None).await
}

async fn project_observed_turn(
    mut events: mpsc::Receiver<CompatibilityStreamEvent>,
    completion: InterfaceStreamCompletion<
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
    >,
    mut projector: ResponsesWebSocketProjector,
    frames: mpsc::Sender<String>,
    recorder: Option<ClientTrajectoryRecorder>,
) -> Result<(), ResponsesTurnBridgeError> {
    // The socket actor may abort this task or projection/writing may fail.
    // Detaching this handle preserves the sole Kernel finalization owner;
    // transport shutdown must not drop hooks/receipt or cancel the business run.
    let completion = tokio::spawn(completion.complete());
    while let Some(input) = events.recv().await {
        let (run_snapshot, envelope, mut delivery) = input.into_parts();
        if let Some(recorder) = recorder.as_ref() {
            recorder.bind_run(run_snapshot.id, None);
            if envelope.source == control_plane::ports::RuntimeEventSource::Provider {
                if let Some(node_run_id) = envelope.node_run_id {
                    recorder.link_llm_node(run_snapshot.id, node_run_id);
                }
            }
        }
        // A projection failure drops the receipt before any write: released.
        let projected = projector
            .project(&run_snapshot, envelope)
            .map_err(|_| ResponsesTurnBridgeError::ProjectionFailed)?;
        if let Some(delivery) = delivery.as_mut() {
            delivery.begin_write();
        }
        for frame in projected {
            // A closed writer drops the receipt after the write started: uncertain.
            frames
                .send(frame)
                .await
                .map_err(|_| ResponsesTurnBridgeError::SocketWriterClosed)?;
        }
        if let Some(delivery) = delivery.take() {
            delivery.projected();
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
