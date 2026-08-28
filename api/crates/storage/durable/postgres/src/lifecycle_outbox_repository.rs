use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use control_plane_contracts::ports::{
    LifecycleOutboxRecord, LifecycleOutboxRepository, LifecycleOutboxStatus,
    RecordLifecycleFactInput,
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
    sqlx::query(
        r#"
        insert into lifecycle_outbox (
            event_id, transaction_id, contract_id, contract_version,
            canonical_payload, occurred_at
        ) values ($1, $2, $3, $4, $5, $6)
        on conflict (event_id) do nothing
        "#,
    )
    .bind(input.event_id)
    .bind(input.transaction_id)
    .bind(&input.contract_id)
    .bind(&input.contract_version)
    .bind(&input.canonical_payload)
    .bind(occurred_at)
    .execute(&mut **transaction)
    .await?;

    let record = find_by_id(&mut **transaction, input.event_id)
        .await?
        .ok_or_else(|| anyhow!("lifecycle outbox record was not persisted"))?;
    if record.transaction_id != input.transaction_id
        || record.contract_id != input.contract_id
        || record.contract_version != input.contract_version
        || record.canonical_payload != input.canonical_payload
        || record.occurred_at != occurred_at
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
    ) -> Result<Vec<LifecycleOutboxRecord>> {
        if limit == 0 || limit > 1_000 {
            bail!("lifecycle outbox claim limit must be between 1 and 1000");
        }
        let rows = sqlx::query(
            r#"
            with candidates as (
                select event_id from lifecycle_outbox
                where status = 'pending' and available_at <= now()
                order by available_at, occurred_at, event_id
                for update skip locked
                limit $2
            )
            update lifecycle_outbox as outbox
            set status = 'claimed', claimed_by = $1, claimed_at = now(),
                attempt_count = attempt_count + 1
            from candidates
            where outbox.event_id = candidates.event_id
            returning outbox.*
            "#,
        )
        .bind(worker_id)
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(map_record).collect()
    }

    async fn mark_lifecycle_fact_delivered(
        &self,
        event_id: Uuid,
        worker_id: Uuid,
    ) -> Result<LifecycleOutboxRecord> {
        update_claim(self, event_id, worker_id, "delivered", None, None).await
    }

    async fn retry_lifecycle_fact(
        &self,
        event_id: Uuid,
        worker_id: Uuid,
        available_at: OffsetDateTime,
        error: &str,
    ) -> Result<LifecycleOutboxRecord> {
        if error.is_empty() || error.len() > MAX_ERROR_BYTES {
            bail!("lifecycle outbox retry error must contain 1 to {MAX_ERROR_BYTES} bytes");
        }
        update_claim(
            self,
            event_id,
            worker_id,
            "pending",
            Some(available_at),
            Some(error),
        )
        .await
    }
}

async fn update_claim(
    store: &PgControlPlaneStore,
    event_id: Uuid,
    worker_id: Uuid,
    target_status: &str,
    available_at: Option<OffsetDateTime>,
    error: Option<&str>,
) -> Result<LifecycleOutboxRecord> {
    let row = sqlx::query(
        r#"
        update lifecycle_outbox
        set status = $3, available_at = coalesce($4, available_at),
            claimed_by = null, claimed_at = null,
            delivered_at = case when $3 = 'delivered' then now() else null end,
            last_error = $5
        where event_id = $1 and status = 'claimed' and claimed_by = $2
        returning *
        "#,
    )
    .bind(event_id)
    .bind(worker_id)
    .bind(target_status)
    .bind(available_at)
    .bind(error)
    .fetch_optional(store.pool())
    .await?
    .ok_or_else(|| anyhow!("lifecycle outbox claim is missing or owned by another worker"))?;
    map_record(row)
}

async fn find_by_id<'e, E>(executor: E, event_id: Uuid) -> Result<Option<LifecycleOutboxRecord>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query("select * from lifecycle_outbox where event_id = $1")
        .bind(event_id)
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
    Ok(())
}

fn map_record(row: sqlx::postgres::PgRow) -> Result<LifecycleOutboxRecord> {
    let status = match row.try_get::<String, _>("status")?.as_str() {
        "pending" => LifecycleOutboxStatus::Pending,
        "claimed" => LifecycleOutboxStatus::Claimed,
        "delivered" => LifecycleOutboxStatus::Delivered,
        other => bail!("invalid lifecycle outbox status {other}"),
    };
    Ok(LifecycleOutboxRecord {
        event_id: row.try_get("event_id")?,
        transaction_id: row.try_get("transaction_id")?,
        contract_id: row.try_get("contract_id")?,
        contract_version: row.try_get("contract_version")?,
        canonical_payload: row.try_get("canonical_payload")?,
        occurred_at: row.try_get("occurred_at")?,
        status,
        attempt_count: row.try_get("attempt_count")?,
        available_at: row.try_get("available_at")?,
        claimed_by: row.try_get("claimed_by")?,
        claimed_at: row.try_get("claimed_at")?,
        delivered_at: row.try_get("delivered_at")?,
    })
}
