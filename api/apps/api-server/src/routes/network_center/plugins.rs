use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    handler::Handler,
    http::{HeaderMap, StatusCode},
    middleware, Json,
};
use control_plane::plugin_management::official_plugin_host_compatibility;
use control_plane::ports::NetworkEgressRepository;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::{
        console_route_assembly::{console_delete, console_get, console_post, ConsoleRouteAssembly},
        plugins::{
            enforce_plugin_upload_limit, read_upload_file, requested_locales, to_install_response,
            InstallOfficialPluginBody, InstallPluginResponse, OfficialPluginArtifactResponse,
            OfficialPluginCatalogPageResponse, OfficialPluginCatalogQuery,
            PluginUploadMultipartBody,
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

#[derive(Debug, Clone, serde::Serialize, ToSchema)]
pub struct NetworkEgressPluginInstalledVersionResponse {
    pub installation_id: String,
    pub plugin_version: String,
    pub is_current: bool,
    pub can_uninstall: bool,
}

#[derive(Debug, Clone, serde::Serialize, ToSchema)]
pub struct NetworkEgressPluginFamilyResponse {
    pub provider_code: String,
    pub display_name: String,
    pub current_installation_id: String,
    pub current_version: String,
    pub can_uninstall: bool,
    pub installed_versions: Vec<NetworkEgressPluginInstalledVersionResponse>,
}

impl NetworkEgressPluginFamilyResponse {
    pub(super) fn contains_installed_version(&self, target_version: &str) -> bool {
        self.installed_versions
            .iter()
            .any(|version| version.plugin_version == target_version)
    }
}

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct SwitchNetworkEgressPluginVersionBody {
    pub installation_id: String,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    route_assembly_with_plugin_upload_max_bytes(crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES)
}

pub(crate) mod plugins_interface;

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
            "/settings/network-center/proxy-plugins/families",
            console_get(
                list_plugin_families,
                ConsoleOperation("network_egress_plugins.families.view".to_string()),
            ),
        )
        .route(
            "/settings/network-center/proxy-plugins/families/:provider_code/switch-version",
            console_post(
                switch_plugin_version,
                ConsoleOperation("network_egress_plugins.families.switch".to_string()),
            ),
        )
        .route(
            "/settings/network-center/proxy-plugins/families/:provider_code/versions/:installation_id",
            console_delete(
                uninstall_plugin_version,
                ConsoleOperation("network_egress_plugins.families.uninstall".to_string()),
            ),
        )
        .route(
            "/settings/network-center/proxy-plugins/families/:provider_code",
            console_delete(
                uninstall_plugin_family,
                ConsoleOperation("network_egress_plugins.families.uninstall".to_string()),
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-plugins.official-catalog.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        plugins_interface::NetworkPluginInput::ListOfficial { query, locale },
    )
    .await?;
    let plugins_interface::NetworkPluginOutput::Official(catalog) = output else {
        unreachable!("network plugin catalog binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(catalog)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/network-center/proxy-plugins/families",
    operation_id = "network_egress_plugins_list_families",
    responses((status = 200, body = [NetworkEgressPluginFamilyResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_plugin_families(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<NetworkEgressPluginFamilyResponse>>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-plugins.families.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        plugins_interface::NetworkPluginInput::ListFamilies { locale },
    )
    .await?;
    let plugins_interface::NetworkPluginOutput::Families(families) = output else {
        unreachable!("network plugin families binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(families)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/network-center/proxy-plugins/families/{provider_code}/switch-version",
    operation_id = "network_egress_plugins_switch_family_version",
    request_body = SwitchNetworkEgressPluginVersionBody,
    responses((status = 204), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn switch_plugin_version(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SwitchNetworkEgressPluginVersionBody>,
) -> Result<StatusCode, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-plugins.switch-version.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        plugins_interface::NetworkPluginInput::SwitchVersion {
            provider_code,
            body,
            locale,
        },
    )
    .await?;
    let plugins_interface::NetworkPluginOutput::Empty = output else {
        unreachable!("network plugin switch binding returned a different output")
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/network-center/proxy-plugins/families/{provider_code}/versions/{installation_id}",
    operation_id = "network_egress_plugins_uninstall_family_version",
    responses((status = 204), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn uninstall_plugin_version(
    State(state): State<Arc<ApiState>>,
    Path((provider_code, installation_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-plugins.uninstall-version.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        plugins_interface::NetworkPluginInput::UninstallVersion {
            provider_code,
            installation_id,
            locale,
        },
    )
    .await?;
    let plugins_interface::NetworkPluginOutput::Empty = output else {
        unreachable!("network plugin uninstall version binding returned a different output")
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/network-center/proxy-plugins/families/{provider_code}",
    operation_id = "network_egress_plugins_uninstall_family",
    responses((status = 204), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn uninstall_plugin_family(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-plugins.uninstall-family.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        plugins_interface::NetworkPluginInput::UninstallFamily {
            provider_code,
            locale,
        },
    )
    .await?;
    let plugins_interface::NetworkPluginOutput::Empty = output else {
        unreachable!("network plugin uninstall family binding returned a different output")
    };
    Ok(StatusCode::NO_CONTENT)
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-plugins.install-official.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        plugins_interface::NetworkPluginInput::InstallOfficial(body),
    )
    .await?;
    let plugins_interface::NetworkPluginOutput::Installed(result) = output else {
        unreachable!("network plugin install binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(result))))
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
    let (file_name, package_bytes) = read_upload_file(&mut multipart).await?;
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-plugins.install-upload.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        plugins_interface::NetworkPluginInput::InstallUploaded {
            file_name,
            package_bytes,
        },
    )
    .await?;
    let plugins_interface::NetworkPluginOutput::Installed(result) = output else {
        unreachable!("network plugin upload binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(result))))
}

pub(super) struct OfficialFilter {
    pub(super) search_query: Option<String>,
    pub(super) limit: usize,
}

pub(super) fn official_filter(query: &OfficialPluginCatalogQuery) -> OfficialFilter {
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

pub(super) fn project_catalog_entry(
    source: &dyn crate::official_extension_catalog::OfficialExtensionCatalogSourcePort,
    entry: crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    installed: &HashMap<String, NetworkEgressPluginFamilyResponse>,
) -> anyhow::Result<Option<NetworkEgressOfficialPluginCatalogEntryResponse>> {
    if metadata_optional(&entry, "plugin_type").as_deref()
        != Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE)
    {
        return Ok(None);
    }
    let plugin_id = metadata_required(&entry, "plugin_id")?;
    let provider_code = metadata_required(&entry, "provider_code")?;
    let descriptor = source.resolve_artifact(&entry)?;
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
    let installed_family = installed.get(&provider_code);
    let current_version = installed_family.map(|family| family.current_version.clone());
    let is_installed =
        installed_family.is_some_and(|family| family.contains_installed_version(&entry.version));
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

pub(super) fn project_plugin_families(
    entries: Vec<control_plane::plugin_management::PluginCatalogEntry>,
) -> anyhow::Result<HashMap<String, NetworkEgressPluginFamilyResponse>> {
    let mut versions = HashMap::<String, Vec<NetworkEgressPluginInstalledVersionResponse>>::new();
    let mut display_names = HashMap::<String, String>::new();
    for entry in entries {
        if !entry.local_artifact.artifact_status.is_ready() {
            continue;
        }
        display_names
            .entry(entry.installation.provider_code.clone())
            .or_insert(entry.installation.display_name.clone());
        versions
            .entry(entry.installation.provider_code)
            .or_default()
            .push(NetworkEgressPluginInstalledVersionResponse {
                installation_id: entry.installation.id.to_string(),
                plugin_version: entry.installation.plugin_version,
                is_current: entry.local_artifact.is_current,
                // A current version is always retained. The storage delete guard also rejects
                // versions still referenced by a proxy instance.
                can_uninstall: !entry.local_artifact.is_current,
            });
    }
    let mut families = HashMap::new();
    for (provider_code, mut installed_versions) in versions {
        installed_versions.sort_by(|left, right| {
            semver::Version::parse(&right.plugin_version)
                .ok()
                .cmp(&semver::Version::parse(&left.plugin_version).ok())
                .then_with(|| right.installation_id.cmp(&left.installation_id))
        });
        let current = installed_versions
            .iter()
            .find(|version| version.is_current)
            .ok_or_else(|| anyhow::anyhow!("network egress family has no current ready version"))?;
        families.insert(
            provider_code.clone(),
            NetworkEgressPluginFamilyResponse {
                display_name: display_names
                    .remove(&provider_code)
                    .unwrap_or(provider_code.clone()),
                provider_code,
                current_installation_id: current.installation_id.clone(),
                current_version: current.plugin_version.clone(),
                can_uninstall: true,
                installed_versions,
            },
        );
    }
    Ok(families)
}

pub(super) async fn mark_referenced_versions_not_uninstallable(
    store: &storage_durable_postgres::MainDurableStore,
    families: &mut HashMap<String, NetworkEgressPluginFamilyResponse>,
) -> anyhow::Result<()> {
    let referenced_families = store
        .list_network_egress_providers()
        .await?
        .into_iter()
        .filter_map(|provider| provider.extension_family)
        .map(|family| family.artifact_id().to_string())
        .collect::<std::collections::HashSet<_>>();
    for family in families.values_mut() {
        if referenced_families.contains(&family.provider_code) {
            family.can_uninstall = false;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests/plugins.rs"]
mod tests;
