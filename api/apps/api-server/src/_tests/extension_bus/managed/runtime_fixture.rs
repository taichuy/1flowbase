#[cfg_attr(test, api_test_selection::fixture(contracts))]
pub(crate) struct RuntimeFixture {
    pub(crate) host: Arc<runtime_extension_host::RuntimeExtensionHost>,
    pub(crate) services: Arc<ApiRuntimeServices>,
    pub(crate) composition: Arc<ManagedExtensionComposition>,
    pub(super) delivery: Arc<ApiLifecycleFactDelivery>,
    pub(crate) store: MainDurableStore,
}
impl RuntimeFixture {
    #[cfg_attr(test, api_test_selection::group(contracts))]
    pub(crate) fn new(state: &crate::app_state::ApiState) -> Self {
        Self::with_configured_resolver(state, false)
    }

    /// Opt in to startup_assembly's actual data/credential HostCall adapters.
    /// Existing fixture callers keep their original missing-port behavior.
    #[cfg_attr(test, api_test_selection::group(contracts))]
    pub(crate) fn with_configured_resolver(
        state: &crate::app_state::ApiState,
        managed_host_calls: bool,
    ) -> Self {
        Self::with_artifact_resolver(
            state,
            Arc::new(ApiRuntimeArtifactResolver::new(
                state.store.clone(),
                &state.api_node_id,
                &state.provider_install_root,
            )),
            managed_host_calls,
        )
    }

    pub(crate) fn with_artifact_resolver(
        state: &crate::app_state::ApiState,
        resolver: Arc<dyn runtime_extension_host::RuntimeArtifactResolver>,
        managed_host_calls: bool,
    ) -> Self {
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            crate::api_workspace_root().unwrap(),
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            vec![],
        )
        .unwrap();
        let graph = Arc::new(assembly.compile_graph().unwrap());
        let native = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let notification_publication = Arc::new(
            crate::host_extensions::notification_publication::HostOptionalNotificationPublication::default(),
        );
        let bindings = crate::host_extensions::lifecycle::production_lifecycle_handler_factories(
            notification_publication.clone(),
        )
        .unwrap()
        .activate(assembly.host_extension_manifests())
        .unwrap();
        let (delivery, catalog) = ApiLifecycleFactDelivery::bind(&native, bindings).unwrap();
        let store = state
            .store
            .clone()
            .with_lifecycle_publication_catalog(catalog.clone());
        notification_publication
            .activate(Arc::new(store.clone()))
            .unwrap();
        let host = Arc::new(if managed_host_calls {
            runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver_plugin_data_and_profile_source(
                time::OffsetDateTime::now_utc(),
                resolver,
                Arc::new(ObservedManagedPluginData(control_plane::plugin_management::ManagedPluginDataService::new(
                    Arc::new(store.clone()),
                    Arc::new(store.clone()),
                ))),
                Arc::new(runtime_profile::RuntimeSampleSource::new(
                    std::time::Duration::from_secs(1),
                )),
            )
            .unwrap()
            .with_plugin_credentials(Arc::new(
                control_plane::plugin_management::ManagedPluginCredentialService::new(
                    Arc::new(store.clone()),
                    Arc::new(storage_durable_postgres::PgPluginCredentialRepository::new(
                        store.pool().clone(),
                        state.provider_secret_master_key.clone(),
                    ).unwrap()),
                ),
            ))
            .with_execution_gate(Arc::new(
                control_plane::plugin_management::ManagedPluginExecutionGateAuthority::new(
                    Arc::new(store.clone()),
                ),
            ))
        } else {
            runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver(
                time::OffsetDateTime::now_utc(),
                resolver,
            )
            .unwrap()
        });
        host.mark_ready().unwrap();
        let (services, composition) = crate::managed_runtime_assembly::bind_managed_runtime(
            crate::provider_runtime::services_for_current_cfg(
                ApiRuntimeServices::new_with_runtime_backend(host.clone(), graph).unwrap(),
            ),
            store.clone(),
            state.api_node_id.clone(),
            assembly.module_descriptors().to_vec(),
        );
        let services = Arc::new(services);
        composition.attach_native_lifecycle_plan(native).unwrap();
        catalog
            .attach_workspace_source(Arc::new(ManagedWorkspacePublicationSource(Arc::downgrade(
                &composition,
            ))))
            .unwrap();
        Self {
            host,
            services,
            delivery: Arc::new(delivery.with_managed(
                &(composition.clone() as Arc<dyn crate::extension_bus::ManagedCompositionPort>),
            )),
            composition,
            store,
        }
    }
}

// Transparent test-only diagnostic: retain the real authorization and storage result.
struct ObservedManagedPluginData(control_plane::plugin_management::ManagedPluginDataService);
impl extension_contracts::PluginDataPort for ObservedManagedPluginData {
    fn execute<'a>(
        &'a self,
        binding: &'a extension_contracts::PluginDataBinding,
        request: &'a extension_contracts::PluginDataRequest,
    ) -> extension_contracts::PluginDataFuture<'a> {
        Box::pin(async move {
            let result =
                extension_contracts::PluginDataPort::execute(&self.0, binding, request).await;
            if let Err(error) = &result {
                eprintln!(
                    "R7_HOST_DATA_ERROR: kind={:?} code={} owner={}/{}",
                    error.kind, error.code, binding.publisher_namespace, binding.plugin_code
                );
            }
            result
        })
    }
}
