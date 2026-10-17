//! Dedicated storage owner for retained legacy event recovery obligations.
//!
//! Live host publication belongs to the memory EventBus. This adapter preserves
//! historical FIFO snapshots, explicit inspection/take, and permanent credit
//! handoff receipts. A destructive take is not consumer processing acknowledgement.
//! The retained model lifecycle v1 writer and explicit legacy import keep their
//! original protocol; this module does not replay or garbage-collect history.
mod handoff;
mod inspection;

use anyhow::Result;
use control_plane_contracts::ports::{CreditOutboxEvent, EPHEMERAL_PAYLOAD_MAX_BYTES};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

/// PostgreSQL owner of legacy recovery snapshots, receipts, and sequence state.
/// This is not the host's live EventBus provider.
#[derive(Clone)]
pub struct PgLegacyEventRecovery {
    pool: PgPool,
}

impl PgLegacyEventRecovery {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // Every mutation takes this lock FIRST. It is held until commit, so allocation order
    // is publication visibility order, and concurrent polls cannot skip a locked head.
    pub(crate) async fn lock_queue(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        sqlx::query(
            "select sequence from legacy_event_recovery_ledger where sequence=0 for update",
        )
        .fetch_one(&mut **tx)
        .await?;
        Ok(())
    }

    pub(crate) async fn next_sequence(tx: &mut Transaction<'_, Postgres>) -> Result<i64> {
        // Restored rows may have larger identities than a target's current counter.
        // Never allocate an entry_ref already used by pending entries or handoff receipts.
        Ok(sqlx::query_scalar("update legacy_event_recovery_ledger set last_sequence = greatest(last_sequence, (select max(sequence) from legacy_event_recovery_ledger)) + 1 where sequence=0 returning last_sequence")
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

impl PgLegacyEventRecovery {
    pub async fn poll(&self, topic: &str) -> Result<Option<Value>> {
        let mut tx = self.pool.begin().await?;
        Self::lock_queue(&mut tx).await?;
        let row = sqlx::query("select sequence,payload from legacy_event_recovery_ledger where disposition='pending' and topic=$1 order by sequence limit 1")
            .bind(topic.as_bytes()).fetch_optional(&mut *tx).await?;
        let value = match row {
            Some(row) => {
                let payload = Self::payload(&row)?;
                sqlx::query("update legacy_event_recovery_ledger set disposition='taken',payload=null where sequence=$1")
                    .bind(row.try_get::<i64, _>("sequence")?).execute(&mut *tx).await?;
                Some(payload)
            }
            None => None,
        };
        tx.commit().await?;
        Ok(value)
    }

    pub fn ephemeral_inspection_capabilities(
        &self,
    ) -> control_plane_contracts::ports::EphemeralInspectionCapabilities {
        let mut capabilities =
            control_plane_contracts::ports::EphemeralInspectionCapabilities::supported();
        capabilities.storage_medium = Some("postgresql-durable");
        capabilities
    }

    pub async fn list_ephemeral_entries(
        &self,
    ) -> Result<Vec<control_plane_contracts::ports::EphemeralEntrySnapshot>> {
        self.list_metadata().await
    }

    pub async fn reveal_ephemeral_entry(
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
