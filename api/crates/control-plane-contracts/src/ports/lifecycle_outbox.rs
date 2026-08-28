use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleSubscriberTarget {
    pub subscriber_id: String,
    pub handler_id: String,
    pub handler_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecyclePublicationPlan {
    pub graph_fingerprint: String,
    pub subscribers: Vec<LifecycleSubscriberTarget>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LifecyclePublicationCatalog {
    plans: BTreeMap<(String, String), LifecyclePublicationPlan>,
}

impl LifecyclePublicationCatalog {
    pub fn new(
        plans: impl IntoIterator<Item = ((String, String), LifecyclePublicationPlan)>,
    ) -> anyhow::Result<Self> {
        let mut indexed = BTreeMap::new();
        for (contract, plan) in plans {
            if indexed.insert(contract.clone(), plan).is_some() {
                anyhow::bail!(
                    "duplicate lifecycle publication plan for {}@{}",
                    contract.0,
                    contract.1
                );
            }
        }
        Ok(Self { plans: indexed })
    }

    pub fn plan_for(
        &self,
        contract_id: &str,
        contract_version: &str,
    ) -> Option<&LifecyclePublicationPlan> {
        self.plans
            .get(&(contract_id.to_string(), contract_version.to_string()))
    }
}

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
    pub publication: LifecyclePublicationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleOutboxRecord {
    pub event_id: Uuid,
    pub transaction_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
    pub canonical_payload: Vec<u8>,
    pub occurred_at: OffsetDateTime,
    pub graph_fingerprint: String,
    pub subscriber_id: String,
    pub handler_id: String,
    pub handler_version: String,
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
        subscriber_id: &str,
        worker_id: Uuid,
    ) -> anyhow::Result<LifecycleOutboxRecord>;

    async fn retry_lifecycle_fact(
        &self,
        event_id: Uuid,
        subscriber_id: &str,
        worker_id: Uuid,
        available_at: OffsetDateTime,
        error: &str,
    ) -> anyhow::Result<LifecycleOutboxRecord>;
}
