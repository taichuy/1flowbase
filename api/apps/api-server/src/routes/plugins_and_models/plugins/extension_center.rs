use std::{collections::HashMap, sync::Arc};

use access_control::ConsoleRouteOwnership::ConsoleOperation;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    handler::Handler,
    http::{HeaderMap, StatusCode},
    middleware, Json,
};
use control_plane::plugin_management::{
    ExtensionCatalogCategory, InstallOfficialExtensionCommand, InstallUploadedPluginCommand,
    LocalExtensionInventoryEntry, PluginManagementService,
};
use serde::{Deserialize, Serialize};
use storage_durable::MainDurableStore;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    official_extension_catalog::OfficialExtensionCatalogEntry,
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

use super::{
    base_service, enforce_plugin_upload_limit, parse_uuid, read_upload_file,
    to_artifact_instance_response, to_compatibility_override, to_install_response,
    to_installation_response_with_artifact, to_risk_override, InstallPluginResponse,
    PluginArtifactInstanceResponse, PluginCompatibilityOverrideBody, PluginInstallationResponse,
    PluginRiskOverrideBody, PluginUploadMultipartBody, MAX_PLUGIN_UPLOAD_BYTES,
};

#[derive(Debug, Deserialize, IntoParams, Clone)]
pub struct LocalExtensionInventoryQuery {
    pub category: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct ExtensionRiskWarningResponse {
    pub code: String,
    pub message: String,
    pub overridable: bool,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct ExtensionCompatibilityWarningResponse {
    pub reason: String,
    pub current_host_version: String,
    pub minimum_host_version: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocalExtensionInventoryEntryResponse {
    pub category: String,
    pub artifact_kind: Option<String>,
    pub artifact_id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub current_version: String,
    pub system_requirements: Option<String>,
    pub installation_status: String,
    pub source: String,
    pub trust: String,
    pub warnings: Vec<ExtensionRiskWarningResponse>,
    pub installation: PluginInstallationResponse,
    pub local_artifact: PluginArtifactInstanceResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocalExtensionInventoryPageResponse {
    pub limit: usize,
    pub next_cursor: Option<String>,
    pub entries: Vec<LocalExtensionInventoryEntryResponse>,
}

#[derive(Debug, Deserialize, IntoParams, Clone)]
pub struct ExtensionCatalogGatewayQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct ExtensionCatalogGatewayEntryResponse {
    pub category: String,
    pub id: String,
    pub name: String,
    pub organization: String,
    pub artifact: String,
    pub version: String,
    pub description: String,
    pub host_version_requirement: String,
    #[schema(value_type = Object)]
    pub source: serde_json::Value,
    #[schema(value_type = Option<Object>)]
    pub signature: Option<serde_json::Value>,
    pub checksum: Option<String>,
    #[schema(value_type = Object)]
    pub download_locator: serde_json::Value,
    pub catalog_page: u32,
    pub catalog_source: String,
    pub current_version: Option<String>,
    pub installation_status: String,
    pub artifact_kind: Option<String>,
    pub installation_source: Option<String>,
    pub trust: String,
    pub warnings: Vec<ExtensionRiskWarningResponse>,
    pub compatibility: Option<ExtensionCompatibilityWarningResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExtensionCatalogGatewayPageResponse {
    pub category: String,
    pub catalog_page: String,
    pub catalog_page_number: u32,
    pub catalog_page_checksum: String,
    pub catalog_page_locator: String,
    pub limit: usize,
    pub next_cursor: Option<String>,
    pub total_entries: usize,
    pub entries: Vec<ExtensionCatalogGatewayEntryResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExtensionUpdateCheckItemBody {
    pub artifact_id: String,
    pub current_version: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExtensionUpdateCheckBody {
    pub category: String,
    pub catalog_page: Option<String>,
    pub items: Vec<ExtensionUpdateCheckItemBody>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExtensionUpdateCheckItemResponse {
    pub artifact_id: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExtensionUpdateCheckResponse {
    pub category: String,
    pub catalog_page: Option<String>,
    pub items: Vec<ExtensionUpdateCheckItemResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InstallOfficialExtensionBody {
    pub category: String,
    pub artifact_id: String,
    pub artifact_kind: Option<String>,
    pub compatibility_override: Option<PluginCompatibilityOverrideBody>,
    pub risk_override: Option<PluginRiskOverrideBody>,
}

pub(super) fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/settings/extension-center/installed",
            console_get(
                list_local_extension_inventory,
                ConsoleOperation("extension_center.installed.view".to_string()),
            ),
        )
        .route(
            "/settings/extension-center/catalog/:category",
            console_get(
                list_extension_catalog_gateway,
                ConsoleOperation("extension_center.catalog.view".to_string()),
            ),
        )
        .route(
            "/settings/extension-center/catalog/:category/:artifact_id",
            console_get(
                get_extension_catalog_entry,
                ConsoleOperation("extension_center.catalog.detail".to_string()),
            ),
        )
        .route(
            "/settings/extension-center/update-check",
            console_post(
                check_extension_catalog_page_updates,
                ConsoleOperation("extension_center.update_check".to_string()),
            ),
        )
        .route(
            "/settings/extension-center/install",
            console_post(
                install_official_extension,
                ConsoleOperation("extension_center.install".to_string()),
            ),
        )
        .route(
            "/settings/extension-center/update",
            console_post(
                update_official_extension,
                ConsoleOperation("extension_center.update".to_string()),
            ),
        )
        .route(
            "/settings/extension-center/install-upload",
            console_post(
                install_uploaded_extension
                    .layer(DefaultBodyLimit::max(MAX_PLUGIN_UPLOAD_BYTES))
                    .layer(middleware::from_fn(enforce_plugin_upload_limit)),
                ConsoleOperation("extension_center.install.upload".to_string()),
            ),
        )
}

fn service(
    state: &ApiState,
    operation_id: &'static str,
) -> PluginManagementService<MainDurableStore, ApiProviderRuntime> {
    base_service(state).for_extension_center_console_operation(operation_id)
}

fn to_local_inventory_entry(
    entry: LocalExtensionInventoryEntry,
) -> LocalExtensionInventoryEntryResponse {
    LocalExtensionInventoryEntryResponse {
        category: entry.category,
        artifact_kind: entry.artifact_kind,
        artifact_id: entry.artifact_id,
        display_name: entry.display_name,
        description: entry.description,
        current_version: entry.current_version,
        system_requirements: entry.system_requirements,
        installation_status: entry.installation_status,
        source: entry.source,
        trust: entry.trust,
        warnings: entry
            .warnings
            .into_iter()
            .map(|warning| ExtensionRiskWarningResponse {
                message: warning_message(&warning.code),
                code: warning.code,
                overridable: warning.overridable,
            })
            .collect(),
        installation: to_installation_response_with_artifact(
            entry.installation,
            Some(entry.local_artifact.clone()),
        ),
        local_artifact: to_artifact_instance_response(entry.local_artifact),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/settings/extension-center/installed",
    params(LocalExtensionInventoryQuery),
    operation_id = "extension_center_list_installed",
    responses((status = 200, body = LocalExtensionInventoryPageResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_local_extension_inventory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<LocalExtensionInventoryQuery>,
) -> Result<Json<ApiSuccess<LocalExtensionInventoryPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let category = query
        .category
        .as_deref()
        .map(ExtensionCatalogCategory::parse)
        .transpose()?;
    let cursor = query
        .cursor
        .as_deref()
        .map(|value| parse_uuid(value, "cursor"))
        .transpose()?;
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let page = service(&state, "extension_center.installed.view")
        .list_local_inventory(context.user.id, category, cursor, limit)
        .await?;
    Ok(Json(ApiSuccess::new(LocalExtensionInventoryPageResponse {
        limit: page.limit,
        next_cursor: page.next_cursor,
        entries: page
            .entries
            .into_iter()
            .map(to_local_inventory_entry)
            .collect(),
    })))
}

#[derive(Debug, Clone)]
struct InstalledCatalogJoin {
    current_version: String,
    artifact_kind: Option<String>,
    source: String,
    trust: String,
}

async fn installed_catalog_joins(
    state: &ApiState,
    actor_user_id: Uuid,
    category: ExtensionCatalogCategory,
) -> Result<HashMap<String, InstalledCatalogJoin>, ApiError> {
    let mut cursor = None;
    let mut joins = HashMap::new();
    loop {
        let page = service(state, "extension_center.catalog.view")
            .list_local_inventory(actor_user_id, Some(category), cursor, 50)
            .await?;
        for entry in page.entries {
            joins.insert(
                entry.artifact_id,
                InstalledCatalogJoin {
                    current_version: entry.current_version,
                    artifact_kind: entry.artifact_kind,
                    source: entry.source,
                    trust: entry.trust,
                },
            );
        }
        let Some(next_cursor) = page.next_cursor else {
            break;
        };
        cursor = Some(parse_uuid(&next_cursor, "extension_inventory_cursor")?);
    }
    Ok(joins)
}

fn catalog_entry_join<'a>(
    entry: &OfficialExtensionCatalogEntry,
    installed: &'a HashMap<String, InstalledCatalogJoin>,
) -> Option<&'a InstalledCatalogJoin> {
    installed
        .get(&entry.id)
        .or_else(|| {
            entry
                .source
                .metadata
                .get("plugin_id")
                .and_then(serde_json::Value::as_str)
                .and_then(|plugin_id| installed.get(plugin_id))
        })
        .or_else(|| installed.get(&format!("{}.{}", entry.organization, entry.artifact)))
}

fn warning_message(code: &str) -> String {
    match code {
        "signature_missing" => "The catalog entry has no signature metadata.",
        "signing_key_unknown" => "The catalog entry references an unknown signing key.",
        "checksum_missing" => "The catalog entry has no artifact checksum.",
        "below_minimum_host_version" => "The extension requires a newer 1flowbase host version.",
        _ => "The extension requires confirmation before installation.",
    }
    .to_string()
}

fn catalog_entry_warnings(
    entry: &OfficialExtensionCatalogEntry,
    trusted_key_ids: &[String],
) -> Vec<ExtensionRiskWarningResponse> {
    let mut warnings = Vec::new();
    if entry.signature.is_none() {
        warnings.push("signature_missing");
    } else if entry.signing_key_id().map_or(true, |key_id| {
        !trusted_key_ids.iter().any(|trusted| trusted == key_id)
    }) {
        warnings.push("signing_key_unknown");
    }
    if entry.checksum.is_none() {
        warnings.push("checksum_missing");
    }
    if catalog_entry_compatibility(entry).is_some() {
        warnings.push("below_minimum_host_version");
    }
    warnings
        .into_iter()
        .map(|code| ExtensionRiskWarningResponse {
            code: code.to_string(),
            message: warning_message(code),
            overridable: true,
        })
        .collect()
}

fn current_host_version() -> &'static str {
    option_env!("FLOWBASE_API_SERVER_VERSION")
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
}

fn catalog_entry_compatibility(
    entry: &OfficialExtensionCatalogEntry,
) -> Option<ExtensionCompatibilityWarningResponse> {
    if host_version_requirement_is_satisfied(
        current_host_version(),
        &entry.host_version_requirement,
    ) {
        return None;
    }
    let minimum_host_version = entry
        .host_version_requirement
        .trim()
        .strip_prefix(">=")?
        .trim()
        .to_string();
    Some(ExtensionCompatibilityWarningResponse {
        reason: "below_minimum_host_version".to_string(),
        current_host_version: current_host_version().to_string(),
        minimum_host_version,
    })
}

fn host_version_requirement_is_satisfied(current: &str, requirement: &str) -> bool {
    let requirement = requirement.trim();
    if requirement == "*" || requirement.is_empty() {
        return true;
    }
    let Some(minimum) = requirement.strip_prefix(">=").map(str::trim) else {
        // Unknown requirement syntax remains visible in the original catalog field. It must not
        // be mislabeled as a below-minimum result without a comparable minimum version.
        return true;
    };
    let parse = |value: &str| {
        let mut parts = value.trim().trim_start_matches('v').split('.');
        Some((
            parts.next()?.parse::<u64>().ok()?,
            parts.next()?.parse::<u64>().ok()?,
            parts.next()?.split('-').next()?.parse::<u64>().ok()?,
        ))
    };
    match (parse(current), parse(minimum)) {
        (Some(current), Some(minimum)) => current >= minimum,
        _ => true,
    }
}

fn project_catalog_entry(
    entry: OfficialExtensionCatalogEntry,
    catalog_source: &str,
    installed: &HashMap<String, InstalledCatalogJoin>,
    trusted_key_ids: &[String],
) -> ExtensionCatalogGatewayEntryResponse {
    let installation = catalog_entry_join(&entry, installed);
    let catalog_trust = match (
        catalog_source,
        entry
            .signing_key_id()
            .is_some_and(|key_id| trusted_key_ids.iter().any(|trusted| trusted == key_id)),
    ) {
        ("official", true) => "official",
        (_, true) => "trusted",
        _ => "unknown",
    };
    let source =
        serde_json::to_value(&entry.source).expect("typed extension catalog source must serialize");
    let warnings = catalog_entry_warnings(&entry, trusted_key_ids);
    let compatibility = catalog_entry_compatibility(&entry);
    ExtensionCatalogGatewayEntryResponse {
        category: entry.category,
        id: entry.id,
        name: entry.name,
        organization: entry.organization,
        artifact: entry.artifact,
        version: entry.version,
        description: entry.description,
        host_version_requirement: entry.host_version_requirement,
        source,
        signature: entry.signature,
        checksum: entry.checksum,
        download_locator: entry.download_locator,
        catalog_page: entry.catalog_page,
        catalog_source: catalog_source.to_string(),
        current_version: installation.map(|value| value.current_version.clone()),
        installation_status: if installation.is_some() {
            "installed".to_string()
        } else {
            "not_installed".to_string()
        },
        artifact_kind: installation.and_then(|value| value.artifact_kind.clone()),
        installation_source: installation.map(|value| value.source.clone()),
        trust: installation
            .map(|value| value.trust.clone())
            .unwrap_or_else(|| catalog_trust.to_string()),
        warnings,
        compatibility,
    }
}

async fn load_catalog_page(
    state: &ApiState,
    actor_user_id: Uuid,
    category: ExtensionCatalogCategory,
    cursor: Option<String>,
) -> Result<ExtensionCatalogGatewayPageResponse, ApiError> {
    let page = state
        .official_extension_catalog_source
        .list_page(category.as_str(), cursor.as_deref())
        .await?;
    let installed = installed_catalog_joins(state, actor_user_id, category).await?;
    let trusted_key_ids = state
        .official_plugin_source
        .trusted_public_keys()
        .iter()
        .map(|key| key.key_id.clone())
        .collect::<Vec<_>>();
    let catalog_source = match page.source_kind.as_str() {
        "official_repository" => "official",
        _ => "mirror",
    };
    let entries = page
        .entries
        .into_iter()
        .map(|entry| project_catalog_entry(entry, catalog_source, &installed, &trusted_key_ids))
        .collect();
    Ok(ExtensionCatalogGatewayPageResponse {
        category: page.category,
        catalog_page: page.metadata.cursor,
        catalog_page_number: page.metadata.page,
        catalog_page_checksum: page.metadata.checksum,
        catalog_page_locator: page.metadata.locator,
        limit: page.metadata.page_size,
        next_cursor: page.metadata.next_cursor,
        total_entries: page.metadata.total_entries,
        entries,
    })
}

#[utoipa::path(
    get,
    path = "/api/console/settings/extension-center/catalog/{category}",
    params(ExtensionCatalogGatewayQuery),
    operation_id = "extension_center_list_catalog",
    responses((status = 200, body = ExtensionCatalogGatewayPageResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_extension_catalog_gateway(
    State(state): State<Arc<ApiState>>,
    Path(category): Path<String>,
    headers: HeaderMap,
    Query(query): Query<ExtensionCatalogGatewayQuery>,
) -> Result<Json<ApiSuccess<ExtensionCatalogGatewayPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let category = ExtensionCatalogCategory::parse(&category)?;
    let page = load_catalog_page(&state, context.user.id, category, query.cursor).await?;
    Ok(Json(ApiSuccess::new(page)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/extension-center/catalog/{category}/{artifact_id}",
    params(ExtensionCatalogGatewayQuery),
    operation_id = "extension_center_get_catalog_entry",
    responses((status = 200, body = ExtensionCatalogGatewayEntryResponse), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn get_extension_catalog_entry(
    State(state): State<Arc<ApiState>>,
    Path((category, artifact_id)): Path<(String, String)>,
    headers: HeaderMap,
    Query(_query): Query<ExtensionCatalogGatewayQuery>,
) -> Result<Json<ApiSuccess<ExtensionCatalogGatewayEntryResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let category = ExtensionCatalogCategory::parse(&category)?;
    let located = state
        .official_extension_catalog_source
        .find_entry(category.as_str(), &artifact_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_catalog_entry",
        ))?;
    let installed = installed_catalog_joins(&state, context.user.id, category).await?;
    let trusted_key_ids = state
        .official_plugin_source
        .trusted_public_keys()
        .iter()
        .map(|key| key.key_id.clone())
        .collect::<Vec<_>>();
    let catalog_source = if located.source_kind == "official_repository" {
        "official"
    } else {
        "mirror"
    };
    Ok(Json(ApiSuccess::new(project_catalog_entry(
        located.entry,
        catalog_source,
        &installed,
        &trusted_key_ids,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/update-check",
    operation_id = "extension_center_check_current_page_updates",
    request_body = ExtensionUpdateCheckBody,
    responses((status = 200, body = ExtensionUpdateCheckResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn check_extension_catalog_page_updates(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ExtensionUpdateCheckBody>,
) -> Result<Json<ApiSuccess<ExtensionUpdateCheckResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    if body.items.len() > 50 {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_update_check_page",
        )
        .into());
    }
    let category = ExtensionCatalogCategory::parse(&body.category)?;
    let page =
        load_catalog_page(&state, context.user.id, category, body.catalog_page.clone()).await?;
    let latest = page
        .entries
        .into_iter()
        .map(|entry| (entry.id, entry.version))
        .collect::<std::collections::HashMap<_, _>>();
    let items = body
        .items
        .into_iter()
        .map(|item| {
            let latest_version = latest.get(&item.artifact_id).cloned();
            let status = match latest_version.as_deref() {
                Some(version) if version == item.current_version => "current",
                Some(_) => "update_available",
                None => "unknown_error",
            };
            ExtensionUpdateCheckItemResponse {
                artifact_id: item.artifact_id,
                current_version: item.current_version,
                latest_version,
                status: status.to_string(),
            }
        })
        .collect();
    Ok(Json(ApiSuccess::new(ExtensionUpdateCheckResponse {
        category: body.category,
        catalog_page: body.catalog_page,
        items,
    })))
}

async fn install_or_update_official_extension(
    state: &ApiState,
    headers: &HeaderMap,
    body: InstallOfficialExtensionBody,
    operation_id: &'static str,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    let context = require_session(state, headers).await?;
    require_csrf(headers, &context)?;
    let category = ExtensionCatalogCategory::parse(&body.category)?;
    let expected_plugin_type = body
        .artifact_kind
        .as_deref()
        .or_else(|| category.fixed_plugin_type());
    if !category.application().installs_node_artifact || expected_plugin_type.is_none() {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_requires_domain_application",
        )
        .into());
    }
    let result = service(state, operation_id)
        .install_official_extension(InstallOfficialExtensionCommand {
            actor_user_id: context.user.id,
            artifact_id: body.artifact_id,
            expected_plugin_type: expected_plugin_type
                .expect("installable categories require an artifact kind")
                .to_string(),
            compatibility_override: to_compatibility_override(body.compatibility_override),
            risk_override: to_risk_override(body.risk_override),
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_install_response(result))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/install",
    operation_id = "extension_center_install",
    request_body = InstallOfficialExtensionBody,
    responses((status = 201, body = InstallPluginResponse), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn install_official_extension(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InstallOfficialExtensionBody>,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    install_or_update_official_extension(&state, &headers, body, "extension_center.install").await
}

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/update",
    operation_id = "extension_center_update",
    request_body = InstallOfficialExtensionBody,
    responses((status = 201, body = InstallPluginResponse), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn update_official_extension(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InstallOfficialExtensionBody>,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    install_or_update_official_extension(&state, &headers, body, "extension_center.update").await
}

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/install-upload",
    operation_id = "extension_center_install_upload",
    request_body(content = inline(PluginUploadMultipartBody), content_type = "multipart/form-data"),
    responses((status = 201, body = InstallPluginResponse), (status = 400, body = crate::error_response::ErrorBody))
)]
pub async fn install_uploaded_extension(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let (file_name, package_bytes) = read_upload_file(&mut multipart).await?;
    let result = service(&state, "extension_center.install.upload")
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: context.user.id,
            file_name,
            package_bytes,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_install_response(result))),
    ))
}

#[cfg(test)]
mod _tests;
