use std::sync::Arc;

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    http::HeaderMap,
    response::Response,
};
use control_plane::application_public_api::{
    api_keys::{ApplicationApiKeyActor, ApplicationApiKeyService},
    native::NativeRunValidationError,
};

use crate::{
    app_state::ApiState,
    routes::application_public_api::native::{bearer_token, native_error, NativeApiError},
};

mod actor;
mod projector;
pub(crate) mod schema;
#[cfg(test)]
mod tests;
mod turn_bridge;

const NATIVE_WEBSOCKET_PROTOCOL: &str = "1flowbase.native.v1";

pub(crate) struct NativeWebSocketAuthorization {
    pub(crate) bearer_token: String,
    pub(crate) actor: ApplicationApiKeyActor,
}

#[utoipa::path(
    get,
    path = "/api/agent/v1/runs/websocket",
    operation_id = "application_native_runs_websocket",
    summary = "Open an AI Native run WebSocket",
    description = "Upgrades an Application API key authenticated request. Client frames use run.create, run.cancel, run.resume, or run.attach; server frames project the same typed Runtime events as Native SSE.",
    responses(
        (status = 101, description = "WebSocket upgrade"),
        (status = 401, body = crate::routes::application_public_api::native::NativeErrorBody)
    )
)]
pub(crate) async fn upgrade(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    websocket: WebSocketUpgrade,
) -> Result<Response, NativeApiError> {
    let token = bearer_token(&headers)?;
    require_native_websocket_protocol(&headers)?;
    let actor = ApplicationApiKeyService::new(state.store.clone())
        .with_last_used_cache(state.infrastructure.cache_store())
        .authenticate_bearer_token(&token)
        .await
        .map_err(|_| native_error(NativeRunValidationError::NotAuthenticated))?;
    let authorization = Arc::new(NativeWebSocketAuthorization {
        bearer_token: token,
        actor,
    });
    Ok(websocket
        .protocols([NATIVE_WEBSOCKET_PROTOCOL])
        .on_upgrade(move |socket| async move {
            actor::run_connection(socket, state, authorization).await;
        }))
}

fn require_native_websocket_protocol(headers: &HeaderMap) -> Result<(), NativeApiError> {
    let accepted = headers
        .get(axum::http::header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|protocol| protocol == NATIVE_WEBSOCKET_PROTOCOL);
    if accepted {
        return Ok(());
    }
    Err(NativeApiError::new(
        axum::http::StatusCode::BAD_REQUEST,
        "native_websocket_protocol_required",
        "Sec-WebSocket-Protocol must include 1flowbase.native.v1",
    ))
}
