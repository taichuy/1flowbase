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
    fn open_queue(
        self: Arc<Self>,
        _: EventSourceQueueSnapshot,
    ) -> Option<Arc<dyn control_plane_contracts::ports::EventSourceQueueRegistration>> {
        // This durable observer fixture only exercises authoritative full refreshes.
        None
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

#[derive(Default)]
struct SourceRegistry(Mutex<Vec<(EventSourceDescriptor, Arc<SourceLease>)>>);
impl EventSourceObserver for SourceRegistry {
    fn register_source(
        &self,
        descriptor: EventSourceDescriptor,
    ) -> Result<Arc<dyn EventSourceRegistration>> {
        let lease = Arc::new(SourceLease::default());
        self.0.lock().unwrap().push((descriptor, lease.clone()));
        Ok(lease)
    }
}

fn native_template() -> EventSourceDescriptor {
    use control_plane_contracts::ports::ModelDefinitionCommittedFact;
    use extension_contracts::LifecycleContract;
    EventSourceDescriptor {
        source_id: lifecycle_source_id(
            &EventSourceScope::Unbound,
            ModelDefinitionCommittedFact::CONTRACT_ID,
            ModelDefinitionCommittedFact::CONTRACT_VERSION,
        ),
        owner_id: "native-owner".into(),
        point_id: extension_contracts::MANAGED_CREATE_EVENT_POINT.into(),
        contract_id: ModelDefinitionCommittedFact::CONTRACT_ID.into(),
        contract_version: ModelDefinitionCommittedFact::CONTRACT_VERSION.into(),
        name: "Native template".into(),
        description: "Historical scope is unavailable; scoped queues are a trusted subset".into(),
        payload_schema: ModelDefinitionCommittedFact::event_source_schema(),
        origin: EventDefinitionOrigin::Core,
        scope: EventSourceScope::Unbound,
        binding: EventSourceBinding::Declaration,
        coverage: EventSourceCoverage::Unavailable,
        supported_phases: Vec::new(),
    }
}

#[test]
fn boot_template_activates_first_new_workspace_publish_without_inventing_a_queue_or_subscription() {
    let registry = Arc::new(SourceRegistry::default());
    let observer = LifecycleSourceObserver::new(registry.clone());
    let template = native_template();
    observer.register(
        template.clone(),
        &[LifecycleSubscriberTarget {
            subscriber_id: "template-is-not-a-queue".into(),
            handler_id: "not-active".into(),
            handler_version: "v1".into(),
        }],
    );
    {
        let registered = registry.0.lock().unwrap();
        assert_eq!(registered.len(), 1);
        assert!(registered[0].1.queues.lock().unwrap().is_empty());
        assert!(registered[0].1.facts.lock().unwrap().is_empty());
    }
    // This workspace was not enumerated or registered at boot, even if its UUID is SYSTEM_SCOPE_ID.
    let scope = EventSourceScope::Workspace {
        workspace_id: domain::SYSTEM_SCOPE_ID,
    };
    let input = RecordLifecycleFactInput {
        observation_context: Some(LifecycleObservationContext {
            scope: scope.clone(),
            producer_id: "actual-native-owner".into(),
            workflow_ancestor_ids: vec![Uuid::now_v7()],
        }),
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: template.contract_id.clone(),
        contract_version: template.contract_version.clone(),
        canonical_payload: b"no body target".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: control_plane_contracts::ports::LifecyclePublicationPlan {
            graph_fingerprint: "original-commit".into(),
            subscribers: Vec::new(),
        },
    };
    observer.published(&input);
    observer.published(&input);
    let registered = registry.0.lock().unwrap();
    assert_eq!(registered.len(), 2, "one stable scoped lease is reused");
    assert_eq!(registered[1].0.scope, scope);
    assert_eq!(
        registered[1].0.source_id,
        lifecycle_source_id(&scope, &input.contract_id, &input.contract_version)
    );
    assert_eq!(registered[1].1.facts.lock().unwrap().len(), 2);
    assert!(registered[1].1.queues.lock().unwrap().is_empty());
    assert_eq!(registered[1].1.projections.load(Ordering::SeqCst), 0);
}

#[test]
fn complete_queue_refresh_recovers_actual_scope_and_clears_removed_backlog_without_activity() {
    let registry = Arc::new(SourceRegistry::default());
    let observer = LifecycleSourceObserver::new(registry.clone());
    let template = native_template();
    observer.register(template.clone(), &[]);
    let scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let queue = EventSourceQueueSnapshot {
        queue_id: "lifecycle-outbox/original".into(),
        subscriber_id: "original".into(),
        consumer_ids: vec!["actual-handler".into()],
        buffered: Some(3),
        in_flight: Some(1),
        accepted: None,
        dequeued: None,
        dropped: None,
        observed_at_unix_ms: 1,
    };
    observer.refresh_queue_snapshots(vec![
        control_plane_contracts::ports::LifecycleSourceQueueFacts {
            scope: scope.clone(),
            contract_id: template.contract_id.clone(),
            contract_version: template.contract_version.clone(),
            queues: vec![queue.clone()],
        },
        control_plane_contracts::ports::LifecycleSourceQueueFacts {
            scope: EventSourceScope::Unbound,
            contract_id: "historical.unknown".into(),
            contract_version: "old".into(),
            queues: vec![queue],
        },
    ]);
    {
        let registered = registry.0.lock().unwrap();
        let scoped = registered.iter().find(|(d, _)| d.scope == scope).unwrap();
        assert_eq!(scoped.1.queues.lock().unwrap()[0].buffered, Some(3));
        let legacy = registered
            .iter()
            .find(|(d, _)| d.contract_id == "historical.unknown")
            .unwrap();
        assert_eq!(legacy.0.scope, EventSourceScope::Unbound);
        assert_eq!(legacy.0.coverage, EventSourceCoverage::Unavailable);
        assert!(legacy.0.supported_phases.is_empty());
        assert!(legacy.0.description.contains("lack trusted source scope"));
        assert!(
            registered
                .iter()
                .all(|(_, lease)| lease.facts.lock().unwrap().is_empty()),
            "row recovery does not fabricate lifecycle phases"
        );
    }
    observer.refresh_queue_snapshots(Vec::new());
    assert!(
        registry
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|(_, lease)| lease.queues.lock().unwrap().is_empty()),
        "removed/compacted rows cannot retain stale buffered counts"
    );
}

#[test]
fn canonical_source_identity_distinguishes_namespace_and_component_boundaries() {
    assert_eq!(
        lifecycle_source_id(
            &EventSourceScope::Workspace {
                workspace_id: domain::SYSTEM_SCOPE_ID
            },
            "native.fact",
            "v1"
        ),
        format!(
            "lifecycle/workspace/{}/contract/native.fact/version/v1",
            domain::SYSTEM_SCOPE_ID
        )
    );
    assert_ne!(
        lifecycle_source_id(&EventSourceScope::System, "native.fact", "v1"),
        lifecycle_source_id(
            &EventSourceScope::Workspace {
                workspace_id: domain::SYSTEM_SCOPE_ID
            },
            "native.fact",
            "v1"
        )
    );
    assert_ne!(
        lifecycle_source_id(&EventSourceScope::Unbound, "x/y", "z"),
        lifecycle_source_id(&EventSourceScope::Unbound, "x", "y/z")
    );
}

#[test]
fn credit_template_cannot_activate_a_system_source_but_keeps_workspace_uuid_identity() {
    use control_plane_contracts::ports::CreditNotificationFact;
    use extension_contracts::LifecycleContract;
    let registry = Arc::new(SourceRegistry::default());
    let observer = LifecycleSourceObserver::new(registry.clone());
    let mut template = native_template();
    template.contract_id = CreditNotificationFact::CONTRACT_ID.into();
    template.contract_version = CreditNotificationFact::CONTRACT_VERSION.into();
    template.payload_schema = CreditNotificationFact::event_source_schema();
    template.source_id = lifecycle_source_id(
        &template.scope,
        &template.contract_id,
        &template.contract_version,
    );
    observer.register(template.clone(), &[]);
    let mut input = RecordLifecycleFactInput {
        observation_context: Some(LifecycleObservationContext {
            scope: EventSourceScope::System,
            producer_id: "credit-owner".into(),
            workflow_ancestor_ids: Vec::new(),
        }),
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: template.contract_id,
        contract_version: template.contract_version,
        canonical_payload: b"no matching body target".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: control_plane_contracts::ports::LifecyclePublicationPlan {
            graph_fingerprint: "committed".into(),
            subscribers: Vec::new(),
        },
    };
    observer.published(&input);
    assert_eq!(
        registry.0.lock().unwrap().len(),
        1,
        "credit has no typed System producer"
    );
    input.observation_context.as_mut().unwrap().scope = EventSourceScope::Workspace {
        workspace_id: domain::SYSTEM_SCOPE_ID,
    };
    observer.published(&input);
    let registered = registry.0.lock().unwrap();
    assert_eq!(registered.len(), 2);
    assert_eq!(
        registered[1].0.scope,
        EventSourceScope::Workspace {
            workspace_id: domain::SYSTEM_SCOPE_ID
        }
    );
}
