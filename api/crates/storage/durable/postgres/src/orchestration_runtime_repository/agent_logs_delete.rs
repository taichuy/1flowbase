use super::*;
use control_plane_contracts::ports::{AgentLogsDeleteReceipt, AgentLogsDeleteScope};

pub(super) async fn delete(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    scope_id: Uuid,
    scope: &AgentLogsDeleteScope,
) -> Result<AgentLogsDeleteReceipt> {
    let bounds = scope.bounds()?;
    let mut tx = store.pool().begin().await?;
    // Exclusive counterpart of ingestion's application lock, acquired first.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("agent-logs-application:{application_id}"))
        .execute(&mut *tx)
        .await?;
    let supported: bool = sqlx::query_scalar("select exists(select 1 from applications where id=$1 and scope_id=$2 and application_type='agent_logs')")
        .bind(application_id).bind(scope_id).fetch_one(&mut *tx).await?;
    anyhow::ensure!(supported, "agent_logs.application_type");
    let (from, to) = bounds.map_or((None, None), |(from, to)| (Some(from), Some(to)));
    // Record FKs cascade messages, captures, steps, sections and upload receipts.
    // Deferred reclaim_client_section_body reclaims only last-reference canonical bodies.
    let deleted_records = sqlx::query("delete from application_run_log_tasks where application_id=$1 and scope_id=$2 and source_kind='imported' and ($3::timestamptz is null or started_at >= $3) and ($4::timestamptz is null or started_at < $4)")
        .bind(application_id).bind(scope_id).bind(from).bind(to).execute(&mut *tx).await?.rows_affected();
    tx.commit().await?;
    Ok(AgentLogsDeleteReceipt { deleted_records })
}
