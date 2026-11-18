use axum::http::HeaderMap;

use crate::{error_response::ApiError, middleware::require_session::RequestContext};

pub fn require_csrf(headers: &HeaderMap, context: &RequestContext) -> Result<(), ApiError> {
    if !context.csrf_required() {
        return Ok(());
    }
    let session = context.cookie_session()?;
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;

    if csrf == session.csrf_token {
        Ok(())
    } else {
        Err(control_plane::errors::ControlPlaneError::PermissionDenied("csrf_mismatch").into())
    }
}
