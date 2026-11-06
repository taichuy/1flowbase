use super::*;
use domain::event_center::{EventTriggerPhase, WorkflowEventTriggerInput};
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;
fn trigger() -> WorkflowEventTriggerRecord {
    WorkflowEventTriggerRecord {
        id: Uuid::now_v7(),
        workspace_id: Uuid::now_v7(),
        application_id: Uuid::now_v7(),
        configuration: WorkflowEventTriggerInput {
            source: None,
            enabled: true,
            contract_id: "custom.invoice".into(),
            contract_version: "1".into(),
            trigger_phase: EventTriggerPhase::Published,
            input_mapping: BTreeMap::new(),
        },
        updated_by: Uuid::now_v7(),
        updated_at: OffsetDateTime::now_utc(),
    }
}
#[test]
fn restored_and_unchanged_subscriptions_do_not_emit_activation_facts() {
    let trigger = trigger();
    let id = trigger.application_id;
    let configured = BTreeMap::from([(id, trigger.clone())]);
    let mut logical = BTreeMap::new();
    update_logical(&mut logical, &configured, true);
    assert!(!logical[&id].emit_on_activation);
    let mut implementation_changed = trigger.clone();
    implementation_changed.updated_at += time::Duration::seconds(1);
    implementation_changed
        .configuration
        .input_mapping
        .insert("invoice".into(), "/payload/id".into());
    update_logical(
        &mut logical,
        &BTreeMap::from([(id, implementation_changed)]),
        false,
    );
    assert!(!logical[&id].emit_on_activation);
    // Explicit disable retires the logical subscription; enable forms a new activation.
    update_logical(&mut logical, &BTreeMap::new(), false);
    update_logical(&mut logical, &configured, false);
    assert!(logical[&id].emit_on_activation);
    // A pending activation survives the refresh needed to drain an earlier receiver.
    update_logical(&mut logical, &configured, false);
    assert!(logical[&id].emit_on_activation);
}
#[test]
fn changed_contract_or_phase_is_a_new_logical_subscription() {
    let trigger = trigger();
    let id = trigger.application_id;
    let mut logical = BTreeMap::new();
    update_logical(&mut logical, &BTreeMap::from([(id, trigger.clone())]), true);
    let mut changed = trigger;
    changed.configuration.trigger_phase = EventTriggerPhase::Consumed;
    update_logical(&mut logical, &BTreeMap::from([(id, changed)]), false);
    assert!(logical[&id].emit_on_activation);
}
#[test]
fn latest_consumer_failure_preserves_run_identity_and_keeps_activation_separate() {
    let trigger = trigger();
    let statuses = EventWorkflowStatuses::default();
    statuses.activation(&trigger, "active", None);
    let signal = Uuid::now_v7();
    let run = Uuid::now_v7();
    statuses.delivery(trigger.application_id, signal, Some(run), "executing", None);
    statuses.delivery(
        trigger.application_id,
        signal,
        None,
        "consumer_failed",
        Some("consumer_failed"),
    );
    let snapshot = statuses.snapshot(trigger.workspace_id);
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].activation, "active");
    assert_eq!(snapshot[0].last_run_id, Some(run));
    assert_eq!(snapshot[0].last_delivery, Some("consumer_failed"));
    assert_eq!(
        snapshot[0].last_delivery_reason.as_deref(),
        Some("consumer_failed")
    );
    assert!(statuses.snapshot(Uuid::now_v7()).is_empty());
    statuses.retain(&BTreeSet::new());
    assert!(statuses.snapshot(trigger.workspace_id).is_empty());
}
#[tokio::test]
async fn activation_decision_controls_real_subscribed_signal_and_restoration_does_not_replay() {
    use control_plane::event_center::{
        EventCenterRuntime, EventCenterRuntimePolicy, EventCenterSignalBody,
    };
    use control_plane_contracts::ports::EventBusLimits;
    use storage_ephemeral::MemoryEventBus;
    let runtime = EventCenterRuntime::new(
        Arc::new(MemoryEventBus::new()),
        Some(EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 8,
                max_json_bytes: 32000,
                max_waiters: 8,
                max_topics: 8,
                max_consumers: 8,
                max_topic_name_bytes: 8000,
            },
            observation_max_bytes: 8000,
            observation_ttl_ms: 60000,
        }),
    )
    .unwrap();
    let mut trigger = trigger();
    trigger.configuration.trigger_phase = EventTriggerPhase::Subscribed;
    let id = trigger.application_id;
    let mut logical = BTreeMap::new();
    let configured = BTreeMap::from([(id, trigger.clone())]);
    update_logical(&mut logical, &configured, true);
    let spec = EventRuntimeSubscription {
        subscription_id: trigger.id,
        workspace_id: trigger.workspace_id,
        contract_id: trigger.configuration.contract_id.clone(),
        contract_version: "1".into(),
        trigger_phase: EventTriggerPhase::Subscribed,
        consumer_id: id,
    };
    let observer_id = Uuid::now_v7();
    let observer = runtime
        .subscribe(
            EventRuntimeSubscription {
                subscription_id: observer_id,
                consumer_id: Uuid::now_v7(),
                ..spec.clone()
            },
            false,
        )
        .unwrap();
    assert!(!logical[&id].emit_on_activation);
    let restored = runtime
        .subscribe(spec.clone(), logical[&id].emit_on_activation)
        .unwrap();
    assert!(restored.try_recv().unwrap().is_none());
    assert!(observer.try_recv().unwrap().is_none());
    drop(restored);
    update_logical(&mut logical, &BTreeMap::new(), false);
    update_logical(&mut logical, &configured, false);
    assert!(logical[&id].emit_on_activation);
    let activated = runtime
        .subscribe(spec, logical[&id].emit_on_activation)
        .unwrap();
    // Activation is visible to existing observers, never to the observer joining now.
    assert!(activated.try_recv().unwrap().is_none());
    let delivery = tokio::time::timeout(Duration::from_secs(1), observer.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        delivery.signal().body,
        EventCenterSignalBody::Subscribed {
            subscription_id: trigger.id.to_string(),
            consumer_id: id.to_string(),
        }
    );
    assert_eq!(
        runtime
            .snapshot(trigger.workspace_id)
            .queues
            .iter()
            .find(|queue| queue.subscription_id == observer_id)
            .unwrap()
            .in_flight,
        1
    );
    // Failure or success does not change the already-observed dequeue; only release changes charge.
    delivery.complete();
    assert_eq!(
        runtime
            .snapshot(trigger.workspace_id)
            .queues
            .iter()
            .find(|queue| queue.subscription_id == observer_id)
            .unwrap()
            .in_flight,
        0
    );
}

#[test]
fn pending_activation_is_coalesced_and_old_completion_cannot_erase_a_later_toggle() {
    let mut trigger = trigger();
    let statuses = EventWorkflowStatuses::default();
    statuses.request_activation(&trigger);
    let first = statuses.pending_activation(trigger.id).unwrap();
    trigger.updated_at += time::Duration::seconds(1);
    statuses.request_activation(&trigger);
    let latest = statuses.pending_activation(trigger.id).unwrap();
    assert_ne!(first.token, latest.token);
    statuses.activation_recorded(trigger.id, first);
    assert_eq!(
        statuses.pending_activation(trigger.id).unwrap().token,
        latest.token
    );
    statuses.activation_recorded(trigger.id, latest);
    assert!(statuses.pending_activation(trigger.id).is_none());
    statuses.request_activation(&trigger);
    statuses.retain_activations(&BTreeSet::new());
    assert!(statuses.pending_activation(trigger.id).is_none());
}

#[test]
fn public_status_reasons_do_not_copy_dynamic_error_contents() {
    let trigger = trigger();
    let statuses = EventWorkflowStatuses::default();
    let error = anyhow::anyhow!("SQL/provider failure containing secret payload invoice A-001");
    statuses.activation(
        &trigger,
        "unavailable",
        Some(status::public_error_code(&error, "activation_failed")),
    );
    statuses.delivery(
        trigger.application_id,
        Uuid::now_v7(),
        None,
        "consumer_failed",
        Some(status::public_error_code(&error, "consumer_failed")),
    );
    statuses.refreshed(Some(status::public_error_code(
        &error,
        "refresh_unavailable",
    )));
    let snapshot = statuses.snapshot(trigger.workspace_id);
    assert_eq!(snapshot[0].reason.as_deref(), Some("activation_failed"));
    assert_eq!(
        snapshot[0].last_delivery_reason.as_deref(),
        Some("consumer_failed")
    );
    assert_eq!(
        statuses.refresh_error().as_deref(),
        Some("refresh_unavailable")
    );
    assert!(!serde_json::to_string(&snapshot).unwrap().contains("A-001"));
    let denied = anyhow::Error::from(control_plane::errors::ControlPlaneError::PermissionDenied(
        "permission_denied",
    ));
    assert_eq!(
        status::public_error_code(&denied, "activation_failed"),
        "permission_denied"
    );
}
