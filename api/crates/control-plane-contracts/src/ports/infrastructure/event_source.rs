//! Synchronous, best-effort observation of facts owned by existing sources.
use domain::event_center::{EventSourceDescriptor, EventSourceQueueSnapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventSourceObservation {
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
