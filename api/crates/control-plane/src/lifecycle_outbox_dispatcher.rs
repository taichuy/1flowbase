use std::{sync::Arc, time::Duration as StdDuration};

use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::ports::{LifecycleOutboxRecord, LifecycleOutboxRepository};
use extension_contracts::{
    CompletionOutcome, CompletionTerminal, LifecycleContract, LifecycleOperationId,
};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleFactDeliveryCompletion {
    pub event_id: Uuid,
    pub attempt_count: i32,
}

impl LifecycleContract for LifecycleFactDeliveryCompletion {
    const CONTRACT_ID: &'static str = "lifecycle-fact-delivery";
    const CONTRACT_VERSION: &'static str = "1";
}

#[async_trait]
pub trait LifecycleFactDeliveryPort: Send + Sync {
    async fn deliver(&self, fact: &LifecycleOutboxRecord) -> Result<()>;
}

pub trait LifecycleDeliveryCompletionPort: Send + Sync {
    fn complete(&self, outcome: CompletionOutcome<LifecycleFactDeliveryCompletion>);
}

#[derive(Clone)]
pub struct LifecycleOutboxDispatcher<R> {
    repository: R,
    delivery: Arc<dyn LifecycleFactDeliveryPort>,
    completion: Arc<dyn LifecycleDeliveryCompletionPort>,
    worker_id: Uuid,
    claim_limit: u32,
    claim_lease: Duration,
    poll_interval: StdDuration,
}

impl<R> LifecycleOutboxDispatcher<R>
where
    R: LifecycleOutboxRepository + Clone + Send + Sync + 'static,
{
    pub fn new(
        repository: R,
        delivery: Arc<dyn LifecycleFactDeliveryPort>,
        completion: Arc<dyn LifecycleDeliveryCompletionPort>,
    ) -> Self {
        Self {
            repository,
            delivery,
            completion,
            worker_id: Uuid::now_v7(),
            claim_limit: 32,
            claim_lease: Duration::seconds(30),
            poll_interval: StdDuration::from_millis(500),
        }
    }

    pub async fn run_once(&self) -> Result<usize> {
        let facts = self
            .repository
            .claim_lifecycle_facts(self.worker_id, self.claim_limit, self.claim_lease)
            .await?;
        let count = facts.len();
        for fact in facts {
            let delivery = self.delivery.deliver(&fact).await;
            let terminal = if delivery.is_ok() {
                self.repository
                    .mark_lifecycle_fact_delivered(fact.event_id, self.worker_id)
                    .await?;
                CompletionTerminal::Succeeded
            } else {
                let retry_delay = i64::from(fact.attempt_count.clamp(1, 60));
                self.repository
                    .retry_lifecycle_fact(
                        fact.event_id,
                        self.worker_id,
                        OffsetDateTime::now_utc() + Duration::seconds(retry_delay),
                        &delivery
                            .expect_err("failed delivery must contain an error")
                            .to_string(),
                    )
                    .await?;
                CompletionTerminal::Failed
            };
            self.completion.complete(CompletionOutcome::new(
                LifecycleOperationId::new(fact.event_id.to_string())?,
                terminal,
                OffsetDateTime::now_utc().unix_timestamp_nanos() as i64 / 1_000_000,
                LifecycleFactDeliveryCompletion {
                    event_id: fact.event_id,
                    attempt_count: fact.attempt_count,
                },
            ));
        }
        Ok(count)
    }

    pub async fn run(self) {
        loop {
            if let Err(error) = self.run_once().await {
                tracing::error!(%error, worker_id = %self.worker_id, "lifecycle outbox dispatch failed");
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane_contracts::ports::{LifecycleOutboxStatus, RecordLifecycleFactInput};
    use std::sync::Mutex;

    #[derive(Clone)]
    struct MemoryRepository {
        record: LifecycleOutboxRecord,
        completed: Arc<Mutex<Option<LifecycleOutboxStatus>>>,
    }

    #[async_trait]
    impl LifecycleOutboxRepository for MemoryRepository {
        async fn record_lifecycle_fact(
            &self,
            _input: &RecordLifecycleFactInput,
        ) -> Result<LifecycleOutboxRecord> {
            anyhow::bail!("unused")
        }

        async fn claim_lifecycle_facts(
            &self,
            worker_id: Uuid,
            _limit: u32,
            _claim_lease: Duration,
        ) -> Result<Vec<LifecycleOutboxRecord>> {
            let mut record = self.record.clone();
            record.claimed_by = Some(worker_id);
            Ok(vec![record])
        }

        async fn mark_lifecycle_fact_delivered(
            &self,
            _event_id: Uuid,
            _worker_id: Uuid,
        ) -> Result<LifecycleOutboxRecord> {
            *self.completed.lock().unwrap() = Some(LifecycleOutboxStatus::Delivered);
            Ok(self.record.clone())
        }

        async fn retry_lifecycle_fact(
            &self,
            _event_id: Uuid,
            _worker_id: Uuid,
            _available_at: OffsetDateTime,
            _error: &str,
        ) -> Result<LifecycleOutboxRecord> {
            *self.completed.lock().unwrap() = Some(LifecycleOutboxStatus::Pending);
            Ok(self.record.clone())
        }
    }

    struct Delivery(bool);

    #[async_trait]
    impl LifecycleFactDeliveryPort for Delivery {
        async fn deliver(&self, _fact: &LifecycleOutboxRecord) -> Result<()> {
            if self.0 {
                Ok(())
            } else {
                anyhow::bail!("subscriber unavailable")
            }
        }
    }

    #[derive(Default)]
    struct Completion(Arc<Mutex<Vec<CompletionTerminal>>>);

    impl LifecycleDeliveryCompletionPort for Completion {
        fn complete(&self, outcome: CompletionOutcome<LifecycleFactDeliveryCompletion>) {
            self.0.lock().unwrap().push(outcome.terminal());
        }
    }

    fn repository() -> MemoryRepository {
        MemoryRepository {
            record: LifecycleOutboxRecord {
                event_id: Uuid::now_v7(),
                transaction_id: Uuid::now_v7(),
                contract_id: "model_definition.committed".to_string(),
                contract_version: "1".to_string(),
                canonical_payload: b"{}".to_vec(),
                occurred_at: OffsetDateTime::now_utc(),
                status: LifecycleOutboxStatus::Claimed,
                attempt_count: 1,
                available_at: OffsetDateTime::now_utc(),
                claimed_by: None,
                claimed_at: Some(OffsetDateTime::now_utc()),
                delivered_at: None,
            },
            completed: Arc::new(Mutex::new(None)),
        }
    }

    #[tokio::test]
    async fn successful_delivery_is_acked_and_completed() {
        let repository = repository();
        let completion = Arc::new(Completion::default());
        let dispatcher = LifecycleOutboxDispatcher::new(
            repository.clone(),
            Arc::new(Delivery(true)),
            completion.clone(),
        );
        assert_eq!(dispatcher.run_once().await.unwrap(), 1);
        assert_eq!(
            *repository.completed.lock().unwrap(),
            Some(LifecycleOutboxStatus::Delivered)
        );
        assert_eq!(
            completion.0.lock().unwrap().as_slice(),
            &[CompletionTerminal::Succeeded]
        );
    }

    #[tokio::test]
    async fn failed_delivery_is_requeued_and_completed_as_failed() {
        let repository = repository();
        let completion = Arc::new(Completion::default());
        let dispatcher = LifecycleOutboxDispatcher::new(
            repository.clone(),
            Arc::new(Delivery(false)),
            completion.clone(),
        );
        assert_eq!(dispatcher.run_once().await.unwrap(), 1);
        assert_eq!(
            *repository.completed.lock().unwrap(),
            Some(LifecycleOutboxStatus::Pending)
        );
        assert_eq!(
            completion.0.lock().unwrap().as_slice(),
            &[CompletionTerminal::Failed]
        );
    }
}
