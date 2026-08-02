use std::{
    collections::BTreeSet,
    io::{Cursor, Read, Write},
    sync::Arc,
};

use access_control::{
    APPLICATIONS_ORCHESTRATION_TEMPLATE_EXPORT_OPERATION_ID,
    APPLICATIONS_ORCHESTRATION_TEMPLATE_IMPORT_OPERATION_ID,
    APPLICATIONS_ORCHESTRATION_VERSION_RESTORE_OPERATION_ID, APPLICATIONS_UPDATE_OPERATION_ID,
    APPLICATIONS_VIEW_OPERATION_ID,
};
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    Json, Router,
};
use control_plane::{
    application::{
        ApplicationArchiveApplication, ApplicationArchiveEntry, ApplicationArchivePackage,
        ApplicationArchiveService, ExportApplicationArchiveCommand,
        ImportApplicationArchiveCommand, PreviewApplicationArchiveCommand,
        APPLICATION_ARCHIVE_SCHEMA_VERSION,
    },
    errors::ControlPlaneError,
    flow::{
        AgentFlowTemplateDependency, AgentFlowTemplateDependencyStatus, AgentFlowTemplatePackage,
        AgentFlowTemplatePreview, AgentFlowTemplateUnresolvedNode, FlowService,
        ImportAgentFlowTemplateResult, SaveFlowDraftCommand, UpdateFlowVersionMetadataCommand,
    },
    i18n_catalog::CatalogResolver,
    plugin_management::ExtensionInstallationService,
    plugin_management::{
        installed_extension_integrity_warnings, validate_extension_integrity_override,
        ExtensionRiskOverride,
    },
};
use domain::CatalogMessageIdentity;
use orchestration_runtime::{
    binding_runtime::referenced_i18n_text_refs,
    compiled_plan::CompiledI18nTextRef,
    compiler::{FlowCompileContext, FlowCompiler},
};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_post, console_put, ConsoleRouteAssembly,
    },
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveDraftBody {
    pub document: serde_json::Value,
    pub change_kind: String,
    pub summary: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateVersionBody {
    pub summary: Option<String>,
    pub summary_is_custom: Option<bool>,
    pub is_user_protected: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExportApplicationArchiveBody {
    pub application_ids: Vec<Uuid>,
}

#[derive(Debug, ToSchema)]
pub struct ApplicationArchiveUploadBody {
    #[schema(value_type = String, format = Binary)]
    pub file: String,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FlowVersionResponse {
    pub id: String,
    pub sequence: i64,
    pub trigger: String,
    pub change_kind: String,
    pub summary: String,
    pub summary_is_custom: bool,
    pub is_user_protected: bool,
    pub is_current_publication: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FlowDraftResponse {
    pub id: String,
    pub flow_id: String,
    pub document: serde_json::Value,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReferencedI18nMessageResponse {
    pub key: String,
    pub text: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OrchestrationStateResponse {
    pub flow_id: String,
    pub draft: FlowDraftResponse,
    pub messages: Vec<ReferencedI18nMessageResponse>,
    pub versions: Vec<FlowVersionResponse>,
    pub autosave_interval_seconds: u16,
    pub user_protection_limit: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateApplicationResponse {
    pub application_type: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateDependencyResponse {
    pub kind: String,
    pub node_id: Option<String>,
    pub node_type: Option<String>,
    pub config_version: Option<i64>,
    pub provider_code: Option<String>,
    pub model_id: Option<String>,
    pub plugin_id: Option<String>,
    pub plugin_version: Option<String>,
    pub contribution_code: Option<String>,
    pub node_shell: Option<String>,
    pub schema_version: Option<String>,
    pub plugin_unique_identifier: Option<String>,
    pub package_id: Option<String>,
    pub contribution_checksum: Option<String>,
    pub compiled_contribution_hash: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateDependencyStatusResponse {
    pub dependency: AgentFlowTemplateDependencyResponse,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateUnresolvedNodeResponse {
    pub node_id: String,
    pub alias: String,
    pub original_type: String,
    pub dependency_status: String,
    pub reason: String,
    #[schema(value_type = Object)]
    pub original_node: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplatePreviewResponse {
    pub schema_version: String,
    pub application: AgentFlowTemplateApplicationResponse,
    pub dependencies: Vec<AgentFlowTemplateDependencyStatusResponse>,
    pub unresolved_nodes: Vec<AgentFlowTemplateUnresolvedNodeResponse>,
    #[schema(value_type = Object)]
    pub document: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateImportedApplicationResponse {
    pub id: String,
    pub application_type: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
    pub created_by: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportAgentFlowTemplateResponse {
    pub application: AgentFlowTemplateImportedApplicationResponse,
    pub orchestration: OrchestrationStateResponse,
    pub preview: AgentFlowTemplatePreviewResponse,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::{Authenticated, ConsoleOperation};

    ConsoleRouteAssembly::new()
        .route(
            "/applications/:id/orchestration",
            console_get(
                get_orchestration,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/orchestration/draft",
            console_put(
                save_draft,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/archive/export",
            console_post(
                export_application_archive,
                ConsoleOperation(
                    APPLICATIONS_ORCHESTRATION_TEMPLATE_EXPORT_OPERATION_ID.to_string(),
                ),
            ),
        )
        .route(
            "/applications/archive/preview",
            console_post(preview_application_archive, Authenticated),
        )
        .route(
            "/applications/archive/import",
            console_post(
                import_application_archive,
                ConsoleOperation(
                    APPLICATIONS_ORCHESTRATION_TEMPLATE_IMPORT_OPERATION_ID.to_string(),
                ),
            ),
        )
        .route(
            "/applications/archive/installed-extension/:installation_id/preview",
            console_get(preview_installed_application_extension, Authenticated),
        )
        .route(
            "/applications/archive/installed-extension/:installation_id/import",
            console_post(import_installed_application_extension, Authenticated),
        )
        .route(
            "/applications/:id/orchestration/versions/:version_id/restore",
            console_post(
                restore_version,
                ConsoleOperation(
                    APPLICATIONS_ORCHESTRATION_VERSION_RESTORE_OPERATION_ID.to_string(),
                ),
            ),
        )
        .route(
            "/applications/:id/orchestration/versions/:version_id",
            console_patch(
                update_version,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
}

struct ApplicationArchiveUpload {
    bytes: Vec<u8>,
    name: Option<String>,
    description: Option<String>,
}

async fn read_application_archive_upload(
    multipart: &mut Multipart,
) -> Result<ApplicationArchiveUpload, ApiError> {
    const MAX_APPLICATION_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;
    let mut bytes = None;
    let mut name = None;
    let mut description = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ControlPlaneError::InvalidInput("application_archive"))?
    {
        match field.name() {
            Some("file") => {
                let value = field
                    .bytes()
                    .await
                    .map_err(|_| ControlPlaneError::InvalidInput("application_archive"))?;
                if value.is_empty() || value.len() > MAX_APPLICATION_ARCHIVE_BYTES {
                    return Err(ControlPlaneError::InvalidInput("application_archive").into());
                }
                bytes = Some(value.to_vec());
            }
            Some("name") => {
                name = field
                    .text()
                    .await
                    .ok()
                    .filter(|value| !value.trim().is_empty());
            }
            Some("description") => {
                description = field.text().await.ok();
            }
            _ => {}
        }
    }
    Ok(ApplicationArchiveUpload {
        bytes: bytes.ok_or(ControlPlaneError::InvalidInput("application_archive"))?,
        name,
        description,
    })
}

fn parse_application_archive(bytes: &[u8]) -> Result<ApplicationArchivePackage, ApiError> {
    const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
    if bytes.starts_with(b"PK") {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| ControlPlaneError::InvalidInput("application_archive"))?;
        let mut manifest = archive
            .by_name("manifest.json")
            .map_err(|_| ControlPlaneError::InvalidInput("application_archive_manifest"))?;
        if manifest.size() > MAX_MANIFEST_BYTES {
            return Err(ControlPlaneError::InvalidInput("application_archive_manifest").into());
        }
        let mut manifest_json = Vec::with_capacity(manifest.size() as usize);
        manifest
            .read_to_end(&mut manifest_json)
            .map_err(|_| ControlPlaneError::InvalidInput("application_archive_manifest"))?;
        let package: ApplicationArchivePackage = serde_json::from_slice(&manifest_json)
            .map_err(|_| ControlPlaneError::InvalidInput("application_archive_manifest"))?;
        if package.schema_version != APPLICATION_ARCHIVE_SCHEMA_VERSION {
            return Err(
                ControlPlaneError::InvalidInput("application_archive_schema_version").into(),
            );
        }
        return Ok(package);
    }

    if let Ok(package) = serde_json::from_slice::<ApplicationArchivePackage>(bytes) {
        if package.schema_version != APPLICATION_ARCHIVE_SCHEMA_VERSION {
            return Err(
                ControlPlaneError::InvalidInput("application_archive_schema_version").into(),
            );
        }
        return Ok(package);
    }

    let template: AgentFlowTemplatePackage = serde_json::from_slice(bytes)
        .map_err(|_| ControlPlaneError::InvalidInput("application_archive"))?;
    Ok(ApplicationArchivePackage {
        schema_version: APPLICATION_ARCHIVE_SCHEMA_VERSION.to_string(),
        applications: vec![ApplicationArchiveEntry {
            template_id: String::new(),
            release_version: 0,
            exported_from_system_version: String::new(),
            exported_at: String::new(),
            application: ApplicationArchiveApplication {
                application_type: template.application.application_type,
                workflow_trigger_type: None,
                name: template.application.name,
                description: template.application.description,
                icon: template.application.icon,
                icon_type: template.application.icon_type,
                icon_background: template.application.icon_background,
            },
            flow_document: template.flow_document,
            dependencies: template.dependencies,
            workflow_trigger_config: None,
        }],
    })
}

fn single_application_entry(
    package: ApplicationArchivePackage,
) -> Result<ApplicationArchiveEntry, ApiError> {
    let [entry] = <[ApplicationArchiveEntry; 1]>::try_from(package.applications)
        .map_err(|_| ControlPlaneError::InvalidInput("application_archive_application_count"))?;
    Ok(entry)
}

async fn uploaded_application_archive_entry(
    multipart: &mut Multipart,
) -> Result<(ApplicationArchiveEntry, Option<String>, Option<String>), ApiError> {
    let upload = read_application_archive_upload(multipart).await?;
    let name = upload.name;
    let description = upload.description;
    let package = tokio::task::spawn_blocking(move || parse_application_archive(&upload.bytes))
        .await
        .map_err(|_| ControlPlaneError::InvalidInput("application_archive"))??;
    Ok((single_application_entry(package)?, name, description))
}

async fn installed_application_archive_entry(
    state: &ApiState,
    installation_id: Uuid,
) -> Result<
    (
        ApplicationArchiveEntry,
        Vec<domain::ExtensionIntegrityWarning>,
    ),
    ApiError,
> {
    let installation =
        ExtensionInstallationService::new(state.store.clone(), &state.provider_install_root)
            .find_local_installation_by_id(&state.api_node_id, installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("extension_installation"))?;
    if installation.identity.category != domain::ExtensionCategory::AgentFlow
        || installation.application_action != domain::ExtensionApplicationAction::ImportAgentFlow
    {
        return Err(ControlPlaneError::InvalidInput("agent_flow_extension_installation").into());
    }
    let bytes = tokio::fs::read(&installation.local_path).await?;
    let warnings = installed_extension_integrity_warnings(&installation, &bytes);
    let package = tokio::task::spawn_blocking(move || parse_application_archive(&bytes))
        .await
        .map_err(|_| ControlPlaneError::InvalidInput("application_archive"))??;
    Ok((single_application_entry(package)?, warnings))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportInstalledApplicationExtensionBody {
    pub name: Option<String>,
    pub description: Option<String>,
    pub integrity_override: Option<crate::routes::plugins::PluginRiskOverrideBody>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstalledApplicationExtensionPreviewResponse {
    pub extension_installation_id: String,
    pub application_status: String,
    #[schema(value_type = Vec<Object>)]
    pub integrity_warnings: Vec<domain::ExtensionIntegrityWarning>,
    #[schema(value_type = Option<Object>)]
    pub required_integrity_override: Option<domain::ExtensionRiskChallenge>,
    pub preview: AgentFlowTemplatePreviewResponse,
}

#[utoipa::path(
    get,
    path = "/api/console/applications/archive/installed-extension/{installation_id}/preview",
    summary = "Preview an installed Agent Flow extension",
    description = "Loads the exact local extension artifact and returns the existing application import preview without creating an application.",
    params(("installation_id" = Uuid, Path, description = "Extension installation ID")),
    responses((status = 200, body = InstalledApplicationExtensionPreviewResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn preview_installed_application_extension(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(installation_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<InstalledApplicationExtensionPreviewResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let (entry, warnings) = installed_application_archive_entry(&state, installation_id).await?;
    let resources = FlowService::new(state.store.clone())
        .load_agent_flow_template_resources(context.user.id)
        .await?;
    let preview = ApplicationArchiveService::new(state.store.clone())
        .preview_archive(PreviewApplicationArchiveCommand {
            actor_user_id: context.user.id,
            entry,
            resources,
        })
        .await?;
    let applied = control_plane::ports::ApplicationRepository::has_application_extension_source(
        &state.store,
        context.actor.current_workspace_id,
        installation_id,
    )
    .await?;
    Ok(Json(ApiSuccess::new(
        InstalledApplicationExtensionPreviewResponse {
            extension_installation_id: installation_id.to_string(),
            application_status: if applied { "applied" } else { "not_applied" }.to_string(),
            required_integrity_override: (!warnings.is_empty()).then(|| {
                domain::ExtensionRiskChallenge {
                    warnings: warnings.clone(),
                    compatibility: None,
                }
            }),
            integrity_warnings: warnings,
            preview: to_template_preview_response(preview),
        },
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/archive/installed-extension/{installation_id}/import",
    summary = "Import an installed Agent Flow extension",
    description = "Imports the exact local extension artifact into the current workspace and records its application provenance.",
    params(("installation_id" = Uuid, Path, description = "Extension installation ID")),
    request_body = ImportInstalledApplicationExtensionBody,
    responses((status = 201, body = ImportAgentFlowTemplateResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn import_installed_application_extension(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(installation_id): Path<Uuid>,
    Json(body): Json<ImportInstalledApplicationExtensionBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<ImportAgentFlowTemplateResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let locale =
        crate::app_state::request_catalog_locale(&headers, context.user.preferred_locale.clone());
    let (entry, warnings) = installed_application_archive_entry(&state, installation_id).await?;
    let risk_override = body.integrity_override.map(|value| ExtensionRiskOverride {
        reason: value.reason,
        acknowledged_warnings: value.acknowledged_warnings,
    });
    if !validate_extension_integrity_override(&warnings, risk_override.as_ref())? {
        return Err(ControlPlaneError::Conflict(
            "agent_flow_extension_integrity_confirmation_required",
        )
        .into());
    }
    let resources = FlowService::new(state.store.clone())
        .load_agent_flow_template_resources(context.user.id)
        .await?;
    let imported = ApplicationArchiveService::new(state.store.clone())
        .import_archive(ImportApplicationArchiveCommand {
            actor_user_id: context.user.id,
            entry,
            name: body.name,
            description: body.description,
            resources,
            source_extension_installation_id: Some(installation_id),
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(
            to_import_response(&state, &locale, imported).await?,
        )),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/archive/preview",
    request_body(content = inline(ApplicationArchiveUploadBody), content_type = "multipart/form-data"),
    responses(
        (status = 200, body = AgentFlowTemplatePreviewResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody)
    )
)]
pub async fn preview_application_archive(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiSuccess<AgentFlowTemplatePreviewResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let (entry, _, _) = uploaded_application_archive_entry(&mut multipart).await?;
    let resources = FlowService::new(state.store.clone())
        .load_agent_flow_template_resources(context.user.id)
        .await?;
    let preview = ApplicationArchiveService::new(state.store.clone())
        .preview_archive(PreviewApplicationArchiveCommand {
            actor_user_id: context.user.id,
            entry,
            resources,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_template_preview_response(preview))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/archive/import",
    request_body(content = inline(ApplicationArchiveUploadBody), content_type = "multipart/form-data"),
    responses(
        (status = 201, body = ImportAgentFlowTemplateResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn import_application_archive(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<ImportAgentFlowTemplateResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    let locale =
        crate::app_state::request_catalog_locale(&headers, context.user.preferred_locale.clone());
    require_csrf(&headers, &context)?;
    let (entry, name, description) = uploaded_application_archive_entry(&mut multipart).await?;
    let resources = FlowService::new(state.store.clone())
        .load_agent_flow_template_resources(context.user.id)
        .await?;
    let imported = ApplicationArchiveService::new(state.store.clone())
        .import_archive(ImportApplicationArchiveCommand {
            actor_user_id: context.user.id,
            entry,
            name,
            description,
            resources,
            source_extension_installation_id: None,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(
            to_import_response(&state, &locale, imported).await?,
        )),
    ))
}

fn safe_archive_name(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if normalized.is_empty() {
        "application".to_string()
    } else {
        normalized
    }
}

fn build_application_archive_zip(package: &ApplicationArchivePackage) -> Result<Vec<u8>, ApiError> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut archive = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    archive.start_file("manifest.json", options)?;
    archive.write_all(&serde_json::to_vec_pretty(package)?)?;
    for (index, application) in package.applications.iter().enumerate() {
        let filename = format!(
            "applications/{:03}-{}.json",
            index + 1,
            safe_archive_name(&application.application.name)
        );
        archive.start_file(filename, options)?;
        archive.write_all(&serde_json::to_vec_pretty(application)?)?;
    }
    Ok(archive.finish()?.into_inner())
}

fn encode_content_disposition_filename(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[(byte >> 4) as usize]));
            encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

fn application_archive_content_disposition(filename: &str) -> Result<HeaderValue, ApiError> {
    let mut ascii_fallback = filename
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        .collect::<String>();
    if ascii_fallback.starts_with('.') {
        ascii_fallback.insert_str(0, "application");
    }
    HeaderValue::from_str(&format!(
        "attachment; filename=\"{ascii_fallback}\"; filename*=UTF-8''{}",
        encode_content_disposition_filename(filename)
    ))
    .map_err(|_| ControlPlaneError::InvalidInput("application_archive_filename").into())
}

#[utoipa::path(
    post,
    path = "/api/console/applications/archive/export",
    request_body = ExportApplicationArchiveBody,
    responses(
        (status = 200, description = "Single application JSON document or multiple application ZIP archive", content(
            (inline(crate::openapi::OpenApiBinaryBody) = "application/json"),
            (inline(crate::openapi::OpenApiBinaryBody) = "application/zip")
        )),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn export_application_archive(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ExportApplicationArchiveBody>,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    let exported_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| ControlPlaneError::InvalidInput("application_archive_exported_at"))?;
    let package = ApplicationArchiveService::new(state.store.clone())
        .export_archive(ExportApplicationArchiveCommand {
            actor_user_id: context.user.id,
            application_ids: body.application_ids,
            exported_from_system_version: env!("CARGO_PKG_VERSION").to_string(),
            exported_at,
        })
        .await?;
    let (content_type, filename, document) = match package.applications.as_slice() {
        [application] => (
            "application/json; charset=utf-8",
            format!(
                "{}.1flowbase-application.json",
                safe_archive_name(&application.application.name)
            ),
            serde_json::to_vec_pretty(&package)?,
        ),
        applications => {
            let filename = format!("applications-{}-items.zip", applications.len());
            let archive =
                tokio::task::spawn_blocking(move || build_application_archive_zip(&package))
                    .await
                    .map_err(|_| ControlPlaneError::InvalidInput("application_archive"))??;
            ("application/zip", filename, archive)
        }
    };
    let mut response = Response::new(Body::from(document));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        application_archive_content_disposition(&filename)?,
    );
    Ok(response)
}

fn collect_referenced_i18n_text_refs(document: &serde_json::Value) -> Vec<CompiledI18nTextRef> {
    let mut references = FlowCompiler::compile(
        Uuid::nil(),
        "i18n-projection",
        document,
        &FlowCompileContext::default(),
    )
    .map(|plan| referenced_i18n_text_refs(&plan))
    .unwrap_or_default()
    .into_iter()
    .collect::<BTreeSet<_>>();

    references.extend(
        document
            .pointer("/graph/nodes")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|node| node.get("bindings").and_then(serde_json::Value::as_object))
            .flat_map(|bindings| bindings.values())
            .filter(|binding| {
                binding.get("kind").and_then(serde_json::Value::as_str) == Some("i18n_text")
            })
            .filter_map(|binding| {
                let binding = binding.as_object()?;
                if binding.len() != 2
                    || !binding.contains_key("kind")
                    || !binding.contains_key("value")
                {
                    return None;
                }
                let value = binding.get("value")?.as_object()?;
                if value.len() != 1 || !value.contains_key("key") {
                    return None;
                }
                Some(CompiledI18nTextRef {
                    key: value.get("key")?.as_str()?.to_owned(),
                })
            })
            .collect::<BTreeSet<_>>(),
    );
    references.into_iter().collect()
}

async fn referenced_messages(
    api_state: &ApiState,
    locale: &domain::CatalogLocale,
    document: &serde_json::Value,
) -> Result<Vec<ReferencedI18nMessageResponse>, ApiError> {
    let resolver = CatalogResolver::new(api_state.store.clone(), api_state.bootstrap_workspace_id);
    let mut messages = Vec::new();

    for reference in collect_referenced_i18n_text_refs(document) {
        let Ok(identity) = CatalogMessageIdentity::new(reference.key.clone()) else {
            continue;
        };
        let text = resolver
            .resolve(api_state.bootstrap_workspace_id, &identity, locale)
            .await?
            .value;
        messages.push(ReferencedI18nMessageResponse {
            key: reference.key,
            text,
        });
    }

    Ok(messages)
}

async fn to_response(
    api_state: &ApiState,
    locale: &domain::CatalogLocale,
    state: domain::FlowEditorState,
) -> Result<OrchestrationStateResponse, ApiError> {
    let messages = referenced_messages(api_state, locale, &state.draft.document).await?;

    Ok(OrchestrationStateResponse {
        flow_id: state.flow.id.to_string(),
        draft: FlowDraftResponse {
            id: state.draft.id.to_string(),
            flow_id: state.draft.flow_id.to_string(),
            document: state.draft.document,
            updated_at: state.draft.updated_at.format(&Rfc3339).unwrap(),
        },
        messages,
        versions: state
            .versions
            .into_iter()
            .map(|version| FlowVersionResponse {
                id: version.id.to_string(),
                sequence: version.sequence,
                trigger: version.trigger.as_str().to_string(),
                change_kind: version.change_kind.as_str().to_string(),
                summary: version.summary,
                summary_is_custom: version.summary_is_custom,
                is_user_protected: version.is_user_protected,
                is_current_publication: version.is_current_publication,
                created_at: version.created_at.format(&Rfc3339).unwrap(),
            })
            .collect(),
        autosave_interval_seconds: state.autosave_interval_seconds,
        user_protection_limit: domain::FLOW_USER_PROTECTION_LIMIT,
    })
}

fn to_template_application_response(
    application: control_plane::flow::AgentFlowTemplateApplication,
) -> AgentFlowTemplateApplicationResponse {
    AgentFlowTemplateApplicationResponse {
        application_type: application.application_type,
        name: application.name,
        description: application.description,
        icon: application.icon,
        icon_type: application.icon_type,
        icon_background: application.icon_background,
    }
}

fn to_template_dependency_response(
    dependency: AgentFlowTemplateDependency,
) -> AgentFlowTemplateDependencyResponse {
    AgentFlowTemplateDependencyResponse {
        kind: dependency.kind,
        node_id: dependency.node_id,
        node_type: dependency.node_type,
        config_version: dependency.config_version,
        provider_code: dependency.provider_code,
        model_id: dependency.model_id,
        plugin_id: dependency.plugin_id,
        plugin_version: dependency.plugin_version,
        contribution_code: dependency.contribution_code,
        node_shell: dependency.node_shell,
        schema_version: dependency.schema_version,
        plugin_unique_identifier: dependency.plugin_unique_identifier,
        package_id: dependency.package_id,
        contribution_checksum: dependency.contribution_checksum,
        compiled_contribution_hash: dependency.compiled_contribution_hash,
    }
}

fn to_template_dependency_status_response(
    dependency: AgentFlowTemplateDependencyStatus,
) -> AgentFlowTemplateDependencyStatusResponse {
    AgentFlowTemplateDependencyStatusResponse {
        dependency: to_template_dependency_response(dependency.dependency),
        status: dependency.status,
        reason: dependency.reason,
    }
}

fn to_unresolved_node_response(
    unresolved_node: AgentFlowTemplateUnresolvedNode,
) -> AgentFlowTemplateUnresolvedNodeResponse {
    AgentFlowTemplateUnresolvedNodeResponse {
        node_id: unresolved_node.node_id,
        alias: unresolved_node.alias,
        original_type: unresolved_node.original_type,
        dependency_status: unresolved_node.dependency_status,
        reason: unresolved_node.reason,
        original_node: unresolved_node.original_node,
    }
}

fn to_template_preview_response(
    preview: AgentFlowTemplatePreview,
) -> AgentFlowTemplatePreviewResponse {
    AgentFlowTemplatePreviewResponse {
        schema_version: preview.schema_version,
        application: to_template_application_response(preview.application),
        dependencies: preview
            .dependencies
            .into_iter()
            .map(to_template_dependency_status_response)
            .collect(),
        unresolved_nodes: preview
            .unresolved_nodes
            .into_iter()
            .map(to_unresolved_node_response)
            .collect(),
        document: preview.document,
    }
}

async fn to_import_response(
    api_state: &ApiState,
    locale: &domain::CatalogLocale,
    imported: ImportAgentFlowTemplateResult,
) -> Result<ImportAgentFlowTemplateResponse, ApiError> {
    Ok(ImportAgentFlowTemplateResponse {
        application: AgentFlowTemplateImportedApplicationResponse {
            id: imported.application.id.to_string(),
            application_type: imported.application.application_type.as_str().to_string(),
            name: imported.application.name,
            description: imported.application.description,
            icon: imported.application.icon,
            icon_type: imported.application.icon_type,
            icon_background: imported.application.icon_background,
            created_by: imported.application.created_by.to_string(),
            updated_at: match imported.application.updated_at.format(&Rfc3339) {
                Ok(updated_at) => updated_at,
                Err(_) => imported.application.updated_at.to_string(),
            },
        },
        orchestration: to_response(api_state, locale, imported.orchestration).await?,
        preview: to_template_preview_response(imported.preview),
    })
}

fn parse_change_kind(value: &str) -> Result<domain::FlowChangeKind, ApiError> {
    match value {
        "layout" => Ok(domain::FlowChangeKind::Layout),
        "logical" => Ok(domain::FlowChangeKind::Logical),
        _ => Err(ControlPlaneError::InvalidInput("change_kind").into()),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, body = OrchestrationStateResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_orchestration(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<OrchestrationStateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let locale = crate::app_state::request_catalog_locale(&headers, context.user.preferred_locale);
    let flow_state = FlowService::new(state.store.clone())
        .get_or_create_editor_state(context.user.id, id)
        .await?;

    Ok(Json(ApiSuccess::new(
        to_response(&state, &locale, flow_state).await?,
    )))
}

#[utoipa::path(
    put,
    path = "/api/console/applications/{id}/orchestration/draft",
    request_body = SaveDraftBody,
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, body = OrchestrationStateResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn save_draft(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveDraftBody>,
) -> Result<Json<ApiSuccess<OrchestrationStateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let locale =
        crate::app_state::request_catalog_locale(&headers, context.user.preferred_locale.clone());
    require_csrf(&headers, &context)?;

    let flow_state = FlowService::new(state.store.clone())
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: context.user.id,
            application_id: id,
            document: body.document,
            change_kind: parse_change_kind(&body.change_kind)?,
            summary: body.summary,
        })
        .await?;

    Ok(Json(ApiSuccess::new(
        to_response(&state, &locale, flow_state).await?,
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/versions/{version_id}/restore",
    params(
        ("id" = String, Path, description = "Application id"),
        ("version_id" = String, Path, description = "Flow version id")
    ),
    responses(
        (status = 200, body = OrchestrationStateResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn restore_version(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, version_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<OrchestrationStateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let locale =
        crate::app_state::request_catalog_locale(&headers, context.user.preferred_locale.clone());
    require_csrf(&headers, &context)?;

    let flow_state = FlowService::new(state.store.clone())
        .restore_version(context.user.id, id, version_id)
        .await?;

    Ok(Json(ApiSuccess::new(
        to_response(&state, &locale, flow_state).await?,
    )))
}

#[utoipa::path(
    patch,
    path = "/api/console/applications/{id}/orchestration/versions/{version_id}",
    request_body = UpdateVersionBody,
    params(
        ("id" = String, Path, description = "Application id"),
        ("version_id" = String, Path, description = "Flow version id")
    ),
    responses(
        (status = 200, body = OrchestrationStateResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn update_version(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, version_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateVersionBody>,
) -> Result<Json<ApiSuccess<OrchestrationStateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let locale =
        crate::app_state::request_catalog_locale(&headers, context.user.preferred_locale.clone());
    require_csrf(&headers, &context)?;

    let flow_state = FlowService::new(state.store.clone())
        .update_version_metadata(UpdateFlowVersionMetadataCommand {
            actor_user_id: context.user.id,
            application_id: id,
            version_id,
            summary: body.summary,
            summary_is_custom: body.summary_is_custom,
            is_user_protected: body.is_user_protected,
        })
        .await?;

    Ok(Json(ApiSuccess::new(
        to_response(&state, &locale, flow_state).await?,
    )))
}
