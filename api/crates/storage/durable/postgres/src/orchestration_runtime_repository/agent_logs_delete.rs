use super::*;
use control_plane_contracts::ports::{AgentLogsDeleteReceipt, AgentLogsDeleteScope};

pub(super) async fn delete(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    scope_id: Uuid,
    scope: &AgentLogsDeleteScope,
) -> Result<AgentLogsDeleteReceipt> {
    let bounds = scope.bounds()?;
    let requested_boundary = scope.ingested_at_before()?;
    let mut tx = store.pool().begin().await?;
    // Exclusive counterpart of ingestion's application lock, acquired first.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("agent-logs-application:{application_id}"))
        .execute(&mut *tx)
        .await?;
    let supported: bool = sqlx::query_scalar("select exists(select 1 from applications where id=$1 and scope_id=$2 and application_type='agent_logs')")
        .bind(application_id).bind(scope_id).fetch_one(&mut *tx).await?;
    anyhow::ensure!(supported, "agent_logs.application_type");
    // Capture after lock acquisition; subsequent batches reuse this server boundary.
    let boundary = match requested_boundary {
        Some(boundary) => boundary,
        None => {
            sqlx::query_scalar::<_, time::OffsetDateTime>("select clock_timestamp()")
                .fetch_one(&mut *tx)
                .await?
        }
    };
    let (from, to) = bounds.map_or((None, None), |(from, to)| (Some(from), Some(to)));
    let limit = scope.batch_size().map(|value| i64::from(value.get()));
    // Record FKs cascade messages, captures, steps, sections and upload receipts.
    // Deferred reclaim_client_section_body reclaims only last-reference canonical bodies.
    // LIMIT NULL preserves the original one-transaction, full-scope deletion API.
    let deleted_records = sqlx::query(
        "with selected as (
            select id from application_run_log_tasks
            where application_id=$1 and scope_id=$2 and source_kind='imported'
              and ($3::timestamptz is null or started_at >= $3)
              and ($4::timestamptz is null or started_at < $4)
              and ingested_at < $6
            order by started_at, id limit $5
        ) delete from application_run_log_tasks t using selected s where t.id=s.id",
    )
    .bind(application_id)
    .bind(scope_id)
    .bind(from)
    .bind(to)
    .bind(limit)
    .bind(boundary)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let has_more: bool = sqlx::query_scalar(
        "select exists(select 1 from application_run_log_tasks
         where application_id=$1 and scope_id=$2 and source_kind='imported'
           and ($3::timestamptz is null or started_at >= $3)
           and ($4::timestamptz is null or started_at < $4) and ingested_at < $5)",
    )
    .bind(application_id)
    .bind(scope_id)
    .bind(from)
    .bind(to)
    .bind(boundary)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(AgentLogsDeleteReceipt {
        deleted_records,
        has_more,
        ingested_at_before: boundary.format(&time::format_description::well_known::Rfc3339)?,
    })
}
