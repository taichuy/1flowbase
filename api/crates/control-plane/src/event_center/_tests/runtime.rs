use super::*;
use crate::event_center::{EventCenterDelivery, EventRuntimeSubscription};
use control_plane_contracts::ports::EventBusLimits;
use domain::event_center::EventTriggerPhase;
use std::time::Duration;
use storage_ephemeral::MemoryEventBus;

// All capacities here are test fixture policy, never runtime defaults.
fn policy(items: usize) -> EventCenterRuntimePolicy {
    EventCenterRuntimePolicy {
        bus_limits: EventBusLimits {
            max_items: items,
            max_json_bytes: 64_000,
            max_waiters: 16,
            max_topics: 16,
            max_consumers: 32,
            max_topic_name_bytes: 16_000,
        },
        observation_max_bytes: 16_000,
        observation_ttl_ms: 60_000,
    }
}
fn runtime(items: usize) -> (EventCenterRuntime, Arc<MemoryEventBus>) {
    let bus = Arc::new(MemoryEventBus::new());
    (
        EventCenterRuntime::new(bus.clone(), Some(policy(items))).unwrap(),
        bus,
    )
}
fn definition() -> EventDefinitionRecord {
    let now = OffsetDateTime::now_utc();
    EventDefinitionRecord {
        event_kind: None,
        interface_event: None,

        id: Uuid::now_v7(),
        workspace_id: Uuid::now_v7(),
        contract_id: "custom.orders".into(),
        name: "Orders".into(),
        description: String::new(),
        payload_schema: serde_json::json!({"type":"object","properties":{"secret":{"type":"string"}},"required":["secret"]}),
        enabled: true,
        created_by: Uuid::now_v7(),
        created_at: now,
        updated_at: now,
    }
}
fn spec(definition: &EventDefinitionRecord, phase: EventTriggerPhase) -> EventRuntimeSubscription {
    EventRuntimeSubscription {
        subscription_id: Uuid::now_v7(),
        workspace_id: definition.workspace_id,
        contract_id: definition.contract_id.clone(),
        trigger_phase: phase,
        consumer_id: Uuid::now_v7(),
    }
}
fn payload() -> Value {
    serde_json::json!({"secret":"business-payload-never-in-activity"})
}
async fn receive(subscription: &EventCenterSubscription) -> EventCenterDelivery {
    tokio::time::timeout(Duration::from_secs(1), subscription.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}
async fn empty(subscription: &EventCenterSubscription) {
    assert!(
        tokio::time::timeout(Duration::from_millis(5), subscription.recv())
            .await
            .is_err()
    );
}

#[test]
fn no_policy_is_observable_but_cannot_allocate_or_publish() {
    let bus = Arc::new(MemoryEventBus::new());
    let runtime = EventCenterRuntime::new(bus.clone(), None).unwrap();
    let definition = definition();
    assert!(!runtime.snapshot(definition.workspace_id).configured);
    assert!(runtime.snapshot(definition.workspace_id).queues.is_empty());
    assert!(matches!(
        runtime
            .subscribe(spec(&definition, EventTriggerPhase::Published), false)
            .err()
            .unwrap()
            .downcast_ref::<EventBusError>(),
        Some(EventBusError::Unconfigured)
    ));
    assert!(matches!(
        runtime
            .publish(&definition, payload())
            .unwrap_err()
            .downcast_ref::<EventBusError>(),
        Some(EventBusError::Unconfigured)
    ));
    assert_eq!(bus.event_bus_snapshot().topics, 0);
    let replacement = EventCenterRuntime::new(bus, None).unwrap();
    assert_ne!(
        runtime.snapshot(definition.workspace_id).instance_id,
        replacement.snapshot(definition.workspace_id).instance_id
    );
}

#[tokio::test]
async fn independent_subscriptions_fan_out_and_same_subscription_consumers_compete() {
    let (runtime, bus) = runtime(8);
    let definition = definition();
    let first_spec = spec(&definition, EventTriggerPhase::Published);
    let first = runtime.subscribe(first_spec.clone(), false).unwrap();
    let mut competitor_spec = first_spec.clone();
    competitor_spec.consumer_id = Uuid::now_v7();
    let competitor = runtime.subscribe(competitor_spec, false).unwrap();
    let separate = runtime
        .subscribe(spec(&definition, EventTriggerPhase::Published), false)
        .unwrap();
    let receipt = runtime.publish(&definition, payload()).unwrap();
    assert_eq!(receipt.targets.len(), 2);
    assert!(receipt
        .targets
        .iter()
        .all(|target| target.status == EventTargetAdmissionStatus::Accepted));
    let delivery = receive(&first).await;
    let other = receive(&separate).await;
    assert_eq!(delivery.signal().event_id, receipt.event_id);
    assert_eq!(other.signal().event_id, receipt.event_id);
    assert!(delivery.signal().occurred_at_unix_ms > 0);
    empty(&competitor).await;
    assert_eq!(bus.event_bus_snapshot().in_flight, 2);
    assert_eq!(bus.event_bus_snapshot().buffered, 0);
    drop(delivery);
    other.complete();
    assert_eq!(bus.event_bus_snapshot().charged_json_bytes, 0);
    let receipt = runtime.publish(&definition, payload()).unwrap();
    assert_eq!(
        receive(&competitor).await.signal().event_id,
        receipt.event_id
    );
    empty(&first).await;
}

#[tokio::test]
async fn consumed_observes_real_handoff_and_never_creates_a_business_target() {
    let (runtime, _) = runtime(8);
    let definition = definition();
    let observer = runtime
        .subscribe(spec(&definition, EventTriggerPhase::Consumed), false)
        .unwrap();
    assert!(runtime
        .publish(&definition, payload())
        .unwrap()
        .targets
        .is_empty());
    empty(&observer).await;
    let business_spec = spec(&definition, EventTriggerPhase::Published);
    let business = runtime.subscribe(business_spec.clone(), false).unwrap();
    let receipt = runtime.publish(&definition, payload()).unwrap();
    assert_eq!(receipt.targets.len(), 1);
    empty(&observer).await;
    let business_delivery = receive(&business).await;
    let control_delivery = receive(&observer).await;
    assert_ne!(receipt.event_id, control_delivery.signal().event_id);
    assert!(
        matches!(&control_delivery.signal().body, EventCenterSignalBody::Consumed { source_event_id, subscription_id, consumer_id }
        if *source_event_id == receipt.event_id.to_string() && *subscription_id == business_spec.subscription_id.to_string() && *consumer_id == business_spec.consumer_id.to_string())
    );
    assert!(!serde_json::to_string(control_delivery.signal())
        .unwrap()
        .contains("business-payload-never-in-activity"));
    drop(control_delivery);
    drop(business_delivery); // Business failure/Drop cannot erase the fact.
    empty(&observer).await;
    let snapshot = runtime.snapshot(definition.workspace_id);
    assert_eq!(
        snapshot
            .activity
            .iter()
            .filter(|activity| activity.phase == EventTriggerPhase::Consumed
                && activity.admission.is_none())
            .count(),
        1
    );
    assert!(!serde_json::to_string(&snapshot.activity)
        .unwrap()
        .contains("business-payload-never-in-activity"));
}

#[tokio::test]
async fn subscribed_restore_is_quiet_and_competing_consumers_do_not_duplicate_logical_registration()
{
    let (runtime, _) = runtime(8);
    let definition = definition();
    let observer = runtime
        .subscribe(spec(&definition, EventTriggerPhase::Subscribed), false)
        .unwrap();
    let restored_spec = spec(&definition, EventTriggerPhase::Published);
    let _restored = runtime.subscribe(restored_spec.clone(), false).unwrap();
    empty(&observer).await;
    let mut competitor = restored_spec;
    competitor.consumer_id = Uuid::now_v7();
    let _competitor = runtime.subscribe(competitor, true).unwrap();
    empty(&observer).await;
    let new_spec = spec(&definition, EventTriggerPhase::Published);
    let _new = runtime.subscribe(new_spec.clone(), true).unwrap();
    let signal = receive(&observer).await;
    assert!(
        matches!(&signal.signal().body, EventCenterSignalBody::Subscribed { subscription_id, .. } if *subscription_id == new_spec.subscription_id.to_string())
    );
    empty(&observer).await;
}

#[tokio::test]
async fn partial_admission_and_full_budget_never_block_consumed_fact_or_release_guard_early() {
    let (runtime, bus) = runtime(1);
    let definition = definition();
    let observer = runtime
        .subscribe(spec(&definition, EventTriggerPhase::Consumed), false)
        .unwrap();
    let first = runtime
        .subscribe(spec(&definition, EventTriggerPhase::Published), false)
        .unwrap();
    let second = runtime
        .subscribe(spec(&definition, EventTriggerPhase::Published), false)
        .unwrap();
    let receipt = runtime.publish(&definition, payload()).unwrap();
    assert_eq!(
        receipt
            .targets
            .iter()
            .filter(|target| target.status == EventTargetAdmissionStatus::Accepted)
            .count(),
        1
    );
    assert_eq!(
        receipt
            .targets
            .iter()
            .filter(|target| target.status == EventTargetAdmissionStatus::ResourceExhausted)
            .count(),
        1
    );
    let accepted = receipt
        .targets
        .iter()
        .find(|target| target.status == EventTargetAdmissionStatus::Accepted)
        .unwrap()
        .subscription_id;
    let delivery = receive(if first.spec().subscription_id == accepted {
        &first
    } else {
        &second
    })
    .await;
    assert_eq!(bus.event_bus_snapshot().in_flight, 1);
    assert!(bus.event_bus_snapshot().charged_json_bytes > 0);
    assert_eq!(bus.event_bus_snapshot().waiters, 0);
    let snapshot = runtime.snapshot(definition.workspace_id);
    assert!(snapshot
        .activity
        .iter()
        .any(|activity| activity.phase == EventTriggerPhase::Consumed
            && activity.admission.is_none()
            && activity.source_event_id == Some(receipt.event_id)));
    assert!(snapshot
        .activity
        .iter()
        .any(|activity| activity.phase == EventTriggerPhase::Consumed
            && activity.admission == Some(EventTargetAdmissionStatus::ResourceExhausted)));
    assert!(runtime
        .publish(&definition, payload())
        .unwrap()
        .targets
        .iter()
        .all(|target| target.status == EventTargetAdmissionStatus::ResourceExhausted));
    empty(&observer).await;
    drop(delivery);
    assert_eq!(bus.event_bus_snapshot().charged_json_bytes, 0);
    assert!(runtime
        .publish(&definition, payload())
        .unwrap()
        .targets
        .iter()
        .any(|target| target.status == EventTargetAdmissionStatus::Accepted));
}

#[tokio::test]
async fn closed_generation_drains_and_final_drop_reclaims_membership_but_not_live_delivery() {
    let (runtime, bus) = runtime(4);
    let definition = definition();
    let first_spec = spec(&definition, EventTriggerPhase::Published);
    let first = runtime.subscribe(first_spec.clone(), false).unwrap();
    let mut second_spec = first_spec.clone();
    second_spec.consumer_id = Uuid::now_v7();
    let second = runtime.subscribe(second_spec, false).unwrap();
    runtime.publish(&definition, payload()).unwrap();
    first.close();
    assert_eq!(
        runtime.publish(&definition, payload()).unwrap().targets[0].status,
        EventTargetAdmissionStatus::Closed
    );
    let delivery = receive(&second).await;
    assert!(first.recv().await.unwrap().is_none());
    assert!(runtime
        .subscribe(
            EventRuntimeSubscription {
                consumer_id: Uuid::now_v7(),
                ..first_spec
            },
            false
        )
        .is_err());
    drop(first);
    drop(second);
    assert!(runtime.snapshot(definition.workspace_id).queues.is_empty());
    assert_eq!(bus.event_bus_snapshot().in_flight, 1);
    assert_eq!(bus.event_bus_snapshot().retired_topics, 1);
    drop(delivery);
    let resources = bus.event_bus_snapshot();
    assert_eq!(
        (
            resources.topics,
            resources.topic_name_bytes,
            resources.charged_json_bytes
        ),
        (0, 0, 0)
    );
}

#[tokio::test]
async fn workspace_contract_phase_and_consumer_identity_cannot_cross_or_silently_rebind() {
    let (runtime, _) = runtime(8);
    let definition = definition();
    let first_spec = spec(&definition, EventTriggerPhase::Published);
    let first = runtime.subscribe(first_spec.clone(), false).unwrap();
    assert!(runtime.subscribe(first_spec.clone(), false).is_err());
    let mut rebind = first_spec.clone();
    rebind.contract_id = "custom.changed".into();
    rebind.consumer_id = Uuid::now_v7();
    assert!(runtime.subscribe(rebind, false).is_err());
    let mut other_definition = definition.clone();
    other_definition.workspace_id = Uuid::now_v7();
    let mut other_spec = first_spec;
    other_spec.workspace_id = other_definition.workspace_id;
    let other = runtime.subscribe(other_spec, false).unwrap();
    runtime.publish(&definition, payload()).unwrap();
    receive(&first).await.complete();
    empty(&other).await;
    let own = runtime.snapshot(definition.workspace_id);
    let outsider = runtime.snapshot(other_definition.workspace_id);
    assert_eq!(own.queues.len(), 1);
    assert_eq!(outsider.queues.len(), 1);
    assert!(own
        .activity
        .iter()
        .all(|activity| activity.workspace_id == definition.workspace_id));
    assert!(outsider.activity.is_empty());
    assert_eq!(outsider.admission_rejected, 0);
    let mut invalid = definition.clone();
    invalid.enabled = false;
    assert!(runtime.publish(&invalid, payload()).is_err());
    assert!(runtime
        .publish(&definition, serde_json::json!({"secret":12}))
        .is_err());
    invalid.enabled = true;
    invalid.payload_schema = serde_json::json!({"$ref":"https://example.com/schema"});
    assert!(runtime.publish(&invalid, payload()).is_err());
}

#[tokio::test]
async fn observation_is_bounded_payload_free_read_only_and_resets_with_instance() {
    let bus = Arc::new(MemoryEventBus::new());
    let definition = definition();
    let mut explicit = policy(4);
    explicit.observation_max_bytes = 1_200;
    explicit.observation_ttl_ms = 60_000;
    let runtime = EventCenterRuntime::new(bus.clone(), Some(explicit)).unwrap();
    for _ in 0..20 {
        runtime.publish(&definition, payload()).unwrap();
    }
    let snapshot = runtime.snapshot(definition.workspace_id);
    assert!(!snapshot.activity.is_empty());
    assert!(snapshot.activity.len() < 20);
    assert!(snapshot.observation_bytes <= explicit.observation_max_bytes);
    assert_eq!(snapshot, runtime.snapshot(definition.workspace_id));
    assert!(!serde_json::to_string(&snapshot)
        .unwrap()
        .contains("business-payload-never-in-activity"));
    assert_eq!(bus.event_bus_snapshot().consumers, 0);
    let replacement = EventCenterRuntime::new(bus, Some(explicit)).unwrap();
    assert_ne!(
        snapshot.instance_id,
        replacement.snapshot(definition.workspace_id).instance_id
    );
    assert!(replacement
        .snapshot(definition.workspace_id)
        .activity
        .is_empty());
}

#[tokio::test]
async fn retired_delivery_release_does_not_decrement_reopened_subscription_generation() {
    let (runtime, bus) = runtime(4);
    let definition = definition();
    let subscription_spec = spec(&definition, EventTriggerPhase::Published);
    let old = runtime.subscribe(subscription_spec.clone(), false).unwrap();
    runtime.publish(&definition, payload()).unwrap();
    let old_delivery = receive(&old).await;
    drop(old);
    let new = runtime.subscribe(subscription_spec, false).unwrap();
    runtime.publish(&definition, payload()).unwrap();
    let new_delivery = receive(&new).await;
    assert_eq!(
        runtime.snapshot(definition.workspace_id).queues[0].in_flight,
        1
    );
    assert_eq!(bus.event_bus_snapshot().in_flight, 2);
    drop(old_delivery);
    assert_eq!(
        runtime.snapshot(definition.workspace_id).queues[0].in_flight,
        1
    );
    assert_eq!(bus.event_bus_snapshot().in_flight, 1);
    drop(new_delivery);
    assert_eq!(
        runtime.snapshot(definition.workspace_id).queues[0].in_flight,
        0
    );
}

#[tokio::test]
async fn new_subscribed_observer_only_observes_later_joins() {
    let (runtime, _) = runtime(8);
    let definition = definition();
    let first = runtime
        .subscribe(spec(&definition, EventTriggerPhase::Subscribed), true)
        .unwrap();
    empty(&first).await;
    let second_spec = spec(&definition, EventTriggerPhase::Subscribed);
    let second = runtime.subscribe(second_spec.clone(), true).unwrap();
    let joined = receive(&first).await;
    assert!(
        matches!(&joined.signal().body, EventCenterSignalBody::Subscribed { subscription_id, .. } if *subscription_id == second_spec.subscription_id.to_string())
    );
    empty(&second).await;
    let business_spec = spec(&definition, EventTriggerPhase::Published);
    let _business = runtime.subscribe(business_spec.clone(), true).unwrap();
    for observer in [&first, &second] {
        let joined = receive(observer).await;
        assert!(
            matches!(&joined.signal().body, EventCenterSignalBody::Subscribed { subscription_id, .. } if *subscription_id == business_spec.subscription_id.to_string())
        );
    }
}

#[tokio::test]
async fn readiness_does_not_create_consumption_and_competing_try_dequeue_has_one_winner() {
    let (runtime, bus) = runtime(8);
    let definition = definition();
    let first_spec = spec(&definition, EventTriggerPhase::Published);
    let first = runtime.subscribe(first_spec.clone(), false).unwrap();
    let second = runtime
        .subscribe(
            EventRuntimeSubscription {
                consumer_id: Uuid::now_v7(),
                ..first_spec
            },
            false,
        )
        .unwrap();
    runtime.publish(&definition, payload()).unwrap();
    assert!(first.ready().await.unwrap());
    assert!(second.ready().await.unwrap());
    assert_eq!(bus.event_bus_snapshot().received, 0);
    assert_eq!(
        runtime.snapshot(definition.workspace_id).queues[0].consumed,
        0
    );
    let delivery = first.try_recv().unwrap().unwrap();
    assert!(second.try_recv().unwrap().is_none());
    assert_eq!(
        runtime.snapshot(definition.workspace_id).queues[0].consumed,
        1
    );
    assert_eq!(bus.event_bus_snapshot().received, 1);
    drop(delivery);
    let mut waiting = Box::pin(second.ready());
    tokio::select! { biased; _ = &mut waiting => panic!("competitor must wait for new work"), _ = tokio::task::yield_now() => {} }
    assert_eq!(bus.event_bus_snapshot().waiters, 1);
    drop(waiting);
    assert_eq!(bus.event_bus_snapshot().waiters, 0);
}

#[tokio::test]
async fn current_schema_replaces_body_validation_without_rebinding_subscription() {
    let (runtime, _) = runtime(8);
    let mut definition = definition();
    let subscription = runtime
        .subscribe(spec(&definition, EventTriggerPhase::Published), false)
        .unwrap();
    definition.payload_schema = serde_json::json!({"type":"object","required":["amount"],"additionalProperties":false,"properties":{"amount":{"type":"integer"}}});
    assert!(runtime.publish(&definition, payload()).is_err());
    runtime
        .publish(&definition, serde_json::json!({"amount":42}))
        .unwrap();
    let delivery = receive(&subscription).await;
    let encoded = serde_json::to_value(delivery.signal()).unwrap();
    assert!(encoded.get("contract_version").is_none());
    assert_eq!(encoded["payload"], serde_json::json!({"amount":42}));
    delivery.complete();
    let snapshot = serde_json::to_value(runtime.snapshot(definition.workspace_id)).unwrap();
    for activity in snapshot["activity"].as_array().unwrap() {
        assert!(activity.get("contract_version").is_none());
    }
    for queue in snapshot["queues"].as_array().unwrap() {
        assert!(queue.get("contract_version").is_none());
    }
}
