// Original category ordinals retain utoipa path and schema collision precedence.
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(crate::routes::mcp_management::move_mcp_group,))]
struct Paths11;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::mcp_management::bundles::list_official_bundles,
    crate::routes::mcp_management::bundles::preview_official_bundle,
    crate::routes::mcp_management::bundles::import_official_bundle,
    crate::routes::mcp_management::bundles::export_bundle,
    crate::routes::mcp_management::bundles::export_bundle_defaults,
    crate::routes::mcp_management::bundles::export_instance_bundle,
    crate::routes::mcp_management::bundles::preview_uploaded_bundle,
    crate::routes::mcp_management::bundles::import_uploaded_bundle,
    crate::routes::mcp_management::bundles::switch_library_bundle,
    crate::routes::mcp_management::bundles::delete_library_bundle_release,
    crate::routes::mcp_management::bundles::repair_library_bundle_release,
    crate::routes::mcp_management::get_mcp_client_credential,
    crate::routes::mcp_management::save_mcp_client_credential,
    crate::routes::mcp_management::delete_mcp_client_credential,
))]
struct Paths54;

#[doc(hidden)]
pub fn paths_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![(11, Paths11::openapi()), (54, Paths54::openapi())]
}

#[doc(hidden)]
pub fn schemas_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![]
}
