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
