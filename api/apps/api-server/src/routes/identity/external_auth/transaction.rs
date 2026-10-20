//! Host-only OAuth state. Neither credentials nor state are projected into interface hooks.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use control_plane::ports::CacheStore;
use rand::rngs::SysRng;
use rand_core::{Rng, UnwrapErr};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error_response::ApiError;

pub(super) const TTL_SECONDS: i64 = 600;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct BindingIntent {
    pub actor_id: Uuid,
    pub session_hash: String,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Purpose {
    #[default]
    Bind,
    PasswordSetup,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Transaction {
    pub login_entry_id: Uuid,
    pub connection_id: Uuid,
    pub auth_type: String,
    pub installation_id: Uuid,
    pub contribution_id: String,
    pub provider_fingerprint: String,
    pub entry_fingerprint: String,
    pub connection_fingerprint: String,
    pub callback_url: String,
    pub browser_hash: String,
    pub code_verifier: String,
    pub expires_at_unix: i64,
    pub binding: Option<BindingIntent>,
    #[serde(default)]
    pub purpose: Purpose,
}

pub(crate) fn random_token() -> String {
    let mut bytes = [0; 32];
    UnwrapErr(SysRng).fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn digest(value: &str) -> String {
    control_plane::auth::hash_api_key_token(value)
}

pub(super) fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub(super) fn valid_token(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

pub(super) fn key(state: &str) -> String {
    format!("external-auth:transaction:{}", digest(state))
}

pub(super) fn browser_cookie_name(state: &str) -> String {
    format!("flowbase_oauth_{}", digest(state))
}

pub(super) fn denied() -> ApiError {
    control_plane::errors::ControlPlaneError::PermissionDenied("external_auth_invalid_state").into()
}

pub(super) async fn store(
    cache: &dyn CacheStore,
    state: &str,
    transaction: &Transaction,
) -> Result<(), ApiError> {
    if !cache
        .set_if_absent_json(
            &key(state),
            serde_json::to_value(transaction)?,
            Some(time::Duration::seconds(TTL_SECONDS)),
        )
        .await?
    {
        return Err(denied());
    }
    Ok(())
}

/// Validate the browser before burning the transaction. Atomic claim has a longer
/// TTL than the transaction and is shared by every server instance.
pub(super) async fn claim(
    cache: &dyn CacheStore,
    state: &str,
    browser: &str,
    now_unix: i64,
) -> Result<Transaction, ApiError> {
    if !valid_token(state) || !valid_token(browser) {
        return Err(denied());
    }
    let transaction: Transaction =
        serde_json::from_value(cache.get_json(&key(state)).await?.ok_or_else(denied)?)?;
    if transaction.browser_hash != digest(browser) || transaction.expires_at_unix <= now_unix {
        return Err(denied());
    }
    if !cache
        .set_if_absent_json(
            &format!("{}:claimed", key(state)),
            serde_json::Value::Bool(true),
            Some(time::Duration::seconds(TTL_SECONDS * 2)),
        )
        .await?
    {
        return Err(denied());
    }
    cache.delete(&key(state)).await?;
    Ok(transaction)
}

pub(super) fn binding_matches(
    binding: &BindingIntent,
    session: &domain::SessionRecord,
    actor: &domain::ActorContext,
    now_unix: i64,
) -> bool {
    binding.actor_id == actor.user_id
        && binding.actor_id == session.user_id
        && binding.session_hash == digest(&session.session_id)
        && session.expires_at_unix > now_unix
}

#[cfg(test)]
#[path = "_tests/transaction_tests.rs"]
mod tests;
