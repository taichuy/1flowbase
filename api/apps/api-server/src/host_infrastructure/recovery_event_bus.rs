//! Live short-lived events stay in the existing ephemeral owner. Pre-cutover durable
//! snapshots remain visible and recoverable, never cleared or acknowledged at boot.
use async_trait::async_trait;
use control_plane_contracts::ports::{
    EphemeralEntrySnapshot, EphemeralEntryValueSnapshot, EphemeralInspectionCapabilities,
    EphemeralValueRevealMode, EventBus, ephemeral_metadata_size_bytes,
};
use serde_json::Value;
use storage_durable_postgres::PgEventBus;
use storage_ephemeral::MemoryEventBus;

pub(super) struct RecoveryEventBus {
    live: MemoryEventBus,
    legacy: PgEventBus,
}
impl RecoveryEventBus {
    pub(super) fn new(pool: sqlx::PgPool) -> Self {
        Self {
            live: MemoryEventBus::lossless(),
            legacy: PgEventBus::new(pool),
        }
    }
    fn legacy_metadata(mut entry: EphemeralEntrySnapshot) -> EphemeralEntrySnapshot {
        entry.entry_ref = format!("legacy:{}", entry.entry_ref);
        entry.key = format!("legacy:{}", entry.key);
        entry.inspection_path.insert(0, "legacy-recovery".into());
        entry.owner = Some("legacy-event-recovery".into());
        entry.status = "recovery_pending".into();
        entry.metadata["lifecycle"] = serde_json::json!("legacy-recovery-only");
        entry.metadata_size_bytes = ephemeral_metadata_size_bytes(&entry.metadata);
        entry
    }
}
#[async_trait]
impl EventBus for RecoveryEventBus {
    async fn publish(&self, topic: &str, payload: Value) -> anyhow::Result<()> {
        self.live.publish(topic, payload).await
    }
    async fn poll(&self, topic: &str) -> anyhow::Result<Option<Value>> {
        // Preserve earlier durable pending/FIFO before newly published local messages.
        // This is the original explicit take contract, not an automatic consumer/GC.
        if let Some(payload) = self.legacy.poll(topic).await? {
            return Ok(Some(payload));
        }
        self.live.poll(topic).await
    }
    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        let mut capabilities = EphemeralInspectionCapabilities::supported();
        capabilities.storage_medium = Some("memory-and-legacy-postgresql");
        capabilities.max_payload_size_bytes = u64::MAX;
        capabilities
    }
    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        let mut entries = self.live.list_ephemeral_entries().await?;
        entries.extend(
            self.legacy
                .list_ephemeral_entries()
                .await?
                .into_iter()
                .map(Self::legacy_metadata),
        );
        Ok(entries)
    }
    async fn reveal_ephemeral_entry(
        &self,
        entry_ref: &str,
        mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        if let Some(legacy_ref) = entry_ref.strip_prefix("legacy:") {
            let mut snapshot = self.legacy.reveal_ephemeral_entry(legacy_ref, mode).await?;
            if let Some(value) = snapshot.as_mut() {
                value.metadata = Self::legacy_metadata(value.metadata.clone());
            }
            return Ok(snapshot);
        }
        self.live.reveal_ephemeral_entry(entry_ref, mode).await
    }
}
