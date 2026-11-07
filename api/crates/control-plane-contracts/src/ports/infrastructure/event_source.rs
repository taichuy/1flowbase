//! Synchronous, best-effort observation of facts owned by existing sources.
use domain::event_center::{EventSourceDescriptor, EventSourceQueueSnapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventSourceObservation {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub attributes: std::collections::BTreeMap<String, String>,
    pub original_event_id: Option<String>,
    pub producer_id: Option<String>,
    pub occurred_at_unix_ms: i64,
    /// Complete causal workflow chain from the trusted execution context; no depth limit.
    pub workflow_ancestor_ids: Vec<Uuid>,
    pub fact: EventSourceFact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum EventSourceFact {
    /// Body is transient and only projected by observe_with_payload for actual targets.
    Published,
    Subscribed {
        subscriber_id: String,
        consumer_id: String,
    },
    /// Emitted at original queue handoff, before handler result, retry or ACK.
    Consumed {
        subscriber_id: String,
        consumer_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSourceObservationStatus {
    Observed,
    Unconfigured,
    Inactive,
    UnsupportedPhase,
    InvalidIdentity,
    InvalidBody,
}

/// Payload-free dequeue identity. Queue subscriber/consumer identities remain in the
/// central registration and are never duplicated in an adapter-owned queue ledger.
#[derive(Debug, Clone)]
pub struct EventSourceQueueConsumption {
    pub attributes: std::collections::BTreeMap<String, String>,
    pub original_event_id: String,
    pub producer_id: Option<String>,
    pub occurred_at_unix_ms: i64,
    pub workflow_ancestor_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy)]
pub enum EventSourceQueueChange {
    Accepted,
    Dropped,
}

/// Incremental queue observer. The central owner budgets identity, counters and handle
/// metadata before admission. Retire removes visible facts; final drop releases its guard.
pub trait EventSourceQueueRegistration: Send + Sync {
    fn record(&self, change: EventSourceQueueChange, observed_at_unix_ms: i64) -> bool;
    fn consumed(&self, event: EventSourceQueueConsumption) -> EventSourceObservationStatus;
    fn retire(&self);
}

/// Keep this lease for the source lifetime. Last drop removes registration and queue metadata.
/// Observation failure must never change publication, dequeue, retry, fence or ACK correctness.
pub trait EventSourceRegistration: Send + Sync {
    /// Atomically checks matching workflow targets before projecting the Published body.
    /// Pass Published. Callback must only project local source data;
    /// it must not perform IO, await, or reenter this observer. Never called without a target.
    /// None reports projection failure without changing original source correctness.
    fn observe_with_payload(
        &self,
        observation: EventSourceObservation,
        payload: &mut dyn FnMut() -> Option<Value>,
    ) -> EventSourceObservationStatus;
    /// Metadata-only facts. Published cannot enqueue a workflow body through this method.
    fn observe(&self, observation: EventSourceObservation) -> EventSourceObservationStatus;
    /// Admission failure marks queue observation incomplete, without changing original delivery.
    /// One original consumer identity is required per incremental handle.
    /// An admitted handle owns no copied queue strings. Do not mix incremental handles with
    /// full replacements on the same source lease while those handles remain alive.
    fn open_queue(
        self: Arc<Self>,
        queue: EventSourceQueueSnapshot,
    ) -> Option<Arc<dyn EventSourceQueueRegistration>>;
    /// Replaces all queue facts for this source; an empty vector clears retired queues.
    fn set_queue_snapshots(&self, queues: Vec<EventSourceQueueSnapshot>) -> bool;
}

pub trait EventSourceObserver: Send + Sync {
    fn register_source(
        &self,
        descriptor: EventSourceDescriptor,
    ) -> anyhow::Result<Arc<dyn EventSourceRegistration>>;
}

#[cfg(test)]
#[path = "_tests/event_source.rs"]
mod tests;
