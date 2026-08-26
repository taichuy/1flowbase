use async_trait::async_trait;
use control_plane_contracts::ports::{
    DistributedLock, EphemeralEntrySnapshot, EphemeralEntryValueSnapshot,
    EphemeralInspectionCapabilities, EphemeralValueRevealMode,
};

use crate::{LeaseStore, MemoryLeaseStore};

#[derive(Clone)]
pub struct MemoryDistributedLock {
    leases: MemoryLeaseStore,
}

impl MemoryDistributedLock {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            leases: MemoryLeaseStore::new(namespace),
        }
    }
}

#[async_trait]
impl DistributedLock for MemoryDistributedLock {
    async fn acquire(&self, key: &str, owner: &str, ttl: time::Duration) -> anyhow::Result<bool> {
        if ttl <= time::Duration::ZERO {
            return Ok(false);
        }

        self.leases.acquire(key, owner, ttl).await
    }

    async fn renew(&self, key: &str, owner: &str, ttl: time::Duration) -> anyhow::Result<bool> {
        if ttl <= time::Duration::ZERO {
            return Ok(false);
        }

        self.leases.renew(key, owner, ttl).await
    }

    async fn release(&self, key: &str, owner: &str) -> anyhow::Result<bool> {
        self.leases.release(key, owner).await
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::supported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        Ok(self.leases.list_ephemeral_entries_for_inspection().await)
    }

    async fn reveal_ephemeral_entry(
        &self,
        entry_ref: &str,
        reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        Ok(self
            .leases
            .reveal_ephemeral_entry_for_inspection(entry_ref, reveal_mode)
            .await)
    }
}
