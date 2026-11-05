use control_plane_contracts::ports::EventBusLimits;
use domain::event_center::EventTriggerPhase;
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
    pub contract_version: String,
    pub trigger_phase: EventTriggerPhase,
    pub consumer_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventCenterSignal {
    pub event_id: Uuid,
    pub occurred_at_unix_ms: i64,
    pub workspace_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
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
        subscription_id: Uuid,
        consumer_id: Uuid,
    },
    Consumed {
        source_event_id: Uuid,
        subscription_id: Uuid,
        consumer_id: Uuid,
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
    pub event_id: Uuid,
    pub workspace_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
    pub phase: EventTriggerPhase,
    pub source_event_id: Option<Uuid>,
    pub subscription_id: Option<Uuid>,
    pub consumer_id: Option<Uuid>,
    pub admission: Option<EventTargetAdmissionStatus>,
    pub observed_at_unix_ms: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRuntimeQueueSnapshot {
    pub subscription_id: Uuid,
    pub workspace_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRuntimeSnapshot {
    pub instance_id: Uuid,
    pub configured: bool,
    pub workspace_id: Uuid,
    pub queues: Vec<EventRuntimeQueueSnapshot>,
    pub activity: Vec<EventRuntimeActivity>,
    /// Rejections retained within this workspace's bounded observation window.
    pub admission_rejected: u64,
    pub observation_bytes: u64,
    pub observation_max_bytes: u64,
    pub observation_ttl_ms: u64,
}
