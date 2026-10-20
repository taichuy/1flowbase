//! Host-only proof cache: never appears in URLs, worker input or managed hooks.
use super::*;
use crate::routes::external_auth::transaction;
use serde::{Deserialize, Serialize};
const TTL: i64 = 300;
#[derive(Serialize, Deserialize)]
pub(crate) struct PasswordSetupProof {
    pub actor_id: Uuid,
    pub session_hash: String,
    pub identity_id: Uuid,
    pub login_entry_id: Uuid,
    pub provider_fingerprint: String,
    pub authority_namespace: String,
    pub entry_fingerprint: String,
    pub connection_fingerprint: String,
    pub nonce: String,
    pub expires_at: i64,
}
fn key(session_hash: &str) -> String {
    format!("authentication:password-setup:{session_hash}")
}
pub(crate) async fn issue(state: &ApiState, proof: PasswordSetupProof) -> Result<(), ApiError> {
    state
        .infrastructure
        .cache_store()
        .set_json(
            &key(&proof.session_hash),
            serde_json::to_value(&proof)?,
            Some(time::Duration::seconds(TTL)),
        )
        .await?;
    Ok(())
}
pub(crate) fn expires_at() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp() + TTL
}
pub(super) async fn read(
    state: &ApiState,
    actor_id: Uuid,
    session_id: &str,
) -> Result<Option<PasswordSetupProof>, ApiError> {
    let session_hash = transaction::digest(session_id);
    let Some(value) = state
        .infrastructure
        .cache_store()
        .get_json(&key(&session_hash))
        .await?
    else {
        return Ok(None);
    };
    let proof: PasswordSetupProof = serde_json::from_value(value)?;
    if proof.actor_id != actor_id
        || proof.session_hash != session_hash
        || proof.expires_at <= time::OffsetDateTime::now_utc().unix_timestamp()
    {
        return Ok(None);
    }
    if state
        .infrastructure
        .cache_store()
        .get_json(&format!(
            "authentication:password-setup:claimed:{}",
            proof.nonce
        ))
        .await?
        .is_some()
    {
        return Ok(None);
    }
    Ok(Some(proof))
}
pub(super) async fn claim(
    state: &ApiState,
    actor_id: Uuid,
    session_id: &str,
) -> Result<PasswordSetupProof, ApiError> {
    let proof = read(state, actor_id, session_id)
        .await?
        .ok_or_else(|| Error::PermissionDenied("password_setup_verification_required"))?;
    let claimed = state
        .infrastructure
        .cache_store()
        .set_if_absent_json(
            &format!("authentication:password-setup:claimed:{}", proof.nonce),
            serde_json::json!(true),
            Some(time::Duration::seconds(TTL * 2)),
        )
        .await?;
    if !claimed {
        return Err(Error::PermissionDenied("password_setup_verification_required").into());
    }
    // Keep the consumed record until TTL: a concurrent new verification must not be deleted.
    Ok(proof)
}
#[cfg(test)]
#[path = "_tests/proof.rs"]
mod tests;
