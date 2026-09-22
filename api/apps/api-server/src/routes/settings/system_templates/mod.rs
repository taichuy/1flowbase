use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};
use axum::{
    extract::{DefaultBodyLimit, State},
    handler::Handler,
    http::HeaderMap,
    Json,
};
use control_plane::portable_template::{PortableTemplatePackage, PortableTemplateSelection};
use serde_json::Value;
use std::sync::Arc;
pub(crate) mod interface;
pub(crate) mod plugins;
fn owned(operation: &str) -> access_control::ConsoleRouteOwnership {
    access_control::ConsoleRouteOwnership::ConsoleOperation(operation.to_owned())
}
pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/settings/system-templates/catalog",
            console_get(catalog, owned("system_templates.catalog")),
        )
        .route(
            "/settings/system-templates/export",
            console_post(export, owned("system_templates.export")),
        )
        .route(
            "/settings/system-templates/preview",
            console_post(
                preview.layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
                owned("system_templates.preview"),
            ),
        )
        .route(
            "/settings/system-templates/install",
            console_post(
                install.layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
                owned("system_templates.install"),
            ),
        )
}

#[utoipa::path(get, path = "/api/console/settings/system-templates/catalog", responses((status = 200, body = Object)))]
pub async fn catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let snapshot = Arc::clone(&state);
    let credential =
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers };
    let output: interface::TemplateOutput = crate::routes::console_interface::invoke(
        snapshot,
        "http.console.settings.system-templates.catalog.v1",
        credential,
        interface::TemplateInput::Catalog,
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[utoipa::path(post, request_body = Object, path = "/api/console/settings/system-templates/export", responses((status = 200, body = Object)))]
pub async fn export(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PortableTemplateSelection>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let snapshot = Arc::clone(&state);
    let credential =
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers };
    let output: interface::TemplateOutput = crate::routes::console_interface::invoke(
        snapshot,
        "http.console.settings.system-templates.export.v1",
        credential,
        interface::TemplateInput::Export(body),
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[utoipa::path(post, request_body = Object, path = "/api/console/settings/system-templates/preview", responses((status = 200, body = Object)))]
pub async fn preview(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PortableTemplatePackage>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let snapshot = Arc::clone(&state);
    let credential =
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers };
    let output: interface::TemplateOutput = crate::routes::console_interface::invoke(
        snapshot,
        "http.console.settings.system-templates.preview.v1",
        credential,
        interface::TemplateInput::Preview(body),
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[utoipa::path(post, request_body = Object, path = "/api/console/settings/system-templates/install", responses((status = 200, body = Object)))]
pub async fn install(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PortableTemplatePackage>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let snapshot = Arc::clone(&state);
    let credential =
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers };
    let output: interface::TemplateOutput = crate::routes::console_interface::invoke(
        snapshot,
        "http.console.settings.system-templates.install.v1",
        credential,
        interface::TemplateInput::Install(body),
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}
