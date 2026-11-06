use super::*;
use control_plane::{
    event_center::{
        EventCenterRuntime, EventCenterRuntimePolicy, EventCenterSignalBody,
        EventRuntimeSubscription, EventSourceTarget,
    },
    ports::*,
};
use domain::event_center::{EventSourceScope, EventTriggerPhase};
use std::sync::Arc;

fn center() -> Arc<EventCenterRuntime> {
    Arc::new(
        EventCenterRuntime::new(
            Arc::new(storage_ephemeral::MemoryEventBus::new()),
            Some(EventCenterRuntimePolicy {
                bus_limits: EventBusLimits {
                    max_items: 128,
                    max_json_bytes: 1_000_000,
                    max_waiters: 16,
                    max_topics: 64,
                    max_consumers: 64,
                    max_topic_name_bytes: 64_000,
                },
                observation_max_bytes: 1_000_000,
                observation_ttl_ms: 60_000,
            }),
        )
        .unwrap(),
    )
}
fn target(
    center: &EventCenterRuntime,
    workspace_id: Uuid,
    source: &str,
    phase: EventTriggerPhase,
    workflow_id: Uuid,
) -> control_plane::event_center::EventCenterSubscription {
    center
        .subscribe_source(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id,
                contract_id: source.into(),
                contract_version: "1".into(),
                trigger_phase: phase,
                consumer_id: Uuid::now_v7(),
            },
            EventSourceTarget {
                scope: EventSourceScope::Workspace { workspace_id },
                source_id: source.into(),
                workflow_id,
            },
        )
        .unwrap()
}
fn phases(center: &EventCenterRuntime, workspace_id: Uuid, phase: EventTriggerPhase) -> usize {
    center
        .snapshot_sources(EventSourceScope::Workspace { workspace_id })
        .activity
        .iter()
        .filter(|activity| activity.phase == phase && activity.admission.is_none())
        .count()
}

#[tokio::test]
async fn real_stream_publication_original_dequeue_and_causal_workflow_handoff() {
    let center = center();
    let workspace = Uuid::now_v7();
    let foreign = Uuid::now_v7();
    let ancestor = Uuid::now_v7();
    // A workflow may subscribe to future runs before any runtime queue exists.
    let published = target(
        &center,
        workspace,
        RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
        EventTriggerPhase::Published,
        Uuid::now_v7(),
    );
    let consumed = target(
        &center,
        workspace,
        RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
        EventTriggerPhase::Consumed,
        Uuid::now_v7(),
    );
    let foreign_target = target(
        &center,
        foreign,
        RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
        EventTriggerPhase::Published,
        Uuid::now_v7(),
    );
    let recursive = target(
        &center,
        workspace,
        RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
        EventTriggerPhase::Published,
        ancestor,
    );
    let stream = LocalRuntimeEventStream::new().with_event_source_observer(center.clone());
    let run = Uuid::now_v7();
    stream
        .open_run_scoped(
            run,
            RuntimeEventStreamPolicy::debug_default(),
            RuntimeEventSourceBinding::workspace(workspace).with_workflow_ancestors(vec![ancestor]),
        )
        .await
        .unwrap();
    let mut original = stream.subscribe(run, None).await.unwrap();
    let envelope = stream.append(run, required_text_delta(1)).await.unwrap();
    assert_eq!(phases(&center, workspace, EventTriggerPhase::Consumed), 0);
    let delivery = published.try_recv().unwrap().unwrap();
    assert_eq!(
        delivery
            .signal()
            .source_context
            .as_ref()
            .unwrap()
            .original_event_id
            .as_deref(),
        Some(envelope.event_id.as_str())
    );
    assert!(delivery.signal().workflow_ancestor_ids.contains(&ancestor));
    let EventCenterSignalBody::Published { payload } = &delivery.signal().body else {
        panic!("expected canonical publication")
    };
    assert_eq!(*payload, serde_json::to_value(&envelope).unwrap());
    drop(delivery);
    // Workflow ingress handoff cannot masquerade as the original consumer.
    assert_eq!(phases(&center, workspace, EventTriggerPhase::Consumed), 0);
    assert!(consumed.try_recv().unwrap().is_none());
    assert!(foreign_target.try_recv().unwrap().is_none());
    assert!(recursive.try_recv().unwrap().is_none());
    assert_eq!(original.live_events.recv().await.unwrap(), envelope);
    assert!(consumed.try_recv().unwrap().is_some());
    assert_eq!(phases(&center, workspace, EventTriggerPhase::Consumed), 1);
    let snapshot = center.snapshot_sources(EventSourceScope::Workspace {
        workspace_id: workspace,
    });
    let queue = &snapshot
        .sources
        .iter()
        .find(|source| source.descriptor.source_id == RUNTIME_EVENT_REQUIRED_CONTRACT_ID)
        .unwrap()
        .queues[0];
    assert_eq!(
        (
            queue.accepted,
            queue.dequeued,
            queue.buffered,
            queue.dropped,
            queue.in_flight
        ),
        (Some(1), Some(1), Some(0), Some(0), None)
    );
    drop(original);
    assert!(center
        .snapshot_sources(EventSourceScope::Workspace {
            workspace_id: workspace
        })
        .sources
        .iter()
        .all(|source| source.queues.is_empty()));
}

#[tokio::test]
async fn run_generations_replay_scope_and_last_lease_cleanup_remain_distinct() {
    let center = center();
    let workspace = Uuid::now_v7();
    let scope = EventSourceScope::Workspace {
        workspace_id: workspace,
    };
    let stream = LocalRuntimeEventStream::new().with_event_source_observer(center.clone());
    let run = Uuid::now_v7();
    let binding = RuntimeEventSourceBinding::workspace(workspace);
    stream
        .open_run_scoped(
            run,
            RuntimeEventStreamPolicy::debug_default(),
            binding.clone(),
        )
        .await
        .unwrap();
    let first = stream.append(run, heartbeat()).await.unwrap();
    let old = stream.subscribe(run, None).await.unwrap();
    assert_eq!(old.replay, vec![first]);
    let published = phases(&center, workspace, EventTriggerPhase::Published);
    stream.replay(run, None, 10).await.unwrap();
    assert_eq!(
        phases(&center, workspace, EventTriggerPhase::Published),
        published
    );
    stream
        .close_run(run, RuntimeEventCloseReason::WaitingCallback)
        .await
        .unwrap();
    stream
        .open_run_scoped(
            run,
            RuntimeEventStreamPolicy::debug_default(),
            binding.clone(),
        )
        .await
        .unwrap();
    let new = stream.subscribe(run, None).await.unwrap();
    assert_ne!(
        old.terminal_writer.generation_id(),
        new.terminal_writer.generation_id()
    );
    assert_eq!(center.snapshot_sources(scope.clone()).sources.len(), 6);
    let parallel_run = Uuid::now_v7();
    stream
        .open_run_scoped(
            parallel_run,
            RuntimeEventStreamPolicy::debug_default(),
            binding,
        )
        .await
        .unwrap();
    assert_eq!(center.snapshot_sources(scope.clone()).sources.len(), 9);
    let unbound = Uuid::now_v7();
    stream
        .open_run(unbound, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    stream.append(unbound, heartbeat()).await.unwrap();
    assert_eq!(center.snapshot_sources(scope.clone()).sources.len(), 9);
    assert!(center
        .snapshot_sources(EventSourceScope::System)
        .activity
        .is_empty());
    drop(old);
    drop(new);
    drop(stream);
    // Expiry task holds only a Weak reference; no observer lease keeps sources alive.
    tokio::time::timeout(Duration::from_secs(1), async {
        while !center.snapshot_sources(scope.clone()).sources.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[test]
fn canonical_envelope_schema_validates_real_serde_time_options_and_rejects_fake_objects() {
    let validator = jsonschema::validator_for(&runtime_event_envelope_schema()).unwrap();
    let mut envelope = RuntimeEventEnvelope::new(Uuid::now_v7(), 1, heartbeat());
    let value = serde_json::to_value(&envelope).unwrap();
    assert!(validator.is_valid(&value));
    assert_eq!(value["occurred_at"].as_array().unwrap().len(), 9);
    assert!(value["node_run_id"].is_null());
    envelope.node_run_id = Some(Uuid::now_v7());
    envelope.text = Some("actual text".into());
    envelope.delta_index = Some(2);
    envelope.content_type = Some("text".into());
    assert!(validator.is_valid(&serde_json::to_value(envelope).unwrap()));
    let mut invalid = value;
    invalid["occurred_at"] = json!({"seconds": 1});
    assert!(!validator.is_valid(&invalid));
    assert!(!validator.is_valid(&json!({})));
}

struct ProjectionObserver {
    center: Arc<EventCenterRuntime>,
    projected: Arc<std::sync::atomic::AtomicUsize>,
}
struct ProjectionLease {
    inner: Arc<dyn EventSourceRegistration>,
    projected: Arc<std::sync::atomic::AtomicUsize>,
}
impl EventSourceObserver for ProjectionObserver {
    fn register_source(
        &self,
        descriptor: domain::event_center::EventSourceDescriptor,
    ) -> anyhow::Result<Arc<dyn EventSourceRegistration>> {
        Ok(Arc::new(ProjectionLease {
            inner: self.center.register_source(descriptor)?,
            projected: self.projected.clone(),
        }))
    }
}
impl EventSourceRegistration for ProjectionLease {
    fn observe_with_payload(
        &self,
        observation: EventSourceObservation,
        payload: &mut dyn FnMut() -> Option<serde_json::Value>,
    ) -> EventSourceObservationStatus {
        self.inner.observe_with_payload(observation, &mut || {
            self.projected
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            payload()
        })
    }
    fn observe(&self, observation: EventSourceObservation) -> EventSourceObservationStatus {
        self.inner.observe(observation)
    }
    fn set_queue_snapshots(
        &self,
        queues: Vec<domain::event_center::EventSourceQueueSnapshot>,
    ) -> bool {
        self.inner.set_queue_snapshots(queues)
    }
}

#[tokio::test]
async fn real_stream_projects_no_observer_body_without_matching_target() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let center = center();
    let workspace = Uuid::now_v7();
    let projected = Arc::new(AtomicUsize::new(0));
    let stream =
        LocalRuntimeEventStream::new().with_event_source_observer(Arc::new(ProjectionObserver {
            center: center.clone(),
            projected: projected.clone(),
        }));
    let run = Uuid::now_v7();
    stream
        .open_run_scoped(
            run,
            RuntimeEventStreamPolicy::debug_default(),
            RuntimeEventSourceBinding::workspace(workspace),
        )
        .await
        .unwrap();
    let _foreign = target(
        &center,
        Uuid::now_v7(),
        RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
        EventTriggerPhase::Published,
        Uuid::now_v7(),
    );
    let _different_lane = target(
        &center,
        workspace,
        RUNTIME_EVENT_DIAGNOSTIC_CONTRACT_ID,
        EventTriggerPhase::Published,
        Uuid::now_v7(),
    );
    stream.append(run, required_text_delta(1)).await.unwrap();
    assert_eq!(projected.load(Ordering::SeqCst), 0);
    let matching = target(
        &center,
        workspace,
        RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
        EventTriggerPhase::Published,
        Uuid::now_v7(),
    );
    stream.append(run, required_text_delta(2)).await.unwrap();
    assert_eq!(projected.load(Ordering::SeqCst), 1);
    assert!(matching.try_recv().unwrap().is_some());
}

#[tokio::test]
async fn observed_original_lanes_preserve_backpressure_and_diagnostic_drop_counters() {
    let center = center();
    let workspace = Uuid::now_v7();
    let binding = RuntimeEventSourceBinding::workspace(workspace);
    let run = Uuid::now_v7();
    let generation = Uuid::now_v7();
    let descriptors = runtime_event_source_descriptors(
        binding.scope.clone(),
        domain::event_center::EventSourceBinding::Active,
    );
    let required_source = RuntimeEventSourceObservation::register(
        center.as_ref(),
        descriptors[0].clone(),
        binding.clone(),
        run,
        generation,
    )
    .unwrap();
    let diagnostic_source = RuntimeEventSourceObservation::register(
        center.as_ref(),
        descriptors[1].clone(),
        binding,
        run,
        generation,
    )
    .unwrap();
    let (required, diagnostic, mut receiver) = RuntimeEventReceiver::bounded_observed_lanes(
        1,
        Some(required_source.queue("original-required".into())),
        Some(diagnostic_source.queue("original-diagnostic".into())),
    );
    required
        .send(RuntimeEventEnvelope::new(run, 1, required_text_delta(1)))
        .await
        .unwrap();
    let pending = required.send(RuntimeEventEnvelope::new(run, 2, required_text_delta(2)));
    tokio::pin!(pending);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut pending)
            .await
            .is_err()
    );
    assert_eq!(receiver.recv().await.unwrap().sequence, 1);
    pending.await.unwrap();
    assert_eq!(receiver.recv().await.unwrap().sequence, 2);
    diagnostic.try_send(RuntimeEventEnvelope::new(run, 3, heartbeat()));
    assert!(matches!(
        diagnostic
            .try_send(RuntimeEventEnvelope::new(run, 4, heartbeat()))
            .status,
        RuntimeEventDiagnosticDeliveryStatus::Dropped(
            RuntimeEventDiagnosticDropReason::ReceiverFull
        )
    ));
    let snapshot = center.snapshot_sources(EventSourceScope::Workspace {
        workspace_id: workspace,
    });
    let required = &snapshot
        .sources
        .iter()
        .find(|source| source.descriptor.source_id == RUNTIME_EVENT_REQUIRED_CONTRACT_ID)
        .unwrap()
        .queues[0];
    assert_eq!(
        (required.accepted, required.dequeued, required.dropped),
        (Some(2), Some(2), Some(0))
    );
    let diagnostic = &snapshot
        .sources
        .iter()
        .find(|source| source.descriptor.source_id == RUNTIME_EVENT_DIAGNOSTIC_CONTRACT_ID)
        .unwrap()
        .queues[0];
    assert_eq!(
        (diagnostic.accepted, diagnostic.dequeued, diagnostic.dropped),
        (Some(1), Some(0), Some(1))
    );
}

#[tokio::test]
async fn actual_host_stream_reports_diagnostic_drop_without_delaying_required_handoff() {
    let center = center();
    let workspace = Uuid::now_v7();
    let stream = LocalRuntimeEventStream::with_broadcast_capacity_for_tests(1)
        .with_event_source_observer(center.clone());
    let run = Uuid::now_v7();
    stream
        .open_run_scoped(
            run,
            RuntimeEventStreamPolicy::debug_default(),
            RuntimeEventSourceBinding::workspace(workspace),
        )
        .await
        .unwrap();
    let mut subscription = stream.subscribe(run, Some(0)).await.unwrap();
    stream.append(run, heartbeat()).await.unwrap();
    stream.append(run, heartbeat()).await.unwrap();
    stream.append(run, required_text_delta(1)).await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(1), subscription.live_events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.sequence, 3);
    let sources = center
        .snapshot_sources(EventSourceScope::Workspace {
            workspace_id: workspace,
        })
        .sources;
    let diagnostic = &sources
        .iter()
        .find(|source| source.descriptor.source_id == RUNTIME_EVENT_DIAGNOSTIC_CONTRACT_ID)
        .unwrap()
        .queues[0];
    assert_eq!(
        (diagnostic.accepted, diagnostic.dropped, diagnostic.dequeued),
        (Some(1), Some(1), Some(0))
    );
    assert_eq!(
        subscription
            .live_events
            .diagnostic_delivery_snapshot()
            .dropped_total,
        1
    );
    let required = &sources
        .iter()
        .find(|source| source.descriptor.source_id == RUNTIME_EVENT_REQUIRED_CONTRACT_ID)
        .unwrap()
        .queues[0];
    assert_eq!(
        (required.accepted, required.dequeued, required.dropped),
        (Some(1), Some(1), Some(0))
    );
}

#[tokio::test]
async fn terminal_retry_and_replay_do_not_fabricate_new_publications() {
    let center = center();
    let workspace = Uuid::now_v7();
    let stream = LocalRuntimeEventStream::new().with_event_source_observer(center.clone());
    let run = Uuid::now_v7();
    stream
        .open_run_scoped(
            run,
            RuntimeEventStreamPolicy::debug_default(),
            RuntimeEventSourceBinding::workspace(workspace),
        )
        .await
        .unwrap();
    let mut terminal = required_text_delta(0);
    terminal.event_type = "flow_finished".into();
    assert_eq!(
        stream
            .append_terminal_if_missing_and_close(run, terminal.clone())
            .await
            .unwrap(),
        AppendTerminalIfMissingAndCloseOutcome::Appended
    );
    assert_eq!(phases(&center, workspace, EventTriggerPhase::Published), 1);
    assert_eq!(
        stream
            .append_terminal_if_missing_and_close(run, terminal)
            .await
            .unwrap(),
        AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal
    );
    stream.replay(run, None, 10).await.unwrap();
    let subscription = stream.subscribe(run, None).await.unwrap();
    assert_eq!(subscription.replay.len(), 1);
    assert_eq!(phases(&center, workspace, EventTriggerPhase::Published), 1);
    assert_eq!(phases(&center, workspace, EventTriggerPhase::Consumed), 0);
}

#[tokio::test]
async fn workspace_and_system_with_same_uuid_remain_distinct_delivery_scopes() {
    let center = center();
    let workspace_id = domain::SYSTEM_SCOPE_ID;
    let workspace_scope = EventSourceScope::Workspace { workspace_id };
    let workspace_binding = RuntimeEventSourceBinding::workspace(workspace_id);
    assert_eq!(workspace_binding.scope, workspace_scope);
    assert_eq!(
        RuntimeEventSourceBinding::system().scope,
        EventSourceScope::System
    );
    let workspace_target = target(
        &center,
        workspace_id,
        RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
        EventTriggerPhase::Published,
        Uuid::now_v7(),
    );
    let system_target = center
        .subscribe_source(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id: domain::SYSTEM_SCOPE_ID,
                contract_id: RUNTIME_EVENT_REQUIRED_CONTRACT_ID.into(),
                contract_version: "1".into(),
                trigger_phase: EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7(),
            },
            EventSourceTarget {
                scope: EventSourceScope::System,
                source_id: RUNTIME_EVENT_REQUIRED_CONTRACT_ID.into(),
                workflow_id: Uuid::now_v7(),
            },
        )
        .unwrap();
    let stream = LocalRuntimeEventStream::new().with_event_source_observer(center.clone());
    let workspace_run = Uuid::now_v7();
    let system_run = Uuid::now_v7();
    stream
        .open_run_scoped(
            workspace_run,
            RuntimeEventStreamPolicy::debug_default(),
            workspace_binding,
        )
        .await
        .unwrap();
    stream
        .open_run_scoped(
            system_run,
            RuntimeEventStreamPolicy::debug_default(),
            RuntimeEventSourceBinding::system(),
        )
        .await
        .unwrap();

    let workspace_event = stream
        .append(workspace_run, required_text_delta(1))
        .await
        .unwrap();
    let workspace_delivery = workspace_target.try_recv().unwrap().unwrap();
    let workspace_context = workspace_delivery.signal().source_context.as_ref().unwrap();
    assert_eq!(workspace_context.scope, workspace_scope);
    assert_eq!(
        workspace_context.original_event_id.as_deref(),
        Some(workspace_event.event_id.as_str())
    );
    assert!(
        system_target.try_recv().unwrap().is_none(),
        "Workspace publication must not enter System despite the same UUID"
    );
    drop(workspace_delivery);

    let system_event = stream
        .append(system_run, required_text_delta(2))
        .await
        .unwrap();
    let system_delivery = system_target.try_recv().unwrap().unwrap();
    let system_context = system_delivery.signal().source_context.as_ref().unwrap();
    assert_eq!(system_context.scope, EventSourceScope::System);
    assert_eq!(
        system_context.original_event_id.as_deref(),
        Some(system_event.event_id.as_str())
    );
    assert!(
        workspace_target.try_recv().unwrap().is_none(),
        "System publication must not enter Workspace despite the same UUID"
    );
    assert_eq!(center.snapshot_sources(workspace_scope).sources.len(), 3);
    assert_eq!(
        center
            .snapshot_sources(EventSourceScope::System)
            .sources
            .len(),
        3
    );
}
