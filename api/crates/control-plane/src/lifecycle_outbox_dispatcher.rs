use std::{sync::Arc, time::Duration as StdDuration};

use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::ports::{
    LifecycleClaimLost, LifecycleDeliveryBlocked, LifecycleDeliveryPauseReason,
    LifecycleOutboxRecord, LifecycleOutboxRepository,
};
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
    pub claim_id: Uuid,
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
        // Start the bounded claimed batch together; no queued claim expires behind slow siblings.
        let mut attempts = tokio::task::JoinSet::new();
        for fact in facts {
            let dispatcher = self.clone();
            attempts.spawn(async move { dispatcher.dispatch_claim(fact).await });
        }
        let mut failure = None;
        while let Some(result) = attempts.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    failure.get_or_insert(error);
                }
                Err(error) => {
                    failure.get_or_insert(anyhow::Error::from(error));
                }
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(count)
    }

    async fn dispatch_claim(&self, fact: LifecycleOutboxRecord) -> Result<()> {
        let claim_id = fact.claim_id.ok_or(LifecycleClaimLost)?;
        let remaining =
            fact.claim_expires_at.ok_or(LifecycleClaimLost)? - OffsetDateTime::now_utc();
        let remaining = StdDuration::try_from(remaining).map_err(|_| LifecycleClaimLost)?;
        if remaining.is_zero() {
            return Ok(());
        }
        let (terminal, error) = match tokio::time::timeout(
            self.delivery_deadline.min(remaining),
            self.delivery.deliver(&fact),
        )
        .await
        {
            Ok(Ok(())) => (CompletionTerminal::Succeeded, None),
            Ok(Err(error)) => (CompletionTerminal::Failed, Some(error)),
            Err(_) => (
                CompletionTerminal::TimedOut,
                Some(anyhow::anyhow!(
                    "lifecycle subscriber delivery deadline elapsed"
                )),
            ),
        };
        let update = if let Some(error) = error {
            let blocked = error
                .downcast_ref::<LifecycleDeliveryBlocked>()
                .map(|e| e.0)
                .or_else(|| {
                    (fact.attempt_count >= 5)
                        .then_some(LifecycleDeliveryPauseReason::RetryBudgetExhausted)
                });
            if let Some(reason) = blocked {
                self.repository
                    .pause_lifecycle_fact(
                        fact.event_id,
                        &fact.subscriber_id,
                        self.worker_id,
                        claim_id,
                        reason,
                    )
                    .await
            } else {
                // Persist bounded diagnostic text; a large worker error must not strand a claim.
                let message = error.to_string();
                let message = if message.is_empty() {
                    "lifecycle subscriber failed".to_string()
                } else {
                    message
                };
                let mut end = message.len().min(4096);
                while !message.is_char_boundary(end) {
                    end -= 1;
                }
                self.repository
                    .retry_lifecycle_fact(
                        fact.event_id,
                        &fact.subscriber_id,
                        self.worker_id,
                        claim_id,
                        OffsetDateTime::now_utc()
                            + Duration::seconds(i64::from(fact.attempt_count.clamp(1, 60))),
                        &message[..end],
                    )
                    .await
            }
        } else {
            self.repository
                .mark_lifecycle_fact_delivered(
                    fact.event_id,
                    &fact.subscriber_id,
                    self.worker_id,
                    claim_id,
                )
                .await
        };
        if let Err(error) = update {
            if error.downcast_ref::<LifecycleClaimLost>().is_none() {
                return Err(error);
            }
            // Completion describes this attempt only; losing a fence never mutates the new claim.
            tracing::info!(event_id = %fact.event_id, %claim_id, "lifecycle attempt lost its claim fence");
        }
        self.completion.complete(CompletionOutcome::new(
            LifecycleOperationId::new(format!("{}:{claim_id}", fact.event_id))?,
            terminal,
            (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64,
            LifecycleFactDeliveryCompletion {
                event_id: fact.event_id,
                attempt_count: fact.attempt_count,
                claim_id,
            },
        ));
        Ok(())
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
        async fn pause_lifecycle_fact(
            &self,
            _event_id: Uuid,
            _subscriber_id: &str,
            _worker_id: Uuid,
            _claim_id: Uuid,
            reason: LifecycleDeliveryPauseReason,
        ) -> Result<LifecycleOutboxRecord> {
            *self.completed.lock().unwrap() = Some(LifecycleOutboxStatus::Paused);
            let mut record = self.record.clone();
            record.status = LifecycleOutboxStatus::Paused;
            record.pause_reason = Some(reason);
            Ok(record)
        }

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
            _claim_id: Uuid,
        ) -> Result<LifecycleOutboxRecord> {
            *self.completed.lock().unwrap() = Some(LifecycleOutboxStatus::Delivered);
            Ok(self.record.clone())
        }

        async fn retry_lifecycle_fact(
            &self,
            _event_id: Uuid,
            _subscriber_id: &str,
            _worker_id: Uuid,
            _claim_id: Uuid,
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
                claim_id: Some(Uuid::now_v7()),
                claim_expires_at: Some(OffsetDateTime::now_utc() + Duration::seconds(30)),
                pause_reason: None,
                paused_at: None,
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
        async fn pause_lifecycle_fact(
            &self,
            _event_id: Uuid,
            _subscriber_id: &str,
            _worker_id: Uuid,
            _claim_id: Uuid,
            _reason: LifecycleDeliveryPauseReason,
        ) -> Result<LifecycleOutboxRecord> {
            anyhow::bail!("pause unused in this fixture")
        }

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
            _claim_id: Uuid,
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
            _claim_id: Uuid,
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
        let completions = completion.0.lock().unwrap();
        assert_eq!(completions.len(), 2);
        assert!(completions.contains(&CompletionTerminal::TimedOut));
        assert!(completions.contains(&CompletionTerminal::Succeeded));
    }
    struct BlockedDelivery;
    #[async_trait]
    impl LifecycleFactDeliveryPort for BlockedDelivery {
        async fn deliver(&self, _fact: &LifecycleOutboxRecord) -> Result<()> {
            Err(
                LifecycleDeliveryBlocked(LifecycleDeliveryPauseReason::FrozenGraphUnavailable)
                    .into(),
            )
        }
    }
    #[tokio::test]
    async fn root_2007_ac_007_dispatcher_pauses_unavailable_and_exhausted_attempts() {
        for unavailable in [true, false] {
            let mut repository = repository();
            repository.record.attempt_count = if unavailable { 1 } else { 5 };
            let delivery: Arc<dyn LifecycleFactDeliveryPort> = if unavailable {
                Arc::new(BlockedDelivery)
            } else {
                Arc::new(Delivery(false))
            };
            let dispatcher = LifecycleOutboxDispatcher::new(
                repository.clone(),
                delivery,
                Arc::new(Completion::default()),
            );
            assert_eq!(dispatcher.run_once().await.unwrap(), 1);
            assert_eq!(
                *repository.completed.lock().unwrap(),
                Some(LifecycleOutboxStatus::Paused)
            );
        }
    }
}
