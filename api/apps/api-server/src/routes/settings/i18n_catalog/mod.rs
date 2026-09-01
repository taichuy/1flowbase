use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

pub(crate) mod interface;
pub(crate) mod management;

#[derive(Debug, Serialize, ToSchema)]
pub struct I18nCatalogStateResponse {
    pub active_catalog_version: Option<String>,
    pub revision: i64,
    pub source: &'static str,
    pub source_locale: String,
    pub locales: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct I18nCatalogUpdateStatusResponse {
    pub status: &'static str,
    pub active_catalog_version: Option<String>,
    pub latest_catalog_version: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ActivateI18nCatalogBody {
    pub expected_revision: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivateI18nCatalogResponse {
    pub status: &'static str,
    pub catalog_version: String,
    pub revision: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstalledI18nCatalogPreviewResponse {
    pub extension_installation_id: String,
    pub application_status: String,
    pub active_catalog_version: Option<String>,
    pub installed_catalog_version: String,
    pub revision: i64,
    #[schema(value_type = Vec<Object>)]
    pub integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
    #[schema(value_type = Option<Object>)]
    pub required_integrity_override: Option<domain::ExtensionRiskChallenge>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ActivateInstalledI18nCatalogBody {
    pub expected_revision: i64,
    pub integrity_override: Option<crate::routes::plugins::PluginRiskOverrideBody>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::{Authenticated, ConsoleOperation};

    ConsoleRouteAssembly::new()
        .route(
            "/settings/i18n/catalog",
            console_get(
                get_i18n_catalog_state,
                ConsoleOperation("i18n_catalog.state.get".to_string()),
            ),
        )
        .route(
            "/settings/i18n/update-check",
            console_get(
                get_i18n_catalog_update_status,
                ConsoleOperation("i18n_catalog.update.check".to_string()),
            ),
        )
        .route(
            "/settings/i18n/activate",
            console_post(
                activate_i18n_catalog_update,
                ConsoleOperation("i18n_catalog.update.activate".to_string()),
            ),
        )
        .route(
            "/settings/i18n/installed-extension/:installation_id/preview",
            console_get(preview_installed_i18n_catalog, Authenticated),
        )
        .route(
            "/settings/i18n/installed-extension/:installation_id/activate",
            console_post(activate_installed_i18n_catalog, Authenticated),
        )
        .merge(management::route_assembly())
}

pub(super) fn invalid_input(name: &'static str) -> ApiError {
    control_plane::errors::ControlPlaneError::InvalidInput(name).into()
}

async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: interface::I18nCatalogInput,
) -> Result<interface::I18nCatalogOutput, ApiError> {
    crate::routes::console_interface::invoke(
        Arc::clone(&state),
        binding_id,
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        input,
    )
    .await
}

async fn invoke_mutating(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: interface::I18nCatalogInput,
) -> Result<interface::I18nCatalogOutput, ApiError> {
    crate::routes::console_interface::invoke(
        Arc::clone(&state),
        binding_id,
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        input,
    )
    .await
}

#[utoipa::path(
    get,
    path = "/api/console/settings/i18n/catalog",
    summary = "Get i18n catalog state",
    description = "Returns the active root i18n catalog manifest state.",
    responses((status = 200, body = I18nCatalogStateResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_i18n_catalog_state(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<I18nCatalogStateResponse>>, ApiError> {
    let interface::I18nCatalogOutput::State(response) = invoke(
        state,
        headers,
        "http.console.settings.i18n.catalog.get.v1",
        interface::I18nCatalogInput::GetState,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/i18n/update-check",
    summary = "Check i18n catalog update",
    description = "Checks the latest official i18n catalog without activating it.",
    responses((status = 200, body = I18nCatalogUpdateStatusResponse), (status = 403, body = crate::error_response::ErrorBody), (status = 502, body = crate::error_response::ErrorBody))
)]
pub async fn get_i18n_catalog_update_status(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<I18nCatalogUpdateStatusResponse>>, ApiError> {
    let interface::I18nCatalogOutput::UpdateStatus(response) = invoke(
        state,
        headers,
        "http.console.i18n.update-check.get.v1",
        interface::I18nCatalogInput::CheckUpdate,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/i18n/activate",
    summary = "Activate latest i18n catalog",
    description = "Checks and activates the latest official root i18n catalog at the expected revision.",
    request_body = ActivateI18nCatalogBody,
    responses((status = 200, body = ActivateI18nCatalogResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody), (status = 502, body = crate::error_response::ErrorBody))
)]
pub async fn activate_i18n_catalog_update(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ActivateI18nCatalogBody>,
) -> Result<Json<ApiSuccess<ActivateI18nCatalogResponse>>, ApiError> {
    let interface::I18nCatalogOutput::Activation(response) = invoke_mutating(
        state,
        headers,
        "http.console.i18n.activate.post.v1",
        interface::I18nCatalogInput::ActivateOfficial(body),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/i18n/installed-extension/{installation_id}/preview",
    summary = "Preview an installed translation catalog",
    description = "Inspects the exact local extension artifact and compares it with the active root translation catalog without a remote fetch.",
    params(("installation_id" = uuid::Uuid, Path, description = "Extension installation ID")),
    responses((status = 200, body = InstalledI18nCatalogPreviewResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn preview_installed_i18n_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(installation_id): Path<uuid::Uuid>,
) -> Result<Json<ApiSuccess<InstalledI18nCatalogPreviewResponse>>, ApiError> {
    let interface::I18nCatalogOutput::InstalledPreview(response) = invoke(
        state,
        headers,
        "http.console.i18n.installed-extension.preview.get.v1",
        interface::I18nCatalogInput::PreviewInstalled { installation_id },
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/i18n/installed-extension/{installation_id}/activate",
    summary = "Activate an installed translation catalog",
    description = "Activates the exact local extension artifact at the expected catalog revision while preserving custom translations and overrides.",
    params(("installation_id" = uuid::Uuid, Path, description = "Extension installation ID")),
    request_body = ActivateInstalledI18nCatalogBody,
    responses((status = 200, body = ActivateI18nCatalogResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn activate_installed_i18n_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(installation_id): Path<uuid::Uuid>,
    Json(body): Json<ActivateInstalledI18nCatalogBody>,
) -> Result<Json<ApiSuccess<ActivateI18nCatalogResponse>>, ApiError> {
    let interface::I18nCatalogOutput::Activation(response) = invoke_mutating(
        state,
        headers,
        "http.console.i18n.installed-extension.activate.post.v1",
        interface::I18nCatalogInput::ActivateInstalled {
            installation_id,
            body,
        },
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}
