use utoipa::OpenApi;

#[derive(OpenApi)]
struct CategoryMetadata;

pub(super) struct ConsoleOpenApi;

impl OpenApi for ConsoleOpenApi {
    fn openapi() -> utoipa::openapi::OpenApi {
        let mut paths = Vec::new();
        paths.extend(api_console_settings::openapi::console::paths_fragments());
        paths.extend(api_mcp::openapi::console::paths_fragments());
        paths.extend(api_plugins_models::openapi::console::paths_fragments());
        paths.extend(api_console_interaction::openapi::console::paths_fragments());
        paths.push((82, RootPaths82::openapi()));
        let mut schemas = Vec::new();
        schemas.extend(api_applications::openapi::console::schemas_fragments());
        schemas.extend(api_console_settings::openapi::console::schemas_fragments());
        schemas.extend(api_plugins_models::openapi::console::schemas_fragments());
        schemas.extend(api_console_interaction::openapi::console::schemas_fragments());
        schemas.push((2, RootSchemas2::openapi()));
        let mut document = super::compose_category(CategoryMetadata::openapi(), paths, schemas);
        api_console_interaction::openapi::console::apply_modifier(&mut document);
        document
    }
}

#[derive(OpenApi)]
#[openapi(paths(crate::health, crate::console_health,))]
struct RootPaths82;

#[derive(OpenApi)]
#[openapi(components(schemas(crate::HealthResponse,)))]
struct RootSchemas2;
