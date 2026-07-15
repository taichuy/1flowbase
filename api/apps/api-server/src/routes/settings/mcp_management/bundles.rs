use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    sync::Arc,
};

use axum::{
    body::Body,
    extract::{Multipart, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    mcp_bundle::{ExportMcpBundleCommand, ImportMcpBundleCommand, PreviewMcpBundleCommand},
    mcp_management::McpManagementService,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

const MAX_BUNDLE_BYTES: usize = 8 * 1024 * 1024;
const MAX_BUNDLE_FILES: usize = 256;

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
struct OfficialMcpBundleBody {
    organization: String,
    bundle_id: String,
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
    let catalog = state.official_mcp_bundle_source.list_catalog().await?;
    Ok(Json(ApiSuccess::new(catalog)))
}

async fn preview_official_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<OfficialMcpBundleBody>,
) -> Result<Json<ApiSuccess<domain::McpBundlePreview>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let service = McpManagementService::new(state.store.clone());
    service.authorize_bundle_management(context.user.id).await?;
    let downloaded = state
        .official_mcp_bundle_source
        .download_bundle(&body.organization, &body.bundle_id)
        .await?;
    let package = parse_downloaded_bundle(downloaded.package_bytes).await?;
    let interface_catalog =
        super::mcp_interface_catalog_entries(state.as_ref(), context.user.id).await?;
    let preview = service
        .preview_bundle(PreviewMcpBundleCommand {
            actor_user_id: context.user.id,
            package,
            interface_catalog,
            current_system_version: current_system_version(),
        })
        .await?;
    Ok(Json(ApiSuccess::new(preview)))
}

async fn import_official_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<OfficialMcpBundleBody>,
) -> Result<Json<ApiSuccess<domain::McpBundleImportReport>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let service = McpManagementService::new(state.store.clone());
    service.authorize_bundle_management(context.user.id).await?;
    let downloaded = state
        .official_mcp_bundle_source
        .download_bundle(&body.organization, &body.bundle_id)
        .await?;
    let package = parse_downloaded_bundle(downloaded.package_bytes).await?;
    let interface_catalog =
        super::mcp_interface_catalog_entries(state.as_ref(), context.user.id).await?;
    let report = service
        .import_bundle(ImportMcpBundleCommand {
            actor_user_id: context.user.id,
            package,
            interface_catalog,
            current_system_version: current_system_version(),
        })
        .await?;
    Ok(Json(ApiSuccess::new(report)))
}

#[derive(Debug, Deserialize)]
struct ExportMcpBundleBody {
    organization: String,
    bundle_id: String,
    bundle_version: String,
    locale: String,
    minimum_host_version: String,
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
            minimum_host_version: body.minimum_host_version,
            current_system_version: current_system_version(),
        })
        .await?;
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
        HeaderValue::from_static("attachment; filename=\"mcp-bundle.zip\""),
    );
    Ok(response)
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
