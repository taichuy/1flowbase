use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use async_trait::async_trait;
use control_plane::ports::{
    ensure_ephemeral_payload_size, ephemeral_metadata_size_bytes, ClaimedTask,
    EphemeralEntrySnapshot, EphemeralEntryValueSnapshot, EphemeralInspectionCapabilities,
    EphemeralValueRevealMode, TaskQueue,
};
use time::OffsetDateTime;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub struct MemoryTaskQueue {
    namespace: String,
    queue_capacities: HashMap<String, usize>,
    inner: Arc<Mutex<TaskQueueState>>,
}

#[derive(Default)]
struct TaskQueueState {
    queues: HashMap<String, VecDeque<String>>,
    tasks: HashMap<String, TaskEntry>,
    idempotency_index: HashMap<(String, String), String>,
}

#[derive(Clone)]
struct TaskEntry {
    queue: String,
    task_id: String,
    payload: serde_json::Value,
    idempotency_key: Option<String>,
    claimed_by: Option<String>,
    claim_expires_at: Option<OffsetDateTime>,
}

impl MemoryTaskQueue {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            queue_capacities: HashMap::new(),
            inner: Arc::new(Mutex::new(TaskQueueState::default())),
        }
    }

    pub fn with_queue_capacity(mut self, queue: impl Into<String>, capacity: usize) -> Self {
        self.queue_capacities.insert(queue.into(), capacity);
        self
    }

    fn queue_key(&self, queue: &str) -> String {
        format!("{}:{}", self.namespace, queue)
    }

    fn queue_without_namespace(&self, queue: &str) -> String {
        queue
            .strip_prefix(&format!("{}:", self.namespace))
            .unwrap_or(queue)
            .to_string()
    }

    fn claimed_task(entry: &TaskEntry) -> Option<ClaimedTask> {
        Some(ClaimedTask {
            task_id: entry.task_id.clone(),
            payload: entry.payload.clone(),
            claimed_by: entry.claimed_by.clone()?,
            idempotency_key: entry.idempotency_key.clone(),
            claim_expires_at_unix: entry.claim_expires_at?.unix_timestamp(),
        })
    }

    fn value_size_bytes(entry: &TaskEntry) -> u64 {
        serde_json::to_vec(&entry.payload)
            .map(|bytes| bytes.len() as u64)
            .unwrap_or(0)
    }

    fn task_status(entry: &TaskEntry, now: OffsetDateTime) -> String {
        match (&entry.claimed_by, entry.claim_expires_at) {
            (Some(_), Some(expires_at)) if expires_at > now => "claimed".to_string(),
            (Some(_), Some(_)) => "claim_expired".to_string(),
            _ => "pending".to_string(),
        }
    }

    fn entry_snapshot(&self, entry: &TaskEntry) -> EphemeralEntrySnapshot {
        let now = OffsetDateTime::now_utc();
        let queue = self.queue_without_namespace(&entry.queue);
        let status = Self::task_status(entry, now);
        let metadata = serde_json::json!({
            "queue": queue.clone(),
            "idempotency_key": entry.idempotency_key,
            "claimed_by": entry.claimed_by,
            "claim_expires_at_unix": entry.claim_expires_at.map(|value| value.unix_timestamp()),
        });
        EphemeralEntrySnapshot {
            contract_code: "task-queue".to_string(),
            group_code: Some(queue.clone()),
            entry_ref: entry.task_id.clone(),
            key: entry.task_id.clone(),
            inspection_path: vec![queue, status.clone(), entry.task_id.clone()],
            entry_kind: "task".to_string(),
            status,
            owner: entry.claimed_by.clone(),
            value_size_bytes: Self::value_size_bytes(entry),
            metadata_size_bytes: ephemeral_metadata_size_bytes(&metadata),
            ttl_seconds: entry
                .claim_expires_at
                .map(|expires_at| (expires_at - now).whole_seconds().max(0)),
            created_at_unix: None,
            expires_at_unix: entry
                .claim_expires_at
                .map(|expires_at| expires_at.unix_timestamp()),
            sensitive: true,
            metadata,
        }
    }
}

#[async_trait]
impl TaskQueue for MemoryTaskQueue {
    async fn enqueue(
        &self,
        queue: &str,
        payload: serde_json::Value,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<String> {
        ensure_ephemeral_payload_size(&payload)?;
        let queue_key = self.queue_key(queue);
        let idempotency_key = idempotency_key
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let mut state = self.inner.lock().await;

        if let Some(idempotency_key) = &idempotency_key {
            let index_key = (queue_key.clone(), idempotency_key.clone());
            if let Some(task_id) = state.idempotency_index.get(&index_key).cloned() {
                if state.tasks.contains_key(&task_id) {
                    return Ok(task_id);
                }
                state.idempotency_index.remove(&index_key);
            }
        }

        if let Some(capacity) = self.queue_capacities.get(queue) {
            let task_count = state.queues.get(&queue_key).map_or(0, VecDeque::len);
            if task_count >= *capacity {
                anyhow::bail!("task queue `{queue}` reached capacity {capacity}");
            }
        }

        let task_id = Uuid::now_v7().to_string();
        state
            .queues
            .entry(queue_key.clone())
            .or_default()
            .push_back(task_id.clone());
        if let Some(idempotency_key) = &idempotency_key {
            state.idempotency_index.insert(
                (queue_key.clone(), idempotency_key.clone()),
                task_id.clone(),
            );
        }
        state.tasks.insert(
            task_id.clone(),
            TaskEntry {
                queue: queue_key,
                task_id: task_id.clone(),
                payload,
                idempotency_key,
                claimed_by: None,
                claim_expires_at: None,
            },
        );

        Ok(task_id)
    }

    async fn claim(
        &self,
        queue: &str,
        worker: &str,
        visibility_timeout: time::Duration,
    ) -> anyhow::Result<Option<ClaimedTask>> {
        let queue_key = self.queue_key(queue);
        let mut state = self.inner.lock().await;
        let Some(task_ids) = state.queues.get(&queue_key).cloned() else {
            return Ok(None);
        };
        let now = OffsetDateTime::now_utc();

        for task_id in task_ids {
            let Some(entry) = state.tasks.get_mut(&task_id) else {
                continue;
            };
            let claim_is_active = entry
                .claim_expires_at
                .is_some_and(|deadline| deadline > now);
            if entry.claimed_by.is_some() && claim_is_active {
                continue;
            }

            entry.claimed_by = Some(worker.to_string());
            entry.claim_expires_at = Some(now + visibility_timeout);
            return Ok(Self::claimed_task(entry));
        }

        Ok(None)
    }

    async fn ack(&self, queue: &str, task_id: &str, worker: &str) -> anyhow::Result<bool> {
        let queue_key = self.queue_key(queue);
        let mut state = self.inner.lock().await;
        let now = OffsetDateTime::now_utc();
        let Some(entry) = state.tasks.get(task_id) else {
            return Ok(false);
        };

        if entry.queue != queue_key
            || entry.claimed_by.as_deref() != Some(worker)
            || entry
                .claim_expires_at
                .is_none_or(|deadline| deadline <= now)
        {
            return Ok(false);
        }

        let entry = state
            .tasks
            .remove(task_id)
            .expect("task existence checked before removal");
        if let Some(task_ids) = state.queues.get_mut(&entry.queue) {
            task_ids.retain(|queued_task_id| queued_task_id != task_id);
        }
        if let Some(idempotency_key) = entry.idempotency_key {
            state
                .idempotency_index
                .remove(&(entry.queue, idempotency_key));
        }

        Ok(true)
    }

    async fn fail(
        &self,
        queue: &str,
        task_id: &str,
        worker: &str,
        _reason: &str,
    ) -> anyhow::Result<bool> {
        let queue_key = self.queue_key(queue);
        let mut state = self.inner.lock().await;
        let Some(entry) = state.tasks.get_mut(task_id) else {
            return Ok(false);
        };
        if entry.queue != queue_key || entry.claimed_by.as_deref() != Some(worker) {
            return Ok(false);
        }

        entry.claimed_by = None;
        entry.claim_expires_at = None;
        Ok(true)
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::supported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        let state = self.inner.lock().await;
        let mut entries = state
            .tasks
            .values()
            .map(|entry| self.entry_snapshot(entry))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.group_code
                .cmp(&right.group_code)
                .then(left.key.cmp(&right.key))
        });
        Ok(entries)
    }

    async fn reveal_ephemeral_entry(
        &self,
        entry_ref: &str,
        reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        let state = self.inner.lock().await;
        let Some(entry) = state.tasks.get(entry_ref) else {
            return Ok(None);
        };
        Ok(Some(EphemeralEntryValueSnapshot::from_value(
            self.entry_snapshot(entry),
            entry.payload.clone(),
            reveal_mode,
        )))
    }
}
