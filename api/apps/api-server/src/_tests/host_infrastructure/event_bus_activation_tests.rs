use super::*;
use control_plane_contracts::ports::EphemeralValueRevealMode;
use serde_json::json;

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

fn activate_builtin_without_database() -> HostInfrastructureRegistry {
    let hosts = declared(LOCAL_PROVIDER_SOURCE, LOCAL_PROVIDER_CODE);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let mut registry = HostInfrastructureRegistry::default();
    builtin_event_bus_factories()
        .unwrap()
        .activate(&hosts, pool, &mut registry)
        .unwrap();
    assert_eq!(registry.default_provider("event-bus"), Some("local"));
    registry
}

#[tokio::test]
async fn builtin_live_events_poll_fifo_per_topic_without_database() {
    let registry = activate_builtin_without_database();
    let bus = registry.event_bus();
    assert_eq!(bus.poll("missing").await.unwrap(), None);
    for sequence in 0..3 {
        for topic in ["first", "second"] {
            bus.publish(topic, json!({"topic": topic, "sequence": sequence}))
                .await
                .unwrap();
        }
    }
    // Drain in a different topic order from publication to detect topic mixing.
    for topic in ["second", "first"] {
        for sequence in 0..3 {
            assert_eq!(
                bus.poll(topic).await.unwrap(),
                Some(json!({"topic": topic, "sequence": sequence}))
            );
        }
        assert_eq!(bus.poll(topic).await.unwrap(), None);
    }
    assert!(bus.list_ephemeral_entries().await.unwrap().is_empty());
}

#[tokio::test]
async fn builtin_live_events_preserve_complete_large_json_without_database() {
    let registry = activate_builtin_without_database();
    let bus = registry.event_bus();
    let body = json!({"nul\u{0}key":[u64::MAX,"原文\u{0}".repeat(200_000),null]});
    assert!(serde_json::to_vec(&body).unwrap().len() > 1024 * 1024);
    bus.publish("large", body.clone()).await.unwrap();
    assert_eq!(bus.poll("large").await.unwrap(), Some(body));
    assert_eq!(bus.poll("large").await.unwrap(), None);
}

#[tokio::test]
async fn builtin_live_events_inspect_and_reveal_without_database() {
    let registry = activate_builtin_without_database();
    let bus = registry.event_bus();
    let body = json!({"nul\u{0}key": [u64::MAX, "完整正文\u{0}", null]});
    bus.publish("inspect", body.clone()).await.unwrap();
    let entries = bus.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.group_code.as_deref(), Some("inspect"));
    let revealed = bus
        .reveal_ephemeral_entry(&entry.entry_ref, EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revealed.metadata, *entry);
    assert_eq!(revealed.value, Some(body.clone()));
    // Inspection must leave the live queue intact.
    assert_eq!(bus.poll("inspect").await.unwrap(), Some(body));
    assert!(bus.list_ephemeral_entries().await.unwrap().is_empty());
    assert!(
        bus.reveal_ephemeral_entry(&entry.entry_ref, EphemeralValueRevealMode::Full)
            .await
            .unwrap()
            .is_none()
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
