use utoipa::OpenApi;

#[derive(OpenApi)]
struct CategoryMetadata;

pub(super) struct ApplicationOpenApi;

impl OpenApi for ApplicationOpenApi {
    fn openapi() -> utoipa::openapi::OpenApi {
        let mut paths = Vec::new();
        paths.extend(api_public_gateway::openapi::application::paths_fragments());
        paths.extend(api_applications::openapi::application::paths_fragments());
        paths.extend(api_console_settings::openapi::application::paths_fragments());
        let mut schemas = Vec::new();
        schemas.extend(api_public_gateway::openapi::application::schemas_fragments());
        schemas.extend(api_applications::openapi::application::schemas_fragments());
        schemas.extend(api_console_settings::openapi::application::schemas_fragments());
        let mut document = super::compose_category(CategoryMetadata::openapi(), paths, schemas);
        api_applications::openapi::application::apply_modifier(&mut document);
        document
    }
}
