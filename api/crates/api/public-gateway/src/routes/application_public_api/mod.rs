pub mod agent_logs;
pub mod anthropic;
#[path = "tool_callbacks/callback_adapter.rs"]
pub mod callback_adapter;
mod client_observer;
pub mod compat_sse;
pub mod compatibility_interface;
pub mod delivery_receipt;
pub mod ex;
#[path = "tool_callbacks/llm_tool_visibility.rs"]
pub mod llm_tool_visibility;
pub mod native;
pub mod native_interface;
pub mod native_read_interface;
pub mod native_websocket;
pub mod openai;
mod provider_session;
pub mod responses_websocket;
pub mod sse;
pub mod stream_terminal_fallback;
#[path = "tool_callbacks/tool_callback_ids.rs"]
pub mod tool_callback_ids;
pub mod workflow_extension_interface;

use std::sync::Arc;

use axum::{extract::DefaultBodyLimit, Router};

use crate::app_state::ApiState;

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
    crate::external_route_assembly::ExternalRouteAssembly::new()
        .route(
            "/models",
            crate::external_route_assembly::get(native::list_native_models),
        )
        .route(
            "/runs",
            crate::external_route_assembly::post(native::create_native_run),
        )
        .route(
            "/runs/websocket",
            crate::external_route_assembly::get(native_websocket::upgrade),
        )
        .route(
            "/runs/:run_id",
            crate::external_route_assembly::get(native::get_native_run),
        )
        .route(
            "/runs/:run_id/cancel",
            crate::external_route_assembly::post(native::cancel_native_run),
        )
        .route(
            "/runs/:run_id/resume",
            crate::external_route_assembly::post(native::resume_native_run),
        )
        .route(
            "/files",
            crate::external_route_assembly::post(native::upload_native_file),
        )
        .default_body_limit(DefaultBodyLimit::disable())
}

pub fn compatible_router() -> Router<Arc<ApiState>> {
    compatible_route_assembly().into_router()
}

pub fn compatible_route_assembly(
) -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
    crate::external_route_assembly::ExternalRouteAssembly::new()
        .route(
            "/models",
            crate::external_route_assembly::get(openai::list_models),
        )
        .route(
            "/chat/completions",
            crate::external_route_assembly::post(openai::create_chat_completion),
        )
        .route(
            "/responses",
            crate::external_route_assembly::post(openai::create_response),
        )
        .route(
            "/v1/models",
            crate::external_route_assembly::get(openai::list_models),
        )
        .route(
            "/v1/chat/completions",
            crate::external_route_assembly::post(openai::create_chat_completion),
        )
        .route(
            "/v1/responses",
            crate::external_route_assembly::get(responses_websocket::upgrade)
                .post(openai::create_response),
        )
        .route(
            "/v1/responses/compact",
            crate::external_route_assembly::post(openai::create_response_compact),
        )
        .route(
            "/v1/chat/completions/models",
            crate::external_route_assembly::get(openai::list_models),
        )
        .route(
            "/v1/messages/count_tokens",
            crate::external_route_assembly::post(anthropic::count_message_tokens),
        )
        .route(
            "/v1/messages",
            crate::external_route_assembly::post(anthropic::create_message),
        )
        .default_body_limit(DefaultBodyLimit::disable())
}
