use std::sync::Arc;

use access_control::ConsoleRouteOwnership::ConsoleOperation;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    handler::Handler,
    http::{HeaderMap, StatusCode},
    middleware, Json,
};
use control_plane::plugin_management::{
    ExtensionCatalogCategory, InstallOfficialExtensionCommand, InstallUploadedPluginCommand,
    LocalExtensionInventoryEntry, OfficialPluginCatalogFilter, PluginManagementService,
};
use serde::{Deserialize, Serialize};
use storage_durable::MainDurableStore;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

use super::{
    base_service, enforce_plugin_upload_limit, parse_uuid, read_upload_file, requested_locales,
    resolve_locale_meta, to_artifact_instance_response, to_compatibility_override,
    to_install_response, to_installation_response_with_artifact, to_risk_override,
    InstallPluginResponse, PluginArtifactInstanceResponse, PluginCompatibilityOverrideBody,
    PluginInstallationResponse, PluginRiskOverrideBody, PluginUploadMultipartBody,
    MAX_PLUGIN_UPLOAD_BYTES,
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
    pub overridable: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocalExtensionInventoryEntryResponse {
    pub category: String,
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
    pub limit: Option<usize>,
    pub locale: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct ExtensionCatalogGatewayEntryResponse {
    pub category: String,
    pub artifact_id: String,
    pub organization: String,
    pub display_name: String,
    pub latest_version: String,
    pub minimum_host_version: Option<String>,
    pub source: String,
    pub trust: String,
    pub warnings: Vec<ExtensionRiskWarningResponse>,
    #[schema(value_type = Object)]
    pub metadata_json: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExtensionCatalogGatewayPageResponse {
    pub category: String,
    pub catalog_page: Option<String>,
    pub limit: usize,
    pub next_cursor: Option<String>,
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
        source: entry.source,
        trust: entry.trust,
        warnings: entry
            .warnings
            .into_iter()
            .map(|warning| ExtensionRiskWarningResponse {
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

async fn load_catalog_page(
    state: &ApiState,
    actor_user_id: Uuid,
    category: ExtensionCatalogCategory,
    cursor: Option<String>,
    limit: usize,
    locales: control_plane::i18n::RequestedLocales,
) -> Result<ExtensionCatalogGatewayPageResponse, ApiError> {
    if let Some(plugin_type) = category.plugin_type() {
        let view = service(state, "extension_center.catalog.view")
            .list_official_catalog(
                actor_user_id,
                OfficialPluginCatalogFilter {
                    plugin_type: Some(plugin_type.to_string()),
                    search_query: None,
                    cursor: cursor.clone(),
                    limit,
                },
                locales,
            )
            .await?;
        let trusted_keys = state.official_plugin_source.trusted_public_keys();
        let source = match view.source_kind.as_str() {
            "mirror_registry" => "mirror",
            _ => "official_registry",
        };
        let entries = view
            .entries
            .into_iter()
            .map(|entry| {
                let key_is_trusted = entry
                    .selected_artifact
                    .signing_key_id
                    .as_deref()
                    .is_some_and(|key_id| trusted_keys.iter().any(|key| key.key_id == key_id));
                let trust = if key_is_trusted && source == "official_registry" {
                    "official"
                } else if key_is_trusted {
                    "trusted"
                } else {
                    "unknown"
                };
                let warnings = entry
                    .compatibility_warning_reason
                    .iter()
                    .map(|code| ExtensionRiskWarningResponse {
                        code: code.clone(),
                        overridable: true,
                    })
                    .collect();
                ExtensionCatalogGatewayEntryResponse {
                    category: category.as_str().to_string(),
                    organization: entry
                        .plugin_id
                        .split('.')
                        .next()
                        .unwrap_or("unknown")
                        .to_string(),
                    artifact_id: entry.plugin_id,
                    display_name: entry.display_name,
                    latest_version: entry.latest_version,
                    minimum_host_version: Some(entry.minimum_host_version),
                    source: source.to_string(),
                    trust: trust.to_string(),
                    warnings,
                    metadata_json: serde_json::json!({
                        "provider_code": entry.provider_code,
                        "protocol": entry.protocol,
                        "icon": entry.icon,
                        "help_url": entry.help_url,
                        "model_discovery_mode": entry.model_discovery_mode,
                        "install_status": entry.install_status.as_str(),
                    }),
                }
            })
            .collect();
        return Ok(ExtensionCatalogGatewayPageResponse {
            category: category.as_str().to_string(),
            catalog_page: cursor,
            limit: view.page.limit,
            next_cursor: view.page.next_cursor,
            entries,
        });
    }

    match category {
        ExtensionCatalogCategory::McpBundle => {
            let snapshot = state.official_mcp_bundle_source.list_catalog().await?;
            let start = cursor
                .as_deref()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            let end = start.saturating_add(limit).min(snapshot.entries.len());
            let entries = snapshot.entries[start..end]
                .iter()
                .map(|entry| ExtensionCatalogGatewayEntryResponse {
                    category: category.as_str().to_string(),
                    artifact_id: format!("{}.{}", entry.organization, entry.bundle_id),
                    organization: entry.organization.clone(),
                    display_name: entry.bundle_id.clone(),
                    latest_version: entry.latest_version.clone(),
                    minimum_host_version: Some(entry.minimum_host_version.clone()),
                    source: match snapshot.source.source_kind.as_str() {
                        "mirror_registry" => "mirror",
                        _ => "official_registry",
                    }
                    .to_string(),
                    trust: "unknown".to_string(),
                    warnings: Vec::new(),
                    metadata_json: serde_json::json!({
                        "locale": entry.locale,
                        "exported_from_system_version": entry.exported_from_system_version,
                        "artifact_sha256": entry.artifact_sha256,
                    }),
                })
                .collect();
            Ok(ExtensionCatalogGatewayPageResponse {
                category: category.as_str().to_string(),
                catalog_page: cursor,
                limit,
                next_cursor: (end < snapshot.entries.len()).then(|| end.to_string()),
                entries,
            })
        }
        ExtensionCatalogCategory::AgentFlowTemplate => {
            let snapshot = state
                .official_agent_flow_template_source
                .list_catalog_page(cursor.clone())
                .await?;
            let entries = snapshot
                .entries
                .into_iter()
                .map(|entry| ExtensionCatalogGatewayEntryResponse {
                    category: category.as_str().to_string(),
                    artifact_id: entry.workflow_id.clone(),
                    organization: "taichuy".to_string(),
                    display_name: entry.application.name.clone(),
                    latest_version: entry.schema_version,
                    minimum_host_version: None,
                    source: match snapshot.source.source_kind.as_str() {
                        "mirror_registry" => "mirror",
                        _ => "official_registry",
                    }
                    .to_string(),
                    trust: "unknown".to_string(),
                    warnings: Vec::new(),
                    metadata_json: serde_json::json!({
                        "application": entry.application,
                        "template_sha256": entry.template_sha256,
                        "updated_at": entry.updated_at,
                    }),
                })
                .collect();
            Ok(ExtensionCatalogGatewayPageResponse {
                category: category.as_str().to_string(),
                catalog_page: cursor,
                limit: snapshot.page.page_size,
                next_cursor: snapshot.page.next_cursor,
                entries,
            })
        }
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_catalog_category",
        )
        .into()),
    }
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
    let locale_meta = resolve_locale_meta(&headers, query.locale, context.user.preferred_locale);
    let category = ExtensionCatalogCategory::parse(&category)?;
    let page = load_catalog_page(
        &state,
        context.user.id,
        category,
        query.cursor,
        query.limit.unwrap_or(20).clamp(1, 50),
        requested_locales(&locale_meta),
    )
    .await?;
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
    Query(query): Query<ExtensionCatalogGatewayQuery>,
) -> Result<Json<ApiSuccess<ExtensionCatalogGatewayEntryResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(&headers, query.locale, context.user.preferred_locale);
    let category = ExtensionCatalogCategory::parse(&category)?;
    let page = load_catalog_page(
        &state,
        context.user.id,
        category,
        query.cursor,
        query.limit.unwrap_or(50).clamp(1, 50),
        requested_locales(&locale_meta),
    )
    .await?;
    let entry = page
        .entries
        .into_iter()
        .find(|entry| entry.artifact_id == artifact_id)
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_catalog_entry",
        ))?;
    Ok(Json(ApiSuccess::new(entry)))
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
    let page = load_catalog_page(
        &state,
        context.user.id,
        category,
        body.catalog_page.clone(),
        50,
        control_plane::i18n::RequestedLocales::new("en_US", "en_US"),
    )
    .await?;
    let latest = page
        .entries
        .into_iter()
        .map(|entry| (entry.artifact_id, entry.latest_version))
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
    if category.plugin_type().is_none() {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_requires_domain_application",
        )
        .into());
    }
    let result = service(state, operation_id)
        .install_official_extension(InstallOfficialExtensionCommand {
            actor_user_id: context.user.id,
            artifact_id: body.artifact_id,
            expected_plugin_type: category
                .plugin_type()
                .expect("non-installable categories returned above")
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
