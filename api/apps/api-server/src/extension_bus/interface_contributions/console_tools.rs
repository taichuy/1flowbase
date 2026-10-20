use super::*;

pub(super) fn contributions(
    state: &Arc<crate::app_state::ApiState>,
    console_frontstage_callable_dispatch: &Arc<dyn crate::openapi_interface::CallableDispatchPort>,
) -> Result<Vec<InterfaceRegistryContribution>, RegistryCompilationError> {
    Ok(vec![

        InterfaceRegistryContribution::new(
            "api-server.console-mcp-debug",
            &["mcp.debug.execute"],
            &["api-server.console-mcp-debug"],
            crate::routes::mcp_management::interface_debug::compile_registry(
                crate::routes::mcp_management::interface_debug::port(
                    crate::routes::mcp_management::interface_debug::McpDebugDependencies {
                        store: state.store.clone(),
                        catalog: crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
                            store: state.store.clone(),
                            openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                                store: state.store.clone(), console_operations: state.console_operation_registry.inventory().clone(), interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()), api_docs: Arc::clone(&state.api_docs), template_catalog: state.runtime_engine.template_catalog().clone(),
                            },
                        },
                        dispatcher: Arc::clone(&console_frontstage_callable_dispatch),
                    },
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontend-blocks",
            &["frontend_blocks.view"],
            &["api-server.console-frontend-blocks"],
            crate::routes::frontend_block_catalog::compile_registry(
                crate::routes::frontend_block_catalog::FrontendBlockDependencies {
                    store: state.store.clone(),
                    api_node_id: state.api_node_id.clone(),
                    graph: Arc::clone(
                        state
                            .extension_boot_snapshot
                            .as_ref()
                            .expect("production interface publication requires ExtensionBootSnapshot")
                            .graph_arc(),
                    ),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-js-dependencies",
            &["js_dependencies.view"],
            &["api-server.console-js-dependencies"],
            crate::routes::js_dependencies::compile_registry(
                state.store.clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-node-contributions",
            &["node_contributions.view"],
            &["api-server.console-node-contributions"],
            crate::routes::node_contributions::compile_registry(
                state.store.clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-data-capabilities",
            &["frontstage.data_capabilities.view"],
            &["api-server.console-frontstage-data-capabilities"],
            crate::routes::frontstage::data_capabilities::compile_catalog_registry(
                state.runtime_engine.registry().clone(),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-block-context-contract",
            &["frontstage.block_context.view"],
            &["api-server.console-frontstage-block-context-contract"],
            crate::routes::frontstage::block_context_contract::compile_registry()?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-components",
            &["frontstage.components.view"],
            &["api-server.console-frontstage-components"],
            crate::routes::frontstage::components::compile_registry(
                crate::routes::frontstage::components::FrontstageComponentsDependencies {
                    store: state.store.clone(),
                    api_node_id: state.api_node_id.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-pages",
            &[
                "frontstage.pages.view", "frontstage.pages.create", "frontstage.groups.create",
                "frontstage.pages.update", "frontstage.pages.move", "frontstage.pages.delete",
                "frontstage.tabs.view", "frontstage.tabs.create", "frontstage.tabs.update",
                "frontstage.tabs.delete", "frontstage.tabs.document.save",
                "frontstage.ui_templates.view",
                "frontstage.queries.dispatch", "frontstage.actions.dispatch",
            ],
            &["api-server.console-frontstage-pages"],
            crate::routes::frontstage::interface_pages::compile_registry(
                crate::routes::frontstage::interface_pages::FrontstagePagesDependencies {
                    navigation_cache: control_plane::navigation_cache::NavigationCache(state.infrastructure.cache_store()),
                    store: state.store.clone(),
                    bootstrap_workspace_id: state.bootstrap_workspace_id,
                    api_node_id: state.api_node_id.clone(),
                    runtime_engine: Arc::clone(&state.runtime_engine),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-blocks",
            &[
                "frontstage.blocks.open", "frontstage.blocks.view", "frontstage.blocks.create",
                "frontstage.blocks.search", "frontstage.blocks.update", "frontstage.blocks.delete",
                "frontstage.blocks.move", "frontstage.blocks.code.view",
                "frontstage.blocks.code.update", "frontstage.blocks.runtime.view",
            ],
            &["api-server.console-frontstage-blocks"],
            crate::routes::frontstage::block_tree::interface::compile_registry(
                crate::routes::frontstage::block_tree::interface::FrontstageBlocksDependencies {
                    block_code_cache: control_plane::frontstage::BlockCodeCache(state.infrastructure.cache_store()),
                    store: state.store.clone(),
                    api_node_id: state.api_node_id.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-callable-catalog",
            &[
                "frontstage.callable_interfaces.list",
                "frontstage.callable_interfaces.view",
            ],
            &["api-server.console-frontstage-callable-catalog"],
            crate::routes::frontstage::callable_interface_catalog::compile_registry(
                crate::routes::frontstage::callable_interface_catalog::port(
                    crate::routes::frontstage::callable_interface_catalog::FrontstageCallableCatalogDependencies {
                        openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                            store: state.store.clone(),
                            console_operations: state.console_operation_registry.inventory().clone(),
                            interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()),
                            api_docs: Arc::clone(&state.api_docs),
                            template_catalog: state.runtime_engine.template_catalog().clone(),
                        },
                    },
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-frontstage-callable-dispatch",
            &["frontstage.callable_interfaces.dispatch"],
            &["api-server.console-frontstage-callable-dispatch"],
            crate::routes::frontstage::callable_interface_dispatch::compile_registry(
                crate::routes::frontstage::callable_interface_dispatch::port(
                    crate::routes::frontstage::callable_interface_dispatch::FrontstageCallableDispatchDependencies {
                        store: state.store.clone(),
                        openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                            store: state.store.clone(),
                            console_operations: state.console_operation_registry.inventory().clone(),
                            interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()),
                            api_docs: Arc::clone(&state.api_docs),
                            template_catalog: state.runtime_engine.template_catalog().clone(),
                        },
                        dispatcher: Arc::clone(&console_frontstage_callable_dispatch),
                    },
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-runtime-i18n",
            &["i18n.catalog.view"],
            &["api-server.console-runtime-i18n"],
            crate::routes::runtime_i18n_catalog::compile_registry(
                state.store.clone(),
                state.bootstrap_workspace_id,
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-application-management",
            &[access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION],
            &["api-server.console-application-management"],
            crate::routes::application_management::compile_registry(state.store.clone())?,
        ),
    ])
}
