use super::*;
use control_plane_contracts::ports::{EphemeralValueRevealMode, EventBusError, EventBusLimits};
use serde_json::json;
use std::time::{Duration, Instant};

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

fn fixture_limits() -> EventBusLimits {
    EventBusLimits {
        max_items: 8,
        max_json_bytes: 16 * 1024 * 1024,
        max_waiters: 8,
        max_topics: 4,
        max_consumers: 4,
        max_topic_name_bytes: 128,
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

#[tokio::test]
async fn builtin_live_events_require_explicit_budget_and_consumers_without_database() {
    let registry = activate_builtin_without_database();
    let bus = registry.event_bus();
    assert!(bus.event_bus_snapshot().limits.is_none());
    assert!(bus.list_ephemeral_entries().await.unwrap().is_empty());
    assert!(matches!(
        bus.reserve("missing", 4, deadline()).await,
        Err(EventBusError::Unconfigured)
    ));
    bus.configure_budget(fixture_limits()).unwrap();
    assert!(matches!(
        bus.reserve("missing", 4, deadline()).await,
        Err(EventBusError::NoConsumer)
    ));
    assert_eq!(bus.event_bus_snapshot().topics, 0);
}

#[tokio::test]
async fn builtin_live_events_receive_fifo_per_topic_without_database() {
    let registry = activate_builtin_without_database();
    let bus = registry.event_bus();
    bus.configure_budget(fixture_limits()).unwrap();
    let first = bus.open_consumer("first").unwrap();
    let second = bus.open_consumer("second").unwrap();
    for sequence in 0..3 {
        for topic in ["first", "second"] {
            let permit = bus.reserve(topic, 128, deadline()).await.unwrap();
            permit
                .send(json!({"topic": topic, "sequence": sequence}))
                .unwrap();
        }
    }
    // Drain in a different topic order to detect topic mixing.
    for (topic, consumer) in [("second", second), ("first", first)] {
        consumer.close();
        for sequence in 0..3 {
            let event = consumer.recv().await.unwrap().unwrap();
            assert_eq!(event.body(), &json!({"topic": topic, "sequence": sequence}));
            event.complete();
        }
        assert!(consumer.recv().await.unwrap().is_none());
    }
    let snapshot = bus.event_bus_snapshot();
    assert_eq!(snapshot.topics, 0);
    assert_eq!(snapshot.charged_json_bytes, 0);
}

#[tokio::test]
async fn builtin_live_events_preserve_complete_large_json_without_database() {
    let registry = activate_builtin_without_database();
    let bus = registry.event_bus();
    bus.configure_budget(fixture_limits()).unwrap();
    let consumer = bus.open_consumer("large").unwrap();
    let permit = bus
        .reserve("large", 8 * 1024 * 1024, deadline())
        .await
        .unwrap();
    let body = json!({"nul\u{0}key":[u64::MAX,"原文\u{0}".repeat(200_000),null]});
    assert!(serde_json::to_vec(&body).unwrap().len() > 1024 * 1024);
    permit.send(body.clone()).unwrap();
    let event = consumer.recv().await.unwrap().unwrap();
    assert_eq!(event.body(), &body);
    event.complete();
    consumer.close();
    assert!(consumer.recv().await.unwrap().is_none());
}

#[tokio::test]
async fn builtin_live_events_inspect_and_reveal_without_database() {
    let registry = activate_builtin_without_database();
    let bus = registry.event_bus();
    bus.configure_budget(fixture_limits()).unwrap();
    let consumer = bus.open_consumer("inspect").unwrap();
    let permit = bus.reserve("inspect", 256, deadline()).await.unwrap();
    let body = json!({"nul\u{0}key": [u64::MAX, "完整正文\u{0}", null]});
    permit.send(body.clone()).unwrap();
    let entries = bus.list_ephemeral_entries().await.unwrap();
    let entry = entries
        .iter()
        .find(|entry| entry.entry_kind == "event")
        .unwrap();
    assert_eq!(entry.group_code.as_deref(), Some("inspect"));
    let revealed = bus
        .reveal_ephemeral_entry(&entry.entry_ref, EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revealed.metadata, *entry);
    assert_eq!(revealed.value, Some(body.clone()));
    let event = consumer.recv().await.unwrap().unwrap();
    assert_eq!(event.body(), &body);
    assert_eq!(bus.event_bus_snapshot().in_flight, 1);
    event.complete();
    assert!(bus
        .reveal_ephemeral_entry(&entry.entry_ref, EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .is_none());
    consumer.close();
}

#[tokio::test]
async fn another_declared_provider_is_not_replaced_by_the_official_factory() {
    let hosts = declared("acme.queue-host", "acme-queue");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let mut registry = HostInfrastructureRegistry::default();
    assert!(builtin_event_bus_factories()
        .unwrap()
        .activate(&hosts, pool.clone(), &mut registry)
        .is_err());
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
