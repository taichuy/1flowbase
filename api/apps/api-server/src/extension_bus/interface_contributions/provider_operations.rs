use super::*;

pub(super) fn contributions(
    state: &Arc<crate::app_state::ApiState>,
) -> Result<Vec<InterfaceRegistryContribution>, RegistryCompilationError> {
    Ok(vec![
        InterfaceRegistryContribution::new(
            "api-server.console-provider-catalog-logs",
            &[
                "model_providers.catalog.view",
                "model_providers.request_logs.view",
                "model_providers.request_logs.delete",
                "model_providers.request_logs.clear",
            ],
            &["api-server.console-provider-catalog-logs"],
            crate::routes::model_providers::catalog_logs_interface::compile_registry(
                crate::routes::model_providers::catalog_logs_interface::ProviderCatalogLogsDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                    install_root: state.provider_install_root.clone(),
                    cache_store: state.infrastructure.cache_store(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-provider-instance-lifecycle",
            &[
                "model_providers.instances.view",
                "model_providers.instances.create",
                "model_providers.instances.update",
                "model_providers.instances.validate",
                "model_providers.instances.delete",
            ],
            &["api-server.console-provider-instance-lifecycle"],
            crate::routes::model_providers::instance_lifecycle_interface::compile_registry(
                crate::routes::model_providers::instance_lifecycle_interface::ProviderInstanceLifecycleDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                    install_root: state.provider_install_root.clone(),
                    cache_store: state.infrastructure.cache_store(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-provider-instance-operations",
            &[
                "model_providers.instances.authenticate",
                "model_providers.instances.usage.view",
                "model_providers.instances.reset_credits.view",
                "model_providers.instances.reset_credits.consume",
                "model_providers.balance.view",
                "model_providers.preview.view",
                "model_providers.instances.secrets.reveal",
            ],
            &["api-server.console-provider-instance-operations"],
            crate::routes::model_providers::instance_operations_interface::compile_registry(
                crate::routes::model_providers::instance_operations_interface::ProviderInstanceOperationsDependencies {
                    store: state.store.clone(),
                    provider_runtime: state.provider_runtime.clone(),
                    secret_key: state.provider_secret_master_key.clone(),
                    api_node_id: state.api_node_id.clone(),
                    install_root: state.provider_install_root.clone(),
                    cache_store: state.infrastructure.cache_store(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-provider-routing",
            &["model_providers.main_instance.view", "model_providers.main_instance.update"],
            &["api-server.console-provider-routing"],
            crate::routes::model_providers::routing_interface::compile_registry(
                crate::routes::model_providers::routing_interface::ProviderRoutingDependencies {
                    store: state.store.clone(), provider_runtime: state.provider_runtime.clone(), secret_key: state.provider_secret_master_key.clone(), api_node_id: state.api_node_id.clone(), install_root: state.provider_install_root.clone(), cache_store: state.infrastructure.cache_store(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-provider-discovery",
            &["model_providers.instances.models.view", "model_providers.instances.models.refresh", "model_providers.options.view", "model_providers.settings_options.view"],
            &["api-server.console-provider-discovery"],
            crate::routes::model_providers::discovery_interface::compile_registry(crate::routes::model_providers::discovery_interface::ProviderDiscoveryDependencies { store: state.store.clone(), provider_runtime: state.provider_runtime.clone(), secret_key: state.provider_secret_master_key.clone(), api_node_id: state.api_node_id.clone(), install_root: state.provider_install_root.clone(), cache_store: state.infrastructure.cache_store() })?,
        ),
    ])
}
