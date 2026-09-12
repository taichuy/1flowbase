//! Durable log identities are separate from model conversation history.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRunLogContext {
    pub identity_status: String,
    #[serde(default)]
    pub identity_sources: Vec<String>,
    /// Client protocol that declared the identity; written by the mapping layer.
    #[serde(default)]
    pub protocol: Option<String>,
    /// AI Native operation kind (generate / compact / count_tokens) owned by the
    /// Native layer. Grouping counts rely on it, never on `request_kind`.
    #[serde(default)]
    pub call_kind: Option<String>,
    #[serde(default)]
    pub subagent_kind: Option<String>,
    pub forked_from_thread_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub session_id: Option<String>,
    pub parent_thread_id: Option<String>,
    pub parent_turn_id: Option<String>,
    pub root_turn_id: Option<String>,
    pub request_kind: Option<String>,
    pub previous_response_id: Option<String>,
    pub prompt: Option<Value>,
    pub tool_results: Vec<Value>,
}

/// Multiple declarations of the same identity must agree. Missing values do
/// not invent identities from a connection, text, cursor or timestamp.
pub fn resolve_client_log_identity(
    declarations: &[Option<&str>],
) -> Result<Option<String>, &'static str> {
    let mut identity: Option<&str> = None;
    for value in declarations
        .iter()
        .flatten()
        .copied()
        .filter(|v| !v.is_empty())
    {
        if value.len() > 256 || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err("invalid_identity");
        }
        if identity.is_some_and(|prior| prior != value) {
            return Err("conflicting_identity");
        }
        identity = Some(value);
    }
    Ok(identity.map(str::to_owned))
}

#[cfg(test)]
#[path = "_tests/client_log_identity.rs"]
mod tests;
