use utoipa::OpenApi;

#[derive(OpenApi)]
struct CategoryMetadata;

pub(super) struct ExtensionsOpenApi;

impl OpenApi for ExtensionsOpenApi {
    fn openapi() -> utoipa::openapi::OpenApi {
        let mut paths = Vec::new();
        paths.extend(api_console_settings::openapi::extensions::paths_fragments());
        paths.extend(api_mcp::openapi::extensions::paths_fragments());
        paths.extend(api_plugins_models::openapi::extensions::paths_fragments());
        paths.extend(api_console_interaction::openapi::extensions::paths_fragments());
        let mut schemas = Vec::new();
        schemas.extend(api_console_settings::openapi::extensions::schemas_fragments());
        schemas.extend(api_mcp::openapi::extensions::schemas_fragments());
        schemas.extend(api_plugins_models::openapi::extensions::schemas_fragments());
        schemas.extend(api_console_interaction::openapi::extensions::schemas_fragments());
        super::compose_category(CategoryMetadata::openapi(), paths, schemas)
    }
}
