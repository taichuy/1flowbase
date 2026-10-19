use super::*;

pub(super) fn contributions(
    state: &Arc<crate::app_state::ApiState>,
    mcp_interface_catalog_dependencies: &crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies,
) -> Result<Vec<InterfaceRegistryContribution>, RegistryCompilationError> {
    Ok(vec![
        InterfaceRegistryContribution::new(
            "api-server.console-system",
            &[
                "system.runtime_profile.view",
                "system.runtime_processes.view",
                "system.release_status.view",
                "system.runtime_process.terminate",
            ],
            &["api-server.console-system"],
            crate::routes::system::compile_registry(crate::routes::system::SystemInterfaceDependencies {
                store: state.store.clone(),
                profiles: crate::runtime_profile_client::RuntimeProfileSnapshotCache::new(
                    state.infrastructure.cache_store(), state.infrastructure.distributed_lock(),
                    Arc::clone(&state.api_runtime_profile), Arc::clone(&state.runtime_host_system),
                    state.api_node_id.clone(), state.process_started_at,
                ),
                api_node_id: state.api_node_id.clone(),
                provider_install_root: state.provider_install_root.clone(),
                host_extension_dropin_root: state.host_extension_dropin_root.clone(),
                process_sampler: Arc::clone(&state.runtime_process_sampler),
                network_egress: state.network_egress_http_clients(),
            })?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-system-templates",
            &["system_templates.catalog", "system_templates.export", "system_templates.preview", "system_templates.install"],
            &["api-server.console-system-templates"],
            crate::routes::system_templates::interface::compile_registry(
                crate::routes::system_templates::plugins::TemplateDependencies {
                    bootstrap_workspace_id: state.bootstrap_workspace_id,
                    application_template_root: state.application_template_root.clone(),
                    store: state.store.clone(), provider_runtime: state.provider_runtime.clone(),
                    runtime_registry_sync: crate::runtime_registry_sync::ApiRuntimeRegistrySync::new(
                        state.store.clone(), state.runtime_engine.registry().clone(),
                    ),
                    official_plugin_source: state.official_plugin_source.clone(),
                    official_catalog_source: state.official_extension_catalog_source.clone(),
                    cache_store: state.infrastructure.cache_store(),
                    provider_install_root: state.provider_install_root.clone(), api_node_id: state.api_node_id.clone(),
                    mcp_interface_catalog: mcp_interface_catalog_dependencies.clone(),
                }
            )?,
        ),
        InterfaceRegistryContribution::new(
            "api-server.console-system-backups",
            &[
                "system_backups.catalog",
                "system_backups.list",
                "system_backups.create",
                "system_backups.import",
                "system_backups.recovery.status",
                "system_backups.status",
                "system_backups.detail",
                "system_backups.delete",
                "system_backups.verify",
                "system_backups.download",
                "system_backups.recovery.preflight",
                "system_backups.recovery.reauth",
                "system_backups.recovery.intent",
            ],
            &["api-server.console-system-backups"],
            crate::routes::system_backups::interface::compile_registry(
                crate::routes::system_backups::interface::port(
                    crate::routes::system_backups::interface::SystemBackupsDependencies {
                        runtime: state.system_backup.clone(),
                        store: state.store.clone(),
                        backup_status_access: {
                            let access = crate::middleware::require_settings_feature_permission::compiled_console_route_access(
                                &state.console_operation_registry,
                                "GET",
                                "/api/console/settings/system-backups/jobs/status/:backup_job_id",
                            )
                            .map_err(|_| {
                                RegistryCompilationError::UnknownAuthorizationOperation(
                                    interface_runtime::InterfaceId::new("system_backups.status")
                                        .expect("static system backup status operation is valid"),
                                )
                            })?;
                            crate::routes::system_backups::interface::BackupStatusAccess {
                                operation_id: access.operation_id.to_owned(),
                                policy_group: access.policy_group.clone(),
                                authorization: access.authorization.clone(),
                                resource_access: access.resource_access.cloned(),
                            }
                        },
                    },
                ),
            )?,
        ),
    ])
}
