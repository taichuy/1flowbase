use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use control_plane::ports::{
    ProviderContinuation, ProviderContinuationSlotId, ProviderTransportPayload,
    ProviderTransportSlotId, ProviderTransportStore,
};
use time::{Duration, OffsetDateTime};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct MemoryProviderTransportStore {
    retention: Duration,
    max_payload_bytes: usize,
    entries: Arc<RwLock<HashMap<ProviderTransportSlotId, TransportEntry>>>,
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

impl MemoryProviderTransportStore {
    pub fn new(retention: Duration, max_payload_bytes: usize) -> Self {
        Self {
            retention,
            max_payload_bytes,
            entries: Arc::new(RwLock::new(HashMap::new())),
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

    async fn delete(&self, slot_id: ProviderTransportSlotId) -> anyhow::Result<bool> {
        Ok(self.entries.write().await.remove(&slot_id).is_some())
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

    async fn delete_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<bool> {
        Ok(self.continuations.write().await.remove(&slot_id).is_some())
    }
}
