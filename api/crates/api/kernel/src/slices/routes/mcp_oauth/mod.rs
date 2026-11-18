use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::mcp_oauth::{self as oauth, OAuthError};
use serde_json::json;
#[derive(Debug)]
pub struct ProtocolError(pub OAuthError);

impl From<OAuthError> for ProtocolError {
    fn from(e: OAuthError) -> Self {
        Self(e)
    }
}

impl IntoResponse for ProtocolError {
    fn into_response(self) -> Response {
        let status = match self.0.error {
            "server_error" | "temporarily_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
            "slow_down" => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::BAD_REQUEST,
        };
        no_store(
            (
                status,
                Json(json!({"error":self.0.error,"error_description":self.0.description})),
            )
                .into_response(),
        )
    }
}

pub fn no_store(mut r: Response) -> Response {
    r.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    r.headers_mut()
        .insert(header::PRAGMA, header::HeaderValue::from_static("no-cache"));
    r
}

#[derive(Debug, thiserror::Error)]
#[error("MCP authentication required")]
pub struct McpAuthenticationRequired {
    pub metadata_url: String,
}

pub(crate) fn authentication_challenge(error: &McpAuthenticationRequired) -> Response {
    let mut response=(StatusCode::UNAUTHORIZED,Json(json!({"error":"invalid_token","error_description":"A valid MCP credential is required."}))).into_response();
    if let Ok(value) = format!(
        "Bearer resource_metadata=\"{}\", scope=\"{}\"",
        error.metadata_url,
        oauth::SCOPE
    )
    .parse()
    {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, value);
    }
    no_store(response)
}

pub mod origin;
pub use origin::*;
