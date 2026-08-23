use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{DefaultBodyLimit, Multipart, Query, State},
    handler::Handler,
    http::{HeaderMap, StatusCode},
    middleware, Json,
};
use control_plane::plugin_management::{
    official_plugin_host_compatibility, InstallUploadedPluginCommand, PluginCatalogFilter,
    PluginManagementService,
};
use storage_durable::MainDurableStore;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::{
        console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
        plugins::{
            enforce_plugin_upload_limit, read_upload_file, requested_locales, resolve_locale_meta,
            to_install_response, InstallOfficialPluginBody, InstallPluginResponse,
            OfficialPluginArtifactResponse, OfficialPluginCatalogPageResponse,
            OfficialPluginCatalogQuery, PluginUploadMultipartBody,
        },
    },
};

const NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE: &str = "network_egress_provider";
const DEFAULT_OFFICIAL_PLUGIN_CATALOG_LIMIT: usize = 20;
const MAX_OFFICIAL_PLUGIN_CATALOG_LIMIT: usize = 50;

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct NetworkEgressOfficialPluginCatalogEntryResponse {
    pub plugin_id: String,
    pub plugin_type: String,
    pub provider_code: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub protocol: String,
    pub current_version: Option<String>,
    pub latest_version: String,
    pub has_update: bool,
    pub minimum_host_version: String,
    pub current_host_version: String,
    pub compatibility_status: String,
    pub compatibility_warning_reason: Option<String>,
    pub selected_artifact: OfficialPluginArtifactResponse,
    pub help_url: Option<String>,
    pub model_discovery_mode: String,
    pub install_status: String,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct NetworkEgressOfficialPluginCatalogResponse {
    pub source_kind: String,
    pub source_label: String,
    pub registry_url: String,
    pub source_freshness: String,
    pub locale_meta: crate::routes::system::LocaleMetaResponse,
    pub page: OfficialPluginCatalogPageResponse,
    pub entries: Vec<NetworkEgressOfficialPluginCatalogEntryResponse>,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    route_assembly_with_plugin_upload_max_bytes(crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES)
}

pub(crate) fn route_assembly_with_plugin_upload_max_bytes(
    plugin_upload_max_bytes: usize,
) -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/network-center/proxy-plugins/official-catalog",
            console_get(
                list_official_catalog,
                ConsoleOperation("network_egress_plugins.official_catalog.view".to_string()),
            ),
        )
        .route(
            "/settings/network-center/proxy-plugins/install-official",
            console_post(
                install_official_plugin,
                ConsoleOperation("network_egress_plugins.install.official".to_string()),
            ),
        )
        .route(
            "/settings/network-center/proxy-plugins/install-upload",
            console_post(
                install_uploaded_plugin
                    .layer(DefaultBodyLimit::max(plugin_upload_max_bytes))
                    .layer(middleware::from_fn(move |request, next| {
                        enforce_plugin_upload_limit(plugin_upload_max_bytes, request, next)
                    })),
                ConsoleOperation("network_egress_plugins.install.upload".to_string()),
            ),
        )
}

fn service(
    state: &ApiState,
    actor: &domain::ActorContext,
    operation_id: &'static str,
) -> PluginManagementService<MainDurableStore, ApiProviderRuntime> {
    crate::routes::plugins::base_service(state, actor)
        .for_network_egress_provider_console_operation(operation_id)
}

#[utoipa::path(
    get,
    path = "/api/console/settings/network-center/proxy-plugins/official-catalog",
    params(OfficialPluginCatalogQuery),
    operation_id = "network_egress_plugins_list_official_catalog",
    responses((status = 200, body = NetworkEgressOfficialPluginCatalogResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_official_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<OfficialPluginCatalogQuery>,
) -> Result<Json<ApiSuccess<NetworkEgressOfficialPluginCatalogResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(
        &headers,
        query.locale.clone(),
        context.user.preferred_locale,
    );
    let local_catalog = service(
        &state,
        &context.actor,
        "network_egress_plugins.official_catalog.view",
    )
    .list_catalog(
        context.user.id,
        PluginCatalogFilter {
            plugin_type: Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string()),
        },
        requested_locales(&locale_meta),
    )
    .await?;
    let installed = local_catalog.entries.into_iter().fold(
        HashMap::<String, (Option<String>, bool)>::new(),
        |mut families, entry| {
            let family = entry.installation.provider_code.clone();
            let state = families.entry(family).or_default();
            if entry.local_artifact.artifact_status.is_ready() {
                state.1 = true;
                if entry.local_artifact.is_current {
                    state.0 = Some(entry.installation.plugin_version);
                }
            }
            families
        },
    );
    let filter = official_filter(&query);
    let page = state
        .official_extension_catalog_source
        .search(
            "runtime-extensions",
            crate::official_extension_catalog::OfficialExtensionCatalogSearchQuery {
                slot_code: Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string()),
                q: filter.search_query,
                limit: filter.limit,
                cursor: query.cursor,
            },
        )
        .await?;
    let entries = page
        .entries
        .into_iter()
        .filter_map(
            |entry| match project_catalog_entry(&state, entry, &installed) {
                Ok(Some(entry)) => Some(Ok(entry)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            },
        )
        .collect::<anyhow::Result<Vec<_>>>()?;
    let locale = domain::CatalogLocale::new(locale_meta.resolved_locale.clone())
        .expect("runtime profile must resolve a supported catalog locale");
    let source_label = crate::app_state::resolve_official_source_label(
        &state,
        &locale,
        &page.source_kind,
        page.source_kind.clone(),
    )
    .await?;
    Ok(Json(ApiSuccess::new(
        NetworkEgressOfficialPluginCatalogResponse {
            source_kind: page.source_kind,
            source_label,
            registry_url: page.snapshot_locator,
            source_freshness: "fresh".to_string(),
            locale_meta,
            page: OfficialPluginCatalogPageResponse {
                limit: filter.limit,
                next_cursor: page.next_cursor,
            },
            entries,
        },
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/network-center/proxy-plugins/install-official",
    operation_id = "network_egress_plugins_install_official",
    request_body = InstallOfficialPluginBody,
    responses((status = 201, body = InstallPluginResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn install_official_plugin(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InstallOfficialPluginBody>,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let command = crate::routes::plugins::resolved_official_plugin_install_command(
        &state,
        context.user.id,
        body.plugin_id,
        crate::routes::plugins::to_compatibility_override(body.compatibility_override),
        crate::routes::plugins::to_risk_override(body.risk_override),
    )
    .await?;
    let result = service(
        &state,
        &context.actor,
        "network_egress_plugins.install.official",
    )
    .install_resolved_official_plugin(command)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_install_response(result))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/network-center/proxy-plugins/install-upload",
    operation_id = "network_egress_plugins_install_uploaded",
    request_body(content = inline(PluginUploadMultipartBody), content_type = "multipart/form-data"),
    responses((status = 201, body = InstallPluginResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn install_uploaded_plugin(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let (file_name, package_bytes) = read_upload_file(&mut multipart).await?;
    let result = service(
        &state,
        &context.actor,
        "network_egress_plugins.install.upload",
    )
    .install_uploaded_network_egress_provider(InstallUploadedPluginCommand {
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

struct OfficialFilter {
    search_query: Option<String>,
    limit: usize,
}

fn official_filter(query: &OfficialPluginCatalogQuery) -> OfficialFilter {
    OfficialFilter {
        search_query: query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        limit: query
            .limit
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_OFFICIAL_PLUGIN_CATALOG_LIMIT)
            .min(MAX_OFFICIAL_PLUGIN_CATALOG_LIMIT),
    }
}

fn metadata_required(
    entry: &crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    field: &'static str,
) -> anyhow::Result<String> {
    metadata_optional(entry, field)
        .ok_or_else(|| anyhow::anyhow!("official network-egress catalog entry is missing {field}"))
}

fn metadata_optional(
    entry: &crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    field: &str,
) -> Option<String> {
    entry
        .source
        .metadata
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn project_catalog_entry(
    state: &ApiState,
    entry: crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    installed: &HashMap<String, (Option<String>, bool)>,
) -> anyhow::Result<Option<NetworkEgressOfficialPluginCatalogEntryResponse>> {
    if metadata_optional(&entry, "plugin_type").as_deref()
        != Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE)
    {
        return Ok(None);
    }
    let plugin_id = metadata_required(&entry, "plugin_id")?;
    let provider_code = metadata_required(&entry, "provider_code")?;
    let descriptor = state
        .official_extension_catalog_source
        .resolve_artifact(&entry)?;
    let checksum = descriptor.expected_checksum.ok_or_else(|| {
        anyhow::anyhow!("official network-egress catalog entry is missing checksum")
    })?;
    let platform = descriptor.platform.ok_or_else(|| {
        anyhow::anyhow!("official network-egress catalog entry has no current-platform artifact")
    })?;
    let compatibility = official_plugin_host_compatibility(
        &entry.host_version_requirement,
        &control_plane::plugin_management::current_plugin_host_version(),
    );
    let (current_version, is_installed) =
        installed.get(&provider_code).cloned().unwrap_or_default();
    let icon = metadata_optional(&entry, "icon");
    let protocol = metadata_optional(&entry, "protocol")
        .unwrap_or_else(|| NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string());
    let help_url = metadata_optional(&entry, "help_url");
    Ok(Some(NetworkEgressOfficialPluginCatalogEntryResponse {
        plugin_id,
        plugin_type: NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string(),
        provider_code,
        display_name: entry.name,
        description: (!entry.description.trim().is_empty()).then_some(entry.description),
        icon,
        protocol,
        has_update: current_version.as_deref().is_some_and(|current_version| {
            match (
                semver::Version::parse(&entry.version),
                semver::Version::parse(current_version),
            ) {
                (Ok(latest), Ok(current)) => latest > current,
                _ => false,
            }
        }),
        current_version,
        latest_version: entry.version,
        minimum_host_version: compatibility.minimum_host_version,
        current_host_version: compatibility.current_host_version,
        compatibility_status: compatibility.status,
        compatibility_warning_reason: compatibility.warning_reason,
        selected_artifact: OfficialPluginArtifactResponse {
            os: platform.os,
            arch: platform.arch,
            libc: platform.libc,
            rust_target: platform.rust_target,
            download_url: descriptor.locator,
            checksum,
            signature_algorithm: descriptor
                .signature
                .as_ref()
                .and_then(|signature| signature.get("algorithm"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            signing_key_id: descriptor
                .signature
                .as_ref()
                .and_then(|signature| signature.get("key_id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        },
        help_url,
        model_discovery_mode: "static".to_string(),
        install_status: if is_installed {
            "installed".to_string()
        } else {
            "not_installed".to_string()
        },
    }))
}
