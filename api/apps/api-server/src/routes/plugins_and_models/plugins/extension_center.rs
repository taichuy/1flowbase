use std::{
    collections::{BTreeSet, HashMap},
    io::{Cursor, Read},
    sync::Arc,
};

use access_control::ConsoleRouteOwnership::ConsoleOperation;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    handler::Handler,
    http::{HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    Json,
};
use control_plane::plugin_management::{
    group_installed_extension_families, ExtensionArtifactInstallOutcome, ExtensionCatalogCategory,
    ExtensionInstallationService, ExtensionRiskOverride, InstallExtensionArtifactCommand,
    InstallExtensionNodePluginCommand, InstalledExtensionFamily, PluginManagementService,
    SwitchPluginVersionCommand,
};
use plugin_framework::{intake_package_bytes, PackageIntakePolicy, PluginConsumptionKind};
use storage_durable::MainDurableStore;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    official_extension_catalog::{
        LocatedOfficialExtensionCatalogEntry, OfficialExtensionArtifactDescriptor,
        OfficialExtensionCatalogEntry, OfficialExtensionCatalogFreshness,
        OfficialExtensionCatalogSearchQuery,
    },
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_post, ConsoleRouteAssembly,
    },
};

use super::{
    base_service, enforce_plugin_upload_limit, format_time, PluginCompatibilityOverrideBody,
    PluginRiskOverrideBody, MAX_PLUGIN_UPLOAD_BYTES,
};

mod dto;

pub use dto::*;

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
            "/settings/extension-center/installed/:installation_id/select",
            console_post(
                select_local_extension_installation,
                ConsoleOperation("extension_center.installed.select".to_string()),
            ),
        )
        .route(
            "/settings/extension-center/installed/:installation_id",
            console_delete(
                delete_local_extension_installation,
                ConsoleOperation("extension_center.installed.delete".to_string()),
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
            "/settings/extension-center/catalog/:category/:catalog_id",
            console_get(
                get_extension_catalog_entry,
                ConsoleOperation("extension_center.catalog.detail".to_string()),
            ),
        )
        .route(
            "/settings/extension-center/update-check",
            console_post(
                check_extension_catalog_updates,
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

fn extension_installation_service(
    state: &ApiState,
) -> ExtensionInstallationService<MainDurableStore> {
    ExtensionInstallationService::new(state.store.clone(), &state.provider_install_root)
}

fn to_risk_warnings(
    warnings: &[domain::ExtensionIntegrityWarning],
) -> Vec<ExtensionRiskWarningResponse> {
    warnings
        .iter()
        .map(|warning| ExtensionRiskWarningResponse {
            message: warning.message.clone(),
            code: warning.code.clone(),
            overridable: warning.overridable,
        })
        .collect()
}

fn to_installed_version(
    entry: &domain::ExtensionInstallationRecord,
) -> LocalExtensionInstalledVersionResponse {
    LocalExtensionInstalledVersionResponse {
        id: entry.id.to_string(),
        version: entry.identity.version.clone(),
        source_kind: entry.source_kind.clone(),
        trust_level: entry.trust_level.clone(),
        warnings: to_risk_warnings(&entry.warnings),
        local_path: entry.local_path.clone(),
        expected_checksum: entry.expected_checksum.clone(),
        local_checksum: entry.local_checksum.clone(),
        signature_status: entry.signature_status.as_str().to_string(),
        signature_algorithm: entry.signature_algorithm.clone(),
        signing_key_id: entry.signing_key_id.clone(),
        status: entry.status.as_str().to_string(),
        is_current: entry.is_current,
        deletable: false,
        delete_reasons: Vec::new(),
        created_by: entry.created_by.to_string(),
        created_at: format_time(entry.created_at),
        updated_at: format_time(entry.updated_at),
    }
}

fn to_local_inventory_family(
    entry: domain::ExtensionInstallationRecord,
    installed_versions: Vec<domain::ExtensionInstallationRecord>,
) -> LocalExtensionInventoryEntryResponse {
    LocalExtensionInventoryEntryResponse {
        id: entry.id.to_string(),
        catalog_id: entry.identity.catalog_id(),
        category: entry.identity.category.as_str().to_string(),
        organization: entry.identity.organization,
        artifact_id: entry.identity.artifact_id,
        version: entry.identity.version,
        node_id: entry.node_id,
        source_kind: entry.source_kind,
        trust_level: entry.trust_level,
        warnings: to_risk_warnings(&entry.warnings),
        local_path: entry.local_path,
        expected_checksum: entry.expected_checksum,
        local_checksum: entry.local_checksum,
        signature_status: entry.signature_status.as_str().to_string(),
        signature_algorithm: entry.signature_algorithm,
        signing_key_id: entry.signing_key_id,
        status: entry.status.as_str().to_string(),
        is_current: entry.is_current,
        application_action: entry.application_action.as_str().to_string(),
        application_status: default_application_status(entry.application_action).to_string(),
        created_by: entry.created_by.to_string(),
        created_at: format_time(entry.created_at),
        updated_at: format_time(entry.updated_at),
        installed_versions: installed_versions
            .iter()
            .map(to_installed_version)
            .collect(),
    }
}

fn to_local_inventory_entry(
    entry: domain::ExtensionInstallationRecord,
) -> LocalExtensionInventoryEntryResponse {
    to_local_inventory_family(entry.clone(), vec![entry])
}

fn to_local_inventory_family_entry(
    family: InstalledExtensionFamily,
) -> LocalExtensionInventoryEntryResponse {
    to_local_inventory_family(family.current, family.installed_versions)
}

fn paginate_installed_families(
    families: Vec<InstalledExtensionFamily>,
    cursor: Option<&str>,
    limit: usize,
) -> (usize, Option<String>, Vec<InstalledExtensionFamily>) {
    let start = cursor
        .and_then(|cursor| {
            families
                .iter()
                .position(|family| family.catalog_id() == cursor)
        })
        .map_or(0, |index| index.saturating_add(1));
    let total_entries = families.len();
    let page_entries = families
        .into_iter()
        .skip(start)
        .take(limit)
        .collect::<Vec<_>>();
    let next_cursor = (start.saturating_add(page_entries.len()) < total_entries)
        .then(|| {
            page_entries
                .last()
                .map(InstalledExtensionFamily::catalog_id)
        })
        .flatten();
    (total_entries, next_cursor, page_entries)
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
        .map(|value| {
            domain::ExtensionCategory::parse(value).ok_or(
                control_plane::errors::ControlPlaneError::InvalidInput(
                    "extension_catalog_category",
                ),
            )
        })
        .transpose()?;
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let mut families = extension_installation_service(&state)
        .list_installed_families_for_node(&state.api_node_id)
        .await?;
    if let Some(category) = category {
        families.retain(|family| family.current.identity.category == category);
    }
    let (total_entries, next_cursor, page_entries) =
        paginate_installed_families(families, query.cursor.as_deref(), limit);
    let mut entries = Vec::with_capacity(page_entries.len());
    for family in page_entries {
        let status = workspace_application_status(
            &state,
            context.actor.current_workspace_id,
            &family.current,
        )
        .await?;
        let mut response = to_local_inventory_family_entry(family);
        for version in &mut response.installed_versions {
            if let Some(decision) =
                control_plane::ports::ExtensionInstallationRepository::extension_deletion_decision(
                    &state.store,
                    &state.api_node_id,
                    Uuid::parse_str(&version.id).map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput(
                            "extension_installation_id",
                        )
                    })?,
                )
                .await?
            {
                version.deletable = decision.deletable;
                version.delete_reasons = decision.reasons;
            }
        }
        response.application_status = status.to_string();
        entries.push(response);
    }
    Ok(Json(ApiSuccess::new(LocalExtensionInventoryPageResponse {
        limit,
        total_entries,
        next_cursor,
        entries,
    })))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/installed/{installation_id}/select",
    operation_id = "extension_center_select_installed_version",
    summary = "Select an installed extension version",
    description = "Selects one locally installed version as the database current version for its extension family.",
    responses((status = 200, body = LocalExtensionInventoryEntryResponse), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn select_local_extension_installation(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<LocalExtensionInventoryEntryResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let installation = extension_installation_service(&state)
        .select_current_installation(&state.api_node_id, installation_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_installation",
        ))?;
    Ok(Json(ApiSuccess::new(to_local_inventory_entry(
        installation,
    ))))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/extension-center/installed/{installation_id}",
    operation_id = "extension_center_delete_installed_version",
    summary = "Delete an installed extension version",
    description = "Deletes one non-current, unreferenced local artifact. Current, reserved, assigned, active, or referenced versions return a conflict.",
    responses((status = 200, body = LocalExtensionInventoryEntryResponse), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn delete_local_extension_installation(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<LocalExtensionInventoryEntryResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let installation_service = extension_installation_service(&state);
    let existing = installation_service
        .find_local_installation_by_id(&state.api_node_id, installation_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_installation",
        ))?;
    let installation = if existing.identity.category == domain::ExtensionCategory::Mcp {
        state
            .official_mcp_bundle_source
            .delete_local_version(
                &existing.identity.organization,
                &existing.identity.artifact_id,
                &existing.identity.version,
            )
            .await?;
        domain::ExtensionInstallationRecord {
            status: domain::ExtensionInstallationStatus::Missing,
            is_current: false,
            ..existing
        }
    } else {
        installation_service
            .delete_local_installation(&state.api_node_id, installation_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "extension_installation",
            ))?
    };
    Ok(Json(ApiSuccess::new(to_local_inventory_entry(
        installation,
    ))))
}

#[derive(Debug, Clone)]
struct InstalledCatalogJoin {
    current_version: String,
    source: String,
    trust: String,
}

async fn installed_catalog_joins(
    state: &ApiState,
    category: ExtensionCatalogCategory,
) -> Result<HashMap<String, InstalledCatalogJoin>, ApiError> {
    let records = extension_installation_service(state)
        .list_installed_for_node(&state.api_node_id)
        .await?;
    Ok(project_installed_catalog_joins(records, category))
}

fn project_installed_catalog_joins(
    records: impl IntoIterator<Item = domain::ExtensionInstallationRecord>,
    category: ExtensionCatalogCategory,
) -> HashMap<String, InstalledCatalogJoin> {
    let mut joins = HashMap::new();
    for family in group_installed_extension_families(records) {
        let entry = family.current;
        if entry.identity.category.as_str() != category.as_str() {
            continue;
        }
        joins.insert(
            entry.identity.catalog_id(),
            InstalledCatalogJoin {
                current_version: entry.identity.version,
                source: entry.source_kind,
                trust: entry.trust_level,
            },
        );
    }
    joins
}

fn catalog_entry_join<'a>(
    entry: &OfficialExtensionCatalogEntry,
    installed: &'a HashMap<String, InstalledCatalogJoin>,
) -> Option<&'a InstalledCatalogJoin> {
    installed.get(&entry.id)
}

fn extension_update_status(
    latest_version: Option<&str>,
    installed_versions: &[String],
) -> &'static str {
    match latest_version {
        Some(version) if installed_versions.iter().any(|local| local == version) => "current",
        Some(_) => "update_available",
        None => "unknown_error",
    }
}

async fn find_catalog_entry_for_requested_identity(
    state: &ApiState,
    category: ExtensionCatalogCategory,
    catalog_id: &str,
) -> Result<Option<LocatedOfficialExtensionCatalogEntry>, ApiError> {
    state
        .official_extension_catalog_source
        .find_entry(category.as_str(), catalog_id)
        .await
        .map_err(Into::into)
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
    compatibility_for_requirement(&entry.host_version_requirement)
}

fn compatibility_for_requirement(
    host_version_requirement: &str,
) -> Option<ExtensionCompatibilityWarningResponse> {
    if host_version_requirement_is_satisfied(current_host_version(), host_version_requirement) {
        return None;
    }
    let minimum_host_version = host_version_requirement
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
        slot_codes: entry.slot_codes,
        keywords: entry.keywords,
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
        artifact_kind: None,
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
    category: ExtensionCatalogCategory,
    query: ExtensionCatalogGatewayQuery,
) -> Result<ExtensionCatalogGatewayPageResponse, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let page = state
        .official_extension_catalog_source
        .search(
            category.as_str(),
            OfficialExtensionCatalogSearchQuery {
                slot_code: query.slot_code,
                q: query.q,
                limit,
                cursor: query.cursor,
            },
        )
        .await?;
    let installed = installed_catalog_joins(state, category).await?;
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
        freshness: match page.freshness {
            OfficialExtensionCatalogFreshness::Fresh => "fresh",
            OfficialExtensionCatalogFreshness::Stale => "stale",
        }
        .to_string(),
        catalog_page: page.snapshot_checksum.clone(),
        catalog_page_number: 0,
        catalog_page_checksum: page.snapshot_checksum,
        catalog_page_locator: page.snapshot_locator,
        limit,
        next_cursor: page.next_cursor,
        total_entries: page.total_entries,
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
    let _context = require_session(&state, &headers).await?;
    let category = ExtensionCatalogCategory::parse(&category)?;
    let page = load_catalog_page(&state, category, query).await?;
    Ok(Json(ApiSuccess::new(page)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/extension-center/catalog/{category}/{catalog_id}",
    params(ExtensionCatalogGatewayQuery),
    operation_id = "extension_center_get_catalog_entry",
    responses((status = 200, body = ExtensionCatalogGatewayEntryResponse), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn get_extension_catalog_entry(
    State(state): State<Arc<ApiState>>,
    Path((category, catalog_id)): Path<(String, String)>,
    headers: HeaderMap,
    Query(_query): Query<ExtensionCatalogGatewayQuery>,
) -> Result<Json<ApiSuccess<ExtensionCatalogGatewayEntryResponse>>, ApiError> {
    let _context = require_session(&state, &headers).await?;
    let category = ExtensionCatalogCategory::parse(&category)?;
    let identity = catalog_identity(category, &catalog_id)?;
    let located = find_catalog_entry_for_requested_identity(&state, category, &catalog_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_catalog_entry",
        ))?;
    if located.entry.category != category.as_str()
        || located.entry.artifact != identity.artifact_id()
        || located.entry.id != catalog_id
        || located.entry.organization != identity.organization()
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_catalog_identity",
        )
        .into());
    }
    let installed = installed_catalog_joins(&state, category).await?;
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
    operation_id = "extension_center_check_updates",
    request_body = ExtensionUpdateCheckBody,
    responses((status = 200, body = ExtensionUpdateCheckResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn check_extension_catalog_updates(
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
    for item in &body.items {
        catalog_identity(category, &item.catalog_id)?;
        if !valid_extension_segment(&item.current_version) {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "extension_version",
            )
            .into());
        }
        if item.installed_versions.is_empty()
            || item
                .installed_versions
                .iter()
                .any(|version| !valid_extension_segment(version))
        {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "installed_extension_versions",
            )
            .into());
        }
    }
    let mut items = Vec::with_capacity(body.items.len());
    for item in body.items {
        let latest_version =
            find_catalog_entry_for_requested_identity(&state, category, &item.catalog_id)
                .await?
                .map(|located| located.entry.version);
        let status = extension_update_status(latest_version.as_deref(), &item.installed_versions);
        items.push(ExtensionUpdateCheckItemResponse {
            catalog_id: item.catalog_id,
            current_version: item.current_version,
            latest_version,
            status: status.to_string(),
        });
    }
    Ok(Json(ApiSuccess::new(ExtensionUpdateCheckResponse {
        category: body.category,
        items,
    })))
}

#[derive(Debug)]
struct NodePluginInspection {
    category: ExtensionCatalogCategory,
    organization: String,
    artifact_id: String,
    plugin_id: String,
    version: String,
    minimum_host_version: String,
    signature_status: domain::ExtensionSignatureStatus,
    signature_algorithm: Option<String>,
    signing_key_id: Option<String>,
    application_action: domain::ExtensionApplicationAction,
}

#[derive(Debug)]
struct UploadedExtensionArtifact {
    category: ExtensionCatalogCategory,
    organization: String,
    artifact_id: String,
    version: String,
    minimum_host_version: Option<String>,
    node_plugin: bool,
    signature_status: domain::ExtensionSignatureStatus,
    signature_algorithm: Option<String>,
    signing_key_id: Option<String>,
    application_action: domain::ExtensionApplicationAction,
}

#[derive(Default)]
struct ExtensionUploadFields {
    file_name: Option<String>,
    artifact_bytes: Option<Vec<u8>>,
    category: Option<String>,
    organization: Option<String>,
    artifact_id: Option<String>,
    version: Option<String>,
    risk_override: Option<PluginRiskOverrideBody>,
    compatibility_override: Option<PluginCompatibilityOverrideBody>,
}

enum PreflightDecision {
    Challenge,
    Accepted(Option<serde_json::Value>),
}

fn extension_identity(
    category: ExtensionCatalogCategory,
    organization: &str,
    artifact_id: &str,
    version: &str,
    node_id: &str,
) -> Result<domain::ExtensionInstallationIdentity, ApiError> {
    let category = domain::ExtensionCategory::parse(category.as_str()).ok_or(
        control_plane::errors::ControlPlaneError::InvalidInput("extension_catalog_category"),
    )?;
    if !valid_extension_segment(organization)
        || !valid_extension_segment(artifact_id)
        || !valid_extension_segment(version)
        || node_id.trim().is_empty()
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_catalog_identity",
        )
        .into());
    }
    Ok(domain::ExtensionInstallationIdentity {
        category,
        organization: organization.to_string(),
        artifact_id: artifact_id.to_string(),
        version: version.to_string(),
    })
}

fn valid_extension_segment(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !matches!(value, "." | "..")
        && !value.contains([':', '/', '\\', '\0'])
}

fn requested_installation_identity(
    category: ExtensionCatalogCategory,
    catalog_id: &str,
    version: &str,
    node_id: &str,
) -> Result<domain::ExtensionInstallationIdentity, ApiError> {
    let catalog_identity = catalog_identity(category, catalog_id)?;
    extension_identity(
        category,
        catalog_identity.organization(),
        catalog_identity.artifact_id(),
        version,
        node_id,
    )
}

fn catalog_identity(
    category: ExtensionCatalogCategory,
    catalog_id: &str,
) -> Result<domain::ExtensionCatalogIdentity, ApiError> {
    let domain_category = domain::ExtensionCategory::parse(category.as_str()).ok_or(
        control_plane::errors::ControlPlaneError::InvalidInput("extension_catalog_category"),
    )?;
    domain::ExtensionCatalogIdentity::parse(domain_category, catalog_id).ok_or_else(|| {
        control_plane::errors::ControlPlaneError::InvalidInput("extension_catalog_identity").into()
    })
}

fn is_node_plugin_category(category: ExtensionCatalogCategory) -> bool {
    matches!(
        category,
        ExtensionCatalogCategory::HostExtensions
            | ExtensionCatalogCategory::RuntimeExtensions
            | ExtensionCatalogCategory::CapabilityPlugins
    )
}

fn risk_warning(code: &str, message: &str) -> ExtensionRiskWarningResponse {
    ExtensionRiskWarningResponse {
        code: code.to_string(),
        message: message.to_string(),
        overridable: true,
    }
}

fn artifact_preflight_challenge(
    entry: &OfficialExtensionCatalogEntry,
    descriptor: &OfficialExtensionArtifactDescriptor,
    trusted_key_ids: &[String],
) -> ExtensionRiskChallengeResponse {
    let mut warnings = Vec::new();
    if descriptor.expected_checksum.is_none() {
        warnings.push(risk_warning(
            "checksum_missing",
            "The artifact does not include an expected checksum.",
        ));
    }
    match descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("key_id"))
        .and_then(serde_json::Value::as_str)
    {
        None => warnings.push(risk_warning(
            "signature_missing",
            "The artifact does not include a verifiable signature.",
        )),
        Some(key_id) if !trusted_key_ids.iter().any(|trusted| trusted == key_id) => {
            warnings.push(risk_warning(
                "signing_key_unknown",
                "The artifact was signed by a key that is not configured as trusted.",
            ));
        }
        Some(_) => {}
    }
    warnings.sort_by(|left, right| left.code.cmp(&right.code));
    ExtensionRiskChallengeResponse {
        warnings,
        compatibility: catalog_entry_compatibility(entry),
    }
}

fn validate_preflight_overrides(
    challenge: &ExtensionRiskChallengeResponse,
    risk_override: Option<&PluginRiskOverrideBody>,
    compatibility_override: Option<&PluginCompatibilityOverrideBody>,
) -> Result<PreflightDecision, ApiError> {
    if !challenge.warnings.is_empty() {
        let Some(risk_override) = risk_override else {
            return Ok(PreflightDecision::Challenge);
        };
        let acknowledged = risk_override
            .acknowledged_warnings
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if risk_override.reason.trim().is_empty()
            || challenge
                .warnings
                .iter()
                .any(|warning| !acknowledged.contains(&warning.code))
        {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "extension_risk_override",
            )
            .into());
        }
    }
    if let Some(compatibility) = challenge.compatibility.as_ref() {
        let Some(override_value) = compatibility_override else {
            return Ok(PreflightDecision::Challenge);
        };
        if override_value.reason != compatibility.reason
            || override_value.acknowledged_current_host_version
                != compatibility.current_host_version
            || override_value.acknowledged_minimum_host_version
                != compatibility.minimum_host_version
        {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "extension_compatibility_override",
            )
            .into());
        }
    } else if compatibility_override.is_some() {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_compatibility_override",
        )
        .into());
    }
    let receipt = (risk_override.is_some() || compatibility_override.is_some()).then(|| {
        serde_json::json!({
            "risk_override": risk_override,
            "compatibility_override": compatibility_override,
        })
    });
    Ok(PreflightDecision::Accepted(receipt))
}

fn challenge_response(challenge: ExtensionRiskChallengeResponse) -> Response {
    (
        StatusCode::CONFLICT,
        Json(ExtensionRiskChallengeErrorResponse {
            status: StatusCode::CONFLICT.as_u16(),
            code: "extension_risk_confirmation_required".to_string(),
            message: "Extension installation requires risk confirmation.".to_string(),
            risk_challenge: challenge,
        }),
    )
        .into_response()
}

fn domain_challenge_response(
    challenge: domain::ExtensionRiskChallenge,
    compatibility: Option<ExtensionCompatibilityWarningResponse>,
) -> Response {
    challenge_response(ExtensionRiskChallengeResponse {
        warnings: challenge
            .warnings
            .into_iter()
            .map(|warning| ExtensionRiskWarningResponse {
                code: warning.code,
                message: warning.message,
                overridable: warning.overridable,
            })
            .collect(),
        compatibility,
    })
}

fn signature_status_from_descriptor(
    descriptor: &OfficialExtensionArtifactDescriptor,
    trusted_key_ids: &[String],
) -> domain::ExtensionSignatureStatus {
    match descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("key_id"))
        .and_then(serde_json::Value::as_str)
    {
        None => domain::ExtensionSignatureStatus::Missing,
        Some(key_id) if trusted_key_ids.iter().any(|trusted| trusted == key_id) => {
            domain::ExtensionSignatureStatus::Verified
        }
        Some(_) => domain::ExtensionSignatureStatus::UnknownKey,
    }
}

fn signature_status_from_intake(status: &str) -> domain::ExtensionSignatureStatus {
    match status {
        "verified" => domain::ExtensionSignatureStatus::Verified,
        "unsigned" => domain::ExtensionSignatureStatus::Missing,
        "unknown_key" => domain::ExtensionSignatureStatus::UnknownKey,
        _ => domain::ExtensionSignatureStatus::Invalid,
    }
}

async fn inspect_node_plugin(
    state: &ApiState,
    file_name: &str,
    artifact_bytes: &[u8],
    source_kind: &str,
) -> Result<NodePluginInspection, ApiError> {
    let intake = intake_package_bytes(
        artifact_bytes,
        &PackageIntakePolicy {
            source_kind: source_kind.to_string(),
            trust_mode: "allow_unsigned".to_string(),
            expected_artifact_sha256: None,
            trusted_public_keys: state.official_plugin_source.trusted_public_keys(),
            original_filename: Some(file_name.to_string()),
        },
    )
    .await?;
    let manifest = intake.manifest.clone();
    let artifact_id = manifest.plugin_code()?.to_string();
    let category = match manifest.consumption_kind {
        PluginConsumptionKind::HostExtension => ExtensionCatalogCategory::HostExtensions,
        PluginConsumptionKind::RuntimeExtension => ExtensionCatalogCategory::RuntimeExtensions,
        PluginConsumptionKind::CapabilityPlugin => ExtensionCatalogCategory::CapabilityPlugins,
    };
    let inspection = NodePluginInspection {
        category,
        organization: manifest.publisher_namespace,
        artifact_id,
        plugin_id: manifest.plugin_id,
        version: manifest.version,
        minimum_host_version: manifest.minimum_host_version,
        signature_status: signature_status_from_intake(&intake.signature_status),
        signature_algorithm: intake.signature_algorithm,
        signing_key_id: intake.signing_key_id,
        application_action: if manifest
            .slot_codes
            .iter()
            .any(|slot| slot == "model_provider")
        {
            domain::ExtensionApplicationAction::ConfigureModelProvider
        } else {
            domain::ExtensionApplicationAction::None
        },
    };
    let _ = tokio::fs::remove_dir_all(&intake.extracted_root).await;
    Ok(inspection)
}

async fn install_or_update_official_extension(
    state: &ApiState,
    headers: &HeaderMap,
    body: InstallOfficialExtensionBody,
    operation_id: &'static str,
) -> Result<Response, ApiError> {
    let context = require_session(state, headers).await?;
    require_csrf(headers, &context)?;
    let category = ExtensionCatalogCategory::parse(&body.category)?;
    let identity = requested_installation_identity(
        category,
        &body.catalog_id,
        &body.version,
        &state.api_node_id,
    )?;
    let install_service = extension_installation_service(state);
    if let Some(installation) = install_service
        .find_local_installation(&state.api_node_id, &identity)
        .await?
    {
        let node_plugin_installation_id = if is_node_plugin_category(category) {
            Some(installation.id.to_string())
        } else {
            None
        };
        return Ok((
            StatusCode::OK,
            Json(ApiSuccess::new(ExtensionInstallResponse {
                application_action: installation.application_action.as_str().to_string(),
                application_status: default_application_status(installation.application_action)
                    .to_string(),
                installation: to_local_inventory_entry(installation),
                local_artifact_was_present: true,
                node_plugin_installation_id,
            })),
        )
            .into_response());
    }

    let located = state
        .official_extension_catalog_source
        .find_entry(category.as_str(), &body.catalog_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_catalog_entry",
        ))?;
    if located.entry.category != category.as_str()
        || located.entry.id != body.catalog_id
        || located.entry.version != body.version
        || located.entry.organization != identity.organization
        || located.entry.artifact != identity.artifact_id
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_catalog_identity",
        )
        .into());
    }
    let source_kind = if located.source_kind == "official_repository" {
        "official_registry"
    } else {
        "mirror_registry"
    };

    let descriptor = state
        .official_extension_catalog_source
        .resolve_artifact(&located.entry)?;
    let trusted_key_ids = state
        .official_plugin_source
        .trusted_public_keys()
        .iter()
        .map(|key| key.key_id.clone())
        .collect::<Vec<_>>();
    let preflight = artifact_preflight_challenge(&located.entry, &descriptor, &trusted_key_ids);
    let declared_warnings = preflight
        .warnings
        .iter()
        .map(|warning| domain::ExtensionIntegrityWarning {
            code: warning.code.clone(),
            message: warning.message.clone(),
            overridable: warning.overridable,
        })
        .collect();
    let confirmation_receipt = match validate_preflight_overrides(
        &preflight,
        body.risk_override.as_ref(),
        body.compatibility_override.as_ref(),
    )? {
        PreflightDecision::Challenge => return Ok(challenge_response(preflight)),
        PreflightDecision::Accepted(receipt) => receipt,
    };
    let downloaded = state
        .official_extension_catalog_source
        .download_artifact(&located.entry)
        .await
        .map_err(|_| {
            control_plane::errors::ControlPlaneError::UpstreamUnavailable(
                "extension_artifact_download_unavailable",
            )
        })?;
    let mut signature_status =
        signature_status_from_descriptor(&downloaded.descriptor, &trusted_key_ids);
    let mut signature_algorithm = downloaded
        .descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("algorithm"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mut signing_key_id = downloaded
        .descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("key_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mut application_action = catalog_application_action(category);
    if is_node_plugin_category(category) {
        let inspection = inspect_node_plugin(
            state,
            &downloaded.file_name,
            &downloaded.artifact_bytes,
            source_kind,
        )
        .await?;
        if inspection.category != category
            || inspection.organization != located.entry.organization
            || inspection.version != located.entry.version
            || inspection.artifact_id != located.entry.artifact
            || inspection.plugin_id != located.entry.artifact
        {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "extension_artifact_identity",
            )
            .into());
        }
        signature_status = inspection.signature_status;
        signature_algorithm = inspection.signature_algorithm;
        signing_key_id = inspection.signing_key_id;
        application_action = inspection.application_action;
    }
    let trust = match (source_kind, signature_status) {
        ("official_registry", domain::ExtensionSignatureStatus::Verified) => "official",
        (_, domain::ExtensionSignatureStatus::Verified) => "trusted",
        _ => "unknown",
    };
    let risk_override = body.risk_override.map(|value| ExtensionRiskOverride {
        reason: value.reason,
        acknowledged_warnings: value.acknowledged_warnings,
    });
    if is_node_plugin_category(category) {
        let installed = service(state, operation_id)
            .install_extension_node_plugin(InstallExtensionNodePluginCommand {
                actor_user_id: context.user.id,
                category,
                file_name: downloaded.file_name,
                package_bytes: downloaded.artifact_bytes,
                source_kind: source_kind.to_string(),
            })
            .await?;
        if operation_id == "extension_center.update"
            && installed
                .installation
                .metadata_json
                .get("plugin_type")
                .and_then(serde_json::Value::as_str)
                == Some("model_provider")
        {
            let current = control_plane::ports::PluginRepository::list_assignments(
                &state.store,
                context.actor.current_workspace_id,
            )
            .await?
            .into_iter()
            .find(|assignment| assignment.provider_code == installed.installation.provider_code);
            if current
                .is_some_and(|assignment| assignment.installation_id != installed.installation.id)
            {
                service(state, operation_id)
                    .switch_version(SwitchPluginVersionCommand {
                        actor_user_id: context.user.id,
                        provider_code: installed.installation.provider_code.clone(),
                        target_installation_id: installed.installation.id,
                    })
                    .await?;
            }
        }
        let installation = control_plane::ports::ExtensionInstallationRepository::find_extension_installation_by_id(
            &state.store,
            &state.api_node_id,
            installed.installation.id,
        )
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_installation",
        ))?;
        return Ok((
            StatusCode::CREATED,
            Json(ApiSuccess::new(ExtensionInstallResponse {
                application_action: installation.application_action.as_str().to_string(),
                application_status: default_application_status(installation.application_action)
                    .to_string(),
                installation: to_local_inventory_entry(installation.clone()),
                local_artifact_was_present: false,
                node_plugin_installation_id: Some(installation.id.to_string()),
            })),
        )
            .into_response());
    }
    let outcome = install_service
        .install_from_bytes(InstallExtensionArtifactCommand {
            actor_user_id: context.user.id,
            category,
            organization: located.entry.organization,
            artifact_id: located.entry.artifact,
            version: located.entry.version,
            node_id: state.api_node_id.clone(),
            artifact_bytes: downloaded.artifact_bytes,
            source: if source_kind == "official_registry" {
                "official".to_string()
            } else {
                "mirror".to_string()
            },
            trust: trust.to_string(),
            expected_checksum: downloaded.descriptor.expected_checksum,
            signature_status,
            signature_algorithm,
            signing_key_id,
            declared_warnings,
            risk_override,
            confirmation_receipt,
            application_action,
        })
        .await?;
    let (installation, local_artifact_was_present) = match outcome {
        ExtensionArtifactInstallOutcome::RiskConfirmationRequired { risk_challenge } => {
            return Ok(domain_challenge_response(
                risk_challenge,
                preflight.compatibility,
            ));
        }
        ExtensionArtifactInstallOutcome::Installed {
            installation,
            local_artifact_was_present,
        } => (installation, local_artifact_was_present),
    };
    let node_plugin_installation_id = None;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(ExtensionInstallResponse {
            application_action: installation.application_action.as_str().to_string(),
            application_status: default_application_status(installation.application_action)
                .to_string(),
            installation: to_local_inventory_entry(installation),
            local_artifact_was_present,
            node_plugin_installation_id,
        })),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/install",
    operation_id = "extension_center_install",
    request_body = InstallOfficialExtensionBody,
    responses((status = 201, body = ExtensionInstallResponse), (status = 409, body = ExtensionRiskChallengeErrorResponse))
)]
pub async fn install_official_extension(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InstallOfficialExtensionBody>,
) -> Result<Response, ApiError> {
    install_or_update_official_extension(&state, &headers, body, "extension_center.install").await
}

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/update",
    operation_id = "extension_center_update",
    request_body = InstallOfficialExtensionBody,
    responses((status = 201, body = ExtensionInstallResponse), (status = 409, body = ExtensionRiskChallengeErrorResponse))
)]
pub async fn update_official_extension(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InstallOfficialExtensionBody>,
) -> Result<Response, ApiError> {
    install_or_update_official_extension(&state, &headers, body, "extension_center.update").await
}

pub(crate) mod upload;
pub use upload::install_uploaded_extension;
mod application;
use application::{
    catalog_application_action, default_application_status, workspace_application_status,
};
#[cfg(test)]
mod _tests;
