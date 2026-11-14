use axum::http::StatusCode;
use control_plane::application_public_api::native::NativeError;
use serde_json::{json, Value};

/// Project Native upstream facts back to the Anthropic protocol without host diagnostics.
pub(crate) fn project(
    runtime: Option<&NativeError>,
    status: StatusCode,
    fallback_code: &str,
    fallback_message: &str,
) -> Value {
    let upstream = runtime.and_then(NativeError::upstream_error);
    let is_upstream =
        upstream.is_some() || runtime.is_some_and(|error| error.code == "provider_upstream_error");
    let error_type = upstream
        .and_then(|error| error.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            if is_upstream || fallback_code == "api_error" {
                status_error_type(status)
            } else {
                fallback_code
            }
        });
    let message = upstream
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .unwrap_or(fallback_message);
    let mut body = json!({"type": "error", "error": {"type": error_type, "message": message}});
    if let Some(request_id) = upstream
        .and_then(|error| error.get("request_id"))
        .and_then(Value::as_str)
    {
        body["request_id"] = json!(request_id);
    }
    body
}

pub(crate) fn runtime_status(runtime: Option<&NativeError>) -> StatusCode {
    runtime
        .and_then(|error| {
            error
                .details
                .get("status_code")
                .or_else(|| error.details.get("status"))
        })
        .and_then(Value::as_u64)
        .and_then(|status| u16::try_from(status).ok())
        .and_then(|status| StatusCode::from_u16(status).ok())
        .filter(|status| status.is_client_error() || status.is_server_error())
        .unwrap_or_else(|| {
            if runtime.is_some_and(|error| error.code == "rate_limited") {
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })
}

fn status_error_type(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 | 422 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        413 => "request_too_large",
        429 => "rate_limit_error",
        529 => "overloaded_error",
        _ => "api_error",
    }
}

#[cfg(test)]
#[path = "_tests/error_projection.rs"]
mod tests;
