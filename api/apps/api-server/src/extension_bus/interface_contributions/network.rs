use super::*;

pub(super) fn contributions(
    state: &Arc<crate::app_state::ApiState>,
) -> Result<Vec<InterfaceRegistryContribution>, RegistryCompilationError> {
    Ok(vec![
        InterfaceRegistryContribution::new(
            "api-server.console-network-pools",
            &[
                "network_egress_pools.list",
                "network_egress_proxies.create",
                "network_egress_proxies.get",
                "network_egress_proxies.update",
                "network_egress_pool_members.test_connection",
                "network_egress_pools.create",
                "network_egress_pools.update",
                "network_egress_pools.delete",
                "network_egress_pool_members.create",
                "network_egress_pool_members.create_static_http",
                "network_egress_pool_members.add_provider_egresses",
                "network_egress_pool_members.update",
                "network_egress_pool_members.delete",
                "network_egress_pool_members.batch_delete",
            ],
            &["api-server.console-network-pools"],
            crate::routes::network_center::pools_interface::compile_registry(
                crate::routes::network_center::pools_interface::NetworkPoolsDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-network-center",
            &[
                "network_egress_providers.list",
                "network_egress_proxy_types.list",
                "network_egress_providers.create",
                "network_egress_providers.lifecycle.update",
                "network_egress_providers.sync",
                "network_egress_routes.list",
                "network_egress_routes.create",
                "network_egress_routes.update",
                "network_egress_routes.delete",
            ],
            &["api-server.console-network-center"],
            crate::routes::network_center::core_interface::compile_registry(
                crate::routes::network_center::core_interface::NetworkCenterDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-network-plugins",
            &[
                "network_egress_plugins.official_catalog.view",
                "network_egress_plugins.families.view",
                "network_egress_plugins.families.switch",
                "network_egress_plugins.families.uninstall",
                "network_egress_plugins.install.official",
                "network_egress_plugins.install.upload",
            ],
            &["api-server.console-network-plugins"],
            crate::routes::network_center::plugins::plugins_interface::compile_registry(
                crate::routes::network_center::plugins::plugins_interface::NetworkPluginDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    official_plugin_source: state.official_plugin_source.clone(),
                    official_catalog_source: state.official_extension_catalog_source.clone(),
                    cache_store: state.infrastructure.cache_store(),
                    provider_install_root: state.provider_install_root.clone(),
                    provider_secret_master_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                    bootstrap_workspace_id: state.bootstrap_workspace_id,
                    allow_uploaded_host_extensions: state.allow_uploaded_host_extensions,
                },
            )?,
        ),
    ])
}
