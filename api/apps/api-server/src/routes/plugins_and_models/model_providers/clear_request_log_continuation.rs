use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use control_plane::errors::ControlPlaneError;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::error_response::ApiError;

type HmacSha256 = Hmac<Sha256>;

const TOKEN_PREFIX: &str = "v1";
const TOKEN_PAYLOAD_VERSION: u8 = 1;
const MAX_TOKEN_LENGTH: usize = 4096;
const KEY_DERIVATION_CONTEXT: &[u8] =
    b"1flowbase/api-server/key-derivation/v1\0model-provider-request-log-clear-continuation";
const SIGNING_CONTEXT: &[u8] = b"1flowbase/model-provider-request-log-clear-continuation/v1\0";

#[derive(Debug, Serialize, Deserialize)]
struct ContinuationPayload {
    version: u8,
    workspace_id: Uuid,
    snapshot_created_before: String,
}

pub(super) fn issue(
    master_secret: &str,
    workspace_id: Uuid,
    snapshot_created_before: OffsetDateTime,
) -> Result<String, ApiError> {
    let payload = serde_json::to_vec(&ContinuationPayload {
        version: TOKEN_PAYLOAD_VERSION,
        workspace_id,
        snapshot_created_before: snapshot_created_before.format(&Rfc3339)?,
    })?;
    let signature = signature(master_secret, &payload)?;
    Ok(format!(
        "{TOKEN_PREFIX}.{}.{}",
        URL_SAFE_NO_PAD.encode(payload),
        URL_SAFE_NO_PAD.encode(signature)
    ))
}

pub(super) fn verify(
    master_secret: &str,
    current_workspace_id: Uuid,
    token: &str,
) -> Result<OffsetDateTime, ApiError> {
    if token.len() > MAX_TOKEN_LENGTH {
        return Err(invalid_token());
    }
    let mut segments = token.split('.');
    let prefix = segments.next().ok_or_else(invalid_token)?;
    let payload_segment = segments.next().ok_or_else(invalid_token)?;
    let signature_segment = segments.next().ok_or_else(invalid_token)?;
    if prefix != TOKEN_PREFIX || segments.next().is_some() {
        return Err(invalid_token());
    }

    let payload = URL_SAFE_NO_PAD
        .decode(payload_segment)
        .map_err(|_| invalid_token())?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature_segment)
        .map_err(|_| invalid_token())?;
    verify_signature(master_secret, &payload, &signature)?;

    let payload: ContinuationPayload =
        serde_json::from_slice(&payload).map_err(|_| invalid_token())?;
    if payload.version != TOKEN_PAYLOAD_VERSION {
        return Err(invalid_token());
    }
    if payload.workspace_id != current_workspace_id {
        return Err(ControlPlaneError::PermissionDenied(
            "request_log_clear_continuation_workspace",
        )
        .into());
    }
    OffsetDateTime::parse(&payload.snapshot_created_before, &Rfc3339).map_err(|_| invalid_token())
}

fn signature(master_secret: &str, payload: &[u8]) -> Result<Vec<u8>, ApiError> {
    let key = derive_key(master_secret)?;
    let mut mac = <HmacSha256 as Mac>::new_from_slice(&key)
        .map_err(|_| anyhow::anyhow!("request log continuation signing key is invalid"))?;
    mac.update(SIGNING_CONTEXT);
    mac.update(payload);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn verify_signature(master_secret: &str, payload: &[u8], signature: &[u8]) -> Result<(), ApiError> {
    let key = derive_key(master_secret)?;
    let mut mac = <HmacSha256 as Mac>::new_from_slice(&key)
        .map_err(|_| anyhow::anyhow!("request log continuation signing key is invalid"))?;
    mac.update(SIGNING_CONTEXT);
    mac.update(payload);
    mac.verify_slice(signature).map_err(|_| invalid_token())
}

fn derive_key(master_secret: &str) -> Result<[u8; 32], ApiError> {
    let mut derivation = <HmacSha256 as Mac>::new_from_slice(master_secret.as_bytes())
        .map_err(|_| anyhow::anyhow!("provider master key is invalid"))?;
    derivation.update(KEY_DERIVATION_CONTEXT);
    let derived = derivation.finalize().into_bytes();
    let mut key = [0_u8; 32];
    key.copy_from_slice(&derived);
    Ok(key)
}

fn invalid_token() -> ApiError {
    ControlPlaneError::InvalidInput("continuation_token").into()
}
