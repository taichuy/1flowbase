//! OpenAI-compatible Responses WebSocket handshake and client-frame contract.
//!
//! Clients connect with `GET /v1/responses`, an application API credential, and
//! `openai-beta: responses_websockets=2026-02-06`. After the upgrade they send
//! flat text JSON envelopes shaped as `{ "type": "response.create", "model": "...", ... }`.
//! Connection ownership and run lifecycle are assembled here. Canonical
//! server-event projection is supplied by the following delivery packet.

#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use crate::{
    app_state::ApiState,
    routes::application_public_api::{compatibility_interface, openai, openai::OpenAiRouteError},
};
#[cfg(test)]
use axum::{extract::State, http::HeaderMap, response::Response};

pub use api_public_gateway::routes::application_public_api::responses_websocket::*;
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
mod tests {
    use api_public_gateway::routes::application_public_api::responses_websocket::test_support::*;
    mod turn_finalization;
}
