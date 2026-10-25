//! Minimal Host-injected caller identity for opted-in managed services.
use serde::{Deserialize, Serialize};

/// Only the Host constructs this input from its captured caller, never worker payload identity.
/// This projection deliberately excludes sessions and authentication credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedServiceCallContext {
    pub actor_id: String,
    /// Source caller workspace; independent of the system execution mount.
    pub workspace_id: String,
    pub deadline_unix_ms: i64,
}
