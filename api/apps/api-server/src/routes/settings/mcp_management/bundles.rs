use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    sync::Arc,
};

use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    mcp_bundle::{
        ExportMcpBundleCommand, ExportMcpInstanceBundleCommand, ImportMcpBundleCommand,
        PreviewMcpBundleCommand,
    },
    mcp_management::McpManagementService,
    plugin_management::ExtensionInstallationService,
    plugin_management::{
        installed_extension_integrity_warnings, validate_extension_integrity_override,
        ExtensionRiskOverride,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    official_extension_catalog::{OfficialExtensionCatalogEntry, OfficialExtensionCatalogPage},
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_post, ConsoleRouteAssembly,
    },
};

const MAX_BUNDLE_BYTES: usize = 8 * 1024 * 1024;
const MAX_BUNDLE_FILES: usize = 256;

pub(super) fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
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
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum McpBundleSourceBody {
    OfficialCatalog(OfficialMcpBundleSelector),
    InstalledExtension(InstalledMcpExtensionSelector),
}

#[derive(Debug, Deserialize)]
struct OfficialMcpBundleSelector {
    organization: String,
    bundle_id: String,
}

#[derive(Debug, Deserialize)]
struct InstalledMcpExtensionSelector {
    extension_installation_id: String,
    integrity_override: Option<crate::routes::plugins::PluginRiskOverrideBody>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum McpBundlePreviewSourceResponse {
    OfficialCatalog(domain::McpBundlePreview),
    InstalledExtension(InstalledMcpExtensionPreviewResponse),
}

#[derive(Debug, Serialize)]
struct InstalledMcpExtensionPreviewResponse {
    extension_installation_id: String,
    artifact_installation_status: String,
    workspace_application_status: String,
    integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
    required_integrity_override: Option<domain::ExtensionRiskChallenge>,
    preview: domain::McpBundlePreview,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum McpBundleImportSourceResponse {
    OfficialCatalog(domain::McpBundleImportReport),
    InstalledExtension(InstalledMcpExtensionImportResponse),
}

#[derive(Debug, Serialize)]
struct InstalledMcpExtensionImportResponse {
    extension_installation_id: String,
    artifact_installation_status: String,
    workspace_application_status: String,
    integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
    import_report: domain::McpBundleImportReport,
}

#[derive(Debug, Serialize)]
struct InstalledMcpExtensionIntegrityChallengeResponse {
    status: u16,
    code: String,
    message: String,
    extension_installation_id: String,
    artifact_installation_status: String,
    workspace_application_status: String,
    integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
    required_integrity_override: domain::ExtensionRiskChallenge,
    preview: domain::McpBundlePreview,
}

struct LoadedMcpBundleSource {
    extension_installation_id: Option<uuid::Uuid>,
    package: domain::McpBundlePackage,
    integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
}

#[derive(Debug, Default, Deserialize)]
struct McpBundleLibraryVersionBody {
    bundle_version: Option<String>,
}

async fn list_bundle_library(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::McpBundleLibraryCatalog>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    McpManagementService::new(state.store.clone())
        .authorize_bundle_management(context.user.id)
        .await?;
    let catalog = state.official_mcp_bundle_source.library_catalog().await?;
    Ok(Json(ApiSuccess::new(catalog)))
}

async fn sync_library_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id)): Path<(String, String)>,
    Json(body): Json<McpBundleLibraryVersionBody>,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::LocalMcpBundleReceipt>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .authorize_bundle_management(context.user.id)
        .await?;
    let receipt = state
        .official_mcp_bundle_source
        .sync(&organization, &bundle_id, body.bundle_version.as_deref())
        .await?;
    Ok(Json(ApiSuccess::new(receipt)))
}

async fn preview_library_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id)): Path<(String, String)>,
    Json(body): Json<McpBundleLibraryVersionBody>,
) -> Result<Json<ApiSuccess<domain::McpBundlePreview>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .authorize_bundle_management(context.user.id)
        .await?;
    let bytes = state
        .official_mcp_bundle_source
        .resolve_artifact(&organization, &bundle_id, body.bundle_version.as_deref())
        .await?;
    let package = parse_downloaded_bundle(bytes).await?;
    let interface_catalog =
        super::mcp_interface_catalog_entries(state.as_ref(), context.user.id).await?;
    let preview = McpManagementService::new(state.store.clone())
        .preview_bundle(PreviewMcpBundleCommand {
            actor_user_id: context.user.id,
            package,
            interface_catalog,
            current_system_version: current_system_version(),
        })
        .await?;
    Ok(Json(ApiSuccess::new(preview)))
}

async fn import_library_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id)): Path<(String, String)>,
    Json(body): Json<McpBundleLibraryVersionBody>,
) -> Result<Json<ApiSuccess<domain::McpBundleImportReport>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .authorize_bundle_management(context.user.id)
        .await?;
    let bytes = state
        .official_mcp_bundle_source
        .resolve_artifact(&organization, &bundle_id, body.bundle_version.as_deref())
        .await?;
    let package = parse_downloaded_bundle(bytes).await?;
    let interface_catalog =
        super::mcp_interface_catalog_entries(state.as_ref(), context.user.id).await?;
    let report = McpManagementService::new(state.store.clone())
        .import_bundle(ImportMcpBundleCommand {
            actor_user_id: context.user.id,
            package,
            interface_catalog,
            current_system_version: current_system_version(),
        })
        .await?;
    Ok(Json(ApiSuccess::new(report)))
}

async fn switch_library_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id, bundle_version)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::LocalMcpBundleReceipt>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .authorize_bundle_management(context.user.id)
        .await?;
    let receipt = state
        .official_mcp_bundle_source
        .switch_current(&organization, &bundle_id, &bundle_version)
        .await?;
    Ok(Json(ApiSuccess::new(receipt)))
}

async fn delete_library_bundle_release(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id, bundle_version)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .authorize_bundle_management(context.user.id)
        .await?;
    state
        .official_mcp_bundle_source
        .delete_local_version(&organization, &bundle_id, &bundle_version)
        .await?;
    Ok(Json(ApiSuccess::new(
        serde_json::json!({ "deleted": true }),
    )))
}

async fn repair_library_bundle_release(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((organization, bundle_id, bundle_version)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::LocalMcpBundleReceipt>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .authorize_bundle_management(context.user.id)
        .await?;
    let receipt = state
        .official_mcp_bundle_source
        .repair(&organization, &bundle_id, &bundle_version)
        .await?;
    Ok(Json(ApiSuccess::new(receipt)))
}

async fn list_official_bundles(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot>>, ApiError>
{
    let context = require_session(&state, &headers).await?;
    McpManagementService::new(state.store.clone())
        .authorize_bundle_management(context.user.id)
        .await?;
    let first_page = state
        .official_extension_catalog_source
        .list_page("mcp", None)
        .await?;
    let mut next_cursor = first_page.metadata.next_cursor.clone();
    let mut pages = vec![first_page];
    while let Some(cursor) = next_cursor {
        let page = state
            .official_extension_catalog_source
            .list_page("mcp", Some(&cursor))
            .await?;
        next_cursor = page.metadata.next_cursor.clone();
        pages.push(page);
    }
    let mut catalog = project_official_mcp_catalog(&state, pages)?;
    let locale = crate::app_state::request_catalog_locale(&headers, context.user.preferred_locale);
    catalog.source.source_label = crate::app_state::resolve_official_source_label(
        &state,
        &locale,
        &catalog.source.source_kind,
        catalog.source.source_label,
    )
    .await?;
    Ok(Json(ApiSuccess::new(catalog)))
}

fn project_official_mcp_catalog(
    state: &ApiState,
    pages: Vec<OfficialExtensionCatalogPage>,
) -> anyhow::Result<crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot> {
    let first = pages
        .first()
        .ok_or_else(|| anyhow::anyhow!("official MCP catalog has no page"))?;
    let source_kind = first.source_kind.clone();
    let catalog_url = first.metadata.locator.clone();
    let entries = pages
        .into_iter()
        .flat_map(|page| page.entries)
        .map(|entry| project_official_mcp_entry(state, entry))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(
        crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot {
            source: crate::official_mcp_bundles::OfficialMcpBundleCatalogSource {
                source_label: source_kind.clone(),
                source_kind,
                catalog_url,
            },
            entries,
        },
    )
}

fn project_official_mcp_entry(
    state: &ApiState,
    entry: OfficialExtensionCatalogEntry,
) -> anyhow::Result<crate::official_mcp_bundles::OfficialMcpBundleCatalogEntry> {
    if entry.category != "mcp" {
        anyhow::bail!("official MCP catalog projection received another category");
    }
    let locale = catalog_metadata_string(&entry, "locale")?;
    let exported_from_system_version =
        catalog_metadata_string(&entry, "exported_from_system_version")?;
    let release_tag = catalog_metadata_string(&entry, "release_tag")?;
    let descriptor = state
        .official_extension_catalog_source
        .resolve_artifact(&entry)?;
    Ok(crate::official_mcp_bundles::OfficialMcpBundleCatalogEntry {
        organization: entry.organization,
        bundle_id: entry.artifact,
        latest_version: entry.version,
        locale,
        minimum_host_version: entry.host_version_requirement,
        exported_from_system_version,
        release_tag,
        download_url: descriptor.locator,
        artifact_sha256: descriptor.expected_checksum,
    })
}

fn catalog_metadata_string(
    entry: &OfficialExtensionCatalogEntry,
    field: &'static str,
) -> anyhow::Result<String> {
    entry
        .source
        .metadata
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("official extension catalog entry is missing {field}"))
}

async fn preview_official_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<McpBundleSourceBody>,
) -> Result<Json<ApiSuccess<McpBundlePreviewSourceResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let source = load_mcp_bundle_source(&state, body).await?;
    let interface_catalog =
        super::mcp_interface_catalog_entries(state.as_ref(), context.user.id).await?;
    let preview = McpManagementService::new(state.store.clone())
        .preview_bundle(PreviewMcpBundleCommand {
            actor_user_id: context.user.id,
            package: source.package,
            interface_catalog,
            current_system_version: current_system_version(),
        })
        .await?;
    let response = match source.extension_installation_id {
        Some(extension_installation_id) => {
            let has_import_receipt = McpManagementService::new(state.store.clone())
                .extension_bundle_is_imported(context.user.id, extension_installation_id)
                .await?;
            let workspace_application_status = if preview.effect_summary.changes > 0 {
                "ready_to_import"
            } else if has_import_receipt {
                "imported"
            } else {
                "already_present"
            };
            McpBundlePreviewSourceResponse::InstalledExtension(
                InstalledMcpExtensionPreviewResponse {
                    extension_installation_id: extension_installation_id.to_string(),
                    artifact_installation_status: "installed".to_string(),
                    workspace_application_status: workspace_application_status.to_string(),
                    required_integrity_override: (!source.integrity_warnings.is_empty()).then(
                        || domain::ExtensionRiskChallenge {
                            warnings: source.integrity_warnings.clone(),
                            compatibility: None,
                        },
                    ),
                    integrity_warnings: source.integrity_warnings,
                    preview,
                },
            )
        }
        None => McpBundlePreviewSourceResponse::OfficialCatalog(preview),
    };
    Ok(Json(ApiSuccess::new(response)))
}

async fn import_official_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<McpBundleSourceBody>,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let interface_catalog =
        super::mcp_interface_catalog_entries(state.as_ref(), context.user.id).await?;
    let integrity_override = match &body {
        McpBundleSourceBody::InstalledExtension(selector) => selector
            .integrity_override
            .as_ref()
            .map(|value| ExtensionRiskOverride {
                reason: value.reason.clone(),
                acknowledged_warnings: value.acknowledged_warnings.clone(),
            }),
        McpBundleSourceBody::OfficialCatalog(_) => None,
    };
    let source = load_mcp_bundle_source(&state, body).await?;
    let service = McpManagementService::new(state.store.clone());
    if let Some(extension_installation_id) = source.extension_installation_id {
        let preview = service
            .preview_bundle(PreviewMcpBundleCommand {
                actor_user_id: context.user.id,
                package: source.package.clone(),
                interface_catalog: interface_catalog.clone(),
                current_system_version: current_system_version(),
            })
            .await?;
        if !validate_extension_integrity_override(
            &source.integrity_warnings,
            integrity_override.as_ref(),
        )? {
            return Ok((
                StatusCode::CONFLICT,
                Json(InstalledMcpExtensionIntegrityChallengeResponse {
                    status: StatusCode::CONFLICT.as_u16(),
                    code: "mcp_bundle_integrity_confirmation_required".to_string(),
                    message: "Installed MCP artifact integrity warnings require confirmation."
                        .to_string(),
                    extension_installation_id: extension_installation_id.to_string(),
                    artifact_installation_status: "installed".to_string(),
                    workspace_application_status: "not_imported".to_string(),
                    integrity_warnings: source.integrity_warnings.clone(),
                    required_integrity_override: domain::ExtensionRiskChallenge {
                        warnings: source.integrity_warnings.clone(),
                        compatibility: None,
                    },
                    preview,
                }),
            )
                .into_response());
        }
    }
    let report = service
        .import_bundle(ImportMcpBundleCommand {
            actor_user_id: context.user.id,
            package: source.package,
            interface_catalog,
            current_system_version: current_system_version(),
        })
        .await?;
    let is_reconciled = report.effect_summary.conflicts == 0 && report.effect_summary.failed == 0;
    if let Some(extension_installation_id) = source.extension_installation_id {
        if is_reconciled {
            service
                .record_extension_bundle_import(
                    context.user.id,
                    extension_installation_id,
                    &report.status,
                )
                .await?;
        }
    }
    let response = match source.extension_installation_id {
        Some(extension_installation_id) => {
            let workspace_application_status = if is_reconciled {
                "imported"
            } else if report.effect_summary.changes > 0 {
                "partially_imported"
            } else {
                "not_imported"
            };
            McpBundleImportSourceResponse::InstalledExtension(InstalledMcpExtensionImportResponse {
                extension_installation_id: extension_installation_id.to_string(),
                artifact_installation_status: "installed".to_string(),
                workspace_application_status: workspace_application_status.to_string(),
                integrity_warnings: source.integrity_warnings,
                import_report: report,
            })
        }
        None => McpBundleImportSourceResponse::OfficialCatalog(report),
    };
    Ok(Json(ApiSuccess::new(response)).into_response())
}

async fn load_mcp_bundle_source(
    state: &ApiState,
    body: McpBundleSourceBody,
) -> Result<LoadedMcpBundleSource, ApiError> {
    match body {
        McpBundleSourceBody::OfficialCatalog(selector) => {
            let catalog_id = format!("mcp:{}/{}", selector.organization, selector.bundle_id);
            let located = state
                .official_extension_catalog_source
                .find_entry("mcp", &catalog_id)
                .await?;
            let located = located.ok_or(ControlPlaneError::NotFound("official_mcp_bundle"))?;
            if located.entry.organization != selector.organization
                || located.entry.artifact != selector.bundle_id
            {
                return Err(ControlPlaneError::InvalidInput("official_mcp_bundle").into());
            }
            let downloaded = state
                .official_extension_catalog_source
                .download_artifact(&located.entry)
                .await?;
            Ok(LoadedMcpBundleSource {
                extension_installation_id: None,
                package: parse_downloaded_bundle(downloaded.artifact_bytes).await?,
                integrity_warnings: Vec::new(),
            })
        }
        McpBundleSourceBody::InstalledExtension(selector) => {
            let installation_id = uuid::Uuid::parse_str(&selector.extension_installation_id)
                .map_err(|_| ControlPlaneError::InvalidInput("extension_installation_id"))?;
            let installation = ExtensionInstallationService::new(
                state.store.clone(),
                &state.provider_install_root,
            )
            .find_local_installation_by_id(&state.api_node_id, installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("extension_installation"))?;
            if installation.identity.category != domain::ExtensionCategory::Mcp {
                return Err(ControlPlaneError::InvalidInput("extension_installation_id").into());
            }
            let bytes = tokio::fs::read(&installation.local_path).await?;
            let integrity_warnings = installed_extension_integrity_warnings(&installation, &bytes);
            Ok(LoadedMcpBundleSource {
                extension_installation_id: Some(installation_id),
                package: parse_downloaded_bundle(bytes).await?,
                integrity_warnings,
            })
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportMcpBundleBody {
    organization: String,
    bundle_id: String,
    bundle_version: String,
    locale: String,
}

#[derive(Debug, Serialize)]
struct McpBundleExportDefaults {
    minimum_host_version: String,
    current_system_version: String,
}

async fn export_bundle_defaults(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpBundleExportDefaults>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    McpManagementService::new(state.store.clone())
        .authorize_bundle_management(context.user.id)
        .await?;
    let current_system_version = current_system_version();
    Ok(Json(ApiSuccess::new(McpBundleExportDefaults {
        minimum_host_version: current_system_version.clone(),
        current_system_version,
    })))
}

async fn export_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ExportMcpBundleBody>,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let package = McpManagementService::new(state.store.clone())
        .export_bundle(ExportMcpBundleCommand {
            actor_user_id: context.user.id,
            organization: body.organization,
            bundle_id: body.bundle_id,
            bundle_version: body.bundle_version,
            locale: body.locale,
            current_system_version: current_system_version(),
        })
        .await?;
    bundle_archive_response(package, "mcp-bundle.zip").await
}

async fn export_instance_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<ExportMcpBundleBody>,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let filename = format!("mcp-instance-{}.zip", safe_filename_segment(&instance_id));
    let package = McpManagementService::new(state.store.clone())
        .export_instance_bundle(ExportMcpInstanceBundleCommand {
            actor_user_id: context.user.id,
            instance_id,
            organization: body.organization,
            bundle_id: body.bundle_id,
            bundle_version: body.bundle_version,
            locale: body.locale,
            current_system_version: current_system_version(),
        })
        .await?;
    bundle_archive_response(package, &filename).await
}

async fn bundle_archive_response(
    package: domain::McpBundlePackage,
    filename: &str,
) -> Result<Response, ApiError> {
    let archive = tokio::task::spawn_blocking(move || build_bundle_archive(package))
        .await
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))??;
    let mut response = Response::new(Body::from(archive));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_filename"))?,
    );
    Ok(response)
}

fn safe_filename_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

async fn preview_uploaded_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<domain::McpBundlePreview>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let package = read_bundle_package(&mut multipart).await?;
    let interface_catalog =
        super::mcp_interface_catalog_entries(state.as_ref(), context.user.id).await?;
    let preview = McpManagementService::new(state.store.clone())
        .preview_bundle(PreviewMcpBundleCommand {
            actor_user_id: context.user.id,
            package,
            interface_catalog,
            current_system_version: current_system_version(),
        })
        .await?;
    Ok(Json(ApiSuccess::new(preview)))
}

async fn import_uploaded_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<domain::McpBundleImportReport>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let package = read_bundle_package(&mut multipart).await?;
    let interface_catalog =
        super::mcp_interface_catalog_entries(state.as_ref(), context.user.id).await?;
    let report = McpManagementService::new(state.store.clone())
        .import_bundle(ImportMcpBundleCommand {
            actor_user_id: context.user.id,
            package,
            interface_catalog,
            current_system_version: current_system_version(),
        })
        .await?;
    Ok(Json(ApiSuccess::new(report)))
}

fn current_system_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

async fn read_bundle_package(
    multipart: &mut Multipart,
) -> Result<domain::McpBundlePackage, ApiError> {
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
        return tokio::task::spawn_blocking(move || parse_bundle_archive(bytes.as_ref()))
            .await
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?
            .map_err(Into::into);
    }
    Err(ControlPlaneError::InvalidInput("mcp_bundle_file").into())
}

async fn parse_downloaded_bundle(bytes: Vec<u8>) -> Result<domain::McpBundlePackage, ApiError> {
    if bytes.is_empty() || bytes.len() > MAX_BUNDLE_BYTES {
        return Err(ControlPlaneError::InvalidInput("mcp_bundle_file").into());
    }
    tokio::task::spawn_blocking(move || parse_bundle_archive(&bytes))
        .await
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?
        .map_err(Into::into)
}

fn parse_bundle_archive(bytes: &[u8]) -> Result<domain::McpBundlePackage, ControlPlaneError> {
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

fn build_bundle_archive(
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
