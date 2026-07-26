use std::sync::Arc;

use async_trait::async_trait;
use control_plane::ports::{
    AppendTerminalIfMissingAndCloseOutcome, CacheStore, ClaimedTask, DistributedLock, EventBus,
    RateLimitDecision, RateLimitStore, RuntimeEventCloseReason, RuntimeEventEnvelope,
    RuntimeEventPayload, RuntimeEventSource, RuntimeEventStream, RuntimeEventStreamPolicy,
    RuntimeEventSubscription, RuntimeEventTrimPolicy, TaskQueue,
};
use serde_json::json;
use time::{Duration, OffsetDateTime};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Default)]
struct FakeInfrastructure;

#[async_trait]
impl CacheStore for FakeInfrastructure {
    async fn get_json(&self, _key: &str) -> anyhow::Result<Option<serde_json::Value>> {
        Ok(Some(json!({ "ok": true })))
    }

    async fn set_json(
        &self,
        _key: &str,
        _value: serde_json::Value,
        _ttl: Option<Duration>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn set_if_absent_json(
        &self,
        _key: &str,
        _value: serde_json::Value,
        _ttl: Option<Duration>,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn increment_counter(
        &self,
        _key: &str,
        amount: i64,
        _ttl: Option<Duration>,
    ) -> anyhow::Result<i64> {
        Ok(amount)
    }

    async fn delete(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn touch(&self, _key: &str, _ttl: Duration) -> anyhow::Result<bool> {
        Ok(true)
    }
}

#[async_trait]
impl DistributedLock for FakeInfrastructure {
    async fn acquire(&self, _key: &str, _owner: &str, _ttl: Duration) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn renew(&self, _key: &str, _owner: &str, _ttl: Duration) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn release(&self, _key: &str, _owner: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}

#[async_trait]
impl EventBus for FakeInfrastructure {
    async fn publish(&self, _topic: &str, _payload: serde_json::Value) -> anyhow::Result<()> {
        Ok(())
    }

    async fn poll(&self, _topic: &str) -> anyhow::Result<Option<serde_json::Value>> {
        Ok(Some(json!({ "event": true })))
    }
}

#[async_trait]
impl TaskQueue for FakeInfrastructure {
    async fn enqueue(
        &self,
        _queue: &str,
        _payload: serde_json::Value,
        _idempotency_key: Option<&str>,
    ) -> anyhow::Result<String> {
        Ok("task-1".to_string())
    }

    async fn claim(
        &self,
        _queue: &str,
        _worker: &str,
        _visibility_timeout: Duration,
    ) -> anyhow::Result<Option<ClaimedTask>> {
        Ok(Some(ClaimedTask {
            task_id: "task-1".to_string(),
            payload: json!({ "job": true }),
            claimed_by: "worker-1".to_string(),
            idempotency_key: Some("task-key-1".to_string()),
            claim_expires_at_unix: OffsetDateTime::now_utc().unix_timestamp() + 60,
        }))
    }

    async fn ack(&self, _queue: &str, _task_id: &str, _worker: &str) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn fail(
        &self,
        _queue: &str,
        _task_id: &str,
        _worker: &str,
        _reason: &str,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }
}

#[async_trait]
impl RateLimitStore for FakeInfrastructure {
    async fn consume(
        &self,
        _key: &str,
        _limit: u64,
        _window: Duration,
    ) -> anyhow::Result<RateLimitDecision> {
        Ok(RateLimitDecision {
            allowed: true,
            remaining: 9,
            reset_after_ms: 1_000,
        })
    }

    async fn reset(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl RuntimeEventStream for FakeInfrastructure {
    async fn open_run(
        &self,
        _run_id: Uuid,
        _policy: RuntimeEventStreamPolicy,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn append(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<RuntimeEventEnvelope> {
        Ok(RuntimeEventEnvelope::new(run_id, 1, event))
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        _run_id: Uuid,
        _event: RuntimeEventPayload,
    ) -> anyhow::Result<AppendTerminalIfMissingAndCloseOutcome> {
        anyhow::bail!("object-safety fake does not support atomic terminal claims")
    }

    async fn subscribe(
        &self,
        _run_id: Uuid,
        _from_sequence: Option<i64>,
    ) -> anyhow::Result<RuntimeEventSubscription> {
        let (_sender, receiver) = mpsc::unbounded_channel();
        let (_closure_sender, closure) = tokio::sync::watch::channel(None);

        Ok(RuntimeEventSubscription {
            replay: vec![],
            live_events: crate::ports::RuntimeEventReceiver::from_unbounded(receiver),
            closure,
        })
    }

    async fn replay(
        &self,
        _run_id: Uuid,
        _from_sequence: Option<i64>,
        _limit: usize,
    ) -> anyhow::Result<Vec<RuntimeEventEnvelope>> {
        Ok(vec![])
    }

    async fn close_run(
        &self,
        _run_id: Uuid,
        _reason: RuntimeEventCloseReason,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn trim(&self, _run_id: Uuid, _policy: RuntimeEventTrimPolicy) -> anyhow::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn infrastructure_contracts_are_object_safe_and_async() {
    let cache: Arc<dyn CacheStore> = Arc::new(FakeInfrastructure);
    let lock: Arc<dyn DistributedLock> = Arc::new(FakeInfrastructure);
    let events: Arc<dyn EventBus> = Arc::new(FakeInfrastructure);
    let queue: Arc<dyn TaskQueue> = Arc::new(FakeInfrastructure);
    let rate_limit: Arc<dyn RateLimitStore> = Arc::new(FakeInfrastructure);
    let runtime_events: Arc<dyn RuntimeEventStream> = Arc::new(FakeInfrastructure);

    assert_eq!(
        cache.get_json("key").await.unwrap(),
        Some(json!({ "ok": true }))
    );
    assert!(lock
        .acquire("lock", "owner", Duration::seconds(1))
        .await
        .unwrap());
    assert_eq!(
        events.poll("topic").await.unwrap(),
        Some(json!({ "event": true }))
    );
    assert_eq!(
        queue
            .claim("queue", "worker-1", Duration::seconds(1))
            .await
            .unwrap()
            .unwrap()
            .idempotency_key,
        Some("task-key-1".to_string())
    );
    assert!(
        rate_limit
            .consume("key", 10, Duration::seconds(60))
            .await
            .unwrap()
            .allowed
    );

    let run_id = Uuid::now_v7();
    runtime_events
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let envelope = runtime_events
        .append(
            run_id,
            RuntimeEventPayload {
                event_type: "heartbeat".to_string(),
                source: RuntimeEventSource::System,
                durability: control_plane::ports::RuntimeEventDurability::Ephemeral,
                persist_required: false,
                trace_visible: false,
                payload: json!({ "type": "heartbeat" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(envelope.sequence, 1);
}
