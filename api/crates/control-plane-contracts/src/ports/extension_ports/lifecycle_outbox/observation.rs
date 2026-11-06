//! Trusted publication metadata. Historical rows without this context remain unavailable.
use super::{LifecycleOutboxRecord, RecordLifecycleFactInput};
use domain::event_center::EventSourceScope;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleObservationContext {
    pub scope: EventSourceScope,
    pub producer_id: String,
    pub workflow_ancestor_ids: Vec<Uuid>,
}

tokio::task_local! {
    static WORKFLOW_ANCESTORS: Vec<Uuid>;
}

/// Host-only execution boundary. Never populate this from an event's user payload.
pub async fn with_lifecycle_workflow_ancestors<F: std::future::Future>(
    ancestors: Vec<Uuid>,
    future: F,
) -> F::Output {
    WORKFLOW_ANCESTORS.scope(ancestors, future).await
}

pub fn lifecycle_workflow_ancestor_ids() -> Vec<Uuid> {
    WORKFLOW_ANCESTORS
        .try_with(Clone::clone)
        .unwrap_or_default()
}

impl LifecycleObservationContext {
    pub fn trusted(scope: EventSourceScope, producer_id: impl Into<String>) -> Self {
        Self {
            scope,
            producer_id: producer_id.into(),
            workflow_ancestor_ids: lifecycle_workflow_ancestor_ids(),
        }
    }
}

/// Called at existing transaction/queue owners. Implementations must be best effort.
pub trait LifecycleFactObserver: Send + Sync {
    fn published(&self, input: &RecordLifecycleFactInput);
    fn consumed(&self, record: &LifecycleOutboxRecord, consumer_id: &str);
}

#[derive(Debug, Clone)]
pub struct LifecycleSourceQueueFacts {
    pub scope: EventSourceScope,
    pub contract_id: String,
    pub contract_version: String,
    pub queues: Vec<domain::event_center::EventSourceQueueSnapshot>,
}

#[cfg(test)]
#[path = "_tests/observation.rs"]
mod tests;
