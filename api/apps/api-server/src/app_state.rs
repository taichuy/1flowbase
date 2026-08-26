use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Arc, Mutex},
};

use axum::http::{header::ACCEPT_LANGUAGE, HeaderMap};
use control_plane::i18n_catalog::CatalogResolver;
use control_plane::ports::{OfficialPluginSourcePort, RuntimeEventStream, SessionStore};
use domain::{CatalogLocale, CatalogMessageIdentity};
use plugin_framework::HostExtensionContributionManifest;
use runtime_core::runtime_engine::RuntimeEngine;
use serde::Serialize;
use storage_durable_postgres::MainDurableStore;

/// Durable store selected by the API composition root.
///
/// Protocol modules depend on this state-owned name instead of importing a concrete adapter.
pub type ApiModelProviderService = control_plane::model_provider::ModelProviderService<
    MainDurableStore,
    crate::provider_runtime::ApiProviderRuntime,
>;
pub type ApiPluginManagementService = control_plane::plugin_management::PluginManagementService<
    MainDurableStore,
    crate::provider_runtime::ApiProviderRuntime,
>;
pub type ApiExtensionInstallationService =
    control_plane::plugin_management::ExtensionInstallationService<MainDurableStore>;
pub type ApiDataSourceService = control_plane::data_source::DataSourceService<
    MainDurableStore,
    crate::provider_runtime::ApiProviderRuntime,
>;
pub type ApiModelDefinitionMutationService =
    control_plane::runtime_registry_sync::ModelDefinitionMutationService<
        MainDurableStore,
        crate::runtime_registry_sync::ApiRuntimeRegistrySync,
    >;
pub type ApiModelDefinitionService =
    control_plane::model_definition::ModelDefinitionService<MainDurableStore>;
pub type ApiNetworkEgressProviderService =
    control_plane::network_egress::NetworkEgressProviderService<
        MainDurableStore,
        crate::provider_runtime::ApiProviderRuntime,
        control_plane::network_egress_secret::ProviderRegistryNetworkEgressSecretResolver<
            MainDurableStore,
        >,
    >;
pub type ApiNetworkEgressRouteService =
    control_plane::network_egress_route::NetworkEgressRouteService<MainDurableStore>;
pub type ApiNetworkEgressPoolService =
    control_plane::network_egress_pool::NetworkEgressPoolService<MainDurableStore>;
pub type ApiUiComponentCatalogService =
    control_plane::ui_component_catalog::UiComponentCatalogService<
        MainDurableStore,
        crate::ui_component_catalog_source::ApiUiComponentCatalogSource,
    >;
use time::OffsetDateTime;

use crate::error_response::ApiError;
use crate::openapi_docs::ApiDocsRegistry;
use crate::{
    config::ApiConfig,
    console_surface_registry::ConsoleSurfaceRegistry,
    extension_bus::ExtensionBootSnapshot,
    host_extensions::console::ResolvedHostExtensionConsoleContribution,
    host_infrastructure::HostInfrastructureRegistry,
    routes::console_route_assembly::{
        compile_migrated_console_operation_registry, ConsoleRouteAssembly,
    },
};
use crate::{
    official_extension_catalog::OfficialExtensionCatalogSourcePort,
    official_i18n_catalog_source::ApiOfficialI18nCatalogSource,
    official_mcp_bundles::OfficialMcpBundleSourcePort,
    provider_runtime::ApiRuntimeServices,
    routes::assistant::conversation_events::AssistantConversationEventHub,
    runtime_activity::ApplicationRuntimeActivityTracker,
    runtime_profile_client::{ApiRuntimeProfilePort, PluginRunnerSystemPort},
};

pub fn build_official_i18n_catalog_update_service(
    store: MainDurableStore,
    config: &ApiConfig,
) -> Arc<control_plane::i18n_catalog::OfficialI18nCatalogUpdateService<MainDurableStore>> {
    let source = Arc::new(ApiOfficialI18nCatalogSource::new(
        config.resolve_official_i18n_catalog_source(),
    ));
    Arc::new(control_plane::i18n_catalog::OfficialI18nCatalogUpdateService::new(store, source))
}

pub(crate) fn request_catalog_locale(
    headers: &HeaderMap,
    preferred_locale: Option<String>,
) -> CatalogLocale {
    let resolved = runtime_profile::resolve_locale(runtime_profile::LocaleResolutionInput {
        query_locale: None,
        explicit_header_locale: headers
            .get("x-1flowbase-locale")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        user_preferred_locale: preferred_locale,
        accept_language: headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        fallback_locale: runtime_profile::FALLBACK_LOCALE,
        supported_locales: runtime_profile::SUPPORTED_LOCALES
            .iter()
            .map(|value| value.to_string())
            .collect(),
    });
    CatalogLocale::new(resolved.resolved_locale)
        .expect("runtime profile must resolve a supported catalog locale")
}

pub(crate) async fn resolve_request_text(
    state: &ApiState,
    locale: &CatalogLocale,
    key: &str,
) -> Result<String, ApiError> {
    let identity =
        CatalogMessageIdentity::new(key).expect("backend display catalog key must be valid");
    let resolver = CatalogResolver::new(state.store.clone(), state.bootstrap_workspace_id);
    Ok(resolver
        .resolve(state.bootstrap_workspace_id, &identity, locale)
        .await?
        .value)
}

pub(crate) async fn project_canonical_display(
    state: &ApiState,
    locale: &CatalogLocale,
    key: &'static str,
    stored: &str,
) -> Result<String, ApiError> {
    if !stored.trim().is_empty() && stored != key {
        return Ok(stored.to_owned());
    }
    resolve_request_text(state, locale, key).await
}

pub(crate) async fn resolve_official_source_label(
    state: &ApiState,
    locale: &CatalogLocale,
    source_kind: &str,
    fallback: String,
) -> Result<String, ApiError> {
    let key = match source_kind {
        "official_registry" => "Official source",
        "mirror_registry" => "Mirror source",
        _ => return Ok(fallback),
    };
    resolve_request_text(state, locale, key).await
}

pub fn compile_core_settings_feature_registry() -> Result<
    Arc<access_control::SettingsFeatureRegistry>,
    access_control::SettingsFeatureRegistryError,
> {
    compile_settings_feature_registry(&[])
}

fn compile_settings_feature_registry(
    host_contributions: &[HostExtensionContributionManifest],
) -> Result<
    Arc<access_control::SettingsFeatureRegistry>,
    access_control::SettingsFeatureRegistryError,
> {
    access_control::SettingsFeatureRegistry::compile(
        access_control::core_settings_feature_registrations()
            .into_iter()
            .chain(
                host_contributions
                    .iter()
                    .flat_map(|contribution| contribution.settings_features.iter().cloned()),
            ),
    )
    .map(Arc::new)
}

pub fn compile_core_console_operation_registry(
    settings_features: &access_control::SettingsFeatureRegistry,
) -> anyhow::Result<Arc<access_control::ConsoleOperationRegistry>> {
    let bindings = crate::routes::console_route_assembly::migrated_core_console_contract_bindings();
    crate::routes::console_route_assembly::compile_migrated_core_console_operation_registry(
        settings_features,
        &bindings,
    )
    .map(Arc::new)
}

#[derive(Debug, Serialize)]
pub struct CoreConsoleOperationInventorySnapshot {
    pub compiled_inventory: access_control::ConsoleOperationCompiledInventory,
    pub route_assembly: Vec<access_control::ConsoleRouteAssemblyBinding>,
    pub locales: BTreeMap<&'static str, BTreeMap<String, String>>,
}

pub fn compile_core_console_operation_inventory_snapshot(
) -> anyhow::Result<CoreConsoleOperationInventorySnapshot> {
    let settings_features = compile_core_settings_feature_registry()?;
    let contract_bindings =
        crate::routes::console_route_assembly::migrated_core_console_contract_bindings();
    let registry =
        compile_migrated_console_operation_registry(&settings_features, &contract_bindings, &[])?;
    let inventory = registry.inventory();
    let locale_catalog = inventory
        .locale_catalog
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Core console operation inventory has no locale catalog"))?;
    let references = inventory
        .resources
        .iter()
        .flat_map(|resource| {
            std::iter::once(resource.label_ref.as_str())
                .chain(resource.description_ref.as_deref())
                .chain(resource.actions.iter().flat_map(|action| {
                    std::iter::once(action.label_ref.as_str())
                        .chain(action.description_ref.as_deref())
                }))
        })
        .collect::<BTreeSet<_>>();
    let locales = ["zh_Hans", "en_US"]
        .into_iter()
        .map(|locale| {
            let values = references
                .iter()
                .map(|reference| {
                    let value = locale_catalog.text(locale, reference).ok_or_else(|| {
                        anyhow::anyhow!(
                            "compiled locale catalog is missing {locale} reference {reference}"
                        )
                    })?;
                    Ok(((*reference).to_string(), value.to_string()))
                })
                .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
            Ok((locale, values))
        })
        .collect::<anyhow::Result<BTreeMap<_, _>>>()?;

    Ok(CoreConsoleOperationInventorySnapshot {
        compiled_inventory: inventory.clone(),
        route_assembly: contract_bindings,
        locales,
    })
}

pub(crate) struct CompiledConsoleBootPlan {
    pub(crate) settings_feature_registry: Arc<access_control::SettingsFeatureRegistry>,
    pub(crate) console_operation_registry: Arc<access_control::ConsoleOperationRegistry>,
    pub(crate) console_surface_registry: Arc<ConsoleSurfaceRegistry>,
    pub(crate) route_assembly: ConsoleRouteAssembly<Arc<ApiState>>,
}

pub(crate) fn compile_console_boot_plan(
    host_extensions: impl IntoIterator<Item = ResolvedHostExtensionConsoleContribution>,
) -> anyhow::Result<CompiledConsoleBootPlan> {
    compile_console_boot_plan_with_interface_operations(host_extensions, None)
}

pub(crate) fn compile_console_boot_plan_with_interface_operations(
    host_extensions: impl IntoIterator<Item = ResolvedHostExtensionConsoleContribution>,
    interface_operations: Option<
        &crate::routes::host_infrastructure::interface_operation::InterfaceOperationCatalog,
    >,
) -> anyhow::Result<CompiledConsoleBootPlan> {
    compile_console_boot_plan_with_interface_operations_and_plugin_upload_max_bytes(
        host_extensions,
        interface_operations,
        crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
    )
}

pub(crate) fn compile_console_boot_plan_with_interface_operations_and_plugin_upload_max_bytes(
    host_extensions: impl IntoIterator<Item = ResolvedHostExtensionConsoleContribution>,
    interface_operations: Option<
        &crate::routes::host_infrastructure::interface_operation::InterfaceOperationCatalog,
    >,
    plugin_upload_max_bytes: usize,
) -> anyhow::Result<CompiledConsoleBootPlan> {
    let host_extensions = host_extensions.into_iter().collect::<Vec<_>>();
    let host_contributions = host_extensions
        .iter()
        .map(|host| host.contribution.clone())
        .collect::<Vec<_>>();
    let settings_feature_registry = compile_settings_feature_registry(&host_contributions)?;
    let mut route_assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations_and_plugin_upload_max_bytes(
        interface_operations,
        plugin_upload_max_bytes,
    );
    for host in host_extensions {
        if let Some(host_assembly) = host.route_assembly {
            validate_linked_host_console_route_assembly(&host.contribution, &host_assembly)?;
            route_assembly = route_assembly.merge(host_assembly);
        }
    }
    let console_operation_registry = Arc::new(compile_migrated_console_operation_registry(
        &settings_feature_registry,
        route_assembly.bindings(),
        &host_contributions,
    )?);
    let console_surface_registry = Arc::new(
        ConsoleSurfaceRegistry::from_host_extension_contributions(&host_contributions)?,
    );

    Ok(CompiledConsoleBootPlan {
        settings_feature_registry,
        console_operation_registry,
        console_surface_registry,
        route_assembly,
    })
}

fn validate_linked_host_console_route_assembly(
    contribution: &HostExtensionContributionManifest,
    assembly: &ConsoleRouteAssembly<Arc<ApiState>>,
) -> anyhow::Result<()> {
    let allowed_operation_ids = contribution
        .console_operations
        .iter()
        .map(|operation| operation.operation_id.clone())
        .chain(
            contribution
                .settings_features
                .iter()
                .filter(|feature| !feature.api_routes.is_empty())
                .map(|feature| format!("settings_feature.access.{}", feature.feature_id)),
        )
        .collect::<BTreeSet<_>>();

    for binding in assembly.bindings() {
        let access_control::ConsoleRouteOwnership::ConsoleOperation(operation_id) =
            &binding.ownership
        else {
            anyhow::bail!(
                "linked HostExtension {}@{} must declare every console route as a ConsoleOperation",
                contribution.extension_id,
                contribution.version,
            );
        };
        if !allowed_operation_ids.contains(operation_id) {
            anyhow::bail!(
                "linked HostExtension {}@{} mounted undeclared console operation {operation_id}",
                contribution.extension_id,
                contribution.version,
            );
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct ApiState {
    #[cfg(test)]
    pub(crate) test_resources: Option<Arc<TestResources>>,
    pub store: MainDurableStore,
    pub system_backup: Option<Arc<crate::system_backup::SystemBackupRuntime>>,
    pub system_maintenance: Arc<control_plane::system_recovery::SystemMaintenance>,
    pub authenticator_registry: Arc<control_plane::auth::AuthenticatorRegistry>,
    pub settings_feature_registry: Arc<access_control::SettingsFeatureRegistry>,
    pub console_operation_registry: Arc<access_control::ConsoleOperationRegistry>,
    pub infrastructure: Arc<HostInfrastructureRegistry>,
    /// Present on production boot; lightweight test states may omit the production graph.
    pub extension_boot_snapshot: Option<Arc<ExtensionBootSnapshot>>,
    pub console_surface_registry: Arc<ConsoleSurfaceRegistry>,
    pub file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    pub runtime_engine: Arc<RuntimeEngine>,
    pub provider_runtime: Arc<ApiRuntimeServices>,
    pub process_started_at: OffsetDateTime,
    pub runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    pub assistant_conversation_events: Arc<AssistantConversationEventHub>,
    /// Process-local owner for detached embedded Assistant executions. Durable run state remains
    /// owned by the runtime repository.
    pub assistant_executions: Arc<Mutex<HashMap<uuid::Uuid, tokio::task::AbortHandle>>>,
    /// Active browser capability lease for each detached Assistant execution.
    pub assistant_client_sessions:
        Arc<Mutex<HashMap<uuid::Uuid, Arc<crate::routes::assistant::AssistantClientToolBridge>>>>,
    pub api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
    pub plugin_runner_system: Arc<dyn PluginRunnerSystemPort>,
    pub official_plugin_source: Arc<dyn OfficialPluginSourcePort>,
    pub official_mcp_bundle_source: Arc<dyn OfficialMcpBundleSourcePort>,
    pub official_extension_catalog_source: Arc<dyn OfficialExtensionCatalogSourcePort>,
    pub official_i18n_catalog_update_service:
        Arc<control_plane::i18n_catalog::OfficialI18nCatalogUpdateService<MainDurableStore>>,
    pub api_node_id: String,
    pub provider_install_root: String,
    pub provider_secret_master_key: String,
    pub host_extension_dropin_root: String,
    pub allow_unverified_filesystem_dropins: bool,
    pub allow_uploaded_host_extensions: bool,
    pub session_store: Arc<dyn SessionStore>,
    pub runtime_event_stream: Arc<dyn RuntimeEventStream>,
    pub api_docs: Arc<ApiDocsRegistry>,
    pub cookie_name: String,
    pub cookie_secure: bool,
    pub session_ttl_days: i64,
    pub bootstrap_workspace_id: uuid::Uuid,
    pub bootstrap_workspace_name: String,
}

#[cfg(test)]
pub(crate) struct TestResources {
    _database: postgres_test_support::PostgresTestSchema,
    filesystem_roots: Vec<std::path::PathBuf>,
}

#[cfg(test)]
impl TestResources {
    pub(crate) fn new(
        database: postgres_test_support::PostgresTestSchema,
        filesystem_roots: Vec<std::path::PathBuf>,
    ) -> Self {
        Self {
            _database: database,
            filesystem_roots,
        }
    }
}

#[cfg(test)]
impl Drop for TestResources {
    fn drop(&mut self) {
        for root in &self.filesystem_roots {
            if let Err(error) = std::fs::remove_dir_all(root) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!(
                        "failed to clean API test filesystem root {}: {error}",
                        root.display()
                    );
                }
            }
        }
    }
}
