use super::{EventDefinitionOrigin, EventTriggerPhase};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Bound only by trusted source owners. Unbound declarations cannot deliver workflow triggers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventSourceScope {
    Workspace { workspace_id: Uuid },
    System,
    Unbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSourceBinding {
    Declaration,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSourceCoverage {
    Live,
    Unavailable,
}

/// Source-owned registration, independent of custom event definitions in durable storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventSourceDescriptor {
    pub source_id: String,
    pub owner_id: String,
    pub point_id: String,
    pub contract_id: String,
    pub contract_version: String,
    pub name: String,
    pub description: String,
    /// Actual source schema; null means unknown, not an empty object schema.
    pub payload_schema: Value,
    pub origin: EventDefinitionOrigin,
    pub scope: EventSourceScope,
    pub binding: EventSourceBinding,
    pub coverage: EventSourceCoverage,
    /// Only phases actually emitted by this adapter.
    pub supported_phases: Vec<EventTriggerPhase>,
}

/// Original queue owner facts, never inferred from workflow completion or observer dequeue.
/// None means unavailable. All values are absolute snapshots, not increments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSourceQueueSnapshot {
    pub queue_id: String,
    pub subscriber_id: String,
    pub consumer_ids: Vec<String>,
    pub buffered: Option<u64>,
    pub in_flight: Option<u64>,
    pub accepted: Option<u64>,
    pub dequeued: Option<u64>,
    pub dropped: Option<u64>,
    pub observed_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSourceContext {
    pub registration_id: Uuid,
    pub scope: EventSourceScope,
    pub source_id: String,
    pub point_id: String,
    /// Native event identities are not necessarily UUIDs. Subscribed may have no event yet.
    pub original_event_id: Option<String>,
    pub producer_id: Option<String>,
}
