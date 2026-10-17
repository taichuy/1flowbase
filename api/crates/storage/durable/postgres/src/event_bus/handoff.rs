use super::*;
use anyhow::ensure;
use control_plane_contracts::ports::CreditEventHandoff;

impl PgEventBus {
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
                "select exists(select 1 from credit_event_handoffs where event_id=$1)",
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

    async fn handoff_one(
        tx: &mut Transaction<'_, Postgres>,
        event_id: Uuid,
        origin: &str,
    ) -> Result<bool> {
        let received: bool = sqlx::query_scalar(
            "select exists(select 1 from credit_event_handoffs where event_id=$1)",
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
        sqlx::query("insert into credit_event_handoffs(event_id,sequence,cutover_id,origin) select $1,$2,cutover_id,$3 from host_event_bus_counter where singleton")
            .bind(event_id).bind(sequence).bind(origin).execute(&mut **tx).await?;
        sqlx::query("insert into host_event_bus_pending(sequence,topic,credit_event_id,payload,value_size_bytes) values ($1,'credit.events',$2,$3,$4)")
            .bind(sequence).bind(event_id).bind(payload).bind(bytes).execute(&mut **tx).await?;
        let confirmed = sqlx::query("update credit_event_outbox set published_at=coalesce(published_at,now()),locked_by=null,locked_until=null,last_error=null,delivery_attempts=delivery_attempts+1 where event_id=$1")
            .bind(event_id).execute(&mut **tx).await?.rows_affected();
        ensure!(
            confirmed == 1,
            "credit source confirmation lost its locked row"
        );
        Ok(true)
    }
}

#[async_trait]
impl CreditEventHandoff for PgEventBus {
    async fn handoff_credit_events(&self, limit: i64) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        Self::lock_queue(&mut tx).await?;
        let ids: Vec<Uuid> = sqlx::query_scalar("select event_id from credit_event_outbox where published_at is null and (locked_until is null or locked_until < now()) order by created_at,event_id limit $1 for update")
            .bind(limit.clamp(1,100)).fetch_all(&mut *tx).await?;
        let mut inserted = 0;
        for id in ids {
            inserted += usize::from(Self::handoff_one(&mut tx, id, "live").await?);
        }
        tx.commit().await?;
        Ok(inserted)
    }
}
