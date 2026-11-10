//! Finite synchronous public-protocol SSE projection.
//! Ingress, invocation, persistence, transport and completion remain in api-server.
use control_plane::application_public_api::compat::openai::response_id_from_run_id;
use control_plane::application_public_api::native::NativeRunResult;
use control_plane::orchestration_runtime::debug_stream_events;
use control_plane::ports::RuntimeEventEnvelope;
use serde_json::Value;

pub mod anthropic_error;
pub mod llm_tool_visibility;
pub mod protocol_mappers;
mod responses_projection;
pub mod usage;

pub use responses_projection::{ResponsesProjectionMode, ResponsesProjectionSelector};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompatibleAnswerDeltaKind {
    Reasoning,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompatibleTerminalKind {
    Finished,
    Incomplete,
    Failed,
    Cancelled,
    WaitingHuman,
    WaitingCallback,
}

pub struct CompatibleRuntimeEventView {
    envelope: RuntimeEventEnvelope,
    answer_delta: Option<CompatibleAnswerDeltaKind>,
    terminal: Option<CompatibleTerminalKind>,
}

impl CompatibleRuntimeEventView {
    pub fn answer_delta(&self) -> Option<CompatibleAnswerDeltaKind> {
        self.answer_delta
    }

    pub fn terminal(&self) -> Option<CompatibleTerminalKind> {
        self.terminal
    }

    pub fn envelope(&self) -> &RuntimeEventEnvelope {
        &self.envelope
    }

    pub fn into_envelope(self) -> RuntimeEventEnvelope {
        self.envelope
    }
}

impl From<RuntimeEventEnvelope> for CompatibleRuntimeEventView {
    fn from(envelope: RuntimeEventEnvelope) -> Self {
        let answer_delta = is_answer_presentation_delta(&envelope)
            .then_some(match envelope.event_type.as_str() {
                "reasoning_delta" => Some(CompatibleAnswerDeltaKind::Reasoning),
                "text_delta" => Some(CompatibleAnswerDeltaKind::Text),
                _ => None,
            })
            .flatten();
        let terminal = match envelope.event_type.as_str() {
            "flow_finished" => Some(CompatibleTerminalKind::Finished),
            "flow_incomplete" => Some(CompatibleTerminalKind::Incomplete),
            "flow_failed" => Some(CompatibleTerminalKind::Failed),
            "flow_cancelled" => Some(CompatibleTerminalKind::Cancelled),
            "waiting_human" => Some(CompatibleTerminalKind::WaitingHuman),
            "waiting_callback" => Some(CompatibleTerminalKind::WaitingCallback),
            _ => None,
        };
        Self {
            envelope,
            answer_delta,
            terminal,
        }
    }
}

pub fn is_answer_presentation_delta(envelope: &RuntimeEventEnvelope) -> bool {
    matches!(
        envelope.event_type.as_str(),
        "reasoning_delta" | "text_delta"
    ) && debug_stream_events::is_answer_presentation_delta_payload(&envelope.payload)
}

pub fn native_response_id(run: &NativeRunResult) -> String {
    run.metadata
        .get("response_round_id")
        .and_then(Value::as_str)
        .map(|round| format!("resp_{round}"))
        .unwrap_or_else(|| response_id_from_run_id(run.id))
}
