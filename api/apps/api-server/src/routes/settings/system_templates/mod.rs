use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};
use axum::{
    extract::{DefaultBodyLimit, Query, State},
    handler::Handler,
    http::HeaderMap,
    Json,
};
use catalog::{CatalogQuery, ExportQuery, TemplateRequest};
use control_plane::portable_template::PortableTemplateSelection;
use serde_json::Value;
use std::sync::Arc;
pub(crate) mod archive;
pub(crate) mod catalog;
pub(crate) mod interface;
pub(crate) mod plugins;
pub(crate) mod releases;
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
                preview.layer(DefaultBodyLimit::disable()),
                owned("system_templates.preview"),
            ),
        )
        .route(
            "/settings/system-templates/install",
            console_post(
                install.layer(DefaultBodyLimit::disable()),
                owned("system_templates.install"),
            ),
        )
}

#[utoipa::path(get, path = "/api/console/settings/system-templates/catalog", responses((status = 200, body = Object)))]
pub async fn catalog(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<CatalogQuery>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let snapshot = Arc::clone(&state);
    let credential =
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers };
    let output: interface::TemplateOutput = crate::routes::console_interface::invoke(
        snapshot,
        "http.console.settings.system-templates.catalog.v1",
        credential,
        if query.category.is_some() {
            interface::TemplateInput::Library(query)
        } else {
            interface::TemplateInput::Catalog
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[utoipa::path(post, request_body = Object, path = "/api/console/settings/system-templates/export", responses((status = 200, body = Object)))]
pub async fn export(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ExportQuery>,
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
        match query.format.as_deref() {
            None | Some("json") => interface::TemplateInput::Export(body),
            Some("archive") => interface::TemplateInput::ExportArchive(body),
            _ => {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "application_template_export_format",
                )
                .into())
            }
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[utoipa::path(post, request_body = Object, path = "/api/console/settings/system-templates/preview", responses((status = 200, body = Object)))]
pub async fn preview(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<TemplateRequest>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let snapshot = Arc::clone(&state);
    let credential =
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers };
    let output: interface::TemplateOutput = crate::routes::console_interface::invoke(
        snapshot,
        "http.console.settings.system-templates.preview.v1",
        credential,
        interface::TemplateInput::ResolvePreview(body),
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[utoipa::path(post, request_body = Object, path = "/api/console/settings/system-templates/install", responses((status = 200, body = Object)))]
pub async fn install(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<TemplateRequest>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let snapshot = Arc::clone(&state);
    let credential =
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers };
    let output: interface::TemplateOutput = crate::routes::console_interface::invoke(
        snapshot,
        "http.console.settings.system-templates.install.v1",
        credential,
        interface::TemplateInput::ResolveInstall(body),
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[cfg(test)]
#[path = "_tests/releases.rs"]
mod release_tests;

#[cfg(test)]
#[path = "_tests/archive.rs"]
mod archive_tests;
