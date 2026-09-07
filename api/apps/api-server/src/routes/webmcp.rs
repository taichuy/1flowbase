use std::sync::Arc;

use crate::{app_state::ApiState, error_response::ApiError};
use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, HeaderMap},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

mod adapter;
pub(crate) mod interface;
mod protocol;
pub(crate) use adapter::port;

#[derive(Debug, Serialize)]
pub struct WebMcpRegistrationResponse {
    pub instance_id: String,
    pub tools: Vec<WebMcpToolResponse>,
}

#[derive(Debug, Serialize)]
pub struct WebMcpToolResponse {
    pub operation: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub annotations: WebMcpToolAnnotationsResponse,
}

#[derive(Debug, Serialize)]
pub struct WebMcpToolAnnotationsResponse {
    pub read_only_hint: bool,
    pub untrusted_content_hint: bool,
}

#[derive(Debug, Deserialize)]
pub struct InvokeWebMcpToolBody {
    #[serde(default = "empty_arguments")]
    pub arguments: Value,
}

#[derive(Debug, Serialize)]
pub struct InvokeWebMcpToolResponse {
    pub content: Value,
    pub is_error: bool,
}

fn empty_arguments() -> Value {
    json!({})
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub(crate) fn route_assembly(
) -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
    crate::external_route_assembly::ExternalRouteAssembly::new()
        .route(
            "/webmcp/registrations",
            crate::external_route_assembly::get(list_registrations),
        )
        .route(
            "/webmcp/:instance_id/tools/:operation",
            crate::external_route_assembly::post(invoke_tool),
        )
}

async fn list_registrations(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    let credential = crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
        state: Arc::clone(&state),
        headers,
    };
    protocol::invoke(
        state,
        interface::REGISTRATIONS_BINDING,
        credential,
        interface::WebMcpInput::Registrations,
    )
    .await
}

async fn invoke_tool(
    State(state): State<Arc<ApiState>>,
    Path((instance_id, operation)): Path<(String, String)>,
    mut headers: HeaderMap,
    Json(body): Json<InvokeWebMcpToolBody>,
) -> Result<axum::response::Response, ApiError> {
    let credential = crate::extension_bus::ConsoleAuthenticationCredential::CookieSessionWithCsrf {
        state: Arc::clone(&state),
        headers: headers.clone(),
    };
    // The typed invocation never retains the credentials used by the authentication factory.
    headers.remove(AUTHORIZATION);
    headers.remove(axum::http::header::COOKIE);
    headers.remove("x-csrf-token");
    let forward_headers = headers
        .iter()
        .map(|(name, value)| interface::WebMcpForwardHeader {
            name: name.as_str().to_string(),
            value: value.as_bytes().to_vec(),
        })
        .collect();
    protocol::invoke(
        state,
        interface::TOOLS_BINDING,
        credential,
        interface::WebMcpInput::Tool {
            instance_id,
            operation,
            arguments: body.arguments,
            forward_headers,
        },
    )
    .await
}
