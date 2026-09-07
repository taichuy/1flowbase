pub mod anthropic;
pub(crate) mod callback_adapter;
pub mod compat_sse;
pub(crate) mod compatibility_interface;
pub mod ex;
pub(crate) mod llm_tool_visibility;
pub mod native;
pub(crate) mod native_interface;
pub(crate) mod native_read_interface;
pub(crate) mod native_websocket;
pub mod openai;
pub(crate) mod responses_websocket;
pub mod sse;
pub(crate) mod stream_terminal_fallback;
pub(crate) mod tool_callback_ids;
pub(crate) mod workflow_extension_interface;

use std::sync::Arc;

use axum::Router;

use crate::app_state::ApiState;

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub(crate) fn route_assembly(
) -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
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
}

pub fn compatible_router() -> Router<Arc<ApiState>> {
    compatible_route_assembly().into_router()
}

pub(crate) fn compatible_route_assembly(
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
}
