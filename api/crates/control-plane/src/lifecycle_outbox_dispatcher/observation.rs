//! Adapter for existing durable owners; it neither reads nor drains the original outbox.
use control_plane_contracts::ports::{
    EventSourceFact, EventSourceObservation, EventSourceObserver, EventSourceRegistration,
    LifecycleFactObserver, LifecycleObservationContext, LifecycleOutboxRecord,
    LifecycleSubscriberTarget, RecordLifecycleFactInput,
};
use domain::event_center::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

struct RegisteredSource {
    lease: Arc<dyn EventSourceRegistration>,
    origin: EventDefinitionOrigin,
    phases: Vec<EventTriggerPhase>,
}

pub struct LifecycleSourceObserver {
    observer: Arc<dyn EventSourceObserver>,
    sources: Mutex<BTreeMap<(EventSourceScope, String, String), RegisteredSource>>,
}

impl LifecycleSourceObserver {
    pub fn new(observer: Arc<dyn EventSourceObserver>) -> Self {
        Self {
            observer,
            sources: Mutex::new(BTreeMap::new()),
        }
    }

    /// Source owner supplies actual schema/point and frozen subscriber bindings before publish.
    /// Refresh restores queue declarations only; it never fabricates lifecycle observations.
    pub fn register(
        &self,
        descriptor: EventSourceDescriptor,
        targets: &[LifecycleSubscriberTarget],
    ) {
        let key = (
            descriptor.scope.clone(),
            descriptor.contract_id.clone(),
            descriptor.contract_version.clone(),
        );
        let Ok(mut sources) = self.sources.lock() else {
            return;
        };
        if sources
            .get(&key)
            .is_none_or(|source| source.phases != descriptor.supported_phases)
        {
            let origin = descriptor.origin;
            let phases = descriptor.supported_phases.clone();
            let Ok(lease) = self.observer.register_source(descriptor) else {
                return;
            };
            sources.insert(
                key.clone(),
                RegisteredSource {
                    lease,
                    origin,
                    phases,
                },
            );
        }
        if let Some(source) = sources.get(&key) {
            source.lease.set_queue_snapshots(
                targets
                    .iter()
                    .map(|target| EventSourceQueueSnapshot {
                        queue_id: format!("lifecycle-outbox/{}", target.subscriber_id),
                        subscriber_id: target.subscriber_id.clone(),
                        consumer_ids: vec![target.handler_id.clone()],
                        buffered: None,
                        in_flight: None,
                        accepted: None,
                        dequeued: None,
                        dropped: None,
                        observed_at_unix_ms: now_ms(),
                    })
                    .collect(),
            );
        }
    }

    /// Root calls this after the repository's read-only grouped snapshot, including on GET.
    /// Counts come from existing durable delivery rows, including retry/restart backlog.
    pub fn refresh_queue_snapshots(
        &self,
        facts: Vec<control_plane_contracts::ports::LifecycleSourceQueueFacts>,
    ) {
        for source in facts {
            let lease = self.sources.lock().ok().and_then(|sources| {
                sources
                    .get(&(source.scope, source.contract_id, source.contract_version))
                    .map(|source| source.lease.clone())
            });
            if let Some(lease) = lease {
                lease.set_queue_snapshots(source.queues);
            }
        }
    }

    /// Current and retained frozen graph ownership controls managed source lifetime.
    pub fn retain_managed_sources(
        &self,
        keys: &std::collections::BTreeSet<(EventSourceScope, String, String)>,
    ) {
        if let Ok(mut sources) = self.sources.lock() {
            sources.retain(|key, source| {
                source.origin != EventDefinitionOrigin::Plugin || keys.contains(key)
            });
        }
    }

    pub fn remove_scope(&self, scope: &EventSourceScope) {
        if let Ok(mut sources) = self.sources.lock() {
            sources.retain(|(s, _, _), _| s != scope);
        }
    }

    fn source(
        &self,
        context: &LifecycleObservationContext,
        contract: &str,
        version: &str,
    ) -> Option<Arc<dyn EventSourceRegistration>> {
        self.sources
            .lock()
            .ok()?
            .get(&(context.scope.clone(), contract.into(), version.into()))
            .map(|source| source.lease.clone())
    }
}

impl LifecycleFactObserver for LifecycleSourceObserver {
    fn published(&self, input: &RecordLifecycleFactInput) {
        let Some(context) = &input.observation_context else {
            return;
        };
        let Some(lease) = self.source(context, &input.contract_id, &input.contract_version) else {
            return;
        };
        // Parsing is deliberately inside the callback: observing alone never reads/copies body.
        lease.observe_with_payload(
            EventSourceObservation {
                original_event_id: Some(input.event_id.to_string()),
                producer_id: Some(context.producer_id.clone()),
                occurred_at_unix_ms: (input.occurred_at.unix_timestamp_nanos() / 1_000_000) as i64,
                workflow_ancestor_ids: context.workflow_ancestor_ids.clone(),
                fact: EventSourceFact::Published,
            },
            &mut || serde_json::from_slice(&input.canonical_payload).ok(),
        );
    }

    fn consumed(&self, record: &LifecycleOutboxRecord, consumer_id: &str) {
        let Some(context) = &record.observation_context else {
            return;
        };
        let Some(lease) = self.source(context, &record.contract_id, &record.contract_version)
        else {
            return;
        };
        lease.observe(EventSourceObservation {
            original_event_id: Some(record.event_id.to_string()),
            producer_id: Some(context.producer_id.clone()),
            occurred_at_unix_ms: now_ms(),
            workflow_ancestor_ids: context.workflow_ancestor_ids.clone(),
            fact: EventSourceFact::Consumed {
                subscriber_id: record.subscriber_id.clone(),
                consumer_id: consumer_id.into(),
            },
        });
    }
}

fn now_ms() -> i64 {
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}
