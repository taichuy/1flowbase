// Original category ordinals retain utoipa path and schema collision precedence.
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::debug_run_stream::RuntimeDebugSseEventResponse,
    crate::routes::debug_run_stream::RuntimeEventStreamEnvelopeResponse,
)))]
struct Schemas42;

#[doc(hidden)]
pub fn paths_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![]
}

#[doc(hidden)]
pub fn schemas_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![(42, Schemas42::openapi())]
}
