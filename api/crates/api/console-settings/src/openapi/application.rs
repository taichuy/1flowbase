// Original category ordinals retain utoipa path and schema collision precedence.
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(crate::routes::application_management::list_application_management,))]
struct Paths11;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::application_management::ApplicationManagementItemResponse,
    crate::routes::application_management::ApplicationManagementPageResponse,
    crate::routes::application_management::ApplicationManagementTagResponse,
)))]
struct Schemas24;

#[doc(hidden)]
pub fn paths_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![(11, Paths11::openapi())]
}

#[doc(hidden)]
pub fn schemas_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![(24, Schemas24::openapi())]
}
