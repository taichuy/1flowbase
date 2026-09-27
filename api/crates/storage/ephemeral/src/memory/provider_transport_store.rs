use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use control_plane_contracts::ports::{
    ProviderContinuation, ProviderContinuationSlotId, ProviderProtocolContextSlotId,
    ProviderProtocolContextValue, ProviderTransportPayload, ProviderTransportSlotId,
    ProviderTransportStore,
};
use time::{Duration, OffsetDateTime};
use tokio::sync::{watch, RwLock};

const MAX_PROTOCOL_CONTEXT_SLOTS_PER_FLOW_RUN: usize = 16;

#[derive(Clone)]
pub struct MemoryProviderTransportStore {
    retention: Duration,
    max_payload_bytes: usize,
    entries: Arc<RwLock<HashMap<ProviderTransportSlotId, TransportEntry>>>,
    protocol_contexts: Arc<RwLock<HashMap<ProviderProtocolContextSlotId, ProtocolContextEntry>>>,
    continuations: Arc<RwLock<HashMap<ProviderContinuationSlotId, ContinuationEntry>>>,
    maintenance_started: Arc<AtomicBool>,
    maintenance_revision: watch::Sender<u64>,
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
        let (maintenance_revision, _) = watch::channel(0);
        Self {
            retention,
            max_payload_bytes,
            entries: Arc::new(RwLock::new(HashMap::new())),
            protocol_contexts: Arc::new(RwLock::new(HashMap::new())),
            continuations: Arc::new(RwLock::new(HashMap::new())),
            maintenance_started: Arc::new(AtomicBool::new(false)),
            maintenance_revision,
        }
    }

    fn ensure_expiry_maintenance(&self) {
        self.maintenance_revision
            .send_modify(|revision| *revision = revision.wrapping_add(1));
        if self
            .maintenance_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let mut changes = self.maintenance_revision.subscribe();
        let entries_weak = Arc::downgrade(&self.entries);
        let contexts_weak = Arc::downgrade(&self.protocol_contexts);
        let continuations_weak = Arc::downgrade(&self.continuations);
        tokio::spawn(async move {
            loop {
                let (Some(entries), Some(protocol_contexts), Some(continuations)) = (
                    entries_weak.upgrade(),
                    contexts_weak.upgrade(),
                    continuations_weak.upgrade(),
                ) else {
                    break;
                };
                let next_expiry =
                    Self::next_expiry(&entries, &protocol_contexts, &continuations).await;
                drop((entries, protocol_contexts, continuations));
                match next_expiry {
                    None => {
                        if changes.changed().await.is_err() {
                            break;
                        }
                    }
                    Some(expiry) => {
                        let remaining = expiry - OffsetDateTime::now_utc();
                        if remaining <= Duration::ZERO {
                            let (Some(entries), Some(protocol_contexts), Some(continuations)) = (
                                entries_weak.upgrade(),
                                contexts_weak.upgrade(),
                                continuations_weak.upgrade(),
                            ) else {
                                break;
                            };
                            Self::clear_expired_maps(&entries, &protocol_contexts, &continuations)
                                .await;
                        } else {
                            let delay = std::time::Duration::try_from(remaining)
                                .expect("positive expiry duration must convert");
                            tokio::select! {
                                _ = tokio::time::sleep(delay) => {}
                                result = changes.changed() => {
                                    if result.is_err() { break; }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    async fn next_expiry(
        entries: &RwLock<HashMap<ProviderTransportSlotId, TransportEntry>>,
        protocol_contexts: &RwLock<HashMap<ProviderProtocolContextSlotId, ProtocolContextEntry>>,
        continuations: &RwLock<HashMap<ProviderContinuationSlotId, ContinuationEntry>>,
    ) -> Option<OffsetDateTime> {
        let requests = entries
            .read()
            .await
            .values()
            .map(|entry| entry.expires_at)
            .min();
        let contexts = protocol_contexts
            .read()
            .await
            .values()
            .map(|entry| entry.expires_at)
            .min();
        let continuation = continuations
            .read()
            .await
            .values()
            .map(|entry| entry.expires_at)
            .min();
        requests
            .into_iter()
            .chain(contexts)
            .chain(continuation)
            .min()
    }

    async fn clear_expired_maps(
        entries: &RwLock<HashMap<ProviderTransportSlotId, TransportEntry>>,
        protocol_contexts: &RwLock<HashMap<ProviderProtocolContextSlotId, ProtocolContextEntry>>,
        continuations: &RwLock<HashMap<ProviderContinuationSlotId, ContinuationEntry>>,
    ) -> usize {
        let now = OffsetDateTime::now_utc();
        let mut entries = entries.write().await;
        let request_count = entries.len();
        entries.retain(|_, entry| entry.expires_at > now);
        let removed_requests = request_count - entries.len();
        drop(entries);

        let mut contexts = protocol_contexts.write().await;
        let context_count = contexts.len();
        contexts.retain(|_, entry| entry.expires_at > now);
        let removed_contexts = context_count - contexts.len();
        drop(contexts);

        let mut continuations = continuations.write().await;
        let continuation_count = continuations.len();
        continuations.retain(|_, entry| entry.expires_at > now);
        removed_requests + removed_contexts + continuation_count - continuations.len()
    }

    #[cfg(test)]
    pub(crate) async fn retained_counts_for_test(&self) -> (usize, usize, usize) {
        (
            self.entries.read().await.len(),
            self.protocol_contexts.read().await.len(),
            self.continuations.read().await.len(),
        )
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
        self.ensure_expiry_maintenance();
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
        drop(contexts);
        self.ensure_expiry_maintenance();
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
        self.ensure_expiry_maintenance();
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

    async fn claim_continuation(
        &self,
        flow_run_id: uuid::Uuid,
        resume_claim_id: uuid::Uuid,
    ) -> anyhow::Result<ProviderContinuation> {
        let current = ProviderContinuationSlotId::for_flow_run(flow_run_id);
        let claimed = ProviderContinuationSlotId::for_resume_claim(flow_run_id, resume_claim_id);
        let mut continuations = self.continuations.write().await;
        if let Some(entry) = continuations.get(&claimed).cloned() {
            if entry.expires_at > OffsetDateTime::now_utc() {
                return Ok(entry.continuation);
            }
            continuations.remove(&claimed);
        }
        let entry = Self::take_unexpired(&mut continuations, &current, |entry| entry.expires_at)
            .ok_or_else(|| anyhow::anyhow!("ephemeral_continuation_missing"))?;
        let continuation = entry.continuation.clone();
        continuations.insert(claimed, entry);
        Ok(continuation)
    }

    async fn delete_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<bool> {
        Ok(self.continuations.write().await.remove(&slot_id).is_some())
    }

    async fn clear_flow_run(&self, flow_run_id: uuid::Uuid) -> anyhow::Result<()> {
        self.delete(ProviderTransportSlotId::for_flow_run(flow_run_id))
            .await?;
        self.continuations
            .write()
            .await
            .retain(|slot, _| !slot.belongs_to(flow_run_id));
        self.delete_flow_run_protocol_contexts(flow_run_id).await?;
        Ok(())
    }

    async fn clear_expired(&self) -> anyhow::Result<usize> {
        Ok(
            Self::clear_expired_maps(&self.entries, &self.protocol_contexts, &self.continuations)
                .await,
        )
    }
}
