//! Public business facts from committed receive and actual send owners. No runtime envelopes.
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub const COMMUNICATION_RECEIVED_CONTRACT_ID: &str = "system.communication.received";
pub const COMMUNICATION_SEND_RESULT_CONTRACT_ID: &str = "system.communication.send_result";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommunicationFactContent {
    Text {
        message_id: Option<Uuid>,
        content: String,
    },
    File {
        message_id: Uuid,
        file_name: String,
        media_type: String,
        size_bytes: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct CommunicationReceivedFact {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub conversation_id: Uuid,
    pub input_id: Uuid,
    pub provider_message_id: String,
    pub sender_id: String,
    pub received_at_ms: i64,
    pub contents: Vec<CommunicationFactContent>,
    pub workflow_ancestor_ids: Vec<Uuid>,
}

/// Accepted means the provider accepted the submission, never delivered or read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationSendResultKind {
    Accepted,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommunicationSendResultFact {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub send_log_id: Option<Uuid>,
    pub input_id: Option<Uuid>,
    pub turn_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub type_code: Option<String>,
    pub source: Option<domain::NotificationSource>,
    pub result_kind: CommunicationSendResultKind,
    pub failure_code: Option<String>,
    pub target: Option<extension_contracts::NotificationSendTarget>,
    /// Only the existing validated, credential-checked business log projection is used.
    pub content: Option<CommunicationFactContent>,
    pub occurred_at_ms: i64,
    pub workflow_ancestor_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub enum CommunicationLifecycleFact {
    Received(CommunicationReceivedFact),
    SendResult(CommunicationSendResultFact),
}

impl CommunicationLifecycleFact {
    pub fn contract_id(&self) -> &'static str {
        match self {
            Self::Received(_) => COMMUNICATION_RECEIVED_CONTRACT_ID,
            Self::SendResult(_) => COMMUNICATION_SEND_RESULT_CONTRACT_ID,
        }
    }
}

/// Implementations must remain best effort and cannot affect transaction, claim, retry or ACK.
pub trait CommunicationLifecycleObserver: Send + Sync {
    fn observed(&self, fact: CommunicationLifecycleFact);
}

fn text_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["type","message_id","content"],"properties":{
        "type":{"const":"text"},"message_id":{"type":["string","null"],"format":"uuid"},"content":{"type":"string"}}})
}

fn content_schema() -> Value {
    json!({"oneOf":[text_schema(),
        {"type":"object","additionalProperties":false,"required":["type","message_id","file_name","media_type","size_bytes"],"properties":{
            "type":{"const":"file"},"message_id":{"type":"string","format":"uuid"},"file_name":{"type":"string"},"media_type":{"type":"string"},"size_bytes":{"type":["integer","null"]}}}
    ]})
}

impl CommunicationReceivedFact {
    pub fn event_source_schema() -> Value {
        json!({"type":"object","additionalProperties":false,
        "required":["workspace_id","instance_id","conversation_id","input_id","provider_message_id","sender_id","received_at_ms","contents","workflow_ancestor_ids"],
        "properties":{
            "workspace_id":{"type":"string","format":"uuid"},"instance_id":{"type":"string","format":"uuid"},
            "conversation_id":{"type":"string","format":"uuid"},"input_id":{"type":"string","format":"uuid"},
            "provider_message_id":{"type":"string"},"sender_id":{"type":"string"},"received_at_ms":{"type":"integer"},
            "contents":{"type":"array","items":content_schema()},"workflow_ancestor_ids":{"type":"array","items":{"type":"string","format":"uuid"}}
        }})
    }
}

impl CommunicationSendResultFact {
    pub fn event_source_schema() -> Value {
        // Sending currently supports only a validated text projection. File is receive-only.
        json!({"type":"object","additionalProperties":false,
        "required":["workspace_id","instance_id","send_log_id","input_id","turn_id","conversation_id","type_code","source","result_kind","failure_code","target","content","occurred_at_ms","workflow_ancestor_ids"],
        "properties":{
            "workspace_id":{"type":"string","format":"uuid"},"instance_id":{"type":"string","format":"uuid"},
            "send_log_id":{"type":["string","null"],"format":"uuid"},"input_id":{"type":["string","null"],"format":"uuid"},
            "turn_id":{"type":["string","null"],"format":"uuid"},"conversation_id":{"type":["string","null"],"format":"uuid"},
            "type_code":{"type":["string","null"]},"source":{"enum":["api","workflow",null]},
            "result_kind":{"type":"string","enum":["accepted","failed","unknown"]},"failure_code":{"type":["string","null"]},
            "target":{"oneOf":[{"type":"null"},{"type":"object","additionalProperties":false,"required":["sender_id","conversation_id","provider_message_id"],"properties":{
                "sender_id":{"type":"string"},"conversation_id":{"type":"string"},"provider_message_id":{"type":"string"}}}]},
            "content":{"oneOf":[{"type":"null"},text_schema()]},"occurred_at_ms":{"type":"integer"},
            "workflow_ancestor_ids":{"type":"array","items":{"type":"string","format":"uuid"}}
        }})
    }
}

#[cfg(test)]
#[path = "communication_lifecycle/_tests/contract.rs"]
mod tests;
