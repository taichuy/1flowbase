use control_plane::{application_public_api::native::NativeRunResult, ports::RuntimeEventEnvelope};
use serde_json::json;
use thiserror::Error;

use crate::routes::application_public_api::sse::{
    is_public_terminal_runtime_event, native_sse_payload_for_runtime_event, IncludeWorkflowEvents,
};

#[derive(Debug, Error)]
pub(crate) enum NativeWebSocketProjectionError {
    #[error("Native WebSocket event serialization failed")]
    SerializationFailed,
}

pub(crate) struct NativeWebSocketProjector {
    request_id: String,
    visibility: IncludeWorkflowEvents,
    terminal: bool,
}

impl NativeWebSocketProjector {
    pub(crate) fn new(request_id: String, visibility: IncludeWorkflowEvents) -> Self {
        Self {
            request_id,
            visibility,
            terminal: false,
        }
    }

    pub(crate) fn project(
        &mut self,
        run: &NativeRunResult,
        envelope: RuntimeEventEnvelope,
    ) -> Result<Option<String>, NativeWebSocketProjectionError> {
        if self.terminal {
            return Ok(None);
        }
        let terminal = is_public_terminal_runtime_event(&envelope.event_type);
        let event_id = envelope.event_id.clone();
        let sequence = envelope.sequence;
        let Some((event_type, payload)) =
            native_sse_payload_for_runtime_event(run, self.visibility, envelope)
        else {
            return Ok(None);
        };
        self.terminal = terminal;
        serde_json::to_string(&json!({
            "type": event_type,
            "request_id": self.request_id,
            "event_id": event_id,
            "sequence": sequence,
            "run_id": run.id,
            "data": payload,
        }))
        .map(Some)
        .map_err(|_| NativeWebSocketProjectionError::SerializationFailed)
    }

    pub(crate) fn has_terminal(&self) -> bool {
        self.terminal
    }
}
