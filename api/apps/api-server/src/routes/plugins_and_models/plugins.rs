use std::sync::Arc;

use access_control::ConsoleRouteOwnership::ConsoleOperation;
use axum::{
    body::{to_bytes, Body},
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    handler::Handler,
    http::{HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Json, Router,
};
use control_plane::plugin_management::{
    InstallPluginResult, OfficialPluginCatalogEntry, OfficialPluginCatalogFilter,
    PluginCatalogEntry, PluginCatalogFilter, PluginCompatibilityOverride, PluginFamilyView,
    PluginInstalledVersionView, PluginRiskOverride,
};
use plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::{
        console_route_assembly::{console_delete, console_get, console_post, ConsoleRouteAssembly},
        system::LocaleMetaResponse,
    },
};

const DEFAULT_OFFICIAL_PLUGIN_CATALOG_LIMIT: usize = 20;
const MAX_OFFICIAL_PLUGIN_CATALOG_LIMIT: usize = 50;

#[derive(Debug, Deserialize, ToSchema)]
pub struct InstallPluginBody {
    pub package_root: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InstallOfficialPluginBody {
    pub plugin_id: String,
    pub compatibility_override: Option<PluginCompatibilityOverrideBody>,
    pub risk_override: Option<PluginRiskOverrideBody>,
}

#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct PluginUploadMultipartBody {
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct PluginCompatibilityOverrideBody {
    pub reason: String,
    pub acknowledged_current_host_version: String,
    pub acknowledged_minimum_host_version: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct PluginRiskOverrideBody {
    pub reason: String,
    pub acknowledged_warnings: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpgradeLatestPluginFamilyBody {
    pub compatibility_override: Option<PluginCompatibilityOverrideBody>,
    pub risk_override: Option<PluginRiskOverrideBody>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SwitchPluginVersionBody {
    pub installation_id: String,
}

#[derive(Debug, Deserialize, IntoParams, Clone)]
pub struct PluginCatalogQuery {
    /// Optional plugin kind filter for catalog views.
    pub plugin_type: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams, Clone)]
pub struct OfficialPluginCatalogQuery {
    /// Optional plugin kind filter for official catalog views.
    pub plugin_type: Option<String>,
    pub locale: Option<String>,
    pub q: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(description = "Installation record returned by the plugin API.")]
pub struct PluginInstallationResponse {
    pub id: String,
    pub provider_code: String,
    pub runtime_slot: Option<String>,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contract_version: String,
    pub protocol: String,
    pub display_name: String,
    pub source_kind: String,
    pub trust_level: String,
    pub verification_status: String,
    pub desired_state: String,
    pub expected_checksum: Option<String>,
    pub signature_status: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub local_artifact: Option<PluginArtifactInstanceResponse>,
    #[schema(value_type = Object)]
    pub metadata_json: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PluginArtifactInstanceResponse {
    pub node_id: String,
    pub installation_id: String,
    pub local_version: Option<String>,
    pub local_checksum: Option<String>,
    pub local_path: Option<String>,
    pub package_path: Option<String>,
    pub manifest_fingerprint: Option<String>,
    pub artifact_status: String,
    pub runtime_status: String,
    pub availability_status: String,
    pub checked_at: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Catalog entry returned by the plugin API.")]
pub struct PluginCatalogEntryResponse {
    pub installation: PluginInstallationResponse,
    pub local_artifact: PluginArtifactInstanceResponse,
    pub plugin_type: String,
    pub namespace: String,
    pub label_key: String,
    pub description_key: Option<String>,
    pub provider_label_key: String,
    pub help_url: Option<String>,
    pub default_base_url: Option<String>,
    pub model_discovery_mode: String,
    pub assigned_to_current_workspace: bool,
    pub catalog_refresh_status: String,
    pub catalog_last_error_message: Option<String>,
    pub catalog_refreshed_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PluginCatalogResponse {
    pub locale_meta: LocaleMetaResponse,
    #[schema(value_type = Object)]
    pub i18n_catalog: serde_json::Value,
    pub entries: Vec<PluginCatalogEntryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PluginCatalogProjectionResponse {
    pub installation_id: String,
    pub package_code: String,
    pub package_version: String,
    pub projection_status: String,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OfficialPluginArtifactResponse {
    pub os: String,
    pub arch: String,
    pub libc: Option<String>,
    pub rust_target: String,
    pub download_url: String,
    pub checksum: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Official registry entry returned by the plugin API.")]
pub struct OfficialPluginCatalogEntryResponse {
    pub plugin_id: String,
    pub plugin_type: String,
    pub provider_code: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub protocol: String,
    pub latest_version: String,
    pub minimum_host_version: String,
    pub current_host_version: String,
    pub compatibility_status: String,
    pub compatibility_warning_reason: Option<String>,
    pub selected_artifact: OfficialPluginArtifactResponse,
    pub help_url: Option<String>,
    pub model_discovery_mode: String,
    pub install_status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OfficialPluginCatalogPageResponse {
    pub limit: usize,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OfficialPluginCatalogResponse {
    pub source_kind: String,
    pub source_label: String,
    pub registry_url: String,
    pub source_freshness: String,
    pub locale_meta: LocaleMetaResponse,
    pub page: OfficialPluginCatalogPageResponse,
    pub entries: Vec<OfficialPluginCatalogEntryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PluginInstalledVersionResponse {
    pub installation_id: String,
    pub plugin_version: String,
    pub source_kind: String,
    pub trust_level: String,
    pub desired_state: String,
    pub availability_status: String,
    pub local_artifact: PluginArtifactInstanceResponse,
    pub created_at: String,
    pub is_current: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "Family view for plugin entries in the current registry.")]
pub struct PluginFamilyResponse {
    pub provider_code: String,
    pub plugin_type: String,
    pub namespace: String,
    pub label_key: String,
    pub description_key: Option<String>,
    pub provider_label_key: String,
    pub icon: Option<String>,
    pub protocol: String,
    pub help_url: Option<String>,
    pub default_base_url: Option<String>,
    pub model_discovery_mode: String,
    pub current_installation_id: String,
    pub current_version: String,
    pub installation_status: String,
    pub current_local_artifact: PluginArtifactInstanceResponse,
    pub latest_version: Option<String>,
    pub has_update: bool,
    pub installed_versions: Vec<PluginInstalledVersionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PluginFamilyCatalogResponse {
    pub locale_meta: LocaleMetaResponse,
    #[schema(value_type = Object)]
    pub i18n_catalog: serde_json::Value,
    pub entries: Vec<PluginFamilyResponse>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PluginTaskResponse {
    pub id: String,
    pub installation_id: Option<String>,
    pub workspace_id: Option<String>,
    pub provider_code: String,
    pub task_kind: String,
    pub status: String,
    pub status_message: Option<String>,
    #[schema(value_type = Object)]
    pub detail_json: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InstallPluginResponse {
    pub installation: PluginInstallationResponse,
    pub task: PluginTaskResponse,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    route_assembly_with_plugin_upload_max_bytes(crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES)
}

pub(crate) fn route_assembly_with_plugin_upload_max_bytes(
    plugin_upload_max_bytes: usize,
) -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .merge(extension_center::route_assembly(plugin_upload_max_bytes))
        .route(
            "/plugins/catalog",
            console_get(
                list_catalog,
                ConsoleOperation("plugins.catalog.view".to_string()),
            ),
        )
        .route(
            "/plugins/families",
            console_get(
                list_families,
                ConsoleOperation("plugins.families.view".to_string()),
            ),
        )
        .route(
            "/plugins/families/:provider_code/upgrade-latest",
            console_post(
                upgrade_latest,
                ConsoleOperation("plugins.families.upgrade".to_string()),
            ),
        )
        .route(
            "/plugins/families/:provider_code/switch-version",
            console_post(
                switch_version,
                ConsoleOperation("plugins.families.switch".to_string()),
            ),
        )
        .route(
            "/plugins/families/:provider_code",
            console_delete(
                delete_family,
                ConsoleOperation("plugins.families.delete".to_string()),
            ),
        )
        .route(
            "/plugins/official-catalog",
            console_get(
                list_official_catalog,
                ConsoleOperation("plugins.official_catalog.view".to_string()),
            ),
        )
        .route(
            "/plugins/install-upload",
            console_post(
                install_uploaded_plugin
                    .layer(DefaultBodyLimit::max(plugin_upload_max_bytes))
                    .layer(middleware::from_fn(move |request, next| {
                        enforce_plugin_upload_limit(plugin_upload_max_bytes, request, next)
                    })),
                ConsoleOperation("plugins.install.upload".to_string()),
            ),
        )
        .route(
            "/plugins/install",
            console_post(
                install_plugin,
                ConsoleOperation("plugins.install".to_string()),
            ),
        )
        .route(
            "/plugins/install-official",
            console_post(
                install_official_plugin,
                ConsoleOperation("plugins.install.official".to_string()),
            ),
        )
        .route(
            "/plugins/:installation_id/catalog-projection/refresh",
            console_post(
                refresh_catalog_projection,
                ConsoleOperation("plugins.catalog_projection.refresh".to_string()),
            ),
        )
        .route(
            "/plugins/:installation_id/artifact/refresh",
            console_post(
                refresh_current_node_artifact,
                ConsoleOperation("plugins.artifact.refresh".to_string()),
            ),
        )
        .route(
            "/plugins/:installation_id/artifact/install-current-node",
            console_post(
                install_current_node_artifact,
                ConsoleOperation("plugins.artifact.install".to_string()),
            ),
        )
        .route(
            "/plugins/:installation_id/enable",
            console_post(
                enable_plugin,
                ConsoleOperation("plugins.enable".to_string()),
            ),
        )
        .route(
            "/plugins/:installation_id/assign",
            console_post(
                assign_plugin,
                ConsoleOperation("plugins.assign".to_string()),
            ),
        )
        .route(
            "/plugins/tasks",
            console_get(
                list_tasks,
                ConsoleOperation("plugins.tasks.view".to_string()),
            ),
        )
        .route(
            "/plugins/tasks/:task_id",
            console_get(get_task, ConsoleOperation("plugins.tasks.view".to_string())),
        )
        .route(
            "/settings/model-providers/plugins/families",
            console_get(
                settings_routes::list_families,
                ConsoleOperation("model_provider_plugins.families.view".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/plugins/official-catalog",
            console_get(
                settings_routes::list_official_catalog,
                ConsoleOperation("model_provider_plugins.official_catalog.view".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/plugins/install-official",
            console_post(
                settings_routes::install_official_plugin,
                ConsoleOperation("model_provider_plugins.install.official".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/plugins/install-upload",
            console_post(
                settings_routes::install_uploaded_plugin
                    .layer(DefaultBodyLimit::max(plugin_upload_max_bytes))
                    .layer(middleware::from_fn(move |request, next| {
                        enforce_plugin_upload_limit(plugin_upload_max_bytes, request, next)
                    })),
                ConsoleOperation("model_provider_plugins.install.upload".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/plugins/:installation_id/artifact/refresh",
            console_post(
                settings_routes::refresh_current_node_artifact,
                ConsoleOperation("model_provider_plugins.artifact.refresh".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/plugins/:installation_id/artifact/install-current-node",
            console_post(
                settings_routes::install_current_node_artifact,
                ConsoleOperation("model_provider_plugins.artifact.install".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/plugins/families/:provider_code/upgrade-latest",
            console_post(
                settings_routes::upgrade_latest,
                ConsoleOperation("model_provider_plugins.families.upgrade".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/plugins/families/:provider_code/switch-version",
            console_post(
                settings_routes::switch_version,
                ConsoleOperation("model_provider_plugins.families.switch".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/plugins/families/:provider_code",
            console_delete(
                settings_routes::delete_family,
                ConsoleOperation("model_provider_plugins.families.delete".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/plugins/tasks/:task_id",
            console_get(
                settings_routes::get_task,
                ConsoleOperation("model_provider_plugins.tasks.view".to_string()),
            ),
        )
}

pub(crate) mod extension_center;
pub(crate) mod interface;
pub(crate) mod settings_routes;

async fn invoke_plugin_interface(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: interface::PluginInterfaceInput,
    mutating: bool,
) -> Result<interface::PluginInterfaceOutput, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
    };
    crate::routes::console_interface::invoke(snapshot_state, binding_id, credential, input).await
}

fn format_time(value: time::OffsetDateTime) -> Result<String, ApiError> {
    Ok(value.format(&Rfc3339)?)
}

fn format_optional_time(value: Option<time::OffsetDateTime>) -> Result<Option<String>, ApiError> {
    value.map(format_time).transpose()
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

pub(crate) fn to_compatibility_override(
    compatibility_override: Option<PluginCompatibilityOverrideBody>,
) -> Option<PluginCompatibilityOverride> {
    compatibility_override.map(|value| PluginCompatibilityOverride {
        reason: value.reason,
        acknowledged_current_host_version: value.acknowledged_current_host_version,
        acknowledged_minimum_host_version: value.acknowledged_minimum_host_version,
    })
}

pub(crate) fn to_risk_override(
    value: Option<PluginRiskOverrideBody>,
) -> Option<PluginRiskOverride> {
    value.map(|value| PluginRiskOverride {
        reason: value.reason,
        acknowledged_warnings: value.acknowledged_warnings,
    })
}

pub(crate) async fn enforce_plugin_upload_limit(
    plugin_upload_max_bytes: usize,
    request: Request<Body>,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();
    match to_bytes(body, plugin_upload_max_bytes).await {
        Ok(bytes) => {
            next.run(Request::from_parts(parts, Body::from(bytes)))
                .await
        }
        Err(_) => StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    }
}

pub(crate) async fn read_upload_file(
    multipart: &mut Multipart,
) -> Result<(String, Vec<u8>), ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("plugin_file"))?
    {
        if field.name() != Some("file") {
            continue;
        }

        let file_name = field
            .file_name()
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty())
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "plugin_file_name",
            ))?;
        let bytes = field
            .bytes()
            .await
            .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("plugin_file"))?;
        if bytes.is_empty() {
            return Err(
                control_plane::errors::ControlPlaneError::InvalidInput("plugin_file").into(),
            );
        }
        return Ok((file_name, bytes.to_vec()));
    }

    Err(control_plane::errors::ControlPlaneError::InvalidInput("plugin_file").into())
}

pub(crate) fn requested_locales(
    locale_meta: &LocaleMetaResponse,
) -> control_plane::i18n::RequestedLocales {
    control_plane::i18n::RequestedLocales::new(
        locale_meta.resolved_locale.clone(),
        locale_meta.fallback_locale.clone(),
    )
}

fn filter_from_query(query: &PluginCatalogQuery) -> PluginCatalogFilter {
    PluginCatalogFilter {
        plugin_type: query.plugin_type.clone(),
    }
}

fn official_filter_from_query(query: &OfficialPluginCatalogQuery) -> OfficialPluginCatalogFilter {
    let limit = query
        .limit
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_OFFICIAL_PLUGIN_CATALOG_LIMIT)
        .min(MAX_OFFICIAL_PLUGIN_CATALOG_LIMIT);

    OfficialPluginCatalogFilter {
        plugin_type: query.plugin_type.clone(),
        search_query: query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        cursor: query
            .cursor
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        limit,
    }
}

#[utoipa::path(
    get,
    path = "/api/console/plugins/catalog",
    params(PluginCatalogQuery),
    operation_id = "plugin_list_catalog",
    responses((status = 200, body = PluginCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<PluginCatalogQuery>,
) -> Result<Json<ApiSuccess<PluginCatalogResponse>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.catalog.v1",
        interface::PluginInterfaceInput::ListCatalog { query, locale },
        false,
    )
    .await?;
    let interface::PluginInterfaceOutput::Catalog(catalog) = output else {
        unreachable!("plugin catalog binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(catalog)))
}

#[utoipa::path(
    get,
    path = "/api/console/plugins/families",
    params(PluginCatalogQuery),
    operation_id = "plugin_list_families",
    responses((status = 200, body = PluginFamilyCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_families(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<PluginCatalogQuery>,
) -> Result<Json<ApiSuccess<PluginFamilyCatalogResponse>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.families.v1",
        interface::PluginInterfaceInput::ListFamilies { query, locale },
        false,
    )
    .await?;
    let interface::PluginInterfaceOutput::Families(families) = output else {
        unreachable!("plugin families binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(families)))
}

#[utoipa::path(
    get,
    path = "/api/console/plugins/official-catalog",
    params(OfficialPluginCatalogQuery),
    operation_id = "plugin_list_official_catalog",
    responses((status = 200, body = OfficialPluginCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_official_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<OfficialPluginCatalogQuery>,
) -> Result<Json<ApiSuccess<OfficialPluginCatalogResponse>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.official-catalog.v1",
        interface::PluginInterfaceInput::ListOfficial { query, locale },
        false,
    )
    .await?;
    let interface::PluginInterfaceOutput::Official(catalog) = output else {
        unreachable!("official plugin catalog binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(catalog)))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/install",
    operation_id = "plugin_install",
    request_body = InstallPluginBody,
    responses((status = 201, body = InstallPluginResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn install_plugin(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InstallPluginBody>,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.install-path.v1",
        interface::PluginInterfaceInput::InstallPath(body),
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Installed(response) = output else {
        unreachable!("plugin install binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/install-upload",
    operation_id = "plugin_install_upload",
    request_body(content = inline(PluginUploadMultipartBody), content_type = "multipart/form-data"),
    responses((status = 201, body = InstallPluginResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn install_uploaded_plugin(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    let (file_name, package_bytes) = read_upload_file(&mut multipart).await?;
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.install-upload.v1",
        interface::PluginInterfaceInput::InstallUploaded {
            file_name,
            package_bytes,
        },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Installed(response) = output else {
        unreachable!("plugin upload binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/install-official",
    operation_id = "plugin_install_official",
    request_body = InstallOfficialPluginBody,
    responses((status = 201, body = InstallPluginResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn install_official_plugin(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InstallOfficialPluginBody>,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.install-official.v1",
        interface::PluginInterfaceInput::InstallOfficial(body),
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Installed(response) = output else {
        unreachable!("official plugin install binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/{installation_id}/catalog-projection/refresh",
    operation_id = "plugin_refresh_catalog_projection",
    responses((status = 200, body = PluginCatalogProjectionResponse), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn refresh_catalog_projection(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginCatalogProjectionResponse>>, ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.catalog-projection-refresh.v1",
        interface::PluginInterfaceInput::RefreshCatalogProjection { installation_id },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Projection(projection) = output else {
        unreachable!("catalog projection binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(projection)))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/{installation_id}/artifact/refresh",
    operation_id = "plugin_refresh_current_node_artifact",
    responses((status = 200, body = PluginArtifactInstanceResponse), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn refresh_current_node_artifact(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginArtifactInstanceResponse>>, ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.artifact-refresh.v1",
        interface::PluginInterfaceInput::RefreshArtifact { installation_id },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Artifact(artifact) = output else {
        unreachable!("artifact refresh binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(artifact)))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/{installation_id}/artifact/install-current-node",
    operation_id = "plugin_install_current_node_artifact",
    responses((status = 200, body = PluginArtifactInstanceResponse), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn install_current_node_artifact(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginArtifactInstanceResponse>>, ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.artifact-install.v1",
        interface::PluginInterfaceInput::InstallArtifact { installation_id },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Artifact(artifact) = output else {
        unreachable!("artifact install binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(artifact)))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/families/{provider_code}/upgrade-latest",
    operation_id = "plugin_upgrade_family_latest",
    request_body = Option<UpgradeLatestPluginFamilyBody>,
    responses((status = 200, body = PluginTaskResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn upgrade_latest(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
    body: Result<Json<UpgradeLatestPluginFamilyBody>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    // Preserve this endpoint's optional-body fallback across the Axum upgrade.
    let body = body.ok().map(|Json(body)| body);
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.family-upgrade.v1",
        interface::PluginInterfaceInput::UpgradeLatest {
            provider_code,
            body,
        },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("plugin upgrade binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/families/{provider_code}/switch-version",
    operation_id = "plugin_switch_family_version",
    request_body = SwitchPluginVersionBody,
    responses((status = 200, body = PluginTaskResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn switch_version(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SwitchPluginVersionBody>,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.family-switch.v1",
        interface::PluginInterfaceInput::SwitchVersion {
            provider_code,
            body,
        },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("plugin switch binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}

#[utoipa::path(
    delete,
    path = "/api/console/plugins/families/{provider_code}",
    operation_id = "plugin_delete_family",
    responses((status = 200, body = PluginTaskResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn delete_family(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.family-delete.v1",
        interface::PluginInterfaceInput::DeleteFamily { provider_code },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("plugin family delete binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/{installation_id}/enable",
    operation_id = "plugin_enable",
    responses((status = 200, body = PluginTaskResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn enable_plugin(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.enable.v1",
        interface::PluginInterfaceInput::Enable { installation_id },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("plugin enable binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/{installation_id}/assign",
    operation_id = "plugin_assign",
    responses((status = 200, body = PluginTaskResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn assign_plugin(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.assign.v1",
        interface::PluginInterfaceInput::Assign { installation_id },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("plugin assign binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}

#[utoipa::path(
    get,
    path = "/api/console/plugins/tasks",
    operation_id = "plugin_list_tasks",
    responses((status = 200, body = [PluginTaskResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_tasks(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<PluginTaskResponse>>>, ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.tasks.v1",
        interface::PluginInterfaceInput::ListTasks,
        false,
    )
    .await?;
    let interface::PluginInterfaceOutput::Tasks(tasks) = output else {
        unreachable!("plugin tasks binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(tasks)))
}

#[utoipa::path(
    get,
    path = "/api/console/plugins/tasks/{task_id}",
    operation_id = "plugin_get_task",
    responses((status = 200, body = PluginTaskResponse), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn get_task(
    State(state): State<Arc<ApiState>>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let output = invoke_plugin_interface(
        state,
        headers,
        "http.console.plugins.task.v1",
        interface::PluginInterfaceInput::GetTask { task_id },
        false,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("plugin task binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}

#[cfg(test)]
#[path = "plugins/_tests/time_response.rs"]
mod time_response_tests;

#[path = "plugins/response_mapping.rs"]
mod response_mapping;

#[cfg(test)]
pub(crate) use response_mapping::runtime_slot_for_contract;
pub(crate) use response_mapping::to_install_response;
use response_mapping::*;
