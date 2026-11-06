use super::*;
use control_plane_contracts::ports::EventBusLimits;
use serde_json::{json, Value};
use storage_ephemeral::MemoryEventBus;

fn runtime() -> EventCenterRuntime {
    EventCenterRuntime::new(
        Arc::new(MemoryEventBus::new()),
        Some(EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 32,
                max_json_bytes: 64_000,
                max_waiters: 4,
                max_topics: 32,
                max_consumers: 32,
                max_topic_name_bytes: 16_000,
            },
            observation_max_bytes: 4096,
            observation_ttl_ms: 60_000,
        }),
    )
    .unwrap()
}
fn descriptor(scope: EventSourceScope) -> EventSourceDescriptor {
    EventSourceDescriptor {
        source_id: "runtime.required".into(),
        owner_id: "runtime".into(),
        point_id: "runtime.required.handoff".into(),
        contract_id: "runtime.envelope".into(),
        name: "Required".into(),
        description: String::new(),
        payload_schema: Value::Null,
        origin: EventDefinitionOrigin::Core,
        scope,
        binding: EventSourceBinding::Active,
        coverage: EventSourceCoverage::Live,
        supported_phases: vec![EventTriggerPhase::Published, EventTriggerPhase::Consumed],
    }
}
fn observation(fact: EventSourceFact, ancestors: Vec<Uuid>) -> EventSourceObservation {
    EventSourceObservation {
        original_event_id: Some("native:event:42".into()),
        producer_id: Some("native-producer".into()),
        occurred_at_unix_ms: 1234,
        workflow_ancestor_ids: ancestors,
        fact,
    }
}
fn subscribe(
    runtime: &EventCenterRuntime,
    scope: EventSourceScope,
    workflow_id: Uuid,
    phase: EventTriggerPhase,
) -> EventCenterSubscription {
    runtime
        .subscribe_source(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id: workspace(&scope),
                contract_id: "runtime.envelope".into(),
                trigger_phase: phase,
                consumer_id: Uuid::now_v7(),
            },
            EventSourceTarget {
                scope,
                source_id: "runtime.required".into(),
                workflow_id,
            },
        )
        .unwrap()
}

#[test]
fn concurrent_instances_keep_stable_source_and_lease_owned_cleanup() {
    let runtime = runtime();
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let first = runtime.register_source(descriptor(scope.clone())).unwrap();
    let second = runtime.register_source(descriptor(scope.clone())).unwrap();
    let snapshot = runtime.snapshot_sources(scope.clone());
    assert_eq!(snapshot.sources.len(), 2);
    assert_ne!(
        snapshot.sources[0].registration_id,
        snapshot.sources[1].registration_id
    );
    assert_eq!(
        snapshot.sources[0].descriptor.source_id,
        snapshot.sources[1].descriptor.source_id
    );
    assert!(snapshot.sources[0].descriptor.payload_schema.is_null());
    drop(first);
    assert_eq!(runtime.snapshot_sources(scope.clone()).sources.len(), 1);
    drop(second);
    assert!(runtime.snapshot_sources(scope).sources.is_empty());
}

#[test]
fn trusted_scope_isolates_system_other_workspace_unbound_and_custom() {
    let runtime = runtime();
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let other = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let source = runtime.register_source(descriptor(scope.clone())).unwrap();
    let _foreign = subscribe(
        &runtime,
        other.clone(),
        Uuid::now_v7(),
        EventTriggerPhase::Published,
    );
    let _system = subscribe(
        &runtime,
        EventSourceScope::System,
        Uuid::now_v7(),
        EventTriggerPhase::Published,
    );
    assert_eq!(
        source.observe(observation(EventSourceFact::Published, vec![])),
        Status::Observed
    );
    assert_eq!(runtime.snapshot_sources(scope.clone()).activity.len(), 1);
    assert!(runtime.snapshot_sources(other).activity.is_empty());
    assert!(runtime
        .snapshot_sources(EventSourceScope::System)
        .activity
        .is_empty());
    let mut unbound = descriptor(EventSourceScope::Unbound);
    assert!(runtime.register_source(unbound.clone()).is_err());
    unbound.binding = EventSourceBinding::Declaration;
    unbound.coverage = EventSourceCoverage::Unavailable;
    unbound.supported_phases.clear();
    let lease = runtime.register_source(unbound).unwrap();
    assert_eq!(
        lease.observe(observation(EventSourceFact::Published, vec![])),
        Status::Inactive
    );
    let system = runtime
        .register_source(descriptor(EventSourceScope::System))
        .unwrap();
    system.observe_with_payload(observation(EventSourceFact::Published, vec![]), &mut || {
        Some(json!({}))
    });
    assert!(runtime
        .snapshot(domain::SYSTEM_SCOPE_ID)
        .activity
        .is_empty());
}

#[test]
fn original_dequeue_is_before_result_and_observer_dequeue_is_not_original_consumption() {
    let runtime = runtime();
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let source = runtime.register_source(descriptor(scope.clone())).unwrap();
    let business = subscribe(
        &runtime,
        scope.clone(),
        Uuid::now_v7(),
        EventTriggerPhase::Published,
    );
    let consumed = subscribe(
        &runtime,
        scope.clone(),
        Uuid::now_v7(),
        EventTriggerPhase::Consumed,
    );
    source.observe_with_payload(observation(EventSourceFact::Published, vec![]), &mut || {
        Some(json!({"secret":"do-not-retain-body"}))
    });
    let delivery = business.try_recv().unwrap().unwrap();
    assert_eq!(
        delivery
            .signal()
            .source_context
            .as_ref()
            .unwrap()
            .original_event_id
            .as_deref(),
        Some("native:event:42")
    );
    assert!(consumed.try_recv().unwrap().is_none());
    source.observe(observation(
        EventSourceFact::Consumed {
            subscriber_id: "native-subscription".into(),
            consumer_id: "native-consumer".into(),
        },
        vec![],
    ));
    let handoff = consumed.try_recv().unwrap().unwrap();
    assert!(
        matches!(&handoff.signal().body, EventCenterSignalBody::Consumed { subscription_id, consumer_id, source_event_id } if subscription_id == "native-subscription" && consumer_id == "native-consumer" && source_event_id == "native:event:42")
    );
    drop(handoff);
    drop(delivery); // Original handler can fail independently.
    let snapshot = runtime.snapshot_sources(scope);
    assert_eq!(
        snapshot
            .activity
            .iter()
            .filter(|item| item.phase == EventTriggerPhase::Consumed && item.admission.is_none())
            .count(),
        1
    );
    assert!(!serde_json::to_string(&snapshot.activity)
        .unwrap()
        .contains("do-not-retain-body"));
}

#[test]
fn full_causal_chain_rejects_reentry_but_delivers_distinct_workflow_and_propagates_chain() {
    let runtime = runtime();
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let source = runtime.register_source(descriptor(scope.clone())).unwrap();
    let ancestor = Uuid::now_v7();
    let next = Uuid::now_v7();
    let blocked = subscribe(
        &runtime,
        scope.clone(),
        ancestor,
        EventTriggerPhase::Published,
    );
    let allowed = subscribe(&runtime, scope.clone(), next, EventTriggerPhase::Published);
    let mut chain = vec![ancestor];
    chain.extend((0..300).map(|_| Uuid::now_v7()));
    source.observe_with_payload(
        observation(EventSourceFact::Published, chain.clone()),
        &mut || Some(json!({})),
    );
    assert!(blocked.try_recv().unwrap().is_none());
    let delivered = allowed.try_recv().unwrap().unwrap();
    chain.push(next);
    assert_eq!(delivered.signal().workflow_ancestor_ids, chain);
    assert!(runtime
        .snapshot_sources(scope)
        .activity
        .iter()
        .any(|item| item.admission == Some(EventTargetAdmissionStatus::Reentrant)));
}

#[test]
fn queue_snapshots_are_absolute_nullable_read_only_bounded_and_removed_with_lease() {
    let runtime = runtime();
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let source = runtime.register_source(descriptor(scope.clone())).unwrap();
    let queue = EventSourceQueueSnapshot {
        queue_id: "original.queue".into(),
        subscriber_id: "subscriber".into(),
        consumer_ids: vec!["consumer".into()],
        buffered: Some(4),
        in_flight: None,
        accepted: None,
        dequeued: Some(9),
        dropped: None,
        observed_at_unix_ms: 1234,
    };
    assert!(source.set_queue_snapshots(vec![queue.clone()]));
    let snapshot = runtime.snapshot_sources(scope.clone());
    assert_eq!(snapshot, runtime.snapshot_sources(scope.clone()));
    assert_eq!(snapshot.sources[0].queues, vec![queue.clone()]);
    let mut oversize = queue;
    oversize.queue_id = "x".repeat(20_000);
    assert!(!source.set_queue_snapshots(vec![oversize]));
    let mut incomplete = snapshot.clone();
    incomplete.sources[0].queue_observation_complete = false;
    assert_eq!(incomplete, runtime.snapshot_sources(scope.clone()));
    for _ in 0..30 {
        source.observe(observation(EventSourceFact::Published, vec![]));
    }
    let observed = runtime.snapshot_sources(scope.clone());
    assert!(observed.observation_bytes <= 4096);
    assert!(observed.activity.len() < 30);
    assert!(source.set_queue_snapshots(vec![]));
    assert!(
        runtime.snapshot_sources(scope.clone()).sources[0].queue_observation_complete,
        "a successful authoritative full refresh restores completeness"
    );
    drop(source);
    assert!(runtime.snapshot_sources(scope).sources.is_empty());
}

#[test]
fn unsupported_phase_and_missing_body_cannot_fabricate_workflow_delivery() {
    let runtime = runtime();
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let source = runtime.register_source(descriptor(scope.clone())).unwrap();
    assert_eq!(
        source.observe(observation(
            EventSourceFact::Subscribed {
                subscriber_id: "s".into(),
                consumer_id: "c".into()
            },
            vec![]
        )),
        Status::UnsupportedPhase
    );
    let target = subscribe(
        &runtime,
        scope,
        Uuid::now_v7(),
        EventTriggerPhase::Published,
    );
    assert_eq!(
        source.observe(observation(EventSourceFact::Published, vec![])),
        Status::InvalidBody
    );
    assert!(target.try_recv().unwrap().is_none());
}

#[test]
fn lazy_payload_projection_only_runs_for_eligible_target_under_same_admission_decision() {
    let runtime = runtime();
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let source = runtime.register_source(descriptor(scope.clone())).unwrap();
    let workflow = Uuid::now_v7();
    let calls = std::cell::Cell::new(0);
    let mut project = || {
        calls.set(calls.get() + 1);
        Some(json!({"actual":"body"}))
    };
    assert_eq!(
        source.observe_with_payload(
            observation(EventSourceFact::Published, vec![]),
            &mut project
        ),
        Status::Observed
    );
    let target = subscribe(
        &runtime,
        scope.clone(),
        workflow,
        EventTriggerPhase::Published,
    );
    assert_eq!(
        source.observe_with_payload(
            observation(EventSourceFact::Published, vec![workflow]),
            &mut project
        ),
        Status::Observed
    );
    assert!(target.try_recv().unwrap().is_none());
    assert_eq!(
        source.observe_with_payload(
            observation(EventSourceFact::Published, vec![]),
            &mut project
        ),
        Status::Observed
    );
    let delivery = target.try_recv().unwrap().unwrap();
    assert!(
        matches!(&delivery.signal().body, EventCenterSignalBody::Published { payload } if payload["actual"] == "body")
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        source.observe_with_payload(observation(EventSourceFact::Published, vec![]), &mut || {
            None
        }),
        Status::InvalidBody
    );
    assert!(target.try_recv().unwrap().is_none());
}

#[test]
fn sources_share_observation_budget_evict_activity_and_ignore_bus_topic_limits() {
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let other_scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let first_descriptor = descriptor(scope.clone());
    let mut second_descriptor = descriptor(other_scope.clone());
    second_descriptor.payload_schema = json!({"description": "schema".repeat(400)});
    let first_charge = metadata_bytes(&first_descriptor, &[]).unwrap();
    let second_charge = metadata_bytes(&second_descriptor, &[]).unwrap();
    let budget = first_charge + second_charge;
    let runtime = EventCenterRuntime::new(
        Arc::new(MemoryEventBus::new()),
        Some(EventCenterRuntimePolicy {
            // These limits only belong to actual EventBus topics, not source/schema registrations.
            bus_limits: EventBusLimits {
                max_items: 1,
                max_json_bytes: 64_000,
                max_waiters: 1,
                max_topics: 1,
                max_consumers: 1,
                max_topic_name_bytes: 1,
            },
            observation_max_bytes: budget,
            observation_ttl_ms: 60_000,
        }),
    )
    .unwrap();
    let first = runtime.register_source(first_descriptor).unwrap();
    for _ in 0..12 {
        first.observe(observation(EventSourceFact::Published, vec![]));
    }
    let before = runtime.snapshot_sources(scope.clone());
    assert!(!before.activity.is_empty());
    assert_eq!(before.source_metadata_bytes, first_charge);
    assert!(before.observation_bytes > first_charge);
    let second = runtime.register_source(second_descriptor).unwrap();
    let retained = runtime.snapshot_sources(scope.clone());
    let other = runtime.snapshot_sources(other_scope.clone());
    assert!(retained.activity.is_empty());
    assert_eq!(retained.sources.len(), 1);
    assert_eq!(other.sources.len(), 1);
    assert_eq!(retained.sources[0].metadata_bytes, first_charge);
    assert_eq!(
        retained.source_metadata_bytes + other.source_metadata_bytes,
        budget
    );
    assert_eq!(retained.observation_bytes + other.observation_bytes, budget);
    assert_eq!(
        runtime.snapshot(workspace(&scope)).observation_bytes,
        first_charge
    );
    assert!(runtime.register_source(descriptor(scope.clone())).is_err());
    assert_eq!(retained, runtime.snapshot_sources(scope.clone()));
    let queue = EventSourceQueueSnapshot {
        queue_id: "original".into(),
        subscriber_id: "subscriber".into(),
        consumer_ids: vec![],
        buffered: None,
        in_flight: None,
        accepted: None,
        dequeued: None,
        dropped: None,
        observed_at_unix_ms: 1,
    };
    assert!(!first.set_queue_snapshots(vec![queue.clone()]));
    let mut incomplete = retained.clone();
    incomplete.sources[0].queue_observation_complete = false;
    assert_eq!(incomplete, runtime.snapshot_sources(scope.clone()));
    let clone = second.clone();
    drop(second);
    assert_eq!(
        runtime
            .snapshot_sources(other_scope.clone())
            .source_metadata_bytes,
        second_charge
    );
    drop(clone);
    assert_eq!(runtime.snapshot_sources(other_scope).observation_bytes, 0);
    assert!(first.set_queue_snapshots(vec![queue]));
    first.observe(observation(EventSourceFact::Published, vec![]));
    let resumed = runtime.snapshot_sources(scope.clone());
    assert!(!resumed.activity.is_empty());
    assert!(resumed.observation_bytes <= budget);
    assert!(first.set_queue_snapshots(vec![]));
    assert_eq!(
        runtime
            .snapshot_sources(scope.clone())
            .source_metadata_bytes,
        first_charge
    );
    drop(first);
    assert_eq!(runtime.snapshot_sources(scope).source_metadata_bytes, 0);
}

#[test]
fn system_source_projects_to_authorized_workflow_workspace_without_losing_original_scope() {
    let runtime = runtime();
    let target_workspace = Uuid::now_v7();
    let workflow = Uuid::now_v7();
    let system = runtime
        .register_source(descriptor(EventSourceScope::System))
        .unwrap();
    let target = runtime
        .subscribe_source(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id: target_workspace,
                contract_id: "runtime.envelope".into(),
                trigger_phase: EventTriggerPhase::Published,
                consumer_id: workflow,
            },
            EventSourceTarget {
                scope: EventSourceScope::System,
                source_id: "runtime.required".into(),
                workflow_id: workflow,
            },
        )
        .unwrap();
    let same_uuid_workspace = EventSourceScope::Workspace {
        workspace_id: domain::SYSTEM_SCOPE_ID,
    };
    let foreign = subscribe(
        &runtime,
        same_uuid_workspace.clone(),
        Uuid::now_v7(),
        EventTriggerPhase::Published,
    );
    assert!(runtime
        .subscribe_source(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id: target_workspace,
                contract_id: "runtime.envelope".into(),
                trigger_phase: EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7()
            },
            EventSourceTarget {
                scope: same_uuid_workspace,
                source_id: "runtime.required".into(),
                workflow_id: Uuid::now_v7()
            }
        )
        .is_err());
    system.observe_with_payload(observation(EventSourceFact::Published, vec![]), &mut || {
        Some(json!({"actual":"system"}))
    });
    let delivery = target.try_recv().unwrap().unwrap();
    assert_eq!(delivery.signal().workspace_id, target_workspace);
    assert_eq!(
        delivery.signal().source_context.as_ref().unwrap().scope,
        EventSourceScope::System
    );
    assert!(foreign.try_recv().unwrap().is_none());
    assert!(
        runtime.snapshot(target_workspace).activity.is_empty(),
        "system observations never leak through ordinary workspace GET"
    );
    system.observe_with_payload(
        observation(EventSourceFact::Published, vec![workflow]),
        &mut || panic!("reentrant workflow must not project body"),
    );
    assert!(target.try_recv().unwrap().is_none());
    assert!(runtime
        .snapshot_sources(EventSourceScope::System)
        .activity
        .iter()
        .any(|item| item.admission == Some(EventTargetAdmissionStatus::Reentrant)));
}

#[test]
fn current_definition_rejects_conflicting_schema_or_defining_owner() {
    let runtime = runtime();
    let original = descriptor(EventSourceScope::System);
    let _lease = runtime.register_source(original.clone()).unwrap();
    let mut changed = original.clone();
    changed.payload_schema = json!({"type":"string"});
    assert!(runtime.register_source(changed).is_err());
    let mut changed = original.clone();
    changed.owner_id = "other-owner".into();
    assert!(runtime.register_source(changed).is_err());
    let mut other_scope = original;
    other_scope.scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    assert!(runtime.register_source(other_scope).is_ok());
}
