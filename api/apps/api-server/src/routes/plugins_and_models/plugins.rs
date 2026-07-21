use std::sync::Arc;

use access_control::ConsoleRouteOwnership::ConsoleOperation;
use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header::ACCEPT_LANGUAGE, HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::plugin_management::{
    AssignPluginCommand, DeletePluginFamilyCommand, EnablePluginCommand,
    InstallCurrentNodePluginArtifactCommand, InstallOfficialPluginCommand, InstallPluginCommand,
    InstallPluginResult, InstallUploadedPluginCommand, OfficialPluginCatalogEntry,
    OfficialPluginCatalogFilter, OfficialPluginCatalogView, PluginCatalogEntry,
    PluginCatalogFilter, PluginCompatibilityOverride, PluginFamilyView, PluginInstalledVersionView,
    PluginManagementService, RefreshCurrentNodePluginArtifactCommand,
    RefreshPluginPackageCatalogProjectionCommand, SwitchPluginVersionCommand,
    UpgradeLatestPluginFamilyCommand,
};
use control_plane::resource_action::{
    ActionDefinition, ResourceActionKernel, ResourceActionRegistry, ResourceDefinition,
    ResourceScopeKind,
};
use plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT;
use serde::{Deserialize, Serialize};
use storage_durable::MainDurableStore;
use time::format_description::well_known::Rfc3339;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    provider_runtime::ApiProviderRuntime,
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
}

#[derive(ToSchema)]
#[allow(dead_code)]
pub(super) struct PluginUploadMultipartBody {
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct PluginCompatibilityOverrideBody {
    pub reason: String,
    pub acknowledged_current_host_version: String,
    pub acknowledged_minimum_host_version: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpgradeLatestPluginFamilyBody {
    pub compatibility_override: Option<PluginCompatibilityOverrideBody>,
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
    pub artifact_status: String,
    pub runtime_status: String,
    pub availability_status: String,
    pub package_path: Option<String>,
    pub installed_path: String,
    pub checksum: Option<String>,
    pub manifest_fingerprint: Option<String>,
    pub signature_status: Option<String>,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub last_load_error: Option<String>,
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
    pub installed_path: Option<String>,
    pub artifact_status: String,
    pub runtime_status: String,
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

#[derive(Debug, Deserialize, Serialize)]
struct InstallPluginActionInput {
    actor_user_id: Uuid,
    package_root: String,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
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
                install_uploaded_plugin,
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
                settings_routes::install_uploaded_plugin,
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

pub(crate) mod settings_routes;

pub(super) fn base_service(
    state: &ApiState,
) -> PluginManagementService<MainDurableStore, ApiProviderRuntime> {
    PluginManagementService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.official_plugin_source.clone(),
        state.provider_install_root.clone(),
    )
    .with_node_id(state.api_node_id.clone())
    .with_allow_uploaded_host_extensions(state.allow_uploaded_host_extensions)
}

fn service(
    state: &ApiState,
    operation_id: &'static str,
) -> PluginManagementService<MainDurableStore, ApiProviderRuntime> {
    base_service(state).for_plugin_console_operation(
        domain::ConsolePolicyGroup::other("other.plugins")
            .expect("compiled plugin policy group must be valid"),
        operation_id,
    )
}

fn install_plugin_action_kernel(state: Arc<ApiState>) -> Result<ResourceActionKernel, ApiError> {
    let mut registry = ResourceActionRegistry::default();
    registry.register_resource(ResourceDefinition::core(
        "plugins",
        ResourceScopeKind::System,
    ))?;
    registry.register_action(ActionDefinition::core("plugins", "install"))?;

    let mut kernel = ResourceActionKernel::new(registry);
    kernel.register_json_handler("plugins", "install", move |input| {
        let state = state.clone();
        async move {
            let input: InstallPluginActionInput = serde_json::from_value(input).map_err(|_| {
                control_plane::errors::ControlPlaneError::InvalidInput("plugin_install_action")
            })?;
            let result = service(&state, "plugins.install")
                .install_plugin(InstallPluginCommand {
                    actor_user_id: input.actor_user_id,
                    package_root: input.package_root,
                })
                .await?;
            Ok(serde_json::to_value(to_install_response(result))?)
        }
    })?;

    Ok(kernel)
}

fn format_time(value: time::OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap()
}

fn format_optional_time(value: Option<time::OffsetDateTime>) -> Option<String> {
    value.map(format_time)
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn to_compatibility_override(
    compatibility_override: Option<PluginCompatibilityOverrideBody>,
) -> Option<PluginCompatibilityOverride> {
    compatibility_override.map(|value| PluginCompatibilityOverride {
        reason: value.reason,
        acknowledged_current_host_version: value.acknowledged_current_host_version,
        acknowledged_minimum_host_version: value.acknowledged_minimum_host_version,
    })
}

async fn read_upload_file(multipart: &mut Multipart) -> Result<(String, Vec<u8>), ApiError> {
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

fn to_installation_response(
    installation: domain::PluginInstallationRecord,
) -> PluginInstallationResponse {
    to_installation_response_with_artifact(installation, None)
}

fn to_installation_response_with_artifact(
    installation: domain::PluginInstallationRecord,
    local_artifact: Option<domain::PluginArtifactInstanceRecord>,
) -> PluginInstallationResponse {
    PluginInstallationResponse {
        id: installation.id.to_string(),
        provider_code: installation.provider_code,
        runtime_slot: runtime_slot_for_contract(&installation.contract_version),
        plugin_id: installation.plugin_id,
        plugin_version: installation.plugin_version,
        contract_version: installation.contract_version,
        protocol: installation.protocol,
        display_name: installation.display_name,
        source_kind: installation.source_kind,
        trust_level: installation.trust_level,
        verification_status: installation.verification_status.as_str().to_string(),
        desired_state: installation.desired_state.as_str().to_string(),
        artifact_status: installation.artifact_status.as_str().to_string(),
        runtime_status: installation.runtime_status.as_str().to_string(),
        availability_status: installation.availability_status.as_str().to_string(),
        package_path: installation.package_path,
        installed_path: installation.installed_path,
        checksum: installation.checksum,
        manifest_fingerprint: installation.manifest_fingerprint,
        signature_status: installation.signature_status,
        signature_algorithm: installation.signature_algorithm,
        signing_key_id: installation.signing_key_id,
        last_load_error: installation.last_load_error,
        local_artifact: local_artifact.map(to_artifact_instance_response),
        metadata_json: installation.metadata_json,
        created_at: format_time(installation.created_at),
        updated_at: format_time(installation.updated_at),
    }
}

fn to_artifact_instance_response(
    artifact: domain::PluginArtifactInstanceRecord,
) -> PluginArtifactInstanceResponse {
    PluginArtifactInstanceResponse {
        node_id: artifact.node_id,
        installation_id: artifact.installation_id.to_string(),
        local_version: artifact.local_version,
        local_checksum: artifact.local_checksum,
        installed_path: artifact.installed_path,
        artifact_status: artifact.artifact_status.as_str().to_string(),
        runtime_status: artifact.runtime_status.as_str().to_string(),
        checked_at: format_time(artifact.checked_at),
        last_error: artifact.last_error,
    }
}

pub(crate) fn runtime_slot_for_contract(contract_version: &str) -> Option<String> {
    match contract_version {
        CURRENT_PROVIDER_CONTRACT => Some("model_provider".to_string()),
        "1flowbase.data_source/v1" => Some("data_source".to_string()),
        _ => None,
    }
}

fn to_install_response(result: InstallPluginResult) -> InstallPluginResponse {
    InstallPluginResponse {
        installation: to_installation_response(result.installation),
        task: to_task_response(result.task),
    }
}

fn to_catalog_response(entry: PluginCatalogEntry) -> PluginCatalogEntryResponse {
    let local_artifact = entry.local_artifact;
    PluginCatalogEntryResponse {
        installation: to_installation_response_with_artifact(
            entry.installation,
            Some(local_artifact.clone()),
        ),
        local_artifact: to_artifact_instance_response(local_artifact),
        plugin_type: entry.plugin_type,
        namespace: entry.namespace,
        label_key: entry.label_key,
        description_key: entry.description_key,
        provider_label_key: entry.provider_label_key,
        help_url: entry.help_url,
        default_base_url: entry.default_base_url,
        model_discovery_mode: entry.model_discovery_mode,
        assigned_to_current_workspace: entry.assigned_to_current_workspace,
        catalog_refresh_status: entry.catalog_refresh_status,
        catalog_last_error_message: entry.catalog_last_error_message,
        catalog_refreshed_at: format_optional_time(entry.catalog_refreshed_at),
    }
}

fn to_catalog_projection_response(
    projection: domain::PluginPackageCatalogProjectionRecord,
) -> PluginCatalogProjectionResponse {
    PluginCatalogProjectionResponse {
        installation_id: projection.installation_id.to_string(),
        package_code: projection.package_code,
        package_version: projection.package_version,
        projection_status: projection.projection_status.as_str().to_string(),
        last_error_message: projection.last_error_message,
        refreshed_at: format_optional_time(projection.refreshed_at),
        updated_at: format_time(projection.updated_at),
    }
}

fn to_official_catalog_entry_response(
    entry: OfficialPluginCatalogEntry,
) -> OfficialPluginCatalogEntryResponse {
    OfficialPluginCatalogEntryResponse {
        plugin_id: entry.plugin_id,
        plugin_type: entry.plugin_type,
        provider_code: entry.provider_code,
        display_name: entry.display_name,
        description: entry.description,
        icon: entry.icon,
        protocol: entry.protocol,
        latest_version: entry.latest_version,
        minimum_host_version: entry.minimum_host_version,
        current_host_version: entry.current_host_version,
        compatibility_status: entry.compatibility_status,
        compatibility_warning_reason: entry.compatibility_warning_reason,
        selected_artifact: OfficialPluginArtifactResponse {
            os: entry.selected_artifact.os,
            arch: entry.selected_artifact.arch,
            libc: entry.selected_artifact.libc,
            rust_target: entry.selected_artifact.rust_target,
            download_url: entry.selected_artifact.download_url,
            checksum: entry.selected_artifact.checksum,
            signature_algorithm: entry.selected_artifact.signature_algorithm,
            signing_key_id: entry.selected_artifact.signing_key_id,
        },
        help_url: entry.help_url,
        model_discovery_mode: entry.model_discovery_mode,
        install_status: entry.install_status.as_str().to_string(),
    }
}

fn to_official_catalog_response(
    locale_meta: LocaleMetaResponse,
    catalog: OfficialPluginCatalogView,
) -> OfficialPluginCatalogResponse {
    let source_label = localized_official_source_label(
        &catalog.source_kind,
        catalog.source_label,
        &locale_meta.resolved_locale,
    );
    OfficialPluginCatalogResponse {
        source_kind: catalog.source_kind,
        source_label,
        registry_url: catalog.registry_url,
        locale_meta,
        page: OfficialPluginCatalogPageResponse {
            limit: catalog.page.limit,
            next_cursor: catalog.page.next_cursor,
        },
        entries: catalog
            .entries
            .into_iter()
            .map(to_official_catalog_entry_response)
            .collect(),
    }
}

fn to_installed_version_response(
    version: PluginInstalledVersionView,
) -> PluginInstalledVersionResponse {
    PluginInstalledVersionResponse {
        installation_id: version.installation_id.to_string(),
        plugin_version: version.plugin_version,
        source_kind: version.source_kind,
        trust_level: version.trust_level,
        desired_state: version.desired_state,
        availability_status: version.availability_status,
        local_artifact: to_artifact_instance_response(version.local_artifact),
        created_at: format_time(version.created_at),
        is_current: version.is_current,
    }
}

fn to_family_response(entry: PluginFamilyView) -> PluginFamilyResponse {
    PluginFamilyResponse {
        provider_code: entry.provider_code,
        plugin_type: entry.plugin_type,
        namespace: entry.namespace,
        label_key: entry.label_key,
        description_key: entry.description_key,
        provider_label_key: entry.provider_label_key,
        icon: entry.icon,
        protocol: entry.protocol,
        help_url: entry.help_url,
        default_base_url: entry.default_base_url,
        model_discovery_mode: entry.model_discovery_mode,
        current_installation_id: entry.current_installation_id.to_string(),
        current_version: entry.current_version,
        current_local_artifact: to_artifact_instance_response(entry.current_local_artifact),
        latest_version: entry.latest_version,
        has_update: entry.has_update,
        installed_versions: entry
            .installed_versions
            .into_iter()
            .map(to_installed_version_response)
            .collect(),
    }
}

fn to_task_response(task: domain::PluginTaskRecord) -> PluginTaskResponse {
    PluginTaskResponse {
        id: task.id.to_string(),
        installation_id: task.installation_id.map(|id| id.to_string()),
        workspace_id: task.workspace_id.map(|id| id.to_string()),
        provider_code: task.provider_code,
        task_kind: task.task_kind.as_str().to_string(),
        status: task.status.as_str().to_string(),
        status_message: task.status_message,
        detail_json: task.detail_json,
        created_at: format_time(task.created_at),
        updated_at: format_time(task.updated_at),
        finished_at: format_optional_time(task.finished_at),
    }
}

fn resolve_locale_meta(
    headers: &HeaderMap,
    query_locale: Option<String>,
    user_preferred_locale: Option<String>,
) -> LocaleMetaResponse {
    runtime_profile::resolve_locale(runtime_profile::LocaleResolutionInput {
        query_locale,
        explicit_header_locale: headers
            .get("x-1flowbase-locale")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        user_preferred_locale,
        accept_language: headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        fallback_locale: runtime_profile::FALLBACK_LOCALE,
        supported_locales: runtime_profile::SUPPORTED_LOCALES
            .iter()
            .map(|value| value.to_string())
            .collect(),
    })
    .into()
}

fn requested_locales(locale_meta: &LocaleMetaResponse) -> control_plane::i18n::RequestedLocales {
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

fn localized_official_source_label(source_kind: &str, fallback: String, locale: &str) -> String {
    match (source_kind, locale) {
        ("official_registry", "en_US") => "Official source".to_string(),
        ("official_registry", _) => "官方源".to_string(),
        ("mirror_registry", "en_US") => "Mirror source".to_string(),
        ("mirror_registry", _) => "镜像源".to_string(),
        _ => fallback,
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
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(
        &headers,
        query.locale.clone(),
        context.user.preferred_locale,
    );
    let catalog = service(&state, "plugins.catalog.view")
        .list_catalog(
            context.user.id,
            filter_from_query(&query),
            requested_locales(&locale_meta),
        )
        .await?;
    Ok(Json(ApiSuccess::new(PluginCatalogResponse {
        locale_meta,
        i18n_catalog: serde_json::to_value(catalog.i18n_catalog).unwrap(),
        entries: catalog
            .entries
            .into_iter()
            .map(to_catalog_response)
            .collect(),
    })))
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
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(
        &headers,
        query.locale.clone(),
        context.user.preferred_locale,
    );
    let families = service(&state, "plugins.families.view")
        .list_families(
            context.user.id,
            filter_from_query(&query),
            requested_locales(&locale_meta),
        )
        .await?;
    Ok(Json(ApiSuccess::new(PluginFamilyCatalogResponse {
        locale_meta,
        i18n_catalog: serde_json::to_value(families.i18n_catalog).unwrap(),
        entries: families
            .entries
            .into_iter()
            .map(to_family_response)
            .collect(),
    })))
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
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(
        &headers,
        query.locale.clone(),
        context.user.preferred_locale,
    );
    let catalog = service(&state, "plugins.official_catalog.view")
        .list_official_catalog(
            context.user.id,
            official_filter_from_query(&query),
            requested_locales(&locale_meta),
        )
        .await?;
    Ok(Json(ApiSuccess::new(to_official_catalog_response(
        locale_meta,
        catalog,
    ))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let output = install_plugin_action_kernel(state.clone())?
        .dispatch_json(
            "plugins",
            "install",
            serde_json::json!({
                "actor_user_id": context.user.id,
                "package_root": body.package_root,
            }),
        )
        .await?;
    let response = serde_json::from_value(output).map_err(|_| {
        control_plane::errors::ControlPlaneError::InvalidInput("plugin_install_result")
    })?;

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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let (file_name, package_bytes) = read_upload_file(&mut multipart).await?;
    let result = service(&state, "plugins.install.upload")
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let result = service(&state, "plugins.install.official")
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: context.user.id,
            plugin_id: body.plugin_id,
            compatibility_override: to_compatibility_override(body.compatibility_override),
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_install_response(result))),
    ))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let projection = service(&state, "plugins.catalog_projection.refresh")
        .refresh_catalog_projection(RefreshPluginPackageCatalogProjectionCommand {
            actor_user_id: context.user.id,
            installation_id: parse_uuid(&installation_id, "installation_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_catalog_projection_response(
        projection,
    ))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let artifact = service(&state, "plugins.artifact.refresh")
        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
            actor_user_id: context.user.id,
            installation_id: parse_uuid(&installation_id, "installation_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_artifact_instance_response(
        artifact,
    ))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let artifact = service(&state, "plugins.artifact.install")
        .install_current_node_artifact(InstallCurrentNodePluginArtifactCommand {
            actor_user_id: context.user.id,
            installation_id: parse_uuid(&installation_id, "installation_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_artifact_instance_response(
        artifact,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/plugins/families/{provider_code}/upgrade-latest",
    operation_id = "plugin_upgrade_family_latest",
    responses((status = 200, body = PluginTaskResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn upgrade_latest(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
    body: Option<Json<UpgradeLatestPluginFamilyBody>>,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let task = service(&state, "plugins.families.upgrade")
        .upgrade_latest(UpgradeLatestPluginFamilyCommand {
            actor_user_id: context.user.id,
            provider_code,
            compatibility_override: body
                .map(|Json(body)| body.compatibility_override)
                .and_then(to_compatibility_override),
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let task = service(&state, "plugins.families.switch")
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: context.user.id,
            provider_code,
            target_installation_id: parse_uuid(&body.installation_id, "installation_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let task = service(&state, "plugins.families.delete")
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: context.user.id,
            provider_code,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let task = service(&state, "plugins.enable")
        .enable_plugin(EnablePluginCommand {
            actor_user_id: context.user.id,
            installation_id: parse_uuid(&installation_id, "installation_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let task = service(&state, "plugins.assign")
        .assign_plugin(AssignPluginCommand {
            actor_user_id: context.user.id,
            installation_id: parse_uuid(&installation_id, "installation_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
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
    let context = require_session(&state, &headers).await?;
    let tasks = service(&state, "plugins.tasks.view")
        .list_tasks(context.user.id)
        .await?;
    Ok(Json(ApiSuccess::new(
        tasks.into_iter().map(to_task_response).collect(),
    )))
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
    let context = require_session(&state, &headers).await?;
    let task = service(&state, "plugins.tasks.view")
        .get_task(context.user.id, parse_uuid(&task_id, "task_id")?)
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
}
