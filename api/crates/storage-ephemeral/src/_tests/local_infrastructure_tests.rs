use control_plane::ports::{DistributedLock, EphemeralValueRevealMode, EventBus, TaskQueue};
use serde_json::json;
use storage_ephemeral::{MemoryDistributedLock, MemoryEventBus, MemoryTaskQueue};
use time::Duration;

#[tokio::test]
async fn memory_distributed_lock_checks_owner() {
    let lock = MemoryDistributedLock::new("flowbase:lock");

    assert!(lock
        .acquire("install", "owner-a", Duration::seconds(30))
        .await
        .unwrap());
    assert!(!lock.release("install", "owner-b").await.unwrap());
    assert!(lock.release("install", "owner-a").await.unwrap());
}

#[tokio::test]
async fn memory_distributed_lock_exposes_ephemeral_inspection_snapshots() {
    let lock = MemoryDistributedLock::new("flowbase:lock");

    lock.acquire("workflow:compile", "worker-a", Duration::seconds(30))
        .await
        .unwrap();

    let entries = lock.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].contract_code, "distributed-lock");
    assert_eq!(entries[0].key, "workflow:compile");
    assert_eq!(entries[0].owner.as_deref(), Some("worker-a"));
    assert!(!entries[0].sensitive);

    let revealed = lock
        .reveal_ephemeral_entry("workflow:compile", EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revealed.metadata.key, "workflow:compile");
    assert_eq!(revealed.value.unwrap()["owner"], "worker-a");
}

#[tokio::test]
async fn memory_event_bus_delivers_by_topic_in_fifo_order() {
    let bus = MemoryEventBus::new();

    bus.publish("plugin.install", json!({ "id": 1 }))
        .await
        .unwrap();
    bus.publish("plugin.install", json!({ "id": 2 }))
        .await
        .unwrap();
    bus.publish("runtime.debug", json!({ "id": 3 }))
        .await
        .unwrap();

    assert_eq!(bus.poll("other").await.unwrap(), None);
    assert_eq!(
        bus.poll("plugin.install").await.unwrap(),
        Some(json!({ "id": 1 }))
    );
    assert_eq!(
        bus.poll("plugin.install").await.unwrap(),
        Some(json!({ "id": 2 }))
    );
    assert_eq!(
        bus.poll("runtime.debug").await.unwrap(),
        Some(json!({ "id": 3 }))
    );
}

#[tokio::test]
async fn memory_event_bus_exposes_ephemeral_inspection_snapshots_without_polling() {
    let bus = MemoryEventBus::new();

    bus.publish("plugin.install", json!({ "id": 1 }))
        .await
        .unwrap();

    let entries = bus.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].contract_code, "event-bus");
    assert_eq!(entries[0].group_code.as_deref(), Some("plugin.install"));
    assert_eq!(entries[0].key, "plugin.install:1");
    assert!(entries[0].sensitive);

    let revealed = bus
        .reveal_ephemeral_entry(&entries[0].entry_ref, EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revealed.value.unwrap(), json!({ "id": 1 }));
    assert_eq!(
        bus.poll("plugin.install").await.unwrap(),
        Some(json!({ "id": 1 }))
    );
}

#[tokio::test]
async fn memory_event_bus_rejects_oversized_payloads() {
    let bus = MemoryEventBus::new();

    let result = bus
        .publish("large", json!({ "blob": "x".repeat(2 * 1024 * 1024) }))
        .await;

    assert!(result.is_err());
    assert_eq!(bus.list_ephemeral_entries().await.unwrap(), Vec::new());
}

#[tokio::test]
async fn memory_task_queue_idempotency_claim_ack_and_fail_are_worker_checked() {
    let queue = MemoryTaskQueue::new("flowbase:task");

    let task_id = queue
        .enqueue("preview", json!({ "file": "a" }), Some("preview:file:a"))
        .await
        .unwrap();
    let repeated_task_id = queue
        .enqueue("preview", json!({ "file": "a" }), Some("preview:file:a"))
        .await
        .unwrap();
    assert_eq!(repeated_task_id, task_id);

    let task = queue
        .claim("preview", "worker-a", Duration::seconds(30))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(task.task_id, task_id);
    assert_eq!(task.idempotency_key.as_deref(), Some("preview:file:a"));
    assert!(task.claim_expires_at_unix > time::OffsetDateTime::now_utc().unix_timestamp());
    assert!(!queue.ack("preview", &task_id, "worker-b").await.unwrap());
    assert!(queue
        .fail("preview", &task_id, "worker-a", "retry")
        .await
        .unwrap());

    let reclaimed = queue
        .claim("preview", "worker-b", Duration::seconds(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reclaimed.task_id, task_id);
    assert_eq!(reclaimed.claimed_by, "worker-b");
    assert!(queue.ack("preview", &task_id, "worker-b").await.unwrap());
}

#[tokio::test]
async fn memory_task_queue_exposes_ephemeral_inspection_snapshots() {
    let queue = MemoryTaskQueue::new("flowbase:task");
    let task_id = queue
        .enqueue("preview", json!({ "file": "a" }), Some("preview:file:a"))
        .await
        .unwrap();

    let entries = queue.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].contract_code, "task-queue");
    assert_eq!(entries[0].group_code.as_deref(), Some("preview"));
    assert_eq!(entries[0].key, task_id);
    assert_eq!(entries[0].status, "pending");
    assert!(entries[0].sensitive);

    let revealed = queue
        .reveal_ephemeral_entry(&task_id, EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revealed.metadata.key, task_id);
    assert_eq!(revealed.value.unwrap(), json!({ "file": "a" }));
}

#[tokio::test]
async fn memory_task_queue_rejects_oversized_payloads() {
    let queue = MemoryTaskQueue::new("flowbase:task");

    let result = queue
        .enqueue(
            "preview",
            json!({ "blob": "x".repeat(2 * 1024 * 1024) }),
            None,
        )
        .await;

    assert!(result.is_err());
    assert_eq!(queue.list_ephemeral_entries().await.unwrap(), Vec::new());
}

#[tokio::test]
async fn memory_task_queue_reclaims_after_visibility_timeout() {
    let queue = MemoryTaskQueue::new("flowbase:task");
    let task_id = queue
        .enqueue("preview", json!({ "file": "b" }), Some("preview:file:b"))
        .await
        .unwrap();

    assert!(queue
        .claim("preview", "worker-a", Duration::milliseconds(30))
        .await
        .unwrap()
        .is_some());
    assert!(queue
        .claim("preview", "worker-b", Duration::seconds(30))
        .await
        .unwrap()
        .is_none());

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let reclaimed = queue
        .claim("preview", "worker-b", Duration::seconds(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reclaimed.task_id, task_id);
    assert_eq!(reclaimed.claimed_by, "worker-b");
    assert!(!queue.ack("preview", &task_id, "worker-a").await.unwrap());
    assert!(queue.ack("preview", &task_id, "worker-b").await.unwrap());
}

#[tokio::test]
async fn memory_task_queue_keeps_unconfigured_queues_unbounded() {
    let queue = MemoryTaskQueue::new("flowbase:task");
    for index in 0..3 {
        queue
            .enqueue("workflow", json!({ "index": index }), None)
            .await
            .unwrap();
    }
    assert_eq!(queue.list_ephemeral_entries().await.unwrap().len(), 3);
}

#[tokio::test]
async fn memory_task_queue_applies_capacity_per_queue_name() {
    let queue =
        MemoryTaskQueue::new("flowbase:task").with_queue_capacity("provider-request-logs", 1);
    queue
        .enqueue("provider-request-logs", json!({ "attempt": 1 }), None)
        .await
        .unwrap();
    let error = queue
        .enqueue("provider-request-logs", json!({ "attempt": 2 }), None)
        .await
        .unwrap_err();
    queue
        .enqueue("workflow", json!({ "run": 1 }), None)
        .await
        .unwrap();
    queue
        .enqueue("workflow", json!({ "run": 2 }), None)
        .await
        .unwrap();
    assert!(error
        .to_string()
        .contains("task queue `provider-request-logs` reached capacity 1"));
    assert_eq!(queue.list_ephemeral_entries().await.unwrap().len(), 3);
}

#[tokio::test]
async fn memory_task_queue_counts_claimed_tasks_toward_capacity() {
    let queue = MemoryTaskQueue::new("flowbase:task").with_queue_capacity("logs", 1);
    queue
        .enqueue("logs", json!({ "attempt": 1 }), None)
        .await
        .unwrap();
    queue
        .claim("logs", "worker-a", Duration::seconds(30))
        .await
        .unwrap()
        .unwrap();
    let error = queue
        .enqueue("logs", json!({ "attempt": 2 }), None)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("task queue `logs` reached capacity 1"));
}

#[tokio::test]
async fn memory_task_queue_ack_releases_queue_capacity() {
    let queue = MemoryTaskQueue::new("flowbase:task").with_queue_capacity("logs", 1);
    let task_id = queue
        .enqueue("logs", json!({ "attempt": 1 }), None)
        .await
        .unwrap();
    queue
        .claim("logs", "worker-a", Duration::seconds(30))
        .await
        .unwrap()
        .unwrap();
    assert!(queue.ack("logs", &task_id, "worker-a").await.unwrap());
    queue
        .enqueue("logs", json!({ "attempt": 2 }), None)
        .await
        .unwrap();
}
