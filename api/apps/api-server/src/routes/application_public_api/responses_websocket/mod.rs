//! OpenAI-compatible Responses WebSocket handshake and client-frame contract.
//!
//! Clients connect with `GET /v1/responses`, an application API credential, and
//! `openai-beta: responses_websockets=2026-02-06`. After the upgrade they send
//! text JSON envelopes shaped as `{ "type": "response.create", "response": { ... } }`.
//! Connection ownership and run lifecycle are assembled here. Canonical
//! server-event projection is supplied by the following delivery packet.

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

mod actor;
mod auth;
pub(crate) mod schema;
#[cfg(test)]
mod tests;
mod turn_bridge;

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
        actor::run_connection(socket, state, Arc::new(authorization)).await;
    }))
}
