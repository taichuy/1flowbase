use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use control_plane::ports::{
    ProviderContinuation, ProviderContinuationSlotId, ProviderProtocolContextSlotId,
    ProviderProtocolContextValue, ProviderTransportPayload, ProviderTransportSlotId,
    ProviderTransportStore,
};
use time::{Duration, OffsetDateTime};
use tokio::sync::RwLock;

const MAX_PROTOCOL_CONTEXT_SLOTS_PER_FLOW_RUN: usize = 16;

#[derive(Clone)]
pub struct MemoryProviderTransportStore {
    retention: Duration,
    max_payload_bytes: usize,
    entries: Arc<RwLock<HashMap<ProviderTransportSlotId, TransportEntry>>>,
    protocol_contexts: Arc<RwLock<HashMap<ProviderProtocolContextSlotId, ProtocolContextEntry>>>,
    continuations: Arc<RwLock<HashMap<ProviderContinuationSlotId, ContinuationEntry>>>,
}

#[derive(Clone)]
struct TransportEntry {
    payload: ProviderTransportPayload,
    expires_at: OffsetDateTime,
}

#[derive(Clone)]
struct ContinuationEntry {
    continuation: ProviderContinuation,
    expires_at: OffsetDateTime,
}

#[derive(Clone)]
struct ProtocolContextEntry {
    value: ProviderProtocolContextValue,
    expires_at: OffsetDateTime,
}

impl MemoryProviderTransportStore {
    pub fn new(retention: Duration, max_payload_bytes: usize) -> Self {
        Self {
            retention,
            max_payload_bytes,
            entries: Arc::new(RwLock::new(HashMap::new())),
            protocol_contexts: Arc::new(RwLock::new(HashMap::new())),
            continuations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn validate_policy(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.retention > Duration::ZERO && self.max_payload_bytes > 0,
            "provider_transport_policy_invalid"
        );
        Ok(())
    }

    fn take_unexpired<K, V>(
        entries: &mut HashMap<K, V>,
        key: &K,
        expires_at: impl FnOnce(&V) -> OffsetDateTime,
    ) -> Option<V>
    where
        K: Eq + std::hash::Hash,
    {
        let entry = entries.remove(key)?;
        (expires_at(&entry) > OffsetDateTime::now_utc()).then_some(entry)
    }
}

#[async_trait]
impl ProviderTransportStore for MemoryProviderTransportStore {
    async fn put(
        &self,
        slot_id: ProviderTransportSlotId,
        payload: ProviderTransportPayload,
    ) -> anyhow::Result<()> {
        self.validate_policy()?;
        anyhow::ensure!(
            payload.size_bytes() <= self.max_payload_bytes,
            "provider_transport_payload_too_large"
        );
        self.entries.write().await.insert(
            slot_id,
            TransportEntry {
                payload,
                expires_at: OffsetDateTime::now_utc() + self.retention,
            },
        );
        Ok(())
    }

    async fn get(
        &self,
        slot_id: ProviderTransportSlotId,
    ) -> anyhow::Result<Option<ProviderTransportPayload>> {
        let mut entries = self.entries.write().await;
        let Some(entry) = entries.get(&slot_id).cloned() else {
            return Ok(None);
        };
        if entry.expires_at <= OffsetDateTime::now_utc() {
            entries.remove(&slot_id);
            return Ok(None);
        }
        Ok(Some(entry.payload))
    }

    async fn consume(
        &self,
        slot_id: ProviderTransportSlotId,
    ) -> anyhow::Result<ProviderTransportPayload> {
        let mut entries = self.entries.write().await;
        let entry = Self::take_unexpired(&mut entries, &slot_id, |entry| entry.expires_at)
            .ok_or_else(|| anyhow::anyhow!("ephemeral_transport_missing"))?;
        Ok(entry.payload)
    }

    async fn delete(&self, slot_id: ProviderTransportSlotId) -> anyhow::Result<bool> {
        Ok(self.entries.write().await.remove(&slot_id).is_some())
    }

    async fn put_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
        value: ProviderProtocolContextValue,
    ) -> anyhow::Result<()> {
        self.validate_policy()?;
        anyhow::ensure!(
            value.size_bytes() <= self.max_payload_bytes,
            "ephemeral_protocol_context_too_large"
        );
        let mut contexts = self.protocol_contexts.write().await;
        let flow_run_id = slot_id.flow_run_id();
        let owned_slot_count = contexts
            .keys()
            .filter(|stored_slot| stored_slot.belongs_to(flow_run_id))
            .count();
        anyhow::ensure!(
            contexts.contains_key(&slot_id)
                || owned_slot_count < MAX_PROTOCOL_CONTEXT_SLOTS_PER_FLOW_RUN,
            "ephemeral_protocol_context_slot_limit_exceeded"
        );
        contexts.insert(
            slot_id,
            ProtocolContextEntry {
                value,
                expires_at: OffsetDateTime::now_utc() + self.retention,
            },
        );
        Ok(())
    }

    async fn get_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
    ) -> anyhow::Result<Option<ProviderProtocolContextValue>> {
        let mut contexts = self.protocol_contexts.write().await;
        let Some(entry) = contexts.get(&slot_id).cloned() else {
            return Ok(None);
        };
        if entry.expires_at <= OffsetDateTime::now_utc() {
            contexts.remove(&slot_id);
            return Ok(None);
        }
        Ok(Some(entry.value))
    }

    async fn delete_flow_run_protocol_contexts(
        &self,
        flow_run_id: uuid::Uuid,
    ) -> anyhow::Result<usize> {
        let mut contexts = self.protocol_contexts.write().await;
        let count = contexts.len();
        contexts.retain(|slot_id, _| !slot_id.belongs_to(flow_run_id));
        Ok(count - contexts.len())
    }

    async fn put_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
        continuation: ProviderContinuation,
    ) -> anyhow::Result<()> {
        self.validate_policy()?;
        self.continuations.write().await.insert(
            slot_id,
            ContinuationEntry {
                continuation,
                expires_at: OffsetDateTime::now_utc() + self.retention,
            },
        );
        Ok(())
    }

    async fn get_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<Option<ProviderContinuation>> {
        let mut continuations = self.continuations.write().await;
        let Some(entry) = continuations.get(&slot_id).cloned() else {
            return Ok(None);
        };
        if entry.expires_at <= OffsetDateTime::now_utc() {
            continuations.remove(&slot_id);
            return Ok(None);
        }
        Ok(Some(entry.continuation))
    }

    async fn consume_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<ProviderContinuation> {
        let mut continuations = self.continuations.write().await;
        let entry = Self::take_unexpired(&mut continuations, &slot_id, |entry| entry.expires_at)
            .ok_or_else(|| anyhow::anyhow!("ephemeral_continuation_missing"))?;
        Ok(entry.continuation)
    }

    async fn delete_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<bool> {
        Ok(self.continuations.write().await.remove(&slot_id).is_some())
    }

    async fn clear_expired(&self) -> anyhow::Result<usize> {
        let now = OffsetDateTime::now_utc();
        let mut entries = self.entries.write().await;
        let request_count = entries.len();
        entries.retain(|_, entry| entry.expires_at > now);
        let removed_requests = request_count - entries.len();
        drop(entries);

        let mut contexts = self.protocol_contexts.write().await;
        let context_count = contexts.len();
        contexts.retain(|_, entry| entry.expires_at > now);
        let removed_contexts = context_count - contexts.len();
        drop(contexts);

        let mut continuations = self.continuations.write().await;
        let continuation_count = continuations.len();
        continuations.retain(|_, entry| entry.expires_at > now);
        Ok(removed_requests + removed_contexts + continuation_count - continuations.len())
    }
}
