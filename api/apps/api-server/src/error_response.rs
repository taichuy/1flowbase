use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use control_plane::errors::ControlPlaneError;
use plugin_framework::error::PluginFrameworkError;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug)]
pub struct ApiError(pub anyhow::Error);

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self(value.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match self.0.downcast_ref::<ControlPlaneError>() {
            Some(ControlPlaneError::NotAuthenticated) => {
                (StatusCode::UNAUTHORIZED, "not_authenticated")
            }
            Some(ControlPlaneError::PermissionDenied(reason)) => (StatusCode::FORBIDDEN, *reason),
            Some(ControlPlaneError::NotFound(name)) => (StatusCode::NOT_FOUND, *name),
            Some(ControlPlaneError::Conflict(name)) => (StatusCode::CONFLICT, *name),
            Some(ControlPlaneError::InvalidInput(name)) => (StatusCode::BAD_REQUEST, *name),
            Some(ControlPlaneError::InvalidStateTransition { .. }) => {
                (StatusCode::CONFLICT, "invalid_state_transition")
            }
            Some(ControlPlaneError::UpstreamUnavailable(name)) => (StatusCode::BAD_GATEWAY, *name),
            None => match self.0.downcast_ref::<PluginFrameworkError>() {
                Some(PluginFrameworkError::RuntimeContract { .. }) => {
                    (StatusCode::BAD_GATEWAY, "provider_runtime")
                }
                Some(_) => (StatusCode::BAD_REQUEST, "provider_package"),
                None => {
                    tracing::error!(error = %self.0, "internal api error");
                    (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
                }
            },
        };

        // Always expose the real error message, but sanitize sensitive information.
        // Provider runtime errors are upstream passthrough. Surface the message so
        // callers can debug provider and protocol failures without host rewriting.
        let message = sanitize_error_message(&self.0.to_string());

        (
            status,
            Json(ErrorBody {
                status: status.as_u16(),
                code: code.to_string(),
                message,
            }),
        )
            .into_response()
    }
}

/// Sanitize sensitive information from error messages
fn sanitize_error_message(message: &str) -> String {
    let patterns = [
        (
            r"(api[_-]?key|token|secret|password|authorization)[=:\s]+[^\s]+",
            "$1=<redacted>",
        ),
        (r"Bearer\s+[^\s]+", "Bearer <redacted>"),
        (r"(x-[a-z]+-[a-z]+):\s*[^\s]+", "$1: <redacted>"),
    ];

    let mut sanitized = message.to_string();
    for (pattern, replacement) in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            sanitized = re.replace_all(&sanitized, replacement).to_string();
        }
    }
    sanitized
}
