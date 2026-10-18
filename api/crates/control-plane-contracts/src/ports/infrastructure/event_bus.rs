use super::*;

/// Explicit host policy, shared across every topic and clone of one bus.
/// JSON bytes measure serialized logical data, not heap allocations or RSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventBusLimits {
    pub max_items: usize,
    pub max_json_bytes: u64,
    pub max_waiters: usize,
    pub max_topics: usize,
    pub max_consumers: usize,
    /// Total UTF-8 topic-name bytes, including retired generations with live permits or in-flight bodies.
    pub max_topic_name_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventBusError {
    #[error("event bus has no configured budget")]
    Unconfigured,
    #[error("event bus budget is already configured differently")]
    BudgetAlreadyConfigured,
    #[error("event bus resource budget exhausted: {0}")]
    ResourceExhausted(&'static str),
    #[error("event topic has no consumer")]
    NoConsumer,
    #[error("event topic is closed or its generation terminated")]
    Closed,
    #[error("event admission deadline elapsed")]
    DeadlineElapsed,
    #[error("serialized event exceeds its reserved JSON byte bound")]
    BodyExceedsReservation,
    #[error("event JSON serialization failed")]
    InvalidBody,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventBusSnapshot {
    pub limits: Option<EventBusLimits>,
    pub topics: usize,
    pub closed_topics: usize,
    pub retired_topics: usize,
    pub topic_name_bytes: usize,
    pub consumers: usize,
    pub waiters: usize,
    pub reserved: usize,
    pub buffered: usize,
    pub in_flight: usize,
    /// Upper bounds stay charged through reservation, buffering and processing.
    pub charged_json_bytes: u64,
    pub abnormal_terminations: u64,
}

pub trait EventPermit: Send {
    /// Atomic enqueue, in send order. Oversized bodies fail without partial delivery.
    fn send(self: Box<Self>, body: serde_json::Value) -> Result<(), EventBusError>;
}

#[async_trait]
pub trait EventConsumer: Send + Sync {
    /// Competing FIFO dequeue (not broadcast). None means closed and drained.
    /// Pending receives also consume the shared waiter budget and are cancel-safe.
    async fn recv(&self) -> Result<Option<EventDelivery>, EventBusError>;
    /// Stop admission for every consumer of this generation; all may continue draining.
    /// Existing permits become invalid but retain their charge until send/Drop.
    fn close(&self);
}

/// Adapter-owned accounting lease. It must release the in-flight charge on Drop.
pub trait EventDeliveryLease: Send + Sync {}

/// Non-Clone ownership guard: the body remains charged until this guard is dropped.
pub struct EventDelivery {
    body: std::sync::Arc<serde_json::Value>,
    _lease: Box<dyn EventDeliveryLease>,
}

impl EventDelivery {
    pub fn new(
        body: std::sync::Arc<serde_json::Value>,
        lease: Box<dyn EventDeliveryLease>,
    ) -> Self {
        Self {
            body,
            _lease: lease,
        }
    }
    pub fn body(&self) -> &serde_json::Value {
        &self.body
    }
    pub fn complete(self) {}
}

#[async_trait]
pub trait EventBus: Send + Sync {
    /// Configure the single bus-wide budget. Identical repeated configuration is idempotent.
    fn configure_budget(&self, limits: EventBusLimits) -> Result<(), EventBusError>;
    fn open_consumer(&self, topic: &str) -> Result<Box<dyn EventConsumer>, EventBusError>;
    fn event_bus_snapshot(&self) -> EventBusSnapshot;

    /// Admission happens before constructing a body. Dropping this future cancels waiting.
    /// A deadline applies to admission; an issued permit remains valid until close/drop.
    async fn reserve(
        &self,
        topic: &str,
        json_bytes_upper_bound: u64,
        deadline: std::time::Instant,
    ) -> Result<Box<dyn EventPermit>, EventBusError>;

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        Ok(Vec::new())
    }

    async fn summarize_ephemeral_entries(
        &self,
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        Ok(summarize_ephemeral_entries(
            &self.list_ephemeral_entries().await?,
        ))
    }

    async fn list_ephemeral_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        Ok(paginate_ephemeral_tree(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn list_ephemeral_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(paginate_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn search_ephemeral_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(search_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            query,
            request,
        ))
    }

    async fn reveal_ephemeral_entry(
        &self,
        _entry_ref: &str,
        _reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        Ok(None)
    }
}

#[cfg(test)]
#[path = "../../_tests/event_bus_contract_tests.rs"]
mod tests;
