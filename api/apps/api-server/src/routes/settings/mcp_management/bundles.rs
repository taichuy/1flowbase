use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    sync::Arc,
};

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::errors::ControlPlaneError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_post, ConsoleRouteAssembly,
    },
};

use super::bundles_interface;

const MAX_BUNDLE_BYTES: usize = 8 * 1024 * 1024;
const MAX_BUNDLE_FILES: usize = 256;

async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: bundles_interface::McpBundlesInput,
    mutating: bool,
) -> Result<bundles_interface::McpBundlesOutput, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
    };
    crate::routes::console_interface::invoke(snapshot_state, binding_id, credential, input).await
}

pub(super) fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/mcp/bundles/official",
            console_get(
                list_official_bundles,
                ConsoleOperation("mcp.bundles.official.list".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/preview-official",
            console_post(
                preview_official_bundle,
                ConsoleOperation("mcp.bundles.preview".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/import-official",
            console_post(
                import_official_bundle,
                ConsoleOperation("mcp.bundles.import".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/export",
            console_post(
                export_bundle,
                ConsoleOperation("mcp.bundles.export".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/export-defaults",
            console_get(
                export_bundle_defaults,
                ConsoleOperation("mcp.bundles.export".to_string()),
            ),
        )
        .route(
            "/mcp/instances/:instance_id/bundles/export",
            console_post(
                export_instance_bundle,
                ConsoleOperation("mcp.instances.export".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/preview-upload",
            console_post(
                preview_uploaded_bundle,
                ConsoleOperation("mcp.bundles.preview".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/import-upload",
            console_post(
                import_uploaded_bundle,
                ConsoleOperation("mcp.bundles.import".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/library",
            console_get(
                list_bundle_library,
                ConsoleOperation("mcp.bundle_library.list".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/library/:organization/:bundle_id/sync",
            console_post(
                sync_library_bundle,
                ConsoleOperation("mcp.bundle_library.sync".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/library/:organization/:bundle_id/preview",
            console_post(
                preview_library_bundle,
                ConsoleOperation("mcp.bundle_library.preview".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/library/:organization/:bundle_id/import",
            console_post(
                import_library_bundle,
                ConsoleOperation("mcp.bundle_library.import".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/library/:organization/:bundle_id/current/:bundle_version",
            console_post(
                switch_library_bundle,
                ConsoleOperation("mcp.bundle_library.current.switch".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/library/:organization/:bundle_id/releases/:bundle_version",
            console_delete(
                delete_library_bundle_release,
                ConsoleOperation("mcp.bundle_library.releases.delete".to_string()),
            ),
        )
        .route(
            "/mcp/bundles/library/:organization/:bundle_id/releases/:bundle_version/repair",
            console_post(
                repair_library_bundle_release,
                ConsoleOperation("mcp.bundle_library.releases.repair".to_string()),
            ),
        )
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum McpBundleSourceBody {
    OfficialCatalog(OfficialMcpBundleSelector),
    InstalledExtension(InstalledMcpExtensionSelector),
    BuiltinTemplate(BuiltinMcpTemplateSelector),
}

#[derive(Debug, Deserialize)]
pub(crate) struct OfficialMcpBundleSelector {
    pub(crate) organization: String,
    pub(crate) bundle_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct InstalledMcpExtensionSelector {
    pub(crate) extension_installation_id: String,
    pub(crate) instance_id: Option<String>,
    pub(crate) integrity_override: Option<crate::routes::plugins::PluginRiskOverrideBody>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BuiltinMcpTemplateSelector {
    pub(crate) builtin_template_id: String,
    pub(crate) instance_id: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum McpBundlePreviewSourceResponse {
    OfficialCatalog(domain::McpBundlePreview),
    InstalledExtension(InstalledMcpExtensionPreviewResponse),
    BuiltinTemplate(BuiltinMcpTemplatePreviewResponse),
}

#[derive(Debug, Serialize)]
pub(crate) struct BuiltinMcpTemplatePreviewResponse {
    pub(crate) builtin_template_id: String,
    pub(crate) workspace_application_status: String,
    pub(crate) preview: domain::McpBundlePreview,
}

#[derive(Debug, Serialize)]
pub(crate) struct InstalledMcpExtensionPreviewResponse {
    pub(crate) extension_installation_id: String,
    pub(crate) artifact_installation_status: String,
    pub(crate) workspace_application_status: String,
    pub(crate) integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
    pub(crate) required_integrity_override: Option<domain::ExtensionRiskChallenge>,
    pub(crate) preview: domain::McpBundlePreview,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum McpBundleImportSourceResponse {
    OfficialCatalog(domain::McpBundleImportReport),
    InstalledExtension(InstalledMcpExtensionImportResponse),
    BuiltinTemplate(BuiltinMcpTemplateImportResponse),
}

#[derive(Debug, Serialize)]
pub(crate) struct BuiltinMcpTemplateImportResponse {
    pub(crate) builtin_template_id: String,
    pub(crate) workspace_application_status: String,
    pub(crate) import_report: domain::McpBundleImportReport,
}

#[derive(Debug, Serialize)]
pub(crate) struct InstalledMcpExtensionImportResponse {
    pub(crate) extension_installation_id: String,
    pub(crate) artifact_installation_status: String,
    pub(crate) workspace_application_status: String,
    pub(crate) integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
    pub(crate) import_report: domain::McpBundleImportReport,
}

#[derive(Debug, Serialize)]
pub(crate) struct InstalledMcpExtensionIntegrityChallengeResponse {
    pub(crate) status: u16,
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) extension_installation_id: String,
    pub(crate) artifact_installation_status: String,
    pub(crate) workspace_application_status: String,
    pub(crate) integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
    pub(crate) required_integrity_override: domain::ExtensionRiskChallenge,
    pub(crate) preview: domain::McpBundlePreview,
}

#[derive(Debug, Default, Deserialize, utoipa::ToSchema)]
pub(crate) struct McpBundleLibraryVersionBody {
    pub(crate) bundle_version: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/console/mcp/bundles/library",
    operation_id = "list_mcp_bundle_library",
    summary = "List MCP bundle library",
    description = "Lists remote and locally synchronized MCP bundle releases available to the current workspace.",
    params(McpBundleLibraryQuery),
    responses(
        (status = 200, body = crate::official_mcp_bundles::McpBundleLibraryCatalog),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub(crate) async fn list_bundle_library(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<McpBundleLibraryQuery>,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::McpBundleLibraryCatalog>>, ApiError> {
    let bundles_interface::McpBundlesOutput::Library(catalog) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.library.list.v1",
        bundles_interface::McpBundlesInput::ListLibrary {
            refresh_remote: query.refresh_remote.unwrap_or(false),
        },
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(catalog)))
}

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct McpBundleLibraryQuery {
    pub(crate) refresh_remote: Option<bool>,
}

#[utoipa::path(
    post,
    path = "/api/console/mcp/bundles/library/{organization}/{bundle_id}/sync",
    operation_id = "sync_mcp_bundle_library_release",
    summary = "Synchronize an MCP bundle release",
    description = "Downloads and verifies one MCP bundle release into the local bundle library.",
    params(
        ("organization" = String, Path, description = "Bundle publisher organization"),
        ("bundle_id" = String, Path, description = "Stable bundle identifier")
    ),
    request_body = McpBundleLibraryVersionBody,
    responses(
        (status = 200, body = crate::official_mcp_bundles::LocalMcpBundleReceipt),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub(crate) async fn sync_library_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id)): Path<(String, String)>,
    Json(body): Json<McpBundleLibraryVersionBody>,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::LocalMcpBundleReceipt>>, ApiError> {
    let bundles_interface::McpBundlesOutput::LibraryReceipt(receipt) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.library.sync.v1",
        bundles_interface::McpBundlesInput::SyncLibrary {
            organization,
            bundle_id,
            body,
        },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(receipt)))
}

#[utoipa::path(
    post,
    path = "/api/console/mcp/bundles/library/{organization}/{bundle_id}/preview",
    operation_id = "preview_mcp_bundle_library_release",
    summary = "Preview an MCP bundle release",
    description = "Previews the changes produced by importing one locally synchronized MCP bundle release.",
    params(
        ("organization" = String, Path, description = "Bundle publisher organization"),
        ("bundle_id" = String, Path, description = "Stable bundle identifier")
    ),
    request_body = McpBundleLibraryVersionBody,
    responses(
        (status = 200, body = domain::McpBundlePreview),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub(crate) async fn preview_library_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id)): Path<(String, String)>,
    Json(body): Json<McpBundleLibraryVersionBody>,
) -> Result<Json<ApiSuccess<domain::McpBundlePreview>>, ApiError> {
    let bundles_interface::McpBundlesOutput::Preview(preview) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.library.preview.v1",
        bundles_interface::McpBundlesInput::PreviewLibrary {
            organization,
            bundle_id,
            body,
        },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(preview)))
}

#[utoipa::path(
    post,
    path = "/api/console/mcp/bundles/library/{organization}/{bundle_id}/import",
    operation_id = "import_mcp_bundle_library_release",
    summary = "Import an MCP bundle release",
    description = "Imports one locally synchronized MCP bundle release into the current workspace.",
    params(
        ("organization" = String, Path, description = "Bundle publisher organization"),
        ("bundle_id" = String, Path, description = "Stable bundle identifier")
    ),
    request_body = McpBundleLibraryVersionBody,
    responses(
        (status = 200, body = domain::McpBundleImportReport),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub(crate) async fn import_library_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id)): Path<(String, String)>,
    Json(body): Json<McpBundleLibraryVersionBody>,
) -> Result<Json<ApiSuccess<domain::McpBundleImportReport>>, ApiError> {
    let bundles_interface::McpBundlesOutput::Import(report) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.library.import.v1",
        bundles_interface::McpBundlesInput::ImportLibrary {
            organization,
            bundle_id,
            body,
        },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(report)))
}

async fn switch_library_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id, bundle_version)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::LocalMcpBundleReceipt>>, ApiError> {
    let bundles_interface::McpBundlesOutput::LibraryReceipt(receipt) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.library.current.switch.v1",
        bundles_interface::McpBundlesInput::SwitchLibrary {
            organization,
            bundle_id,
            bundle_version,
        },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(receipt)))
}

async fn delete_library_bundle_release(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id, bundle_version)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let bundles_interface::McpBundlesOutput::Deleted = invoke(
        state,
        headers,
        "http.console.mcp.bundles.library.releases.delete.v1",
        bundles_interface::McpBundlesInput::DeleteLibraryRelease {
            organization,
            bundle_id,
            bundle_version,
        },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(
        serde_json::json!({ "deleted": true }),
    )))
}

async fn repair_library_bundle_release(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id, bundle_version)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::LocalMcpBundleReceipt>>, ApiError> {
    let bundles_interface::McpBundlesOutput::LibraryReceipt(receipt) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.library.releases.repair.v1",
        bundles_interface::McpBundlesInput::RepairLibraryRelease {
            organization,
            bundle_id,
            bundle_version,
        },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(receipt)))
}

async fn list_official_bundles(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot>>, ApiError>
{
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let bundles_interface::McpBundlesOutput::OfficialCatalog(catalog) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.official.list.v1",
        bundles_interface::McpBundlesInput::ListOfficial { locale },
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(catalog)))
}

async fn preview_official_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<McpBundleSourceBody>,
) -> Result<Json<ApiSuccess<McpBundlePreviewSourceResponse>>, ApiError> {
    let bundles_interface::McpBundlesOutput::PreviewOfficial(response) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.preview-official.v1",
        bundles_interface::McpBundlesInput::PreviewOfficial(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

async fn import_official_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<McpBundleSourceBody>,
) -> Result<Response, ApiError> {
    match invoke(
        state,
        headers,
        "http.console.mcp.bundles.import-official.v1",
        bundles_interface::McpBundlesInput::ImportOfficial(body),
        true,
    )
    .await?
    {
        bundles_interface::McpBundlesOutput::ImportOfficial(response) => {
            Ok(Json(ApiSuccess::new(response)).into_response())
        }
        bundles_interface::McpBundlesOutput::IntegrityChallenge(response) => {
            Ok((StatusCode::CONFLICT, Json(response)).into_response())
        }
        _ => unreachable!(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExportMcpBundleBody {
    pub(crate) organization: String,
    pub(crate) bundle_id: String,
    pub(crate) bundle_version: String,
    pub(crate) locale: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExportMcpInstanceBundleBody {
    pub(crate) organization: String,
    pub(crate) bundle_id: String,
    pub(crate) bundle_version: String,
    pub(crate) locale: String,
    pub(crate) export_profile: Option<McpInstanceBundleExportProfile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum McpInstanceBundleExportProfile {
    Portable,
    OfficialBuiltin,
}

#[derive(Debug, Serialize)]
pub(crate) struct McpBundleExportDefaults {
    pub(crate) minimum_host_version: String,
    pub(crate) current_system_version: String,
}

async fn export_bundle_defaults(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpBundleExportDefaults>>, ApiError> {
    let bundles_interface::McpBundlesOutput::ExportDefaults(response) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.export-defaults.v1",
        bundles_interface::McpBundlesInput::ExportDefaults,
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

async fn export_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ExportMcpBundleBody>,
) -> Result<Response, ApiError> {
    let bundles_interface::McpBundlesOutput::Archive(archive) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.export.v1",
        bundles_interface::McpBundlesInput::Export(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    bundle_archive_response(archive)
}

async fn export_instance_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<ExportMcpInstanceBundleBody>,
) -> Result<Response, ApiError> {
    let bundles_interface::McpBundlesOutput::Archive(archive) = invoke(
        state,
        headers,
        "http.console.mcp.instances.bundles.export.v1",
        bundles_interface::McpBundlesInput::ExportInstance { instance_id, body },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    bundle_archive_response(archive)
}

fn bundle_archive_response(
    archive: bundles_interface::BundleArchive,
) -> Result<Response, ApiError> {
    let mut response = Response::new(Body::from(archive.bytes));
    *response.status_mut() = StatusCode::from_u16(archive.status)
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_export_status"))?;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(archive.content_type),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", archive.filename))
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_filename"))?,
    );
    for (name, value) in archive.headers {
        response.headers_mut().insert(
            axum::http::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_export_report"))?,
            HeaderValue::from_str(&value)
                .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_export_report"))?,
        );
    }
    Ok(response)
}

async fn preview_uploaded_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<domain::McpBundlePreview>>, ApiError> {
    let bytes = read_bundle_bytes(&mut multipart).await?;
    let bundles_interface::McpBundlesOutput::Preview(preview) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.preview-upload.v1",
        bundles_interface::McpBundlesInput::PreviewUploaded { bytes },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(preview)))
}

async fn import_uploaded_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<domain::McpBundleImportReport>>, ApiError> {
    let bytes = read_bundle_bytes(&mut multipart).await?;
    let bundles_interface::McpBundlesOutput::Import(report) = invoke(
        state,
        headers,
        "http.console.mcp.bundles.import-upload.v1",
        bundles_interface::McpBundlesInput::ImportUploaded { bytes },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(report)))
}

async fn read_bundle_bytes(multipart: &mut Multipart) -> Result<Vec<u8>, ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_file"))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_file"))?;
        if bytes.is_empty() || bytes.len() > MAX_BUNDLE_BYTES {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_file").into());
        }
        return Ok(bytes.to_vec());
    }
    Err(ControlPlaneError::InvalidInput("mcp_bundle_file").into())
}

pub(crate) fn parse_bundle_archive(
    bytes: &[u8],
) -> Result<domain::McpBundlePackage, ControlPlaneError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
    if archive.is_empty() || archive.len() > MAX_BUNDLE_FILES {
        return Err(ControlPlaneError::InvalidInput("mcp_bundle_file_count"));
    }

    let mut entries = BTreeMap::<String, Vec<u8>>::new();
    let mut total_size = 0_usize;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
        if file.is_dir() {
            continue;
        }
        let path = file
            .enclosed_name()
            .ok_or(ControlPlaneError::InvalidInput("mcp_bundle_path"))?
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_empty() || entries.contains_key(&path) {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_path"));
        }
        let declared_size = usize::try_from(file.size())
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_size"))?;
        total_size = total_size
            .checked_add(declared_size)
            .ok_or(ControlPlaneError::InvalidInput("mcp_bundle_size"))?;
        if total_size > MAX_BUNDLE_BYTES {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_size"));
        }
        let mut content = Vec::with_capacity(declared_size);
        file.read_to_end(&mut content)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
        entries.insert(path, content);
    }

    let manifest_bytes = entries
        .remove("manifest.json")
        .ok_or(ControlPlaneError::InvalidInput("mcp_bundle_manifest"))?;
    let manifest = serde_json::from_slice::<domain::McpBundleManifest>(&manifest_bytes)
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_manifest"))?;
    if manifest.files.len() != entries.len() {
        return Err(ControlPlaneError::InvalidInput("mcp_bundle_files"));
    }

    let mut declared_paths = BTreeSet::new();
    let mut tools = Vec::new();
    let mut instances = Vec::new();
    let mut connections = Vec::new();
    for file in &manifest.files {
        if !declared_paths.insert(file.path.as_str()) {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_files"));
        }
        let content = entries
            .get(&file.path)
            .ok_or(ControlPlaneError::InvalidInput("mcp_bundle_files"))?;
        let actual_sha256 = format!("sha256:{:x}", Sha256::digest(content));
        if actual_sha256 != file.sha256 {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_checksum"));
        }
        match file.kind {
            domain::McpBundleFileKind::Tool if file.path.starts_with("tools/") => {
                tools.push(
                    serde_json::from_slice(content)
                        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_tool"))?,
                );
            }
            domain::McpBundleFileKind::Instance if file.path.starts_with("instances/") => {
                instances.push(
                    serde_json::from_slice(content)
                        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_instance"))?,
                );
            }
            domain::McpBundleFileKind::Connection if file.path.starts_with("connections/") => {
                connections.push(
                    serde_json::from_slice(content)
                        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_connection"))?,
                );
            }
            _ => return Err(ControlPlaneError::InvalidInput("mcp_bundle_file_kind")),
        }
    }

    Ok(domain::McpBundlePackage {
        manifest,
        tools,
        instances,
        connections,
    })
}

pub(crate) fn build_bundle_archive(
    mut package: domain::McpBundlePackage,
) -> Result<Vec<u8>, ControlPlaneError> {
    let mut files = Vec::<(String, Vec<u8>, domain::McpBundleFileKind)>::new();
    for tool in &package.tools {
        let path = bundle_file_path("tools", &tool.tool_id);
        let content = serde_json::to_vec_pretty(tool)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_tool"))?;
        files.push((path, content, domain::McpBundleFileKind::Tool));
    }
    for instance in &package.instances {
        let path = bundle_file_path("instances", &instance.instance_id);
        let content = serde_json::to_vec_pretty(instance)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_instance"))?;
        files.push((path, content, domain::McpBundleFileKind::Instance));
    }
    for connection in &package.connections {
        let path = bundle_file_path("connections", &connection.connection_id.to_string());
        let content = serde_json::to_vec_pretty(connection)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_connection"))?;
        files.push((path, content, domain::McpBundleFileKind::Connection));
    }
    files.sort_by(|left, right| {
        bundle_file_kind_rank(left.2)
            .cmp(&bundle_file_kind_rank(right.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    package.manifest.files = files
        .iter()
        .map(|(path, content, kind)| domain::McpBundleFile {
            path: path.clone(),
            kind: *kind,
            sha256: format!("sha256:{:x}", Sha256::digest(content)),
        })
        .collect();
    let manifest = serde_json::to_vec_pretty(&package.manifest)
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_manifest"))?;

    let cursor = Cursor::new(Vec::new());
    let mut archive = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    archive
        .start_file("manifest.json", options)
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
    archive
        .write_all(&manifest)
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
    for (path, content, _) in files {
        archive
            .start_file(path, options)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
        archive
            .write_all(&content)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
    }
    archive
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))
}

fn bundle_file_kind_rank(kind: domain::McpBundleFileKind) -> u8 {
    match kind {
        domain::McpBundleFileKind::Tool => 0,
        domain::McpBundleFileKind::Instance => 1,
        domain::McpBundleFileKind::Connection => 2,
    }
}

fn bundle_file_path(directory: &str, stable_id: &str) -> String {
    format!(
        "{directory}/{:x}.json",
        Sha256::digest(stable_id.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::parse_bundle_archive;

    #[test]
    fn rejects_non_zip_bundle() {
        assert!(parse_bundle_archive(b"not-a-zip").is_err());
    }
}
