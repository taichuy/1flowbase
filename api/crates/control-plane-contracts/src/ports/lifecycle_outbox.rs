use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleOutboxStatus {
    Pending,
    Claimed,
    Delivered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordLifecycleFactInput {
    pub event_id: Uuid,
    pub transaction_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
    pub canonical_payload: Vec<u8>,
    pub occurred_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleOutboxRecord {
    pub event_id: Uuid,
    pub transaction_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
    pub canonical_payload: Vec<u8>,
    pub occurred_at: OffsetDateTime,
    pub status: LifecycleOutboxStatus,
    pub attempt_count: i32,
    pub available_at: OffsetDateTime,
    pub claimed_by: Option<Uuid>,
    pub claimed_at: Option<OffsetDateTime>,
    pub delivered_at: Option<OffsetDateTime>,
}

#[async_trait]
pub trait LifecycleOutboxRepository: Send + Sync {
    async fn record_lifecycle_fact(
        &self,
        input: &RecordLifecycleFactInput,
    ) -> anyhow::Result<LifecycleOutboxRecord>;

    async fn claim_lifecycle_facts(
        &self,
        worker_id: Uuid,
        limit: u32,
        claim_lease: time::Duration,
    ) -> anyhow::Result<Vec<LifecycleOutboxRecord>>;

    async fn mark_lifecycle_fact_delivered(
        &self,
        event_id: Uuid,
        worker_id: Uuid,
    ) -> anyhow::Result<LifecycleOutboxRecord>;

    async fn retry_lifecycle_fact(
        &self,
        event_id: Uuid,
        worker_id: Uuid,
        available_at: OffsetDateTime,
        error: &str,
    ) -> anyhow::Result<LifecycleOutboxRecord>;
}
