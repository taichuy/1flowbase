use super::*;
use control_plane::event_center::{EventCenterRuntime, EventCenterRuntimePolicy};
use control_plane_contracts::ports::EventBusLimits;
use storage_ephemeral::MemoryEventBus;

fn runtime(configured: bool) -> EventCenterRuntime {
    EventCenterRuntime::new(
        Arc::new(MemoryEventBus::new()),
        configured.then_some(EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 8,
                max_json_bytes: 64_000,
                max_waiters: 4,
                max_topics: 8,
                max_consumers: 8,
                max_topic_name_bytes: 4096,
            },
            observation_max_bytes: 64_000,
            observation_ttl_ms: 60_000,
        }),
    )
    .unwrap()
}
#[test]
fn runtime_capabilities_exist_before_first_run_without_fake_queues_or_subscriptions() {
    let runtime = runtime(true);
    let mut actor = ActorContext::root(uuid::Uuid::now_v7(), domain::SYSTEM_SCOPE_ID, "root");
    let catalog = EventSourceCatalog {
        runtime_bound: true,
        ..Default::default()
    };
    let entries = catalog.registered_events(&actor, &runtime);
    assert_eq!(
        entries.len(),
        6 + super::super::business::catalog(EventSourceScope::Workspace {
            workspace_id: actor.current_workspace_id
        })
        .len()
    );
    assert!(entries.iter().all(|entry| entry.enabled.is_none()
        && entry
            .supported_phases
            .contains(&EventTriggerPhase::Published)));
    assert!(entries
        .iter()
        .any(|entry| entry.source.as_ref().unwrap().selection.scope == EventSourceScope::System));
    assert!(entries
        .iter()
        .any(|entry| entry.source.as_ref().unwrap().selection.scope
            == EventSourceScope::Workspace {
                workspace_id: domain::SYSTEM_SCOPE_ID
            }));
    for scope in visible_scopes(&actor) {
        let snapshot = runtime.snapshot_sources(scope);
        assert!(
            snapshot.sources.is_empty()
                && snapshot.workflow_queues.is_empty()
                && snapshot.activity.is_empty()
        );
    }
    actor.is_root = false;
    let workspace_only = catalog.registered_events(&actor, &runtime);
    assert_eq!(
        workspace_only.len(),
        3 + super::super::business::catalog(EventSourceScope::Workspace {
            workspace_id: actor.current_workspace_id
        })
        .len()
    );
    assert!(workspace_only.iter().all(|entry| matches!(
        entry.source.as_ref().unwrap().selection.scope,
        EventSourceScope::Workspace { .. }
    )));
    assert_eq!(visible_scopes(&actor).len(), 1);
}
#[test]
fn policy_and_provider_binding_are_real_capability_requirements() {
    let actor = ActorContext::root(uuid::Uuid::now_v7(), uuid::Uuid::now_v7(), "root");
    for (configured, bound) in [(false, true), (true, false)] {
        let catalog = EventSourceCatalog {
            runtime_bound: bound,
            ..Default::default()
        };
        let entries = catalog.registered_events(&actor, &runtime(configured));
        assert!(
            entries.is_empty(),
            "unavailable declarations are not available definitions"
        );
    }
}

#[test]
fn registered_runtime_schema_and_catalog_do_not_add_unbound_graph_placeholders() {
    use control_plane_contracts::ports::EventSourceObserver;
    let assembly = crate::extension_bus::assemble_extension_graph_input(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )
    .unwrap();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let expected = control_plane::ports::runtime_event_envelope_schema();
    let descriptors = control_plane::ports::runtime_event_source_descriptors(
        EventSourceScope::System,
        EventSourceBinding::Active,
    );
    for source in &descriptors {
        let actual = graph
            .points()
            .iter()
            .find(|point| point.descriptor().point_id.as_str() == source.point_id)
            .unwrap()
            .descriptor();
        assert_eq!(
            actual.contract.payload_schema.as_ref(),
            Some(&expected),
            "the actual registered point contract must own the source schema"
        );
    }
    let runtime = runtime(true);
    let actor = ActorContext::root(uuid::Uuid::now_v7(), uuid::Uuid::now_v7(), "root");
    let catalog = EventSourceCatalog {
        runtime_bound: true,
        ..Default::default()
    };
    let entries = catalog.registered_events(&actor, &runtime);
    let is_runtime = |entry: &&EventCatalogEntry| {
        descriptors.iter().any(|source| {
            entry
                .source
                .as_ref()
                .is_some_and(|metadata| metadata.point_id == source.point_id)
                && entry.contract_id == source.contract_id
        })
    };
    let runtime_entries = entries.iter().filter(is_runtime).collect::<Vec<_>>();
    assert_eq!(
        runtime_entries.len(),
        6,
        "workspace/system capabilities must not gain duplicate graph declarations"
    );
    assert!(runtime_entries
        .iter()
        .all(
            |entry| entry.source.as_ref().unwrap().selection.scope != EventSourceScope::Unbound
                && entry.payload_schema == expected
        ));
    let mut historical = control_plane::ports::runtime_event_source_descriptors(
        EventSourceScope::Unbound,
        EventSourceBinding::Declaration,
    )
    .remove(0);
    historical.source_id = "historical-runtime-owner".into();
    historical.payload_schema = serde_json::Value::Null;
    let lease = runtime.register_source(historical).unwrap();
    assert!(lease.set_queue_snapshots(vec![EventSourceQueueSnapshot {
        queue_id: "legacy.original".into(),
        subscriber_id: "legacy.consumer".into(),
        consumer_ids: vec![],
        buffered: Some(2),
        in_flight: None,
        accepted: None,
        dequeued: None,
        dropped: None,
        observed_at_unix_ms: 42
    }]));
    let with_history = catalog.registered_events(&actor, &runtime);
    assert_eq!(with_history.iter().filter(is_runtime).count(), 6);
    assert!(
        !with_history
            .iter()
            .any(|entry| entry.scope == EventSourceScope::Unbound),
        "historical backlog remains in runtime only"
    );
    assert_eq!(
        runtime.snapshot_sources(EventSourceScope::Unbound).sources[0].queues[0].buffered,
        Some(2)
    );
}

#[test]
fn concurrent_runtime_registrations_share_one_catalog_definition_per_scope() {
    use control_plane_contracts::ports::EventSourceObserver;
    let runtime = runtime(true);
    let actor = ActorContext::root(uuid::Uuid::now_v7(), uuid::Uuid::now_v7(), "root");
    let scope = EventSourceScope::Workspace {
        workspace_id: actor.current_workspace_id,
    };
    let descriptor =
        control_plane::ports::runtime_event_source_descriptors(scope, EventSourceBinding::Active)
            .remove(0);
    let _first = runtime.register_source(descriptor.clone()).unwrap();
    let _second = runtime.register_source(descriptor.clone()).unwrap();
    let catalog = EventSourceCatalog {
        runtime_bound: true,
        ..Default::default()
    };
    let entries = catalog.registered_events(&actor, &runtime);
    let matching = entries
        .iter()
        .filter(|entry| {
            entry.scope == descriptor.scope && entry.contract_id == descriptor.contract_id
        })
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), 1);
    let entry = matching[0];
    assert_eq!(entry.enabled, None);
    assert_eq!(
        entry.owner_id.as_deref(),
        Some(descriptor.owner_id.as_str())
    );
    let json = serde_json::to_value(entry).unwrap();
    assert!(json.get("contract_version").is_none());
    assert!(json["source"].get("coverage").is_none());
    assert!(json["source"].get("binding").is_none());
}
