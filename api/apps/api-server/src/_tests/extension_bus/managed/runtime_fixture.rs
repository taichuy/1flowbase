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
        Self::with_artifact_resolver(
            state,
            Arc::new(ApiRuntimeArtifactResolver::new(
                state.store.clone(),
                &state.api_node_id,
                &state.provider_install_root,
            )),
        )
    }

    pub(crate) fn with_artifact_resolver(
        state: &crate::app_state::ApiState,
        resolver: Arc<dyn runtime_extension_host::RuntimeArtifactResolver>,
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
        let host = Arc::new(
            runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver(
                time::OffsetDateTime::now_utc(),
                resolver,
            )
            .unwrap(),
        );
        host.mark_ready().unwrap();
        let services = Arc::new(
            ApiRuntimeServices::new_with_runtime_backend(host.clone(), graph)
                .unwrap()
                .with_managed_composition(
                    store.clone(),
                    state.api_node_id.clone(),
                    assembly.module_descriptors().to_vec(),
                ),
        );
        let composition = services.managed_composition().unwrap();
        composition.attach_native_lifecycle_plan(native).unwrap();
        catalog
            .attach_workspace_source(Arc::new(ManagedWorkspacePublicationSource(Arc::downgrade(
                &composition,
            ))))
            .unwrap();
        Self {
            host,
            services,
            delivery: Arc::new(delivery.with_managed(&composition)),
            composition,
            store,
        }
    }
}
