//! Host-owned session/turn protocol. All stateful operations fence scope, epoch and lease.
use super::CommunicationPendingAttachment;
use domain::*;
use serde_json::Value;
use uuid::Uuid;

/// Bounded restart scan; classification does not need a history pagination round-trip
/// or private provider credentials. File-only inputs still have their own receipt ID.
pub struct CommunicationPendingAdmission {
    pub conversation: CommunicationConversation,
    pub input_id: Uuid,
    pub external_message_key: String,
    pub received_at_ms: i64,
    pub content: Option<String>,
    pub has_attachments: bool,
}

#[derive(Debug, Clone)]
pub struct CommunicationTurnClaim {
    pub scope: CommunicationConversationScope,
    pub session_id: Uuid,
    pub session_epoch: i64,
    pub turn_id: Uuid,
    pub claim_id: Uuid,
}

pub struct CommunicationTurnInput {
    pub input_id: Uuid,
    pub input_sequence: i64,
    pub external_message_key: String,
    pub received_at_ms: i64,
    pub content: Option<String>,
    pub attachments: Vec<CommunicationPendingAttachment>,
}

/// Peeking never acknowledges input. Persist the continuation and acknowledgement together.
pub struct CommunicationTurnInputs {
    pub turn: CommunicationReplyTurn,
    pub checkpoint: Option<Value>,
    pub inputs: Vec<CommunicationTurnInput>,
    /// Received before this snapshot but not classified yet; seal must wait for admission
    /// or explicit exclusion by the host's command lane. IDs are scoped to this chat.
    pub pending_admission_input_ids: Vec<Uuid>,
}

pub struct ClaimedCommunicationTurn {
    pub binding: CommunicationBindingRecord,
    pub conversation: CommunicationConversation,
    pub session: CommunicationSession,
    pub claim: CommunicationTurnClaim,
    pub pending: CommunicationTurnInputs,
    /// Present when reclaiming a deliberately retried, already sealed reply.
    pub final_reply: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommunicationTurnCheckpoint {
    /// CAS also protects checkpoints that add completed work without consuming new input.
    pub expected_checkpoint_revision: i64,
    pub consumed_watermark: i64,
    /// Opaque durable execution continuation, owned and versioned by the runtime.
    pub checkpoint: Value,
}

#[derive(Debug, Clone)]
pub struct CommunicationTurnFinalCandidate {
    pub expected_received_watermark: i64,
    pub expected_checkpoint_revision: i64,
    pub final_reply: String,
}

impl CommunicationTurnFinalCandidate {
    /// Final send eligibility also requires a live claim and no pending classification.
    /// This predicate covers only the durable continuation/watermark CAS contract.
    pub fn matches_checkpoint(&self, turn: &CommunicationReplyTurn) -> bool {
        turn.received_watermark == self.expected_received_watermark
            && turn.consumed_watermark == turn.received_watermark
            && turn.checkpoint_revision == self.expected_checkpoint_revision
            && turn.checkpoint_revision > 0
    }
}

pub enum CommunicationTurnSeal {
    /// This call alone won the send reservation. Repeating it never grants another send.
    Reserved {
        turn: CommunicationReplyTurn,
        final_reply: String,
    },
    Pending(CommunicationTurnInputs),
    Fenced,
}

#[derive(Debug, Clone)]
pub enum CommunicationTurnProgress {
    RunFailed { failure_code: String },
    SendSucceeded,
    SendFailed { failure_code: String },
    SendUnknown { failure_code: String },
}

#[cfg(test)]
#[path = "_tests/communication_sessions.rs"]
mod tests;
