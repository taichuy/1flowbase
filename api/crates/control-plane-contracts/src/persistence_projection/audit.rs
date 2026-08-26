use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn audit_row_hash(
    prev_hash: Option<&str>,
    fact_table: &str,
    fact_id: Uuid,
    payload: &serde_json::Value,
) -> String {
    let mut hasher = Sha256::new();
    if let Some(prev) = prev_hash {
        hasher.update(prev.as_bytes());
    }
    hasher.update(fact_table.as_bytes());
    hasher.update(fact_id.as_bytes());
    hasher.update(serde_json::to_vec(payload).unwrap_or_default());
    format!("sha256:{:x}", hasher.finalize())
}
