//! Legacy durable EventBus recovery adapter. Not the default live publication path.
mod handoff;
mod inspection;

use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::ports::{CreditOutboxEvent, EPHEMERAL_PAYLOAD_MAX_BYTES, EventBus};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgEventBus {
    pool: PgPool,
}

impl PgEventBus {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // Every mutation takes this lock FIRST. It is held until commit, so allocation order
    // is publication visibility order, and concurrent polls cannot skip a locked head.
    pub(crate) async fn lock_queue(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        sqlx::query("select singleton from host_event_bus_counter where singleton for update")
            .fetch_one(&mut **tx)
            .await?;
        Ok(())
    }

    pub(crate) async fn next_sequence(tx: &mut Transaction<'_, Postgres>) -> Result<i64> {
        // Restored rows may have larger identities than a target's current counter.
        // Never allocate an entry_ref already used by pending entries or handoff receipts.
        Ok(sqlx::query_scalar("update host_event_bus_counter set last_sequence = greatest(last_sequence, (select coalesce(max(sequence),0) from host_event_bus_pending), (select coalesce(max(sequence),0) from credit_event_handoffs)) + 1 where singleton returning last_sequence")
            .fetch_one(&mut **tx).await?)
    }

    fn credit_envelope(row: &sqlx::postgres::PgRow) -> Result<Value> {
        let event = CreditOutboxEvent {
            event_id: row.try_get("event_id")?,
            workspace_id: row.try_get("workspace_id")?,
            account_id: row.try_get("account_id")?,
            event_type: row.try_get("event_type")?,
            payload: row.try_get("payload")?,
            created_at: row.try_get::<OffsetDateTime, _>("created_at")?,
            delivery_attempts: row.try_get("delivery_attempts")?,
        };
        // Preserve the previous publisher's Rust serde representation (including time),
        // rather than PostgreSQL's different timestamptz-to-JSON representation.
        Ok(serde_json::json!({
            "event_id": event.event_id, "event_type": event.event_type,
            "workspace_id": event.workspace_id, "account_id": event.account_id,
            "occurred_at": event.created_at, "payload": event.payload,
        }))
    }

    // Consume one Value in a synchronous frame. One serialization serves both byte count
    // and durable storage, preserving the existing payload-size contract and error code.
    fn serialize_payload(payload: Value) -> Result<String> {
        let serialized = serde_json::to_string(&payload)?;
        anyhow::ensure!(
            serialized.len() <= EPHEMERAL_PAYLOAD_MAX_BYTES,
            "ephemeral_payload_too_large"
        );
        Ok(serialized)
    }

    fn payload(row: &sqlx::postgres::PgRow) -> Result<Value> {
        Ok(serde_json::from_str(&row.try_get::<String, _>("payload")?)?)
    }
}

#[async_trait]
impl EventBus for PgEventBus {
    async fn publish(&self, topic: &str, payload: Value) -> Result<()> {
        let payload = Self::serialize_payload(payload)?;
        let bytes = payload.len() as i64;
        let mut tx = self.pool.begin().await?;
        Self::lock_queue(&mut tx).await?;
        let sequence = Self::next_sequence(&mut tx).await?;
        sqlx::query("insert into host_event_bus_pending(sequence,topic,payload,value_size_bytes) values ($1,$2,$3,$4)")
            .bind(sequence).bind(topic.as_bytes()).bind(payload).bind(bytes).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn poll(&self, topic: &str) -> Result<Option<Value>> {
        let mut tx = self.pool.begin().await?;
        Self::lock_queue(&mut tx).await?;
        let row = sqlx::query("delete from host_event_bus_pending where sequence=(select sequence from host_event_bus_pending where topic=$1 order by sequence limit 1) returning payload")
            .bind(topic.as_bytes()).fetch_optional(&mut *tx).await?;
        let value = match row {
            Some(row) => Some(Self::payload(&row)?),
            None => None,
        };
        tx.commit().await?;
        Ok(value)
    }

    fn ephemeral_inspection_capabilities(
        &self,
    ) -> control_plane_contracts::ports::EphemeralInspectionCapabilities {
        let mut capabilities =
            control_plane_contracts::ports::EphemeralInspectionCapabilities::supported();
        capabilities.storage_medium = Some("postgresql-durable");
        capabilities
    }

    async fn list_ephemeral_entries(
        &self,
    ) -> Result<Vec<control_plane_contracts::ports::EphemeralEntrySnapshot>> {
        self.list_metadata().await
    }

    async fn reveal_ephemeral_entry(
        &self,
        entry_ref: &str,
        mode: control_plane_contracts::ports::EphemeralValueRevealMode,
    ) -> Result<Option<control_plane_contracts::ports::EphemeralEntryValueSnapshot>> {
        self.reveal(entry_ref, mode).await
    }
}

#[cfg(test)]
#[path = "../_tests/event_bus/mod.rs"]
mod tests;
