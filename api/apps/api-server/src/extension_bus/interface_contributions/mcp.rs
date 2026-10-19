use super::*;

pub(super) fn contributions(
    state: &Arc<crate::app_state::ApiState>,
) -> Result<Vec<InterfaceRegistryContribution>, RegistryCompilationError> {
    Ok(vec![
        InterfaceRegistryContribution::new(
            "api-server.console-mcp-core",
            &[
                "mcp.client_credential.reveal",
                "mcp.client_credential.save",
                "mcp.client_credential.delete",
                "mcp.instances.view",
                "mcp.instances.create",
                "mcp.instances.copy",
                "mcp.instances.update",
                "mcp.instances.delete",
                "mcp.groups.upsert",
                "mcp.groups.move",
                "mcp.groups.delete",
                "mcp.tool_bindings.create",
                "mcp.tool_bindings.update",
                "mcp.tool_bindings.delete",
                "mcp.discovery_policy.view",
                "mcp.discovery_policy.update",
            ],
            &["api-server.console-mcp-core"],
            crate::routes::mcp_management::interface_core::compile_registry(
                crate::routes::mcp_management::interface_core::McpCoreDependencies {
                    store: state.store.clone(),
                    provider_secret_master_key: state.provider_secret_master_key.clone(),
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-mcp-catalog",
            &["mcp.catalog.view", "mcp.catalog.export"],
            &["api-server.console-mcp-catalog"],
            crate::routes::mcp_management::interface_catalog_routes::compile_registry(
                crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
                    store: state.store.clone(),
                    openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                        store: state.store.clone(),
                        console_operations: state.console_operation_registry.inventory().clone(),
                        interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()),
                        api_docs: Arc::clone(&state.api_docs),
                        template_catalog: state.runtime_engine.template_catalog().clone(),
                    },
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-mcp-tools",
            &[
                "mcp.tools.view",
                "mcp.tools.create",
                "mcp.tools.update",
                "mcp.tools.delete",
                "mcp.tools.description.refresh",
                "mcp.tools.description.check",
            ],
            &["api-server.console-mcp-tools"],
            crate::routes::mcp_management::interface_tools::compile_registry(
                crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
                    store: state.store.clone(),
                    openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                        store: state.store.clone(),
                        console_operations: state.console_operation_registry.inventory().clone(),
                        interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()),
                        api_docs: Arc::clone(&state.api_docs),
                        template_catalog: state.runtime_engine.template_catalog().clone(),
                    },
                },
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-mcp-bundles",
            &[
                "mcp.bundles.official.list", "mcp.bundles.preview", "mcp.bundles.import",
                "mcp.bundles.export", "mcp.instances.export", "mcp.bundle_library.list",
                "mcp.bundle_library.sync", "mcp.bundle_library.preview", "mcp.bundle_library.import",
                "mcp.bundle_library.current.switch", "mcp.bundle_library.releases.delete",
                "mcp.bundle_library.releases.repair",
            ],
            &["api-server.console-mcp-bundles"],
            crate::routes::mcp_management::bundles_interface::compile_registry(
                crate::routes::mcp_management::bundles_interface::port(
                    crate::routes::mcp_management::bundles_interface::McpBundlesDependencies {
                        store: state.store.clone(),
                        official_mcp_bundle_source: state.official_mcp_bundle_source.clone(),
                        official_extension_catalog_source: state.official_extension_catalog_source.clone(),
                        provider_install_root: state.provider_install_root.clone(),
                        api_node_id: state.api_node_id.clone(),
                        bootstrap_workspace_id: state.bootstrap_workspace_id,
                        interface_catalog: crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
                            store: state.store.clone(),
                            openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                                store: state.store.clone(), console_operations: state.console_operation_registry.inventory().clone(), interface_registry: state.extension_boot_snapshot.as_ref().and_then(|snapshot| snapshot.interface_registry()).map(|registry| registry.snapshot()), api_docs: Arc::clone(&state.api_docs), template_catalog: state.runtime_engine.template_catalog().clone(),
                            },
                        },
                    },
                ),
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-mcp-upstream",
            &[
                "mcp.upstream_connections.view",
                "mcp.upstream_connections.create",
                "mcp.upstream_connections.update",
                "mcp.upstream_connections.delete",
                "mcp.upstream_credentials.update",
                "mcp.upstream_credentials.delete",
                "mcp.upstream_connections.test",
                "mcp.upstream_connections.discover",
                "mcp.upstream_tools.import",
                "mcp.upstream_tools.debug",
            ],
            &["api-server.console-mcp-upstream"],
            crate::routes::mcp_management::upstream_interface::compile_registry(
                crate::routes::mcp_management::upstream_interface::port(
                    crate::routes::mcp_management::upstream_interface::McpUpstreamDependencies {
                        store: state.store.clone(),
                        provider_secret_master_key: state.provider_secret_master_key.clone(),
                        transport: Arc::new(
                            crate::routes::mcp_management::upstream_client::StreamableHttpMcpUpstreamTransport,
                        ),
                    },
                ),
            )?,
        ),
    ])
}
