//! OpenAI-compatible Responses WebSocket handshake and client-frame contract.
//!
//! Clients connect with `GET /v1/responses`, an application API credential, and
//! `openai-beta: responses_websockets=2026-02-06`. After the upgrade they send
//! flat text JSON envelopes shaped as `{ "type": "response.create", "model": "...", ... }`.
//! Connection ownership and run lifecycle are assembled here. Canonical
//! server-event projection is supplied by the following delivery packet.

use std::sync::Arc;

use crate::{
    app_state::ApiState,
    routes::application_public_api::{compatibility_interface, openai, openai::OpenAiRouteError},
};
use axum::{
    extract::{ws::WebSocketUpgrade, State},
    http::HeaderMap,
    response::Response,
};

mod actor;
mod auth;
mod projector;
pub(crate) mod schema;
#[cfg(test)]
mod tests;
mod turn_bridge;

/// Authentication context handed to the connection owner after HTTP upgrade.
pub(crate) struct ResponsesWebSocketAuthorization {
    pub(crate) principal: interface_runtime::ApplicationPrincipal,
    pub(crate) handshake_headers: HeaderMap,
    pub(crate) transport_owner_id: String,
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
    let principal = compatibility_interface::authenticate_application_principal(
        state.clone(),
        compatibility_interface::OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID,
        credential.token,
    )
    .await?
    .into_principal();
    let handshake_headers = auth::responses_handshake_headers(&headers);
    let transport_owner_id = openai::responses_transport_owner_id(&principal, &handshake_headers)?;
    let authorization = ResponsesWebSocketAuthorization {
        principal,
        handshake_headers,
        transport_owner_id,
    };

    Ok(websocket.on_upgrade(move |socket| async move {
        actor::run_connection(socket, state, Arc::new(authorization)).await;
    }))
}
