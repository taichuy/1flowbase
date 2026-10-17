use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use async_trait::async_trait;
use control_plane_contracts::ports::{
    EphemeralEntrySnapshot, EphemeralEntryValueSnapshot, EphemeralInspectionCapabilities,
    EphemeralValueRevealMode, EventBus, ensure_ephemeral_payload_size,
    ephemeral_metadata_size_bytes,
};
use tokio::sync::Mutex;

#[derive(Clone, Default)]
pub struct MemoryEventBus {
    inner: Arc<Mutex<EventBusState>>,
    payload_admission: PayloadAdmission,
}

#[derive(Clone, Copy, Default)]
enum PayloadAdmission {
    #[default]
    Bounded,
    LosslessEventContract,
}

#[derive(Default)]
struct EventBusState {
    topics: HashMap<String, VecDeque<EventBusEntry>>,
    next_sequence: u64,
}

#[derive(Clone)]
struct EventBusEntry {
    sequence: u64,
    payload: serde_json::Value,
}

impl MemoryEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Preserve an EventBus contract which accepts complete JSON without an
    /// additional business-size limit. Inspection retains its own reveal budget.
    pub fn lossless() -> Self {
        Self {
            payload_admission: PayloadAdmission::LosslessEventContract,
            ..Self::default()
        }
    }

    fn entry_key(topic: &str, sequence: u64) -> String {
        format!("{topic}:{sequence}")
    }

    fn value_size_bytes(value: &serde_json::Value) -> u64 {
        serde_json::to_vec(value)
            .map(|bytes| bytes.len() as u64)
            .unwrap_or(0)
    }

    fn entry_snapshot(topic: &str, entry: &EventBusEntry) -> EphemeralEntrySnapshot {
        let entry_ref = entry.sequence.to_string();
        let metadata = serde_json::json!({
            "topic": topic,
            "sequence": entry.sequence,
        });
        EphemeralEntrySnapshot {
            contract_code: "event-bus".to_string(),
            group_code: Some(topic.to_string()),
            entry_ref: entry_ref.clone(),
            key: Self::entry_key(topic, entry.sequence),
            inspection_path: vec![topic.to_string(), entry_ref],
            entry_kind: "event".to_string(),
            status: "buffered".to_string(),
            owner: None,
            value_size_bytes: Self::value_size_bytes(&entry.payload),
            metadata_size_bytes: ephemeral_metadata_size_bytes(&metadata),
            ttl_seconds: None,
            created_at_unix: None,
            expires_at_unix: None,
            sensitive: true,
            metadata,
        }
    }
}

#[async_trait]
impl EventBus for MemoryEventBus {
    async fn publish(&self, topic: &str, payload: serde_json::Value) -> anyhow::Result<()> {
        if matches!(self.payload_admission, PayloadAdmission::Bounded) {
            ensure_ephemeral_payload_size(&payload)?;
        }
        let mut state = self.inner.lock().await;
        state.next_sequence += 1;
        let sequence = state.next_sequence;
        state
            .topics
            .entry(topic.to_string())
            .or_default()
            .push_back(EventBusEntry { sequence, payload });
        Ok(())
    }

    async fn poll(&self, topic: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let mut state = self.inner.lock().await;
        let Some(queue) = state.topics.get_mut(topic) else {
            return Ok(None);
        };
        let entry = queue.pop_front();
        if queue.is_empty() {
            // Remove under the same lock as pop: a concurrent publish must not be lost.
            state.topics.remove(topic);
            if state.topics.is_empty() {
                // Release the topic index high-water allocation as well as the deque.
                state.topics = HashMap::new();
            }
        }
        Ok(entry.map(|entry| entry.payload))
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        let mut capabilities = EphemeralInspectionCapabilities::supported();
        if matches!(
            self.payload_admission,
            PayloadAdmission::LosslessEventContract
        ) {
            capabilities.max_payload_size_bytes = u64::MAX;
        }
        capabilities
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        let state = self.inner.lock().await;
        let mut entries = state
            .topics
            .iter()
            .flat_map(|(topic, values)| {
                values
                    .iter()
                    .map(|entry| Self::entry_snapshot(topic, entry))
                    .collect::<Vec<_>>()
            })
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
        let Some(sequence) = entry_ref.parse::<u64>().ok() else {
            return Ok(None);
        };
        let state = self.inner.lock().await;
        let Some((topic, entry)) = state.topics.iter().find_map(|(topic, entries)| {
            entries
                .iter()
                .find(|entry| entry.sequence == sequence)
                .map(|entry| (topic.clone(), entry.clone()))
        }) else {
            return Ok(None);
        };
        Ok(Some(EphemeralEntryValueSnapshot::from_value(
            Self::entry_snapshot(&topic, &entry),
            entry.payload,
            reveal_mode,
        )))
    }
}

#[cfg(test)]
#[path = "../_tests/memory_event_bus_tests.rs"]
mod tests;
