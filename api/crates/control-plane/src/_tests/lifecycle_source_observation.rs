use super::*;
use control_plane_contracts::ports::{
    LifecycleFactObserver, LifecycleObservationContext, RecordLifecycleFactInput,
};
use domain::event_center::EventSourceScope;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

#[derive(Default)]
struct Facts {
    consumed: AtomicUsize,
    published: AtomicUsize,
    lineage: Mutex<Vec<Uuid>>,
}
impl LifecycleFactObserver for Facts {
    fn published(&self, _: &RecordLifecycleFactInput) {
        self.published.fetch_add(1, Ordering::SeqCst);
    }
    fn consumed(&self, fact: &LifecycleOutboxRecord, _: &str) {
        self.consumed.fetch_add(1, Ordering::SeqCst);
        *self.lineage.lock().unwrap() = fact
            .observation_context
            .as_ref()
            .unwrap()
            .workflow_ancestor_ids
            .clone();
    }
}
struct FailedOriginalConsumer(Arc<Facts>, Vec<Uuid>);
#[async_trait]
impl LifecycleFactDeliveryPort for FailedOriginalConsumer {
    async fn deliver(&self, _: &LifecycleOutboxRecord) -> Result<()> {
        assert!(
            self.0.consumed.load(Ordering::SeqCst) > 0,
            "handoff must precede failing handler"
        );
        assert_eq!(
            control_plane_contracts::ports::lifecycle_workflow_ancestor_ids(),
            self.1
        );
        anyhow::bail!("original handler failed")
    }
}
struct NoCompletion;
impl LifecycleDeliveryCompletionPort for NoCompletion {
    fn complete(&self, _: CompletionOutcome<LifecycleFactDeliveryCompletion>) {}
}

#[tokio::test]
async fn original_failed_handoff_and_retry_are_consumed_without_recovery_publication() {
    let mut repository = super::tests::repository();
    let ancestors = vec![Uuid::now_v7(), Uuid::now_v7()];
    repository.record.observation_context = Some(LifecycleObservationContext {
        scope: EventSourceScope::Workspace {
            workspace_id: Uuid::now_v7(),
        },
        producer_id: "native-model-owner".into(),
        workflow_ancestor_ids: ancestors.clone(),
    });
    let observer = Arc::new(Facts::default());
    let dispatcher = LifecycleOutboxDispatcher::new(
        repository.clone(),
        Arc::new(FailedOriginalConsumer(observer.clone(), ancestors.clone())),
        Arc::new(NoCompletion),
    )
    .with_lifecycle_observer(observer.clone());
    dispatcher.run_once().await.unwrap();
    dispatcher.run_once().await.unwrap();
    assert_eq!(observer.consumed.load(Ordering::SeqCst), 2);
    assert_eq!(
        observer.published.load(Ordering::SeqCst),
        0,
        "claim/recovery cannot masquerade as publish"
    );
    assert_eq!(*observer.lineage.lock().unwrap(), ancestors);
    assert_eq!(
        *repository.completed.lock().unwrap(),
        Some(control_plane_contracts::ports::LifecycleOutboxStatus::Pending)
    );
}

use control_plane_contracts::ports::{
    EventSourceFact, EventSourceObservation, EventSourceObservationStatus, EventSourceObserver,
    EventSourceRegistration, LifecycleSubscriberTarget,
};
use domain::event_center::*;
use std::sync::atomic::AtomicBool;

#[derive(Default)]
struct SourceLease {
    body_targets: AtomicBool,
    projections: AtomicUsize,
    facts: Mutex<Vec<EventSourceObservation>>,
    queues: Mutex<Vec<EventSourceQueueSnapshot>>,
}
impl EventSourceRegistration for SourceLease {
    fn observe_with_payload(
        &self,
        fact: EventSourceObservation,
        payload: &mut dyn FnMut() -> Option<serde_json::Value>,
    ) -> EventSourceObservationStatus {
        self.facts.lock().unwrap().push(fact);
        if self.body_targets.load(Ordering::SeqCst) {
            self.projections.fetch_add(1, Ordering::SeqCst);
            if payload().is_none() {
                return EventSourceObservationStatus::InvalidBody;
            }
        }
        EventSourceObservationStatus::Observed
    }
    fn observe(&self, fact: EventSourceObservation) -> EventSourceObservationStatus {
        self.facts.lock().unwrap().push(fact);
        EventSourceObservationStatus::Observed
    }
    fn set_queue_snapshots(&self, queues: Vec<EventSourceQueueSnapshot>) -> bool {
        *self.queues.lock().unwrap() = queues;
        true
    }
}
struct RegisteredSource(Arc<SourceLease>);
impl EventSourceObserver for RegisteredSource {
    fn register_source(
        &self,
        _: EventSourceDescriptor,
    ) -> Result<Arc<dyn EventSourceRegistration>> {
        Ok(self.0.clone())
    }
}

#[test]
fn source_scope_is_owner_bound_and_observing_without_targets_does_not_project_a_body() {
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let lease = Arc::new(SourceLease::default());
    let observer = LifecycleSourceObserver::new(Arc::new(RegisteredSource(lease.clone())));
    observer.register(
        EventSourceDescriptor {
            source_id: "native/registered-point".into(),
            owner_id: "native-owner".into(),
            point_id: "source.point".into(),
            contract_id: "business.fact".into(),
            contract_version: "v1".into(),
            name: "Fact".into(),
            description: "typed fact fixture".into(),
            payload_schema: serde_json::json!({"type":"object"}),
            origin: EventDefinitionOrigin::Core,
            scope: scope.clone(),
            binding: EventSourceBinding::Active,
            coverage: EventSourceCoverage::Live,
            supported_phases: vec![EventTriggerPhase::Published, EventTriggerPhase::Consumed],
        },
        &[LifecycleSubscriberTarget {
            subscriber_id: "string/original-owner".into(),
            handler_id: "handler".into(),
            handler_version: "v1".into(),
        }],
    );
    assert!(
        lease.facts.lock().unwrap().is_empty(),
        "restored binding cannot fabricate Subscribed"
    );
    assert_eq!(lease.queues.lock().unwrap()[0].buffered, None);
    let mut input = RecordLifecycleFactInput {
        observation_context: Some(LifecycleObservationContext {
            scope: scope.clone(),
            producer_id: "trusted".into(),
            workflow_ancestor_ids: Vec::new(),
        }),
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "business.fact".into(),
        contract_version: "v1".into(),
        canonical_payload: b"deliberately invalid JSON; no body target".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: control_plane_contracts::ports::LifecyclePublicationPlan {
            graph_fingerprint: "frozen".into(),
            subscribers: Vec::new(),
        },
    };
    observer.published(&input);
    assert_eq!(lease.facts.lock().unwrap().len(), 1);
    assert_eq!(lease.projections.load(Ordering::SeqCst), 0);
    input.observation_context.as_mut().unwrap().scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    observer.published(&input);
    input.observation_context = None;
    observer.published(&input);
    assert_eq!(
        lease.facts.lock().unwrap().len(),
        1,
        "unknown or foreign scope cannot use a source lease"
    );
    assert!(matches!(
        lease.facts.lock().unwrap()[0].fact,
        EventSourceFact::Published
    ));
}
