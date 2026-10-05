use axum::http::{header::HOST, uri::Authority, HeaderMap};
use control_plane::mcp_oauth::{validate_origin, OAuthError};

/// The ingress preserves Host and overwrites X-Forwarded-Proto. Never use a
/// caller-supplied Origin or X-Forwarded-Host to choose the public origin.
pub(crate) fn request_origin(headers: &HeaderMap) -> Result<String, OAuthError> {
    if headers.get_all(HOST).iter().count() != 1
        || headers.get_all("x-forwarded-proto").iter().count() > 1
    {
        return Err(OAuthError::invalid());
    }
    let host = headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(OAuthError::invalid)?;
    if host.contains(',') {
        return Err(OAuthError::invalid());
    }
    host.parse::<Authority>()
        .map_err(|_| OAuthError::invalid())?;
    let local_http = validate_origin(&format!("http://{host}"), false).is_ok();
    let scheme = match headers.get("x-forwarded-proto") {
        Some(value) => match value.to_str().map_err(|_| OAuthError::invalid())? {
            "https" => "https",
            "http" if local_http => "http",
            _ => return Err(OAuthError::invalid()),
        },
        None if local_http => "http",
        None => "https",
    };
    validate_origin(&format!("{scheme}://{host}"), false).map_err(|_| OAuthError::invalid())
}

#[cfg(test)]
#[path = "_tests/origin.rs"]
mod tests;
