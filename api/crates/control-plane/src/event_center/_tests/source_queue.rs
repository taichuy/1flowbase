use super::*;
use crate::{event_center::*, ports::*};
use domain::event_center::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use storage_ephemeral::MemoryEventBus;

fn center(bytes: u64) -> EventCenterRuntime {
    EventCenterRuntime::new(
        Arc::new(MemoryEventBus::new()),
        Some(EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 16,
                max_json_bytes: 64_000,
                max_waiters: 4,
                max_topics: 16,
                max_consumers: 16,
                max_topic_name_bytes: 4096,
            },
            observation_max_bytes: bytes,
            observation_ttl_ms: 60_000,
        }),
    )
    .unwrap()
}
fn source(
    center: &EventCenterRuntime,
    workspace: Uuid,
    refreshes: Arc<AtomicUsize>,
) -> Option<RuntimeEventSourceObservation> {
    let scope = EventSourceScope::Workspace {
        workspace_id: workspace,
    };
    RuntimeEventSourceObservation::register(
        &IncrementalObserver {
            center: center.clone(),
            refreshes,
        },
        runtime_event_source_descriptors(scope, EventSourceBinding::Active).remove(0),
        RuntimeEventSourceBinding::workspace(workspace),
        Uuid::now_v7(),
        Uuid::now_v7(),
    )
}
struct IncrementalObserver {
    center: EventCenterRuntime,
    refreshes: Arc<AtomicUsize>,
}
struct IncrementalLease {
    inner: Arc<dyn EventSourceRegistration>,
    refreshes: Arc<AtomicUsize>,
}
impl EventSourceObserver for IncrementalObserver {
    fn register_source(
        &self,
        descriptor: EventSourceDescriptor,
    ) -> anyhow::Result<Arc<dyn EventSourceRegistration>> {
        Ok(Arc::new(IncrementalLease {
            inner: self.center.register_source(descriptor)?,
            refreshes: self.refreshes.clone(),
        }))
    }
}
impl EventSourceRegistration for IncrementalLease {
    fn observe(&self, observation: EventSourceObservation) -> Status {
        self.inner.observe(observation)
    }
    fn observe_with_payload(
        &self,
        observation: EventSourceObservation,
        payload: &mut dyn FnMut() -> Option<serde_json::Value>,
    ) -> Status {
        self.inner.observe_with_payload(observation, payload)
    }
    fn open_queue(
        self: Arc<Self>,
        snapshot: EventSourceQueueSnapshot,
    ) -> Option<Arc<dyn EventSourceQueueRegistration>> {
        self.inner.clone().open_queue(snapshot)
    }
    fn set_queue_snapshots(&self, queues: Vec<EventSourceQueueSnapshot>) -> bool {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        self.inner.set_queue_snapshots(queues)
    }
}
fn event(sequence: i64) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        Uuid::now_v7(),
        sequence,
        RuntimeEventPayload {
            event_type: "text_delta".into(),
            source: RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: true,
            payload: serde_json::json!({"index":sequence}),
        },
    )
}
#[tokio::test]
async fn runtime_adapter_hot_updates_keep_reserved_charge_and_original_handoff_counts() {
    let center = center(16_000);
    let workspace = Uuid::now_v7();
    let scope = EventSourceScope::Workspace {
        workspace_id: workspace,
    };
    let refreshes = Arc::new(AtomicUsize::new(0));
    let source = source(&center, workspace, refreshes.clone()).unwrap();
    let base = center.snapshot_sources(scope.clone()).source_metadata_bytes;
    let observed = source.queue("original-runtime-consumer".into()).unwrap();
    let (sender, diagnostic, mut receiver) =
        RuntimeEventReceiver::bounded_observed_lanes(1, Some(observed.clone()), None);
    let admitted = center.snapshot_sources(scope.clone()).source_metadata_bytes;
    assert!(admitted > base);
    for index in 0..1001 {
        let envelope = event(index);
        sender.send(envelope.clone()).await.unwrap();
        assert_eq!(receiver.recv().await.unwrap(), envelope);
    }
    let snapshot = center.snapshot_sources(scope.clone());
    assert_eq!(
        snapshot.source_metadata_bytes, admitted,
        "crossing counter digit widths must not resize/serialize metadata"
    );
    let queue = &snapshot.sources[0].queues[0];
    assert_eq!(
        (queue.accepted, queue.dequeued, queue.buffered),
        (Some(1001), Some(1001), Some(0))
    );
    assert!(snapshot.sources[0].queue_observation_complete);
    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        0,
        "the actual runtime adapter must never submit a whole queue map"
    );
    drop(receiver);
    let retired = center.snapshot_sources(scope.clone());
    assert!(retired.sources[0].queues.is_empty());
    assert!(retired.source_metadata_bytes < admitted);
    assert!(
        retired.source_metadata_bytes > base,
        "sender/observation clones still own a charged guard"
    );
    drop((sender, diagnostic, observed));
    assert_eq!(
        center.snapshot_sources(scope.clone()).source_metadata_bytes,
        base
    );
    drop(source);
    assert_eq!(center.snapshot_sources(scope).source_metadata_bytes, 0);
}
#[tokio::test]
async fn runtime_adapter_pressure_rejects_metadata_without_rejecting_original_delivery() {
    let center = center(4096);
    let workspace = Uuid::now_v7();
    let scope = EventSourceScope::Workspace {
        workspace_id: workspace,
    };
    let refreshes = Arc::new(AtomicUsize::new(0));
    let source = source(&center, workspace, refreshes.clone()).unwrap();
    let base = center.snapshot_sources(scope.clone()).source_metadata_bytes;
    let workflow = center
        .subscribe_source(
            EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id: workspace,
                contract_id: RUNTIME_EVENT_REQUIRED_CONTRACT_ID.into(),
                contract_version: "1".into(),
                trigger_phase: EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7(),
            },
            EventSourceTarget {
                source_id: RUNTIME_EVENT_REQUIRED_CONTRACT_ID.into(),
                scope: scope.clone(),
                workflow_id: Uuid::now_v7(),
            },
        )
        .unwrap();
    let descriptor = center.snapshot_sources(scope.clone()).sources[0]
        .descriptor
        .clone();

    let mut originals = Vec::new();
    for index in 0..128 {
        originals.push(RuntimeEventReceiver::bounded_observed_lanes(
            1,
            source.queue(format!("original-{index:03}")),
            None,
        ));
    }
    let admitted = center.snapshot_sources(scope.clone());
    assert!(!admitted.sources[0].queue_observation_complete);
    assert!(!admitted.sources[0].queues.is_empty());
    assert!(admitted.sources[0].queues.len() < originals.len());
    assert!(admitted.observation_bytes <= 4096);
    assert_eq!(
        admitted.sources[0].descriptor, descriptor,
        "incomplete queue observation must not change source eligibility"
    );
    let published = event(7);
    source.published(&published);
    assert_eq!(
        workflow
            .try_recv()
            .unwrap()
            .unwrap()
            .signal()
            .source_context
            .as_ref()
            .unwrap()
            .original_event_id
            .as_deref(),
        Some(published.event_id.as_str())
    );

    for (sender, _, receiver) in &mut originals {
        let original = event(42);
        sender.send(original.clone()).await.unwrap();
        assert_eq!(
            receiver.recv().await.unwrap(),
            original,
            "observation admission cannot change required handoff"
        );
    }
    let after = center.snapshot_sources(scope.clone());
    assert_eq!(after.source_metadata_bytes, admitted.source_metadata_bytes);
    assert!(after.sources[0]
        .queues
        .iter()
        .all(|queue| queue.accepted == Some(1)
            && queue.dequeued == Some(1)
            && queue.buffered == Some(0)));
    assert_eq!(refreshes.load(Ordering::SeqCst), 0);
    drop(originals);
    let retired = center.snapshot_sources(scope.clone());
    assert!(retired.sources[0].queues.is_empty());
    assert!(
        !retired.sources[0].queue_observation_complete,
        "empty visible rows cannot conceal previous missing observation"
    );
    assert_eq!(retired.source_metadata_bytes, base);
    let later = source.queue("later-original".into()).unwrap();
    later.accepted();
    assert_eq!(
        center.snapshot_sources(scope).sources[0].queues[0].accepted,
        Some(1),
        "retirement releases budget for later admission"
    );
}
#[tokio::test]
async fn runtime_adapter_source_registration_rejection_leaves_original_channel_usable() {
    let center = center(1);
    let workspace = Uuid::now_v7();
    assert!(source(&center, workspace, Arc::new(AtomicUsize::new(0))).is_none());
    let (sender, _, mut receiver) = RuntimeEventReceiver::bounded_observed_lanes(1, None, None);
    let original = event(1);
    sender.send(original.clone()).await.unwrap();
    assert_eq!(receiver.recv().await.unwrap(), original);
    assert!(center
        .snapshot_sources(EventSourceScope::Workspace {
            workspace_id: workspace
        })
        .sources
        .is_empty());
}
