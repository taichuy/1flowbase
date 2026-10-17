use super::*;
use anyhow::ensure;

impl PgLegacyEventRecovery {
    /// Root-only migration primitive: exact observed IDs, never a historical scan.
    /// Permanent receipts prevent re-enqueue even after a destructive poll.
    pub async fn import_legacy_credit_events(&self, event_ids: &[Uuid]) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        Self::lock_queue(&mut tx).await?;
        // Caller-observed FIFO is authoritative. Deduplicate without sorting its IDs.
        let mut seen = std::collections::HashSet::new();
        let ids: Vec<Uuid> = event_ids
            .iter()
            .copied()
            .filter(|id| seen.insert(*id))
            .collect();
        let mut pending_ids = Vec::new();
        for id in ids {
            let received: bool = sqlx::query_scalar(
                "select exists(select 1 from legacy_event_recovery_ledger where credit_event_id=$1)",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if received {
                // Receipt remains authoritative even after its source is legitimately deleted.
                continue;
            }
            let source: Option<Uuid> = sqlx::query_scalar(
                "select event_id from credit_event_outbox where event_id=$1 for update",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            ensure!(
                source.is_some(),
                "legacy import contains a missing credit source"
            );
            pending_ids.push(id);
        }
        // Validate the complete batch before the first new receipt/snapshot is written.
        let mut inserted = 0;
        for id in pending_ids {
            inserted += usize::from(Self::handoff_one(&mut tx, id, "legacy_snapshot").await?);
        }
        tx.commit().await?;
        Ok(inserted)
    }

    pub(super) async fn handoff_one(
        tx: &mut Transaction<'_, Postgres>,
        event_id: Uuid,
        origin: &str,
    ) -> Result<bool> {
        let received: bool = sqlx::query_scalar(
            "select exists(select 1 from legacy_event_recovery_ledger where credit_event_id=$1)",
        )
        .bind(event_id)
        .fetch_one(&mut **tx)
        .await?;
        if received {
            sqlx::query("update credit_event_outbox set published_at=coalesce(published_at,now()) where event_id=$1")
                .bind(event_id).execute(&mut **tx).await?;
            return Ok(false);
        }
        // One transient envelope at a time, not an in-memory backlog or a batch of bodies.
        let payload = {
            let source = sqlx::query("select event_id,workspace_id,account_id,event_type,payload,created_at,delivery_attempts from credit_event_outbox where event_id=$1")
                .bind(event_id).fetch_one(&mut **tx).await?;
            Self::serialize_payload(Self::credit_envelope(&source)?)?
        };
        let bytes = payload.len() as i64;
        let sequence = Self::next_sequence(tx).await?;
        sqlx::query("insert into legacy_event_recovery_ledger(sequence,disposition,topic,credit_event_id,payload,payload_digest,value_size_bytes,cutover_id,origin,handed_off_at) select $1,'pending','credit.events',$2,$3,sha256(convert_to($3,'UTF8')),$4,cutover_id,$5,now() from legacy_event_recovery_ledger where sequence=0")
            .bind(sequence).bind(event_id).bind(payload).bind(bytes).bind(origin).execute(&mut **tx).await?;
        let confirmed = sqlx::query("update credit_event_outbox set published_at=coalesce(published_at,now()),locked_by=null,locked_until=null,last_error=null,delivery_attempts=delivery_attempts+1 where event_id=$1")
            .bind(event_id).execute(&mut **tx).await?.rows_affected();
        ensure!(
            confirmed == 1,
            "credit source confirmation lost its locked row"
        );
        Ok(true)
    }
}
