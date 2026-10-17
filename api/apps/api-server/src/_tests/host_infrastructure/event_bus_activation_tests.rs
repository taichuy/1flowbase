use super::*;

fn declared(source: &str, provider_code: &str) -> HostExtensionRegistry {
    let mut contribution =
        plugin_framework::parse_host_extension_contribution_manifest(include_str!(
            "../../../../../plugins/host-extensions/official.local-infra-host/host-extension.yaml"
        ))
        .unwrap();
    contribution.extension_id = source.into();
    contribution
        .infrastructure_providers
        .retain(|p| p.contract == "event-bus");
    contribution.infrastructure_providers[0].provider_code = provider_code.into();
    let mut hosts = HostExtensionRegistry::default();
    control_plane::host_extension_boot::register_host_extension_contribution(
        &mut hosts,
        &contribution,
    )
    .unwrap();
    hosts
}

#[tokio::test]
async fn official_factory_uses_ephemeral_live_events_and_explicit_legacy_recovery() {
    let hosts = declared(LOCAL_PROVIDER_SOURCE, LOCAL_PROVIDER_CODE);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let mut registry = HostInfrastructureRegistry::default();
    builtin_event_bus_factories()
        .unwrap()
        .activate(&hosts, pool, &mut registry)
        .unwrap();
    assert_eq!(registry.default_provider("event-bus"), Some("local"));
    assert_eq!(
        registry
            .event_bus()
            .ephemeral_inspection_capabilities()
            .storage_medium,
        Some("memory-and-legacy-postgresql")
    );
}

#[tokio::test]
async fn another_declared_provider_is_not_replaced_by_the_official_factory() {
    let hosts = declared("acme.queue-host", "acme-queue");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let mut registry = HostInfrastructureRegistry::default();
    assert!(
        builtin_event_bus_factories()
            .unwrap()
            .activate(&hosts, pool.clone(), &mut registry)
            .is_err()
    );
    assert!(registry.registered_event_bus().is_none());
    assert!(registry.default_provider("event-bus").is_none());
    let mut factories = EventBusActivationFactoryRegistry::default();
    factories
        .register("acme.queue-host", "acme-queue", |_| {
            Arc::new(storage_ephemeral::MemoryEventBus::new())
        })
        .unwrap();
    factories.activate(&hosts, pool, &mut registry).unwrap();
    assert_eq!(registry.default_provider("event-bus"), Some("acme-queue"));
}
