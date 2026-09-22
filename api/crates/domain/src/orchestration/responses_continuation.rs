use serde::{Deserialize, Serialize};
use serde_json::Value;

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
