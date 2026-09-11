use axum::http::{HeaderMap, StatusCode};

use crate::routes::application_public_api::native::NativeApiError;

pub(super) const RESPONSES_WEBSOCKET_BETA: &str = "responses_websockets=2026-02-06";

pub(super) fn require_responses_websocket_beta(headers: &HeaderMap) -> Result<(), NativeApiError> {
    let enabled = headers
        .get_all("openai-beta")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|value| value == RESPONSES_WEBSOCKET_BETA);

    if enabled {
        return Ok(());
    }

    Err(NativeApiError::new(
        StatusCode::BAD_REQUEST,
        "responses_websocket_beta_required",
        "openai-beta must include responses_websockets=2026-02-06",
    ))
}

/// Freeze only the finite Responses handshake context after authentication.
/// Credentials and hop-by-hop fields must never enter the turn bridge.
pub(super) fn responses_handshake_headers(headers: &HeaderMap) -> HeaderMap {
    let mut captured=HeaderMap::new();
    for name in ["session-id","thread-id","x-codex-turn-state","x-codex-turn-metadata","openai-beta","user-agent","originator"] {
        for value in headers.get_all(name) { captured.append(name,value.clone()); }
    }
    if !captured.contains_key("session-id") {
        captured.insert("session-id",uuid::Uuid::now_v7().to_string().parse().expect("UUID is a header value"));
    }
    captured
}
