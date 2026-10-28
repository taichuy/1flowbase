//! Logical AI sessions are independent of the stable external-chat history identity.
use super::{CommunicationProcessingStatus, CommunicationSendStatus};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Constructed by the trusted host, never deserialized from model/tool arguments.
#[derive(Debug, Clone)]
pub struct CommunicationConversationScope {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub external_identity: String,
    pub conversation_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationSession {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub epoch: i64,
    pub active: bool,
    /// Chat-level override. Rotation preserves it; a turn freezes its effective model.
    pub selected_model: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationTurnState {
    Open,
    Sealed,
    Failed,
    Invalidated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationReplyTurn {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub session_id: Uuid,
    pub session_epoch: i64,
    pub anchor_input_id: Uuid,
    pub state: CommunicationTurnState,
    /// Canonical run association. Constituent inputs must not dispatch their own runs.
    pub run_id: Option<Uuid>,
    pub selected_model: Option<String>,
    pub received_watermark: i64,
    pub consumed_watermark: i64,
    pub checkpoint_revision: i64,
    pub processing_status: CommunicationProcessingStatus,
    pub send_status: CommunicationSendStatus,
    pub failure_code: Option<String>,
    pub outbound_message_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationAdmission {
    pub session: CommunicationSession,
    pub turn: CommunicationReplyTurn,
    pub input_id: Uuid,
    pub input_sequence: i64,
}

/// A replay returns the original rotation receipt, not another new session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationSessionRotation {
    pub session: CommunicationSession,
    pub invalidated_session_id: Option<Uuid>,
    pub cancel_run_ids: Vec<Uuid>,
    pub replayed: bool,
}
