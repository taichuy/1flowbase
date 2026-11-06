//! Runtime-owner scope, generation leases and original queue handoff observations.
use super::*;
use domain::event_center::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEventSourceBinding {
    pub scope: EventSourceScope,
    pub workflow_ancestor_ids: Vec<Uuid>,
}
impl RuntimeEventSourceBinding {
    pub fn workspace(workspace_id: Uuid) -> Self {
        Self {
            scope: EventSourceScope::Workspace { workspace_id },
            workflow_ancestor_ids: Vec::new(),
        }
    }
    pub fn system() -> Self {
        Self {
            scope: EventSourceScope::System,
            workflow_ancestor_ids: Vec::new(),
        }
    }
    pub fn with_workflow_ancestors(mut self, ancestors: Vec<Uuid>) -> Self {
        self.workflow_ancestor_ids = ancestors;
        self
    }
}

/// Typed owner schema for the actual default serde representation (time uses its tuple codec).
pub fn runtime_event_envelope_schema() -> serde_json::Value {
    let nullable = |kind| serde_json::json!({"type": [kind, "null"]});
    serde_json::json!({
        "type": "object", "additionalProperties": false,
        "required": ["run_id", "node_run_id", "sequence", "event_id", "event_type", "occurred_at", "delta_index", "content_type", "text", "source", "durability", "persist_required", "trace_visible", "payload"],
        "properties": {
            "run_id": {"type":"string", "format":"uuid"},
            "node_run_id": {"type":["string","null"], "format":"uuid"},
            "sequence": {"type":"integer"}, "event_id": {"type":"string"}, "event_type": {"type":"string"},
            "occurred_at": {"type":"array", "items":{"type":"integer"}, "minItems":9, "maxItems":9},
            "delta_index": nullable("integer"), "content_type": nullable("string"), "text": nullable("string"),
            "source": {"enum":["runtime","provider","persister","system"]},
            "durability": {"enum":["ephemeral","durable_required","audit_required"]},
            "persist_required":{"type":"boolean"}, "trace_visible":{"type":"boolean"}, "payload": {}
        }
    })
}

pub fn runtime_event_source_descriptors(
    scope: EventSourceScope,
    binding: EventSourceBinding,
) -> Vec<EventSourceDescriptor> {
    [
        (
            RUNTIME_EVENT_REQUIRED_POINT_ID,
            RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
        ),
        (
            RUNTIME_EVENT_DIAGNOSTIC_POINT_ID,
            RUNTIME_EVENT_DIAGNOSTIC_CONTRACT_ID,
        ),
        (
            RUNTIME_EVENT_AFTER_COMMIT_POINT_ID,
            RUNTIME_EVENT_AFTER_COMMIT_CONTRACT_ID,
        ),
    ]
    .into_iter()
    .map(|(point, contract)| EventSourceDescriptor {
        source_id: contract.into(),
        owner_id: RUNTIME_EVENT_LANE_OWNER_MODULE_ID.into(),
        point_id: point.into(),
        contract_id: contract.into(),
        contract_version: RUNTIME_EVENT_LANE_CONTRACT_VERSION.into(),
        name: contract.into(),
        description: "Canonical runtime event envelope at its original delivery owner".into(),
        payload_schema: runtime_event_envelope_schema(),
        origin: EventDefinitionOrigin::Core,
        scope: scope.clone(),
        binding,
        coverage: if scope == EventSourceScope::Unbound {
            EventSourceCoverage::Unavailable
        } else {
            EventSourceCoverage::Live
        },
        supported_phases: if binding == EventSourceBinding::Active
            && scope != EventSourceScope::Unbound
        {
            vec![
                EventTriggerPhase::Published,
                EventTriggerPhase::Subscribed,
                EventTriggerPhase::Consumed,
            ]
        } else {
            Vec::new()
        },
    })
    .collect()
}

#[derive(Clone)]
pub struct RuntimeEventSourceObservation {
    lease: Arc<dyn EventSourceRegistration>,
    binding: RuntimeEventSourceBinding,
    producer: String,
    queues: Arc<Mutex<BTreeMap<String, EventSourceQueueSnapshot>>>,
}
impl RuntimeEventSourceObservation {
    pub fn register(
        observer: &dyn EventSourceObserver,
        descriptor: EventSourceDescriptor,
        binding: RuntimeEventSourceBinding,
        run_id: Uuid,
        generation_id: Uuid,
    ) -> Option<Self> {
        observer.register_source(descriptor).ok().map(|lease| Self {
            lease,
            binding,
            producer: format!("{run_id}:{generation_id}"),
            queues: Default::default(),
        })
    }
    fn fact(
        &self,
        event: Option<&RuntimeEventEnvelope>,
        fact: EventSourceFact,
    ) -> EventSourceObservation {
        EventSourceObservation {
            original_event_id: event.map(|event| event.event_id.clone()),
            producer_id: Some(self.producer.clone()),
            occurred_at_unix_ms: now_ms(),
            workflow_ancestor_ids: self.binding.workflow_ancestor_ids.clone(),
            fact,
        }
    }
    pub fn published(&self, event: &RuntimeEventEnvelope) {
        let _ = self.lease.observe_with_payload(
            self.fact(Some(event), EventSourceFact::Published),
            &mut || serde_json::to_value(event).ok(),
        );
    }
    pub fn queue(&self, subscriber: String) -> RuntimeEventQueueObservation {
        let queue = RuntimeEventQueueObservation {
            source: self.clone(),
            subscriber,
        };
        let mut queues = self.queues.lock().unwrap_or_else(|e| e.into_inner());
        queues.insert(
            queue.subscriber.clone(),
            EventSourceQueueSnapshot {
                queue_id: format!("{}:{}", self.producer, queue.subscriber),
                subscriber_id: queue.subscriber.clone(),
                consumer_ids: vec![queue.subscriber.clone()],
                buffered: Some(0),
                in_flight: None,
                accepted: Some(0),
                dequeued: Some(0),
                dropped: Some(0),
                observed_at_unix_ms: now_ms(),
            },
        );
        let _ = self
            .lease
            .set_queue_snapshots(queues.values().cloned().collect());
        drop(queues);
        let _ = self.lease.observe(self.fact(
            None,
            EventSourceFact::Subscribed {
                subscriber_id: queue.subscriber.clone(),
                consumer_id: queue.subscriber.clone(),
            },
        ));
        queue
    }
}
#[derive(Clone)]
pub struct RuntimeEventQueueObservation {
    source: RuntimeEventSourceObservation,
    subscriber: String,
}
impl RuntimeEventQueueObservation {
    pub fn accepted(&self) {
        self.update(|q| {
            *q.accepted.as_mut().unwrap() += 1;
            *q.buffered.as_mut().unwrap() += 1;
        });
    }
    pub fn dropped(&self) {
        self.update(|q| *q.dropped.as_mut().unwrap() += 1);
    }
    pub fn consumed(&self, event: &RuntimeEventEnvelope) {
        self.update(|q| {
            *q.dequeued.as_mut().unwrap() += 1;
            *q.buffered.as_mut().unwrap() = q.buffered.unwrap().saturating_sub(1);
        });
        let _ = self.source.lease.observe(self.source.fact(
            Some(event),
            EventSourceFact::Consumed {
                subscriber_id: self.subscriber.clone(),
                consumer_id: self.subscriber.clone(),
            },
        ));
    }
    fn update(&self, update: impl FnOnce(&mut EventSourceQueueSnapshot)) {
        let mut queues = self.source.queues.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(queue) = queues.get_mut(&self.subscriber) {
            update(queue);
            queue.observed_at_unix_ms = now_ms();
        }
        let _ = self
            .source
            .lease
            .set_queue_snapshots(queues.values().cloned().collect());
    }
    pub fn retire(&self) {
        let mut queues = self.source.queues.lock().unwrap_or_else(|e| e.into_inner());
        queues.remove(&self.subscriber);
        let _ = self
            .source
            .lease
            .set_queue_snapshots(queues.values().cloned().collect());
    }
}
fn now_ms() -> i64 {
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}
