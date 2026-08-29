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
    delivery_deadline: StdDuration,
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
            delivery_deadline: StdDuration::from_secs(10),
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
            let (terminal, retry_error) =
                match tokio::time::timeout(self.delivery_deadline, self.delivery.deliver(&fact))
                    .await
                {
                    Ok(Ok(())) => (CompletionTerminal::Succeeded, None),
                    Ok(Err(error)) => (CompletionTerminal::Failed, Some(error.to_string())),
                    Err(_) => (
                        CompletionTerminal::TimedOut,
                        Some(format!(
                            "lifecycle subscriber delivery exceeded {}ms deadline",
                            self.delivery_deadline.as_millis()
                        )),
                    ),
                };
            if let Some(error) = retry_error {
                let retry_delay = i64::from(fact.attempt_count.clamp(1, 60));
                self.repository
                    .retry_lifecycle_fact(
                        fact.event_id,
                        &fact.subscriber_id,
                        self.worker_id,
                        OffsetDateTime::now_utc() + Duration::seconds(retry_delay),
                        &error,
                    )
                    .await?;
            } else {
                self.repository
                    .mark_lifecycle_fact_delivered(
                        fact.event_id,
                        &fact.subscriber_id,
                        self.worker_id,
                    )
                    .await?;
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
            _subscriber_id: &str,
            _worker_id: Uuid,
        ) -> Result<LifecycleOutboxRecord> {
            *self.completed.lock().unwrap() = Some(LifecycleOutboxStatus::Delivered);
            Ok(self.record.clone())
        }

        async fn retry_lifecycle_fact(
            &self,
            _event_id: Uuid,
            _subscriber_id: &str,
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
                graph_fingerprint: "graph-v1".to_string(),
                subscriber_id: "subscriber-a".to_string(),
                handler_id: "handler-a".to_string(),
                handler_version: "v1".to_string(),
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

    #[derive(Clone)]
    struct BatchRepository {
        records: Vec<LifecycleOutboxRecord>,
        delivered: Arc<Mutex<Vec<String>>>,
        retried: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl LifecycleOutboxRepository for BatchRepository {
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
            Ok(self
                .records
                .iter()
                .cloned()
                .map(|mut record| {
                    record.claimed_by = Some(worker_id);
                    record
                })
                .collect())
        }

        async fn mark_lifecycle_fact_delivered(
            &self,
            _event_id: Uuid,
            subscriber_id: &str,
            _worker_id: Uuid,
        ) -> Result<LifecycleOutboxRecord> {
            self.delivered
                .lock()
                .unwrap()
                .push(subscriber_id.to_string());
            Ok(self.records[0].clone())
        }

        async fn retry_lifecycle_fact(
            &self,
            _event_id: Uuid,
            subscriber_id: &str,
            _worker_id: Uuid,
            _available_at: OffsetDateTime,
            _error: &str,
        ) -> Result<LifecycleOutboxRecord> {
            self.retried.lock().unwrap().push(subscriber_id.to_string());
            Ok(self.records[0].clone())
        }
    }

    struct HangingFirstDelivery;

    #[async_trait]
    impl LifecycleFactDeliveryPort for HangingFirstDelivery {
        async fn deliver(&self, fact: &LifecycleOutboxRecord) -> Result<()> {
            if fact.subscriber_id == "subscriber-hung" {
                std::future::pending::<()>().await;
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn hung_subscriber_times_out_and_does_not_block_later_delivery() {
        let mut hung = repository().record;
        hung.subscriber_id = "subscriber-hung".to_string();
        let mut healthy = repository().record;
        healthy.subscriber_id = "subscriber-healthy".to_string();
        let repository = BatchRepository {
            records: vec![hung, healthy],
            delivered: Arc::new(Mutex::new(Vec::new())),
            retried: Arc::new(Mutex::new(Vec::new())),
        };
        let completion = Arc::new(Completion::default());
        let mut dispatcher = LifecycleOutboxDispatcher::new(
            repository.clone(),
            Arc::new(HangingFirstDelivery),
            completion.clone(),
        );
        dispatcher.delivery_deadline = StdDuration::from_millis(10);

        assert_eq!(dispatcher.run_once().await.unwrap(), 2);
        assert_eq!(
            repository.retried.lock().unwrap().as_slice(),
            ["subscriber-hung"]
        );
        assert_eq!(
            repository.delivered.lock().unwrap().as_slice(),
            ["subscriber-healthy"]
        );
        assert_eq!(
            completion.0.lock().unwrap().as_slice(),
            &[CompletionTerminal::TimedOut, CompletionTerminal::Succeeded]
        );
    }
}
