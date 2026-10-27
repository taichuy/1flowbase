use serde_json::Value;
use uuid::Uuid;

/// Observe the full uncompacted source before preparing a replacement checkpoint.
#[derive(Debug, Clone)]
pub struct ObserveInternalCompactionInput {
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub session_key: String,
    pub source: Value,
    pub source_hashes: Vec<String>,
    pub prefix_hash: String,
}

/// The last published checkpoint; callers validate its coverage against their view.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InternalCompactionHead {
    pub checkpoint_id: Option<Uuid>,
    pub checkpoint: Option<Value>,
}

/// Publish a durably archived candidate only if its source and previous head still match.
#[derive(Debug, Clone)]
pub struct CommitInternalCompactionInput {
    pub scope_id: Uuid,
    pub application_id: Uuid,
    /// The publishing host run; None is reserved for storage-only callers.
    pub flow_run_id: Option<Uuid>,
    pub session_key: String,
    pub expected_checkpoint_id: Option<Uuid>,
    pub prefix_hash: String,
    pub covered_source_hashes: Vec<String>,
    pub checkpoint: Value,
}
