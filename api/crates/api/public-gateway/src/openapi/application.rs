// Original category ordinals retain utoipa path and schema collision precedence.
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::application_public_api::native::create_native_run,
    crate::routes::application_public_api::agent_logs::ingest_events,
    crate::routes::application_public_api::agent_logs::otlp::ingest_traces,
))]
struct Paths29;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::application_public_api::native::list_native_models,
    crate::routes::application_public_api::native::get_native_run,
    crate::routes::application_public_api::native::cancel_native_run,
    crate::routes::application_public_api::native::resume_native_run,
    crate::routes::application_public_api::native::upload_native_file,
    crate::routes::application_public_api::native_websocket::upgrade,
    crate::routes::application_public_api::openai::list_models,
    crate::routes::application_public_api::openai::create_chat_completion,
    crate::routes::application_public_api::openai::create_response,
    crate::routes::application_public_api::openai::create_response_compact,
    crate::routes::application_public_api::anthropic::create_message,
    crate::routes::application_public_api::anthropic::count_message_tokens,
))]
struct Paths45;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::application_public_api::native::NativeErrorBody,
    crate::routes::application_public_api::native::NativeModelCapabilities,
    crate::routes::application_public_api::native::NativeModelListResponse,
    crate::routes::application_public_api::native::NativeModelObject,
    crate::routes::application_public_api::native::NativeModelReasoning,
    crate::routes::application_public_api::native::NativeRunResponse,
    crate::routes::application_public_api::native::ResumeNativeRunBody,
    crate::routes::application_public_api::openai::OpenAiChatCompletionResponse,
    crate::routes::application_public_api::openai::OpenAiChatCompletionChoice,
    crate::routes::application_public_api::openai::OpenAiChatMessage,
    crate::routes::application_public_api::openai::OpenAiErrorBody,
    crate::routes::application_public_api::openai::OpenAiErrorObject,
    crate::routes::application_public_api::openai::OpenAiModelListResponse,
    crate::routes::application_public_api::openai::OpenAiModelObject,
    crate::routes::application_public_api::openai::OpenAiResponsesObject,
    crate::routes::application_public_api::openai::OpenAiResponsesUsage,
    crate::routes::application_public_api::openai::OpenAiToolCall,
    crate::routes::application_public_api::openai::OpenAiToolCallFunction,
    crate::routes::application_public_api::openai::OpenAiUsage,
    crate::routes::application_public_api::anthropic::AnthropicMessageResponse,
    crate::routes::application_public_api::anthropic::AnthropicCountTokensResponse,
    crate::routes::application_public_api::anthropic::AnthropicErrorBody,
    crate::routes::application_public_api::anthropic::AnthropicErrorObject,
    crate::routes::application_public_api::anthropic::AnthropicUsage,
)))]
struct Schemas45;

#[doc(hidden)]
pub fn paths_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![(29, Paths29::openapi()), (45, Paths45::openapi())]
}

#[doc(hidden)]
pub fn schemas_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![(45, Schemas45::openapi())]
}
