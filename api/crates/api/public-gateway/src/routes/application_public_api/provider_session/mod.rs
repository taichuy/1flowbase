//! Authenticated protocol ingress owns provider session identity, outside wire metadata.
use axum::http::{HeaderMap, StatusCode};
use control_plane::application_public_api::native::NativeRunRequest;
use interface_runtime::ApplicationPrincipal;
use sha2::{Digest, Sha256};

use super::native::NativeApiError;

pub(super) fn session_header(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<String>, NativeApiError> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .map(|value| value.to_str())
        .transpose()
        .map_err(|_| {
            NativeApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "session header must be valid text",
            )
        })?;
    if values.next().is_some() || value.is_some_and(|value| value.is_empty()) {
        return Err(NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "session header must appear once and contain non-empty text",
        ));
    }
    Ok(value.map(ToOwned::to_owned))
}

pub(super) fn bind_chat(
    request: &mut NativeRunRequest,
    principal: &ApplicationPrincipal,
    headers: &HeaderMap,
) -> Result<(), NativeApiError> {
    let session = session_header(headers, "session-id")?;
    let thread = session_header(headers, "thread-id")?;
    let parts = if session.is_some() || thread.is_some() {
        Some((
            "headers",
            vec![session.unwrap_or_default(), thread.unwrap_or_default()],
        ))
    } else {
        conversation_identity(request).map(|id| ("conversation", vec![id.to_owned()]))
    };
    request.metadata.set_provider_session_identity(
        parts.map(|(source, parts)| seal(principal, "openai_chat", source, &parts)),
    );
    Ok(())
}

pub(super) fn bind_messages(request: &mut NativeRunRequest, principal: &ApplicationPrincipal) {
    let identity = conversation_identity(request).map(|id| {
        seal(
            principal,
            "anthropic_messages",
            "conversation",
            &[id.to_owned()],
        )
    });
    request.metadata.set_provider_session_identity(identity);
}

fn conversation_identity(request: &NativeRunRequest) -> Option<&str> {
    request
        .conversation
        .get("id")
        .or_else(|| request.conversation.get("conversation_id"))
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())
}

fn seal(
    principal: &ApplicationPrincipal,
    protocol: &str,
    source: &str,
    parts: &[String],
) -> String {
    let bytes = serde_json::to_vec(&(
        principal.application_id(),
        principal.api_key_id(),
        principal.workspace_id(),
        protocol,
        source,
        parts,
    ))
    .expect("session identity serializes");
    format!("{:x}", base16ct::HexDisplay(&Sha256::digest(bytes)))
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
