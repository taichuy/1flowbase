use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use control_plane_contracts::ports::{
    LifecycleClaimLost, LifecycleDeliveryPauseReason, LifecycleOutboxRecord,
    LifecycleOutboxRepository, LifecycleOutboxStatus, RecordLifecycleFactInput,
};
use sqlx::{Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

const MAX_CONTRACT_ID_BYTES: usize = 256;
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_ERROR_BYTES: usize = 4096;

pub(crate) async fn record_lifecycle_fact_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    input: &RecordLifecycleFactInput,
) -> Result<LifecycleOutboxRecord> {
    validate_input(input)?;
    let occurred_at = postgres_timestamp_precision(input.occurred_at)?;
    let inserted = sqlx::query(
        r#"
        insert into lifecycle_outbox (
            event_id, transaction_id, contract_id, contract_version,
            canonical_payload, occurred_at, graph_fingerprint
        ) values ($1, $2, $3, $4, $5, $6, $7)
        on conflict (event_id) do nothing
        "#,
    )
    .bind(input.event_id)
    .bind(input.transaction_id)
    .bind(&input.contract_id)
    .bind(&input.contract_version)
    .bind(&input.canonical_payload)
    .bind(occurred_at)
    .bind(&input.publication.graph_fingerprint)
    .execute(&mut **transaction)
    .await?
    .rows_affected()
        == 1;

    if inserted {
        for subscriber in &input.publication.subscribers {
            sqlx::query(
                r#"
            insert into lifecycle_outbox_deliveries (
                event_id, subscriber_id, handler_id, handler_version
            ) values ($1, $2, $3, $4)
            on conflict (event_id, subscriber_id) do nothing
            "#,
            )
            .bind(input.event_id)
            .bind(&subscriber.subscriber_id)
            .bind(&subscriber.handler_id)
            .bind(&subscriber.handler_version)
            .execute(&mut **transaction)
            .await?;
        }
    } else {
        let delivery_count: i64 = sqlx::query_scalar(
            "select count(*) from lifecycle_outbox_deliveries where event_id = $1",
        )
        .bind(input.event_id)
        .fetch_one(&mut **transaction)
        .await?;
        if delivery_count != input.publication.subscribers.len() as i64 {
            bail!("lifecycle outbox event ID conflicts with a different publication plan");
        }
        for subscriber in &input.publication.subscribers {
            let matches: bool = sqlx::query_scalar(
                r#"
                select exists (
                    select 1 from lifecycle_outbox_deliveries
                    where event_id = $1 and subscriber_id = $2
                      and handler_id = $3 and handler_version = $4
                )
                "#,
            )
            .bind(input.event_id)
            .bind(&subscriber.subscriber_id)
            .bind(&subscriber.handler_id)
            .bind(&subscriber.handler_version)
            .fetch_one(&mut **transaction)
            .await?;
            if !matches {
                bail!("lifecycle outbox event ID conflicts with a different publication plan");
            }
        }
    }

    let first = input
        .publication
        .subscribers
        .first()
        .ok_or_else(|| anyhow!("lifecycle publication plan has no subscribers"))?;
    let record = find_delivery(&mut **transaction, input.event_id, &first.subscriber_id)
        .await?
        .ok_or_else(|| anyhow!("lifecycle outbox delivery was not persisted"))?;
    if record.transaction_id != input.transaction_id
        || record.contract_id != input.contract_id
        || record.contract_version != input.contract_version
        || record.canonical_payload != input.canonical_payload
        || record.occurred_at != occurred_at
        || record.graph_fingerprint != input.publication.graph_fingerprint
    {
        bail!("lifecycle outbox event ID conflicts with a different fact");
    }
    Ok(record)
}

fn postgres_timestamp_precision(value: OffsetDateTime) -> Result<OffsetDateTime> {
    let microseconds = value.unix_timestamp_nanos().div_euclid(1_000);
    OffsetDateTime::from_unix_timestamp_nanos(microseconds * 1_000)
        .map_err(|error| anyhow!("lifecycle outbox timestamp is out of range: {error}"))
}

#[async_trait]
impl LifecycleOutboxRepository for PgControlPlaneStore {
    async fn record_lifecycle_fact(
        &self,
        input: &RecordLifecycleFactInput,
    ) -> Result<LifecycleOutboxRecord> {
        let mut transaction = self.pool().begin().await?;
        let record = record_lifecycle_fact_in_transaction(&mut transaction, input).await?;
        transaction.commit().await?;
        Ok(record)
    }

    async fn claim_lifecycle_facts(
        &self,
        worker_id: Uuid,
        limit: u32,
        claim_lease: time::Duration,
    ) -> Result<Vec<LifecycleOutboxRecord>> {
        if limit == 0 || limit > 1_000 {
            bail!("lifecycle outbox claim limit must be between 1 and 1000");
        }
        if claim_lease <= time::Duration::ZERO || claim_lease > time::Duration::hours(1) {
            bail!("lifecycle outbox claim lease must be between 1ns and 1h");
        }
        let mut transaction = self.pool().begin().await?;
        let lease_micros = i64::try_from(claim_lease.whole_microseconds().max(1))?;
        let rows = sqlx::query(
            r#"
            with candidates as (
                select event_id, subscriber_id from lifecycle_outbox_deliveries
                where (status = 'pending' and available_at <= now())
                   or (status = 'claimed' and claim_expires_at <= now())
                order by available_at, event_id, subscriber_id
                for update skip locked
                limit $2
            )
            update lifecycle_outbox_deliveries as delivery
            set status = 'claimed', claimed_by = $1, claimed_at = now(),
                claim_id = gen_random_uuid(), claim_expires_at = now() + $3::bigint * interval '1 microsecond',
                attempt_count = attempt_count + 1
            from candidates
            where delivery.event_id = candidates.event_id
              and delivery.subscriber_id = candidates.subscriber_id
            returning delivery.event_id, delivery.subscriber_id
            "#,
        )
        .bind(worker_id)
        .bind(i64::from(limit))
        .bind(lease_micros)
        .fetch_all(&mut *transaction)
        .await?;
        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            records.push(
                find_delivery(
                    &mut *transaction,
                    row.try_get("event_id")?,
                    row.try_get::<String, _>("subscriber_id")?.as_str(),
                )
                .await?
                .ok_or_else(|| anyhow!("claimed lifecycle delivery disappeared"))?,
            );
        }
        transaction.commit().await?;
        Ok(records)
    }

    async fn mark_lifecycle_fact_delivered(
        &self,
        event_id: Uuid,
        subscriber_id: &str,
        worker_id: Uuid,
        claim_id: Uuid,
    ) -> Result<LifecycleOutboxRecord> {
        update_claim(
            self,
            event_id,
            subscriber_id,
            worker_id,
            claim_id,
            "delivered",
            None,
            None,
            None,
        )
        .await
    }
    async fn retry_lifecycle_fact(
        &self,
        event_id: Uuid,
        subscriber_id: &str,
        worker_id: Uuid,
        claim_id: Uuid,
        available_at: OffsetDateTime,
        error: &str,
    ) -> Result<LifecycleOutboxRecord> {
        if error.is_empty() || error.len() > MAX_ERROR_BYTES {
            bail!("lifecycle outbox retry error must contain 1 to {MAX_ERROR_BYTES} bytes");
        }
        update_claim(
            self,
            event_id,
            subscriber_id,
            worker_id,
            claim_id,
            "pending",
            Some(available_at),
            Some(error),
            None,
        )
        .await
    }
    async fn pause_lifecycle_fact(
        &self,
        event_id: Uuid,
        subscriber_id: &str,
        worker_id: Uuid,
        claim_id: Uuid,
        reason: LifecycleDeliveryPauseReason,
    ) -> Result<LifecycleOutboxRecord> {
        update_claim(
            self,
            event_id,
            subscriber_id,
            worker_id,
            claim_id,
            "paused",
            None,
            None,
            Some(reason),
        )
        .await
    }
}

async fn update_claim(
    store: &PgControlPlaneStore,
    event_id: Uuid,
    subscriber_id: &str,
    worker_id: Uuid,
    claim_id: Uuid,
    target_status: &str,
    available_at: Option<OffsetDateTime>,
    error: Option<&str>,
    pause_reason: Option<LifecycleDeliveryPauseReason>,
) -> Result<LifecycleOutboxRecord> {
    let mut transaction = store.pool().begin().await?;
    // Serialize sibling ACK rollup on the fact; the last ACK sees all earlier committed siblings.
    sqlx::query("select event_id from lifecycle_outbox where event_id = $1 for update")
        .bind(event_id)
        .fetch_optional(&mut *transaction)
        .await?;
    let changed = sqlx::query(
        r#"
        update lifecycle_outbox_deliveries
        set status = $5, available_at = coalesce($6, available_at),
            claimed_by = null, claimed_at = null, claim_id = null, claim_expires_at = null,
            delivered_at = case when $5 = 'delivered' then now() else null end,
            last_error = case when $5 = 'paused' then coalesce($7, last_error) else $7 end, pause_reason = $8,
            paused_at = case when $5 = 'paused' then now() else null end
        where event_id = $1 and subscriber_id = $2 and status = 'claimed'
          and claimed_by = $3 and claim_id = $4 and claim_expires_at > clock_timestamp()
        "#,
    )
    .bind(event_id)
    .bind(subscriber_id)
    .bind(worker_id)
    .bind(claim_id)
    .bind(target_status)
    .bind(available_at)
    .bind(error)
    .bind(pause_reason.map(|r| r.as_str()))
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(LifecycleClaimLost.into());
    }
    if target_status == "delivered" {
        sqlx::query("update lifecycle_outbox set status = 'delivered', delivered_at = now() where event_id = $1 and not exists (select 1 from lifecycle_outbox_deliveries where event_id = $1 and status <> 'delivered')").bind(event_id).execute(&mut *transaction).await?;
    }
    let record = find_delivery(&mut *transaction, event_id, subscriber_id)
        .await?
        .ok_or_else(|| anyhow!("updated lifecycle delivery disappeared"))?;
    transaction.commit().await?;
    Ok(record)
}

async fn find_delivery<'e, E>(
    executor: E,
    event_id: Uuid,
    subscriber_id: &str,
) -> Result<Option<LifecycleOutboxRecord>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query(
        r#"
        select outbox.event_id, outbox.transaction_id, outbox.contract_id,
               outbox.contract_version, outbox.canonical_payload, outbox.occurred_at,
               outbox.graph_fingerprint, delivery.subscriber_id, delivery.handler_id,
               delivery.handler_version, delivery.status, delivery.attempt_count,
               delivery.available_at, delivery.claimed_by, delivery.claimed_at,
               delivery.delivered_at, delivery.claim_id, delivery.claim_expires_at, delivery.pause_reason, delivery.paused_at
        from lifecycle_outbox outbox
        join lifecycle_outbox_deliveries delivery using (event_id)
        where outbox.event_id = $1 and delivery.subscriber_id = $2
        "#,
    )
    .bind(event_id)
    .bind(subscriber_id)
    .fetch_optional(executor)
    .await?
    .map(map_record)
    .transpose()
}

fn validate_input(input: &RecordLifecycleFactInput) -> Result<()> {
    if input.contract_id.is_empty() || input.contract_id.len() > MAX_CONTRACT_ID_BYTES {
        bail!("lifecycle contract ID must contain 1 to {MAX_CONTRACT_ID_BYTES} bytes");
    }
    if input.contract_version.is_empty() || input.contract_version.len() > MAX_CONTRACT_ID_BYTES {
        bail!("lifecycle contract version must contain 1 to {MAX_CONTRACT_ID_BYTES} bytes");
    }
    if input.canonical_payload.len() > MAX_PAYLOAD_BYTES {
        bail!("lifecycle payload exceeds {MAX_PAYLOAD_BYTES} bytes");
    }
    if input.publication.graph_fingerprint.is_empty() {
        bail!("lifecycle graph fingerprint must not be empty");
    }
    if input.publication.subscribers.is_empty() {
        bail!("lifecycle publication plan must contain at least one subscriber");
    }
    let mut subscriber_ids = std::collections::BTreeSet::new();
    for subscriber in &input.publication.subscribers {
        if subscriber.subscriber_id.is_empty()
            || subscriber.handler_id.is_empty()
            || subscriber.handler_version.is_empty()
        {
            bail!("lifecycle subscriber identity must not be empty");
        }
        if !subscriber_ids.insert(&subscriber.subscriber_id) {
            bail!("duplicate lifecycle subscriber ID");
        }
    }
    Ok(())
}

fn map_record(row: sqlx::postgres::PgRow) -> Result<LifecycleOutboxRecord> {
    let status = match row.try_get::<String, _>("status")?.as_str() {
        "pending" => LifecycleOutboxStatus::Pending,
        "claimed" => LifecycleOutboxStatus::Claimed,
        "delivered" => LifecycleOutboxStatus::Delivered,
        "paused" => LifecycleOutboxStatus::Paused,
        other => bail!("invalid lifecycle outbox status {other}"),
    };
    Ok(LifecycleOutboxRecord {
        event_id: row.try_get("event_id")?,
        transaction_id: row.try_get("transaction_id")?,
        contract_id: row.try_get("contract_id")?,
        contract_version: row.try_get("contract_version")?,
        canonical_payload: row.try_get("canonical_payload")?,
        occurred_at: row.try_get("occurred_at")?,
        graph_fingerprint: row.try_get("graph_fingerprint")?,
        subscriber_id: row.try_get("subscriber_id")?,
        handler_id: row.try_get("handler_id")?,
        handler_version: row.try_get("handler_version")?,
        status,
        attempt_count: row.try_get("attempt_count")?,
        available_at: row.try_get("available_at")?,
        claimed_by: row.try_get("claimed_by")?,
        claimed_at: row.try_get("claimed_at")?,
        claim_id: row.try_get("claim_id")?,
        claim_expires_at: row.try_get("claim_expires_at")?,
        pause_reason: row
            .try_get::<Option<String>, _>("pause_reason")?
            .map(|value| match value.as_str() {
                "frozen_graph_unavailable" => {
                    Ok(LifecycleDeliveryPauseReason::FrozenGraphUnavailable)
                }
                "frozen_handler_unavailable" => {
                    Ok(LifecycleDeliveryPauseReason::FrozenHandlerUnavailable)
                }
                "authority_revoked" => Ok(LifecycleDeliveryPauseReason::AuthorityRevoked),
                "installation_inactive" => Ok(LifecycleDeliveryPauseReason::InstallationInactive),
                "retry_budget_exhausted" => Ok(LifecycleDeliveryPauseReason::RetryBudgetExhausted),
                _ => Err(anyhow!("invalid lifecycle delivery pause reason")),
            })
            .transpose()?,
        paused_at: row.try_get("paused_at")?,
        delivered_at: row.try_get("delivered_at")?,
    })
}

#[async_trait]
impl control_plane_contracts::ports::DerivedLifecyclePublicationRepository for PgControlPlaneStore {
    async fn record_derived_lifecycle_fact(
        &self,
        input: &RecordLifecycleFactInput,
    ) -> Result<LifecycleOutboxRecord> {
        let mut transaction = self.pool().begin().await?;
        let record = record_derived_lifecycle_fact_in_transaction(&mut transaction, input).await?;
        transaction.commit().await?;
        Ok(record)
    }
}

pub(crate) async fn record_derived_lifecycle_fact_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    input: &RecordLifecycleFactInput,
) -> Result<LifecycleOutboxRecord> {
    // The same stable source/subscriber/output cannot race two first-publication snapshots.
    // A transaction advisory lock complements the existing event_id primary key, without
    // allocating another idempotency table or changing native Create's transaction owner.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("managed-derived-publication:{}", input.event_id))
        .execute(&mut **transaction)
        .await?;
    let targets = sqlx::query("select subscriber_id, handler_id, handler_version from lifecycle_outbox_deliveries where event_id = $1 order by subscriber_id")
            .bind(input.event_id).fetch_all(&mut **transaction).await?;
    let mut frozen = input.clone();
    if let Some(first) = targets.first() {
        let subscriber_id: String = first.try_get("subscriber_id")?;
        let stored = find_delivery(&mut **transaction, input.event_id, &subscriber_id)
            .await?
            .ok_or_else(|| anyhow!("derived lifecycle publication disappeared"))?;
        // Input content still goes through the complete existing fact equality check below.
        // Only the originally frozen host time and plan are reused on repeated publication.
        frozen.occurred_at = stored.occurred_at;
        frozen.publication.graph_fingerprint = stored.graph_fingerprint;
        frozen.publication.subscribers = targets
            .iter()
            .map(|row| {
                Ok(control_plane_contracts::ports::LifecycleSubscriberTarget {
                    subscriber_id: row.try_get("subscriber_id")?,
                    handler_id: row.try_get("handler_id")?,
                    handler_version: row.try_get("handler_version")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
    }
    let record = record_lifecycle_fact_in_transaction(transaction, &frozen).await?;
    Ok(record)
}
