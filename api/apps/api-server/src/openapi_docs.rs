use anyhow::Result;
pub use api_host_kernel::openapi_docs::*;
use utoipa::OpenApi;
pub fn build_default_api_docs_registry() -> Result<ApiDocsRegistry> {
    build_default_api_docs_registry_with_cookie_name(DEFAULT_SESSION_COOKIE_NAME)
}
pub fn build_default_api_docs_registry_with_cookie_name(
    cookie_name: &str,
) -> Result<ApiDocsRegistry> {
    build_api_docs_registry_with_cookie_name(
        serde_json::to_value(crate::openapi::ApiDoc::openapi())?,
        cookie_name,
    )
}
