use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use control_plane::ports::{OfficialPluginSourcePort, RuntimeEventStream, SessionStore};
use plugin_framework::HostExtensionContributionManifest;
use runtime_core::runtime_engine::RuntimeEngine;
use serde::Serialize;
use storage_durable::MainDurableStore;
use time::OffsetDateTime;

use crate::host_infrastructure::HostInfrastructureRegistry;
use crate::openapi_docs::ApiDocsRegistry;
use crate::{
    console_surface_registry::ConsoleSurfaceRegistry,
    host_extensions::console::ResolvedHostExtensionConsoleContribution,
    routes::console_route_assembly::{
        compile_migrated_console_operation_registry, migrated_core_console_route_assembly,
        ConsoleRouteAssembly,
    },
};
use crate::{
    official_agent_flow_templates::OfficialAgentFlowTemplateSourcePort,
    official_mcp_bundles::OfficialMcpBundleSourcePort,
    provider_runtime::ApiRuntimeServices,
    runtime_activity::ApplicationRuntimeActivityTracker,
    runtime_profile_client::{ApiRuntimeProfilePort, PluginRunnerSystemPort},
};

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
    let assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly();
    crate::routes::console_route_assembly::compile_migrated_core_console_operation_registry(
        settings_features,
        assembly.bindings(),
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
    let route_assembly = migrated_core_console_route_assembly();
    let registry = compile_migrated_console_operation_registry(
        &settings_features,
        route_assembly.bindings(),
        &[],
    )?;
    let inventory = registry.inventory();
    let locale_catalog = inventory
        .locale_catalog
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Core console operation inventory has no locale catalog"))?;
    let references = inventory
        .operations
        .iter()
        .flat_map(|operation| {
            std::iter::once(operation.label_ref.as_str())
                .chain(operation.description_ref.as_deref())
        })
        .chain(inventory.resources.iter().flat_map(|resource| {
            std::iter::once(resource.label_ref.as_str())
                .chain(resource.description_ref.as_deref())
                .chain(resource.actions.iter().flat_map(|action| {
                    std::iter::once(action.label_ref.as_str())
                        .chain(action.description_ref.as_deref())
                }))
        }))
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
        route_assembly: route_assembly.bindings().to_vec(),
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
    let host_extensions = host_extensions.into_iter().collect::<Vec<_>>();
    let host_contributions = host_extensions
        .iter()
        .map(|host| host.contribution.clone())
        .collect::<Vec<_>>();
    let settings_feature_registry = compile_settings_feature_registry(&host_contributions)?;
    let mut route_assembly = migrated_core_console_route_assembly();
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
    pub(crate) test_database: Option<Arc<postgres_test_support::PostgresTestSchema>>,
    pub store: MainDurableStore,
    pub authenticator_registry: Arc<control_plane::auth::AuthenticatorRegistry>,
    pub settings_feature_registry: Arc<access_control::SettingsFeatureRegistry>,
    pub console_operation_registry: Arc<access_control::ConsoleOperationRegistry>,
    pub infrastructure: Arc<HostInfrastructureRegistry>,
    pub console_surface_registry: Arc<ConsoleSurfaceRegistry>,
    pub file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    pub runtime_engine: Arc<RuntimeEngine>,
    pub provider_runtime: Arc<ApiRuntimeServices>,
    pub process_started_at: OffsetDateTime,
    pub runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    pub api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
    pub plugin_runner_system: Arc<dyn PluginRunnerSystemPort>,
    pub official_plugin_source: Arc<dyn OfficialPluginSourcePort>,
    pub official_agent_flow_template_source: Arc<dyn OfficialAgentFlowTemplateSourcePort>,
    pub official_mcp_bundle_source: Arc<dyn OfficialMcpBundleSourcePort>,
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
    pub bootstrap_workspace_name: String,
}
