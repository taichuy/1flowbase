//! OpenAI-compatible Responses WebSocket handshake and client-frame contract.
//!
//! Clients connect with `GET /v1/responses`, an application API credential, and
//! `openai-beta: responses_websockets=2026-02-06`. After the upgrade they send
//! text JSON envelopes shaped as `{ "type": "response.create", "response": { ... } }`.
//! Connection ownership, run lifecycle, and server-event projection are assembled by WP-07.

use std::sync::Arc;

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    http::HeaderMap,
    response::Response,
};
use control_plane::application_public_api::api_keys::ApplicationApiKeyActor;

use crate::{
    app_state::ApiState,
    routes::application_public_api::{openai, openai::OpenAiRouteError},
};

mod auth;
pub(crate) mod schema;
#[cfg(test)]
mod tests;

/// Authentication context handed to the connection owner after HTTP upgrade.
pub(crate) struct ResponsesWebSocketAuthorization {
    pub(crate) bearer_token: String,
    pub(crate) actor: ApplicationApiKeyActor,
}

/// Upgrades an authenticated OpenAI Responses request to WebSocket transport.
///
/// The beta header is intentionally exact and versioned: accepting another value
/// would silently opt clients into a protocol contract this server does not implement.
pub(crate) async fn upgrade(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    websocket: WebSocketUpgrade,
) -> Result<Response, OpenAiRouteError> {
    let credential = openai::openai_credential(&headers)?;
    auth::require_responses_websocket_beta(&headers)?;
    let actor =
        openai::authenticate_openai_response_credential(state.as_ref(), &credential).await?;
    let authorization = ResponsesWebSocketAuthorization {
        bearer_token: credential.token,
        actor,
    };

    Ok(websocket.on_upgrade(move |socket| async move {
        // WP-07 replaces this assembly seam with the connection actor. Keeping the
        // authenticated values inside the upgrade future prevents request extensions
        // or a second credential interpretation from becoming connection truth.
        let ResponsesWebSocketAuthorization {
            bearer_token,
            actor,
        } = authorization;
        drop((socket, bearer_token, actor));
    }))
}
