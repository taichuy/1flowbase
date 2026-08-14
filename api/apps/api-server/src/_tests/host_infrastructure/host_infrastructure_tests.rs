#[test]
fn local_infra_host_provides_required_defaults() {
    let registry = crate::host_infrastructure::build_local_host_infrastructure();

    assert_eq!(
        registry.default_provider("storage-ephemeral").unwrap(),
        "local"
    );
    assert_eq!(registry.default_provider("cache-store").unwrap(), "local");
    assert_eq!(
        registry
            .default_provider("provider-transport-store")
            .unwrap(),
        "local"
    );
    assert_eq!(registry.default_provider("event-bus").unwrap(), "local");
    assert_eq!(
        registry.default_provider("runtime-event-stream").unwrap(),
        "local"
    );
    assert!(registry.session_store().is_some());
    assert!(registry.registered_cache_store().is_some());
    assert!(registry.registered_provider_transport_store().is_some());
    assert!(registry.registered_distributed_lock().is_some());
    assert!(registry.registered_event_bus().is_some());
    assert!(registry.registered_task_queue().is_some());
    assert!(registry.registered_rate_limit_store().is_some());
    assert!(registry.runtime_event_stream().is_some());
}

#[test]
fn local_infra_host_default_provider_source_matches_builtin_extension_id() {
    let registry = crate::host_infrastructure::build_local_host_infrastructure();

    assert_eq!(
        registry.default_provider_source("cache-store").unwrap(),
        "official.local-infra-host"
    );
}

#[tokio::test]
async fn compiled_cache_winner_activates_factory_and_publishes_trait_service() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assembly = crate::extension_bus::assemble_extension_graph_input(
        workspace_root,
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )
    .unwrap();
    let graph = std::sync::Arc::new(assembly.compile_graph().unwrap());
    let manifests = assembly.into_host_extension_manifests();
    let host_extensions =
        control_plane::host_extension_boot::register_builtin_host_extension_contributions(
            &manifests,
        )
        .unwrap();
    let registry =
        crate::host_infrastructure::build_local_host_infrastructure_from_host_extensions(
            &host_extensions,
            &graph,
        )
        .unwrap();

    assert_eq!(registry.default_provider("cache-store").unwrap(), "local");
    assert_eq!(
        registry.default_provider_source("cache-store").unwrap(),
        "official.local-infra-host"
    );
    assert!(registry.session_store().is_some());
    assert!(registry.runtime_event_stream().is_some());
    let cache: std::sync::Arc<dyn crate::host_infrastructure::CacheStore> = registry.cache_store();
    cache
        .set_json(
            "extension-bus-factory",
            serde_json::json!({ "activated": true }),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        cache.get_json("extension-bus-factory").await.unwrap(),
        Some(serde_json::json!({ "activated": true }))
    );
}

#[test]
fn missing_cache_factory_fails_before_registry_is_returned() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assembly = crate::extension_bus::assemble_extension_graph_input(
        workspace_root,
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )
    .unwrap();
    let graph = std::sync::Arc::new(assembly.compile_graph().unwrap());
    let manifests = assembly.into_host_extension_manifests();
    let host_extensions =
        control_plane::host_extension_boot::register_builtin_host_extension_contributions(
            &manifests,
        )
        .unwrap();
    let factories = crate::host_infrastructure::CacheStoreActivationFactoryRegistry::default();

    let activation = crate::host_infrastructure::build_local_host_infrastructure_from_host_extensions_with_cache_factories(
        &host_extensions,
        &graph,
        &factories,
    );
    let (error, published_snapshot) = match activation {
        Ok(_) => (
            None,
            Some(crate::extension_bus::ExtensionBootSnapshot::new(graph)),
        ),
        Err(error) => (Some(error), None),
    };

    assert!(published_snapshot.is_none());
    assert!(error
        .unwrap()
        .to_string()
        .contains("no cache-store activation factory registered for winner"));
}

#[test]
fn host_infrastructure_consumer_does_not_branch_on_local_or_moka_types() {
    let source = include_str!("../../host_infrastructure/mod.rs");
    let consumer = source
        .split_once("impl HostInfrastructureRegistry")
        .unwrap()
        .1;

    assert!(!consumer.contains("MokaCacheStore"));
    assert!(!consumer.contains("\"local\""));
}

#[test]
fn empty_infra_registry_reports_contracts_as_unregistered() {
    let registry = crate::host_infrastructure::HostInfrastructureRegistry::default();

    assert!(registry.session_store().is_none());
    assert!(registry.registered_cache_store().is_none());
    assert!(registry.registered_provider_transport_store().is_none());
    assert!(registry.registered_distributed_lock().is_none());
    assert!(registry.registered_event_bus().is_none());
    assert!(registry.registered_task_queue().is_none());
    assert!(registry.registered_rate_limit_store().is_none());
    assert!(registry.runtime_event_stream().is_none());
}

#[test]
fn duplicate_default_provider_is_rejected() {
    let mut registry = crate::host_infrastructure::HostInfrastructureRegistry::default();
    registry
        .register_default_provider("storage-ephemeral", "local", "local-infra-host")
        .unwrap();
    let err = registry
        .register_default_provider("storage-ephemeral", "redis", "redis-infra-host")
        .unwrap_err();

    assert!(err.to_string().contains("default provider"));
}

#[tokio::test]
async fn local_infra_host_exposes_operation_contracts() {
    let registry = crate::host_infrastructure::build_local_host_infrastructure();

    let cache = registry.cache_store();
    cache
        .set_json(
            "provider-catalog",
            serde_json::json!({ "cached": true }),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        cache.get_json("provider-catalog").await.unwrap(),
        Some(serde_json::json!({ "cached": true }))
    );

    assert!(!registry
        .distributed_lock()
        .release("missing", "owner")
        .await
        .unwrap());

    let events = registry.event_bus();
    events
        .publish("runtime.debug", serde_json::json!({ "run": "1" }))
        .await
        .unwrap();
    assert_eq!(
        events.poll("runtime.debug").await.unwrap(),
        Some(serde_json::json!({ "run": "1" }))
    );

    assert!(
        registry
            .rate_limit_store()
            .consume("actor:1", 5, time::Duration::seconds(60))
            .await
            .unwrap()
            .allowed
    );

    let tasks = registry.task_queue();
    let first = tasks
        .enqueue(
            "preview",
            serde_json::json!({ "file": "a" }),
            Some("preview:file:a"),
        )
        .await
        .unwrap();
    let second = tasks
        .enqueue(
            "preview",
            serde_json::json!({ "file": "a" }),
            Some("preview:file:a"),
        )
        .await
        .unwrap();
    assert_eq!(first, second);

    let runtime_events = registry.runtime_event_stream().unwrap();
    let run_id = uuid::Uuid::now_v7();
    runtime_events
        .open_run(
            run_id,
            control_plane::ports::RuntimeEventStreamPolicy::debug_default(),
        )
        .await
        .unwrap();
    let envelope = runtime_events
        .append(
            run_id,
            control_plane::ports::RuntimeEventPayload {
                event_type: "heartbeat".to_string(),
                source: control_plane::ports::RuntimeEventSource::System,
                durability: control_plane::ports::RuntimeEventDurability::Ephemeral,
                persist_required: false,
                trace_visible: false,
                payload: serde_json::json!({ "type": "heartbeat" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(envelope.sequence, 1);
}
