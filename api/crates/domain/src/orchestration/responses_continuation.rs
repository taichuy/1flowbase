use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Constant-size rolling proof of ordered Responses items. No transcript is stored here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsesHistoryProof {
    pub version: u32,
    pub item_count: usize,
    pub digest: String,
}

impl ResponsesHistoryProof {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != 3 {
            return Err("native_history_version_unsupported");
        }
        if self.digest.len() != 64 || !self.digest.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("native_history_evidence_invalid");
        }
        Ok(())
    }
}

/// Host-produced public round evidence. This does not assert native transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsesRoundEvidence {
    pub response_id: String,
    pub history: Value,
    pub output: Vec<Value>,
}

/// Validated Responses suffix, kept in wire order in the atomic callback receipt.
/// Never split this into independent context and tool-result arrays for persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsesContinuation {
    pub ordered_input: Vec<Value>,
    pub ordered_messages: Vec<Value>,
}
