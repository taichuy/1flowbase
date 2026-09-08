use super::*;
use control_plane_contracts::ports::RecordLifecycleFactInput;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
#[derive(Clone)]
struct Repository {
    pending: Arc<Mutex<VecDeque<LifecycleOutboxRecord>>>,
    delivered: Arc<AtomicUsize>,
    claims: Arc<AtomicUsize>,
}
#[async_trait]
impl LifecycleOutboxRepository for Repository {
    async fn record_lifecycle_fact(
        &self,
        _: &RecordLifecycleFactInput,
    ) -> Result<LifecycleOutboxRecord> {
        anyhow::bail!("unused")
    }
    async fn claim_lifecycle_facts(
        &self,
        worker: Uuid,
        limit: u32,
        lease: Duration,
    ) -> Result<Vec<LifecycleOutboxRecord>> {
        assert_eq!(limit, 32);
        assert_eq!(lease, Duration::seconds(30));
        self.claims.fetch_add(1, Ordering::SeqCst);
        let mut pending = self.pending.lock().unwrap();
        Ok((0..limit)
            .filter_map(|_| pending.pop_front())
            .map(|mut record| {
                record.claimed_by = Some(worker);
                record
            })
            .collect())
    }
    async fn mark_lifecycle_fact_delivered(
        &self,
        _: Uuid,
        _: &str,
        _: Uuid,
        _: Uuid,
    ) -> Result<LifecycleOutboxRecord> {
        self.delivered.fetch_add(1, Ordering::SeqCst);
        Ok(super::tests::repository().record)
    }
    async fn retry_lifecycle_fact(
        &self,
        _: Uuid,
        _: &str,
        _: Uuid,
        _: Uuid,
        _: OffsetDateTime,
        _: &str,
    ) -> Result<LifecycleOutboxRecord> {
        anyhow::bail!("unexpected retry")
    }
    async fn pause_lifecycle_fact(
        &self,
        _: Uuid,
        _: &str,
        _: Uuid,
        _: Uuid,
        _: LifecycleDeliveryPauseReason,
    ) -> Result<LifecycleOutboxRecord> {
        anyhow::bail!("unexpected pause")
    }
}
struct BarrierDelivery {
    started: tokio::sync::mpsc::Sender<()>,
    release: Arc<tokio::sync::Semaphore>,
}
#[async_trait]
impl LifecycleFactDeliveryPort for BarrierDelivery {
    async fn deliver(&self, _: &LifecycleOutboxRecord) -> Result<()> {
        self.started.send(()).await.unwrap();
        self.release.acquire().await.unwrap().forget();
        Ok(())
    }
}
struct Completion;
impl LifecycleDeliveryCompletionPort for Completion {
    fn complete(&self, _: CompletionOutcome<LifecycleFactDeliveryCompletion>) {}
}
#[tokio::test]
async fn root_2007_ac_010_lane_budgets_dispatcher_shutdown() {
    let pending = (0..33)
        .map(|i| {
            let mut record = super::tests::repository().record;
            record.subscriber_id = format!("subscriber-{i}");
            record
        })
        .collect();
    let repository = Repository {
        pending: Arc::new(Mutex::new(pending)),
        delivered: Arc::new(AtomicUsize::new(0)),
        claims: Arc::new(AtomicUsize::new(0)),
    };
    let (started, mut entered) = tokio::sync::mpsc::channel(32);
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let dispatcher = LifecycleOutboxDispatcher::new(
        repository.clone(),
        Arc::new(BarrierDelivery {
            started,
            release: release.clone(),
        }),
        Arc::new(Completion),
    );
    assert_eq!(dispatcher.delivery_deadline, StdDuration::from_secs(10));
    let other = dispatcher.clone();
    let worker = dispatcher.spawn();
    tokio::time::timeout(StdDuration::from_secs(2), async {
        for _ in 0..32 {
            entered.recv().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(
        other.run_once().await.is_err(),
        "one batch owns the fixed 32 delivery budget"
    );
    worker.close();
    assert!(worker
        .wait(StdDuration::ZERO)
        .await
        .unwrap_err()
        .to_string()
        .contains("unfinished"));
    assert_eq!(repository.pending.lock().unwrap().len(), 1);
    assert_eq!(repository.delivered.load(Ordering::SeqCst), 0);
    release.add_permits(32);
    worker.wait(StdDuration::from_secs(2)).await.unwrap();
    assert_eq!(repository.claims.load(Ordering::SeqCst), 1);
    assert_eq!(repository.delivered.load(Ordering::SeqCst), 32);
    assert_eq!(
        repository.pending.lock().unwrap().len(),
        1,
        "shutdown never clears durable backlog"
    );
    assert_eq!(other.run_once().await.unwrap(), 0);
    assert_eq!(repository.claims.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn root_2007_ac_010_lane_budgets_retry_capacity() {
    struct Capacity(std::sync::atomic::AtomicBool);
    #[async_trait]
    impl LifecycleFactDeliveryPort for Capacity {
        async fn deliver(&self, _: &LifecycleOutboxRecord) -> Result<()> {
            if self.0.load(Ordering::SeqCst) {
                anyhow::bail!("finite managed admission capacity exhausted");
            }
            Ok(())
        }
    }
    for attempt in [1, 4, 5] {
        let mut repository = super::tests::repository();
        repository.record.attempt_count = attempt;
        let delivery = Arc::new(Capacity(std::sync::atomic::AtomicBool::new(true)));
        let dispatcher = LifecycleOutboxDispatcher::new(
            repository.clone(),
            delivery.clone(),
            Arc::new(Completion),
        );
        dispatcher.run_once().await.unwrap();
        assert_eq!(
            *repository.completed.lock().unwrap(),
            Some(if attempt < 5 {
                control_plane_contracts::ports::LifecycleOutboxStatus::Pending
            } else {
                control_plane_contracts::ports::LifecycleOutboxStatus::Paused
            })
        );
        if attempt < 5 {
            delivery.0.store(false, Ordering::SeqCst);
            dispatcher.run_once().await.unwrap();
            assert_eq!(
                *repository.completed.lock().unwrap(),
                Some(control_plane_contracts::ports::LifecycleOutboxStatus::Delivered)
            );
        }
    }
}
