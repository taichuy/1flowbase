use super::*;

pub(super) async fn app_and_runtime_host_from_config(
    config: &ApiConfig,
) -> Result<(Router, ApiRuntimeShutdown)> {
    let durable = storage_durable_postgres::build_main_durable_postgres_with_connection_settings(
        &config.database_url,
        config.database_connection_settings.clone(),
    )
    .await?;
    let store = durable
        .store
        .clone()
        .with_runtime_table_name_policy(config.runtime_table_name_policy.clone());
    console_policy_migration::require_runtime_console_policy_cutover(&store).await?;
    let prepared_host_extensions = prepare_host_extensions_at_startup(
        &store,
        &config.api_node_id,
        &config.provider_install_root,
        &config.host_extension_dropin_root,
        config.allow_unverified_filesystem_dropins,
    )
    .await?;
    let mut extension_assembly = extension_bus::assemble_extension_graph_input(
        api_workspace_root()?,
        extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )?;
    extension_assembly
        .extend_active_host_extensions(prepared_host_extensions.graph_extensions())?;
    let extension_graph = Arc::new(extension_assembly.compile_graph()?);
    let lifecycle_plan = extension_assembly.compile_lifecycle_subscriber_plan(&extension_graph)?;
    let extension_boot_snapshot = Arc::new(extension_bus::compile_extension_boot_snapshot(
        Arc::clone(&extension_graph),
        &extension_assembly,
    )?);
    let managed_base_modules = extension_assembly.module_descriptors().to_vec();
    let active_host_extensions = extension_assembly.into_host_extension_manifests();
    let host_extension_registry =
        control_plane::host_extension_boot::register_builtin_host_extension_contributions(
            &active_host_extensions,
        )?;
    let mut infrastructure = build_host_infrastructure_from_host_extensions(
        &host_extension_registry,
        &extension_graph,
        store.pool().clone(),
    )?;
    let capsule_store = storage_durable_postgres::PgProviderProtocolCapsuleStore::new(
        store.pool().clone(),
        config.provider_secret_master_key.clone(),
        time::Duration::days(7),
    )?;
    infrastructure.set_provider_transport_store(Arc::new(
        host_infrastructure::LayeredProviderTransportStore::new(
            infrastructure.provider_transport_store(),
            Arc::new(capsule_store),
        ),
    ));
    let infrastructure = Arc::new(infrastructure);
    let notification_publication = Arc::new(
        host_extensions::notification_publication::HostOptionalNotificationPublication::default(),
    );
    let (lifecycle_delivery, lifecycle_publication_catalog) =
        host_extensions::lifecycle::ApiLifecycleFactDelivery::bind(
            &lifecycle_plan,
            host_extensions::lifecycle::production_lifecycle_handler_factories(
                notification_publication.clone(),
            )?
            .activate(&active_host_extensions)?,
        )?;
    let store = store.with_lifecycle_publication_catalog(lifecycle_publication_catalog.clone());
    notification_publication.activate(Arc::new(store.clone()))?;
    let session_store = infrastructure
        .session_store()
        .expect("storage-ephemeral default provider must provide session store");
    let runtime_event_stream = infrastructure
        .runtime_event_stream()
        .expect("runtime-event-stream default provider must be registered");

    let root_password_hash = Argon2::default()
        .hash_password(config.bootstrap_root_password.as_bytes())
        .map_err(|err| anyhow::anyhow!("failed to hash bootstrap root password: {err}"))?
        .to_string();
    let file_storage_registry = Arc::new(storage_object::builtin_driver_registry());

    let bootstrap_result = BootstrapService::new(store.clone())
        .run_with_official_catalog_loader(
            &BootstrapConfig {
                workspace_name: config.bootstrap_workspace_name.clone(),
                root_account: config.bootstrap_root_account.clone(),
                root_email: config.bootstrap_root_email.clone(),
                root_password_hash,
                root_name: config.bootstrap_root_name.clone(),
                root_nickname: config.bootstrap_root_nickname.clone(),
            },
            official_i18n_catalog_seed::load_official_i18n_catalog_seed,
        )
        .await?;
    let default_storage = if let Some(existing) =
        <storage_durable_postgres::MainDurableStore as control_plane::ports::FileManagementRepository>::get_default_file_storage(&store)
            .await?
    {
        existing
    } else {
        control_plane::file_management::FileStorageService::new(store.clone())
            .create_storage(control_plane::file_management::CreateFileStorageCommand {
                actor_user_id: bootstrap_result.root_user_id,
                code: "local_default".into(),
                title: "Local".into(),
                driver_type: "local".into(),
                enabled: true,
                is_default: true,
                config_json: serde_json::json!({
                    "root_path": config.business_file_local_root.clone(),
                    "public_base_url": null
                }),
                rule_json: serde_json::json!({}),
            })
            .await?
    };
    control_plane::file_management::FileManagementBootstrapService::new(store.clone())
        .ensure_builtin_attachments(
            bootstrap_result.root_user_id,
            default_storage.id,
            "attachments",
        )
        .await?;
    let system_metadata_bootstrap =
        control_plane::system_metadata::SystemMetadataBootstrapService::new(store.clone());
    system_metadata_bootstrap
        .ensure_builtin_user_and_role_models(bootstrap_result.root_user_id)
        .await?;
    system_metadata_bootstrap
        .ensure_builtin_model_pricing_rules(bootstrap_result.root_user_id)
        .await?;
    let builtin_pricing_summary = model_pricing_catalog::install_pricing_rules_if_absent(
        &store,
        bootstrap_result.root_user_id,
        model_pricing_catalog::builtin_pricing_rules()?,
    )
    .await
    .map_err(|error| error.0)?;
    let image_pricing_rules = model_pricing_catalog::load_bootstrap_pricing_rules(
        std::path::Path::new(&config.model_pricing_bootstrap_root),
    )?;
    let image_pricing_summary = model_pricing_catalog::install_pricing_rules_if_absent(
        &store,
        bootstrap_result.root_user_id,
        image_pricing_rules,
    )
    .await
    .map_err(|error| error.0)?;
    tracing::info!(
        builtin_inserted = builtin_pricing_summary.inserted,
        builtin_skipped = builtin_pricing_summary.skipped,
        image_inserted = image_pricing_summary.inserted,
        image_skipped = image_pricing_summary.skipped,
        "model pricing startup sources installed without updating existing rules"
    );
    system_metadata_bootstrap
        .ensure_builtin_runtime_read_model_grants(
            bootstrap_result.root_user_id,
            bootstrap_result.workspace_id,
        )
        .await?;
    control_plane::mcp_management::McpManagementService::new(store.clone())
        .read_workspace_catalog(bootstrap_result.root_user_id)
        .await?;
    let process_started_at = OffsetDateTime::now_utc();
    let runtime_artifact_resolver = Arc::new(ApiRuntimeArtifactResolver::new(
        store.clone(),
        config.api_node_id.clone(),
        config.provider_install_root.clone(),
    ));
    // Both logical profiles and the process list observe this one OS source.
    // Match the existing console profile freshness without a background timer.
    let runtime_sample_source = Arc::new(runtime_profile::RuntimeSampleSource::new(
        std::time::Duration::from_secs(1),
    ));
    let file_import_port = Arc::new(resource_import::ApiFileImportPort::default());
    let runtime_extension_host = Arc::new(
        runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver_plugin_data_and_profile_source(
            process_started_at,
            runtime_artifact_resolver,
            Arc::new(control_plane::plugin_management::ManagedPluginDataService::new(Arc::new(store.clone()), Arc::new(store.clone()))),
            Arc::clone(&runtime_sample_source),
        )?.with_file_import(file_import_port.clone()).with_file_read(file_import_port.clone()).with_application_logs(file_import_port.clone()).with_http_egress(file_import_port.clone()).with_plugin_credentials(Arc::new(
            control_plane::plugin_management::ManagedPluginCredentialService::new(
                Arc::new(store.clone()),
                Arc::new(storage_durable_postgres::PgPluginCredentialRepository::new(
                    store.pool().clone(), config.provider_secret_master_key.clone(),
                )?),
            ),
        )),
    );
    runtime_extension_host
        .configure_provider_worker_idle_grace(config.provider_worker_idle_grace)
        .await?;
    let mut runtime_backend_slot = runtime_core::runtime_backend::RuntimeBackendSlot::default();
    runtime_backend_slot.bind(runtime_extension_host.clone())?;
    let runtime_backend = runtime_backend_slot.backend()?;
    let provider_runtime = Arc::new(
        ApiRuntimeServices::new_with_runtime_backend_and_transport_config(
            runtime_backend,
            Arc::clone(&extension_graph),
            config.transport_session_registry.clone(),
        )?
        .with_managed_composition(
            store.clone(),
            config.api_node_id.clone(),
            managed_base_modules,
        ),
    );
    let managed_composition = provider_runtime.managed_composition()?;
    managed_composition.attach_native_lifecycle_plan(lifecycle_plan.clone())?;
    lifecycle_publication_catalog.attach_workspace_source(Arc::new(
        extension_bus::ManagedWorkspacePublicationSource(Arc::downgrade(&managed_composition)),
    ))?;
    let lifecycle_delivery = lifecycle_delivery.with_managed(&managed_composition);
    let lifecycle_worker =
        control_plane::lifecycle_outbox_dispatcher::LifecycleOutboxDispatcher::new(
            store.clone(),
            Arc::new(lifecycle_delivery),
            Arc::new(ApiLifecycleDeliveryCompletion(Arc::downgrade(
                &managed_composition,
            ))),
        )
        .spawn();
    extension_boot_snapshot.attach_managed_composition(provider_runtime.managed_composition()?)?;
    let api_provider_runtime = ApiProviderRuntime::new(provider_runtime.clone());
    let data_model_template_catalog = provider_runtime.data_model_template_catalog();
    let runtime_registry = runtime_core::runtime_model_registry::RuntimeModelRegistry::default();
    let runtime_metadata = store.list_runtime_model_metadata().await?;
    runtime_registry.rebuild(runtime_metadata);
    let runtime_engine = Arc::new(
        runtime_core::runtime_engine::RuntimeEngine::new_with_data_source_backend_templates_and_ordered_tree(
            runtime_registry,
            Arc::new(store.clone()),
            Arc::new(store.clone()),
            Arc::new(ApiDataSourceRuntimeRecordBackend::new(
                store.clone(),
                api_provider_runtime.clone(),
                config.provider_secret_master_key.clone(),
                config.api_node_id.clone(),
            )),
            data_model_template_catalog,
        ).with_computed_field_resolver(Arc::new(control_plane::file_management::FilePreviewProjector::new(store.clone()))),
    );
    let resolved_official_mcp_bundle_source = config.resolve_official_mcp_bundle_source();
    let trusted_public_keys = config.official_plugin_trusted_public_keys()?;
    let network_egress_http_clients =
        Arc::new(network_egress_client::NetworkEgressHttpClientResolver::new(
            store.clone(),
            api_provider_runtime.clone(),
            config.provider_secret_master_key.clone(),
            config.api_node_id.clone(),
        ));
    let api_provider_runtime =
        api_provider_runtime.with_network_egress(Arc::clone(&network_egress_http_clients));
    let official_extension_catalog_source = Arc::new(
        official_extension_catalog::ApiOfficialExtensionCatalogSource::from_config(config)
            .with_network_egress(network_egress_http_clients.as_ref().clone()),
    );
    let official_plugin_source = Arc::new(
        official_extension_catalog::ApiOfficialRuntimeExtensionSource::new(
            official_extension_catalog_source.clone(),
            if config.official_plugin_signature_required {
                "signature_required".to_string()
            } else {
                "allow_unsigned".to_string()
            },
            trusted_public_keys.clone(),
        ),
    );
    let official_mcp_bundle_source =
        Arc::new(official_mcp_bundles::ApiOfficialMcpBundleRegistry::new(
            resolved_official_mcp_bundle_source,
            official_mcp_bundles::OfficialMcpBundleRegistryDependencies {
                root: std::path::PathBuf::from(&config.mcp_template_library_root),
                installation_repository: Arc::new(store.clone()),
                node_id: config.api_node_id.clone(),
                actor_user_id: bootstrap_result.root_user_id,
                trusted_public_keys,
                network_egress: network_egress_http_clients.as_ref().clone(),
                workspace_id: bootstrap_result.workspace_id,
            },
        ));
    if let Err(error) = official_mcp_bundle_source
        .reconcile_local_installations()
        .await
    {
        tracing::warn!(
            error = %error,
            "MCP template installation reconciliation unavailable; core startup continues"
        );
    }
    let official_i18n_catalog_update_service = build_official_i18n_catalog_update_service(
        store.clone(),
        config,
        network_egress_http_clients.as_ref().clone(),
        bootstrap_result.workspace_id,
    );
    let plugin_management = control_plane::plugin_management::PluginManagementService::new(
        store.clone(),
        ApiProviderRuntime::new(provider_runtime.clone()),
        official_plugin_source.clone(),
        config.provider_install_root.clone(),
    )
    .with_node_id(config.api_node_id.clone())
    .with_managed_activation()
    .with_allow_uploaded_host_extensions(config.allow_uploaded_host_extensions);
    plugin_management
        .ensure_builtin_plugin(
            control_plane::plugin_management::EnsureBuiltinPluginCommand {
                actor_user_id: bootstrap_result.root_user_id,
                package_root: builtin_frontstage_code_block_package_root()?
                    .display()
                    .to_string(),
            },
        )
        .await?;
    plugin_management.reconcile_all_installations().await?;
    intake_active_data_source_templates_at_startup(
        &store,
        &api_provider_runtime,
        &config.api_node_id,
        &config.provider_install_root,
    )
    .await?;
    let generation_host_contributions = active_host_extensions
        .iter()
        .map(|(_, contribution)| contribution.clone())
        .collect::<Vec<_>>();
    let console_host_extensions = active_host_extensions
        .iter()
        .map(|(_, contribution)| {
            resolve_linked_host_extension_console_contribution(
                contribution.clone(),
                linked_host_console_route_sources(),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let authenticator_registry = Arc::new(
        control_plane::auth::AuthenticatorRegistry::from_host_extensions(&host_extension_registry)?,
    );
    let interface_snapshot = extension_boot_snapshot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry is absent at production boot"))?
        .snapshot();
    let managed_services = crate::managed_services::load(&store, &config.api_node_id).await?;
    let mut docs_document = serde_json::to_value(openapi::ApiDoc::openapi())?;
    crate::managed_services::append_openapi(&mut docs_document, &managed_services);
    let api_docs = Arc::new(
        openapi_docs::build_api_docs_registry_with_cookie_name(docs_document, &config.cookie_name)?
            .with_default_cache()?,
    );
    let mut compiled_console_plan = app_state::compile_console_boot_plan_with_managed_services(
        console_host_extensions,
        Some(interface_snapshot.as_ref()),
        config.plugin_upload_max_bytes,
        managed_services,
    )?;
    control_plane::role::sync_console_permission_catalog(
        &store,
        compiled_console_plan.console_operation_registry.inventory(),
    )
    .await?;
    compiled_console_plan.console_surface_registry = Arc::new(
        compiled_console_plan
            .console_surface_registry
            .as_ref()
            .clone()
            .with_native_targets(prepared_host_extensions.native_targets()),
    );
    prepared_host_extensions
        .apply_templates(&store, &config.api_node_id)
        .await?;
    let boot_result: Result<(Router, ApiRuntimeShutdown)> = async {

    let runtime_activity = Arc::new(runtime_activity::ApplicationRuntimeActivityTracker::default());
    let assistant_conversation_events =
        Arc::new(routes::assistant::conversation_events::AssistantConversationEventHub::default());
    let notification_events = Arc::new(routes::notifications::events::NotificationEventHub::default());
    let notification_service = Arc::new(control_plane::notifications::NotificationService::new(
        store.clone(), config.provider_secret_master_key.clone(),
    ).with_events(notification_events.clone())
        .with_provider(Arc::new(notification_runtime::ManagedNotificationProvider::new(managed_composition.clone()))));
    notification_service.bootstrap().await?;
    let system_maintenance = Arc::new(control_plane::system_recovery::SystemMaintenance::default());
    let system_backup = resolve_system_backup_startup(
        system_backup::SystemBackupRuntime::open(
            store.clone(),
            file_storage_registry.clone(),
            system_maintenance.clone(),
            config,
        )
        .await
        .map(Arc::new),
    )?;
    let api_runtime_profile = Arc::new(HostApiRuntimeProfileCollector::new_with_sample_source(
        process_started_at,
        Arc::clone(&runtime_sample_source),
    )?);
    let runtime_process_sampler = Arc::new(runtime_profile::RuntimeProcessSampler::with_sample_source(
        runtime_sample_source,
    ));
    if !provider_runtime
        .model_provider_extension_graph()
        .is_some_and(|graph| Arc::ptr_eq(graph, extension_boot_snapshot.graph_arc()))
    {
        anyhow::bail!("model provider slot resolver must use the published extension graph");
    }
    runtime_extension_host.mark_ready()?;

    let state = Arc::new(ApiState {
        #[cfg(test)]
        test_resources: None,
        console_policy_reader: Arc::new(store.clone()),
        store: store.clone(),
        system_backup,
        system_maintenance,
        authenticator_registry,
        settings_feature_registry: compiled_console_plan.settings_feature_registry.clone(),
        console_operation_registry: compiled_console_plan.console_operation_registry.clone(),
        infrastructure,
        extension_boot_snapshot: Some(Arc::clone(&extension_boot_snapshot)),
        console_surface_registry: compiled_console_plan.console_surface_registry.clone(),
        file_storage_registry,
        runtime_engine,
        provider_runtime: provider_runtime.clone(),
        process_started_at,
        runtime_activity,
        assistant_conversation_events,
        notification_service,
        notification_events,
        assistant_executions: Default::default(),
        assistant_client_sessions: Default::default(),
        api_runtime_profile,
        runtime_host_system: runtime_extension_host.clone(),
        runtime_process_sampler,
        official_plugin_source,
        official_mcp_bundle_source,
        official_extension_catalog_source,
        official_i18n_catalog_update_service,
        official_model_pricing_catalog_index_url: config
            .official_model_pricing_catalog_index_url
            .clone(),
        model_billing_require_provider_usage: config.model_billing_require_provider_usage,
        api_node_id: config.api_node_id.clone(),
        provider_install_root: config.provider_install_root.clone(),
        application_template_root: config.application_template_root.clone(),
        provider_secret_master_key: config.provider_secret_master_key.clone(),
        host_extension_dropin_root: config.host_extension_dropin_root.clone(),
        allow_unverified_filesystem_dropins: config.allow_unverified_filesystem_dropins,
        allow_uploaded_host_extensions: config.allow_uploaded_host_extensions,
        session_store,
        runtime_event_stream,
        api_docs,
        cookie_name: config.cookie_name.clone(),
        cookie_secure: config.cookie_secure,
        session_ttl_days: config.session_ttl_days,
        bootstrap_workspace_id: bootstrap_result.workspace_id,
        bootstrap_workspace_name: config.bootstrap_workspace_name.clone(),
    });
    extension_boot_snapshot.publish_complete_catalog(&state)?;
    managed_composition.refresh_boot_snapshots().await?;
    extension_boot_snapshot.set_pinned_managed_snapshots(managed_composition.current_snapshots().await)?;
    let builtin_mcp_interfaces =
        openapi_interface::build_openapi_capability_catalog(&state, bootstrap_result.workspace_id)
            .await
            .map_err(|error| error.0)?
            .into_iter()
            .map(routes::mcp_management::mcp_interface_entry_from_capability)
            .collect();
    control_plane::mcp_management::McpManagementService::new(state.store.clone())
        .seed_builtin_bundle_once(
            control_plane::mcp_bundle::SeedBuiltinMcpBundleCommand {
                actor_user_id: bootstrap_result.root_user_id,
                workspace_id: bootstrap_result.workspace_id,
                package: official_mcp_bundles::ApiOfficialMcpBundleRegistry::bundled_frontstage_assistant_package()?,
                interface_catalog: builtin_mcp_interfaces,
            },
        )
        .await?;
    if config.application_template_auto_update {
        if let Err(error) = routes::system_templates::releases::synchronize_at_startup(
            &state, bootstrap_result.root_user_id, bootstrap_result.workspace_id,
        ).await {
            tracing::error!(error = ?error, "application template discovery failed; retry on next startup");
        }
    }
    crate::workers::workflow_schedule::spawn_workflow_schedule_loops(state.clone());
    crate::workers::provider_request_logs::spawn_provider_request_log_worker(state.clone());
    crate::workers::trace_projection::spawn_trace_projection_worker(state.clone());
    crate::workers::application_log_deletion::spawn(state.clone());
    crate::workers::billing::spawn_billing_worker(state.clone());
    provider_runtime.start_transport_session_scheduler();
    #[cfg(not(test))]
    spawn_default_ui_component_catalog_bootstrap(
        state.store.clone(),
        bootstrap_result.root_user_id,
        state.network_egress_http_clients(),
        bootstrap_result.workspace_id,
    );

    let external_openapi_document = openapi::dynamic_openapi_document(&state)
        .await
        .map_err(|error| error.0)?;
    let initial_router = app_with_state_and_config_and_console_route_assembly(
        state.clone(), config, compiled_console_plan.route_assembly, &external_openapi_document,
    );
    let generation_publisher = managed_publication::ManagedGenerationPublisher::new(
        &state, config.clone(), generation_host_contributions, initial_router,
    )?;
    file_import_port.bind(&generation_publisher)?;
    managed_composition.attach_generation_publisher(Arc::downgrade(&generation_publisher))?;
    let communication_worker = crate::communication_runtime::CommunicationWorker::spawn(state.clone());
    Ok((
        generation_publisher.ingress(),
        ApiRuntimeShutdown {
            host: runtime_extension_host,
            services: provider_runtime,
            operations: store.managed_operation_lifetime(),
            lifecycle_worker,
            communication_worker,
        },
    ))
    }.await;
    match boot_result {
        Ok(result) => {
            activate_prepared_host_extensions(
                &store,
                &config.api_node_id,
                prepared_host_extensions,
            )
            .await?;
            Ok(result)
        }
        Err(error) => {
            prepared_host_extensions
                .record_boot_failure(&store, &config.api_node_id, &error)
                .await;
            Err(error)
        }
    }
}

#[cfg(not(test))]
pub(super) fn spawn_default_ui_component_catalog_bootstrap(
    store: storage_durable_postgres::MainDurableStore,
    actor_user_id: uuid::Uuid,
    network_egress: network_egress_client::NetworkEgressHttpClientResolver,
    workspace_id: uuid::Uuid,
) {
    tokio::spawn(async move {
        let service = control_plane::ui_component_catalog::UiComponentCatalogService::new(
            store,
            ui_component_catalog_source::ApiUiComponentCatalogSource::default_taichuy(
                network_egress,
                workspace_id,
            ),
        );
        match service.bootstrap_empty_system(actor_user_id).await {
            Ok(control_plane::ui_component_catalog::UiComponentBootstrapOutcome::Imported {
                records,
            }) => tracing::info!(
                records,
                "default UI component catalog bootstrapped into an empty system"
            ),
            Ok(
                control_plane::ui_component_catalog::UiComponentBootstrapOutcome::SkippedNonEmpty,
            ) => {}
            Err(error) => tracing::warn!(
                error = %error,
                "default UI component catalog bootstrap failed; startup and existing records are unchanged"
            ),
        }
    });
}

pub(super) fn resolve_system_backup_startup(
    result: Result<
        Arc<system_backup::SystemBackupRuntime>,
        system_backup::SystemBackupRuntimeError,
    >,
) -> Result<Option<Arc<system_backup::SystemBackupRuntime>>, system_backup::SystemBackupRuntimeError>
{
    match result {
        Ok(runtime) => Ok(Some(runtime)),
        Err(error) => {
            tracing::warn!(
                capability = "system_backup",
                error_code = "system_backup_unavailable",
                error = %error,
                "system backup initialization failed; system backup APIs are disabled"
            );
            Ok(None)
        }
    }
}

pub(super) fn api_workspace_root() -> Result<PathBuf> {
    if let Some(root) = std::env::var_os("API_WORKSPACE_ROOT") {
        let root = PathBuf::from(root);
        if !root.join("plugins/host-extensions").is_dir() {
            return Err(anyhow!(
                "API_WORKSPACE_ROOT must contain plugins/host-extensions"
            ));
        }
        return Ok(root);
    }
    let current_dir = std::env::current_dir()?;
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        current_dir.clone(),
        current_dir.join("api"),
    ];

    for candidate in candidates {
        if candidate.join("plugins/host-extensions").is_dir() {
            return Ok(candidate);
        }
    }

    Err(anyhow!(
        "api workspace root with plugins/host-extensions was not found"
    ))
}

pub(super) fn builtin_frontstage_code_block_package_root() -> Result<PathBuf> {
    let package_root = api_workspace_root()?.join("plugins/capability-plugins/1flowbase");
    if package_root.join("manifest.yaml").is_file() {
        return Ok(package_root);
    }

    Err(anyhow!(
        "builtin Frontstage code block package manifest was not found at {}",
        package_root.join("manifest.yaml").display()
    ))
}
