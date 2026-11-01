use axum::{
    http::{HeaderValue, StatusCode},
    response::Response,
};

/// Responses usage comes from provider-call input/output accounting, including any
/// historical reasoning actually submitted to that provider. Codex otherwise adds
/// its own encrypted-history estimate on top of those already counted inputs.
/// The WebSocket contract belongs on the upgrade, before frames can be sampled.
pub(crate) fn with_usage_accounting(mut response: Response) -> Response {
    if response.status().is_success() || response.status() == StatusCode::SWITCHING_PROTOCOLS {
        response
            .headers_mut()
            .insert("x-reasoning-included", HeaderValue::from_static("true"));
    }
    response
}

#[cfg(test)]
#[path = "_tests/responses_headers.rs"]
mod tests;
