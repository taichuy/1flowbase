#[test]
fn local_infra_host_provides_required_defaults() {
    let registry = crate::host_infrastructure::build_local_host_infrastructure();

    assert_eq!(
        registry.default_provider("storage-ephemeral").unwrap(),
        "local"
    );
    assert_eq!(registry.default_provider("cache-store").unwrap(), "local");
    assert_eq!(registry.default_provider("event-bus").unwrap(), "local");
    assert_eq!(
        registry.default_provider("runtime-event-stream").unwrap(),
        "local"
    );
    assert!(registry.session_store().is_some());
    assert!(registry.registered_cache_store().is_some());
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

#[test]
fn local_infra_host_can_be_built_from_builtin_host_extension_contributions() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifests =
        crate::host_extensions::builtin::load_builtin_host_extension_manifests(workspace_root)
            .unwrap();
    let host_extensions =
        control_plane::host_extension_boot::register_builtin_host_extension_contributions(
            &manifests,
        )
        .unwrap();
    let registry =
        crate::host_infrastructure::build_local_host_infrastructure_from_host_extensions(
            &host_extensions,
        )
        .unwrap();

    assert_eq!(registry.default_provider("cache-store").unwrap(), "local");
    assert_eq!(
        registry.default_provider_source("cache-store").unwrap(),
        "official.local-infra-host"
    );
    assert!(registry.session_store().is_some());
    assert!(registry.runtime_event_stream().is_some());
}

#[test]
fn empty_infra_registry_reports_contracts_as_unregistered() {
    let registry = crate::host_infrastructure::HostInfrastructureRegistry::default();

    assert!(registry.session_store().is_none());
    assert!(registry.registered_cache_store().is_none());
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
