pub mod anthropic;
pub mod compat_sse;
pub mod ex;
pub(crate) mod llm_tool_visibility;
pub mod native;
pub mod openai;
pub mod sse;
pub(crate) mod stream_terminal_fallback;
pub(crate) mod tool_callback_ids;

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::app_state::ApiState;

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/models", get(native::list_native_models))
        .route("/runs", post(native::create_native_run))
        .route("/runs/:run_id", axum::routing::get(native::get_native_run))
        .route("/runs/:run_id/cancel", post(native::cancel_native_run))
        .route("/runs/:run_id/resume", post(native::resume_native_run))
        .route("/files", post(native::upload_native_file))
}

pub fn compatible_router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/models", get(openai::list_models))
        .route("/chat/completions", post(openai::create_chat_completion))
        .route("/responses", post(openai::create_response))
        .route("/v1/models", get(openai::list_models))
        .route("/v1/chat/completions", post(openai::create_chat_completion))
        .route("/v1/responses", post(openai::create_response))
        .route("/v1/chat/completions/models", get(openai::list_models))
        .route(
            "/v1/messages/count_tokens",
            post(anthropic::count_message_tokens),
        )
        .route("/v1/messages", post(anthropic::create_message))
}
