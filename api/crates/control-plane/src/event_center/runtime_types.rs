use control_plane_contracts::ports::EventBusLimits;
use domain::event_center::{
    EventSourceContext, EventSourceDescriptor, EventSourceQueueSnapshot, EventSourceScope,
    EventTriggerPhase,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Explicit host policy. No production capacity is selected by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventCenterRuntimePolicy {
    pub bus_limits: EventBusLimits,
    pub observation_max_bytes: u64,
    pub observation_ttl_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRuntimeSubscription {
    pub subscription_id: Uuid,
    pub workspace_id: Uuid,
    pub contract_id: String,
    pub trigger_phase: EventTriggerPhase,
    pub consumer_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventCenterSignal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_context: Option<EventSourceContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workflow_ancestor_ids: Vec<Uuid>,
    pub event_id: Uuid,
    pub occurred_at_unix_ms: i64,
    pub workspace_id: Uuid,
    pub contract_id: String,
    #[serde(flatten)]
    pub body: EventCenterSignalBody,
}

/// Lifecycle signals contain identities only, never a second copy of the business payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum EventCenterSignalBody {
    Published {
        payload: Value,
    },
    Subscribed {
        subscription_id: String,
        consumer_id: String,
    },
    Consumed {
        source_event_id: String,
        subscription_id: String,
        consumer_id: String,
    },
}
impl EventCenterSignal {
    pub fn phase(&self) -> EventTriggerPhase {
        match &self.body {
            EventCenterSignalBody::Published { .. } => EventTriggerPhase::Published,
            EventCenterSignalBody::Subscribed { .. } => EventTriggerPhase::Subscribed,
            EventCenterSignalBody::Consumed { .. } => EventTriggerPhase::Consumed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventTargetAdmissionStatus {
    Accepted,
    Reentrant,
    NoConsumer,
    Closed,
    ResourceExhausted,
    Unconfigured,
    InvalidBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventTargetAdmission {
    pub subscription_id: Uuid,
    pub status: EventTargetAdmissionStatus,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventPublishReceipt {
    pub event_id: Uuid,
    pub workspace_id: Uuid,
    /// One result per logical Published subscription. Empty means zero business targets.
    pub targets: Vec<EventTargetAdmission>,
}

/// Payload-free activity, bounded by the host observation window and serialized byte budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRuntimeActivity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_context: Option<EventSourceContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_subscriber_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_consumer_id: Option<String>,
    pub event_id: Uuid,
    pub workspace_id: Uuid,
    pub contract_id: String,
    pub phase: EventTriggerPhase,
    pub source_event_id: Option<Uuid>,
    pub subscription_id: Option<Uuid>,
    pub consumer_id: Option<Uuid>,
    pub admission: Option<EventTargetAdmissionStatus>,
    pub observed_at_unix_ms: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRuntimeQueueSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_target: Option<EventSourceTarget>,
    pub subscription_id: Uuid,
    pub workspace_id: Uuid,
    pub contract_id: String,
    pub trigger_phase: EventTriggerPhase,
    pub consumer_ids: Vec<Uuid>,
    pub closed: bool,
    pub buffered: u64,
    pub in_flight: u64,
    pub accepted: u64,
    /// Successful queue dequeue; not workflow completion or ACK.
    pub consumed: u64,
    pub admission_rejected: u64,
}
/// Scope-local, finite observation accounting. Missing coverage is not a true zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventObservationLossSnapshot {
    pub accounting_available: bool,
    pub window_started_at_unix_ms: Option<i64>,
    /// Twice the configured activity TTL; no counters are lifetime totals.
    pub window_duration_ms: u64,
    pub expired: u64,
    pub evicted: u64,
    pub oversize_dropped: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRuntimeSnapshot {
    pub instance_id: Uuid,
    pub configured: bool,
    pub workspace_id: Uuid,
    pub queues: Vec<EventRuntimeQueueSnapshot>,
    pub activity: Vec<EventRuntimeActivity>,
    /// Rejections retained within this workspace's bounded observation window.
    pub admission_rejected: u64,
    /// Workspace activity, accounting and live source metadata charge; not process RSS.
    pub observation_bytes: u64,
    /// Included in observation_bytes; only metadata belonging to this projection scope.
    pub source_metadata_bytes: u64,
    pub observation_loss: EventObservationLossSnapshot,
    pub observation_max_bytes: u64,
    pub observation_ttl_ms: u64,
}

/// Explicit source binding on a workflow-owned queue. Native subscriber identity stays separate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSourceTarget {
    pub scope: EventSourceScope,
    pub source_id: String,
    pub workflow_id: Uuid,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRegisteredSourceSnapshot {
    /// False after rejected queue metadata admission/refresh, until an authoritative full
    /// refresh succeeds or this source owner is removed. Does not change trigger capability.
    pub queue_observation_complete: bool,
    pub metadata_bytes: u64,
    pub registration_id: Uuid,
    pub descriptor: EventSourceDescriptor,
    pub queues: Vec<EventSourceQueueSnapshot>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventSourceRuntimeSnapshot {
    pub instance_id: Uuid,
    pub configured: bool,
    pub observation_max_bytes: u64,
    pub observation_ttl_ms: u64,
    pub scope: EventSourceScope,
    pub sources: Vec<EventRegisteredSourceSnapshot>,
    /// Workflow-owned queues; separate from each source owner's original queues.
    pub workflow_queues: Vec<EventRuntimeQueueSnapshot>,
    pub activity: Vec<EventRuntimeActivity>,
    pub observation_bytes: u64,
    /// Included in observation_bytes; only metadata belonging to this projection scope.
    pub source_metadata_bytes: u64,
    pub observation_loss: EventObservationLossSnapshot,
}
