pub use crate::event_center::catalog::EventSourceCatalog;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use plugin_framework::HostExtensionContributionManifest;
use serde::Serialize;
use storage_durable_postgres::MainDurableStore;

/// Durable store selected by the API composition root.
///
/// Protocol modules depend on this state-owned name instead of importing a concrete adapter.
pub type ApiNotificationService =
    control_plane::notifications::NotificationService<MainDurableStore>;

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

use crate::{
    console_surface_registry::ConsoleSurfaceRegistry,
    host_extensions::console::ResolvedHostExtensionConsoleContribution,
    routes::console_route_assembly::{
        compile_complete_migrated_console_operation_registry, ConsoleRouteAssembly,
    },
};

pub use api_host_kernel::app_state::*;

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
    compile_complete_migrated_console_operation_registry(settings_features, &bindings, &[])
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
    let registry = compile_complete_migrated_console_operation_registry(
        &settings_features,
        &contract_bindings,
        &[],
    )?;
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
    interface_registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
) -> anyhow::Result<CompiledConsoleBootPlan> {
    compile_console_boot_plan_with_interface_operations_and_plugin_upload_max_bytes(
        host_extensions,
        interface_registry,
        crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
    )
}

pub(crate) fn compile_console_boot_plan_with_interface_operations_and_plugin_upload_max_bytes(
    host_extensions: impl IntoIterator<Item = ResolvedHostExtensionConsoleContribution>,
    interface_registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
    plugin_upload_max_bytes: usize,
) -> anyhow::Result<CompiledConsoleBootPlan> {
    compile_console_boot_plan_with_managed_services(
        host_extensions,
        interface_registry,
        plugin_upload_max_bytes,
        Vec::new(),
    )
}

pub(crate) fn compile_console_boot_plan_with_managed_services(
    host_extensions: impl IntoIterator<Item = ResolvedHostExtensionConsoleContribution>,
    interface_registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
    plugin_upload_max_bytes: usize,
    managed_services: Vec<crate::managed_services::ManagedServiceRegistration>,
) -> anyhow::Result<CompiledConsoleBootPlan> {
    let host_extensions = host_extensions.into_iter().collect::<Vec<_>>();
    let host_contributions = host_extensions
        .iter()
        .map(|host| host.contribution.clone())
        .collect::<Vec<_>>();
    let settings_feature_registry = Arc::new(access_control::SettingsFeatureRegistry::compile(
        access_control::core_settings_feature_registrations()
            .into_iter()
            .chain(
                host_contributions
                    .iter()
                    .flat_map(|c| c.settings_features.iter().cloned()),
            )
            .chain(
                managed_services
                    .iter()
                    .filter_map(|service| service.feature()),
            ),
    )?);
    let mut route_assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations_and_plugin_upload_max_bytes(
        interface_registry,
        plugin_upload_max_bytes,
    );
    for host in host_extensions {
        if let Some(host_assembly) = host.route_assembly {
            validate_linked_host_console_route_assembly(&host.contribution, &host_assembly)?;
            route_assembly = route_assembly.merge(host_assembly);
        }
    }
    route_assembly =
        route_assembly.merge(crate::managed_services::route_assembly(&managed_services));
    if managed_services.is_empty() {
        compile_complete_migrated_console_operation_registry(
            &settings_feature_registry,
            route_assembly.bindings(),
            &host_contributions,
        )?;
    }
    let console_operation_registry =
        Arc::new(crate::routes::console_route_assembly::compile_migrated_console_operation_registry_with_managed(
            &settings_feature_registry,
            route_assembly.bindings(),
            &host_contributions,
            &managed_services,
        )?);
    let console_surface_registry = Arc::new(
        ConsoleSurfaceRegistry::from_host_extension_contributions(&host_contributions)?
            .with_managed_services(managed_services)?,
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
