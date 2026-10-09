use super::super::*;
use control_plane_contracts::ports::{
    AgentLogsDeleteJob, AgentLogsDeleteJobCreate, AgentLogsDeleteJobStatus, AgentLogsDeletePreview,
    AgentLogsDeleteScope,
};

fn map_job(row: sqlx::postgres::PgRow) -> Result<AgentLogsDeleteJob> {
    let timestamp = |name| -> Result<String> {
        Ok(row
            .get::<OffsetDateTime, _>(name)
            .format(&time::format_description::well_known::Rfc3339)?)
    };
    Ok(AgentLogsDeleteJob {
        job_id: row.get("id"),
        application_id: row.get("application_id"),
        scope: serde_json::from_value(row.get("requested_scope"))?,
        ingested_at_before: timestamp("ingested_at_before")?,
        status: serde_json::from_value(json!(row.get::<String, _>("status")))?,
        total_records: u64::try_from(row.get::<i64, _>("total_records"))?,
        deleted_records: u64::try_from(row.get::<i64, _>("deleted_records"))?,
        stop_requested: row.get("stop_requested"),
        error_code: row.get("error_code"),
        created_at: timestamp("created_at")?,
        updated_at: timestamp("updated_at")?,
    })
}
const JOB_COLUMNS: &str = "j.*, exists(select 1 from application_log_deletion_stop_requests s where s.job_id=j.id) as stop_requested";

pub(in crate::orchestration_runtime_repository) async fn preview(
    store: &PgControlPlaneStore,
    app: Uuid,
    scope_id: Uuid,
    scope: &AgentLogsDeleteScope,
) -> Result<AgentLogsDeletePreview> {
    let (from, to) = scope
        .bounds()?
        .map_or((None, None), |(from, to)| (Some(from), Some(to)));
    let total: i64 = sqlx::query_scalar("select count(*) from application_run_log_tasks where application_id=$1 and scope_id=$2 and source_kind='imported' and ($3::timestamptz is null or started_at >= $3) and ($4::timestamptz is null or started_at < $4)")
        .bind(app).bind(scope_id).bind(from).bind(to).fetch_one(store.pool()).await?;
    Ok(AgentLogsDeletePreview {
        total_records: u64::try_from(total)?,
    })
}

pub(in crate::orchestration_runtime_repository) async fn create(
    store: &PgControlPlaneStore,
    app: Uuid,
    scope_id: Uuid,
    input: &AgentLogsDeleteJobCreate,
) -> Result<AgentLogsDeleteJob> {
    let mut tx = store.pool().begin().await?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("agent-logs-application:{app}"))
        .execute(&mut *tx)
        .await?;
    let supported: bool=sqlx::query_scalar("select exists(select 1 from applications where id=$1 and scope_id=$2 and application_type='agent_logs')").bind(app).bind(scope_id).fetch_one(&mut *tx).await?;
    anyhow::ensure!(supported, "agent_logs.application_type");
    if let Some(row) = sqlx::query(&format!(
        "select {JOB_COLUMNS} from application_log_deletion_jobs j where j.id=$1"
    ))
    .bind(input.job_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        let job = map_job(row)?;
        anyhow::ensure!(
            job.application_id == app
                && serde_json::to_value(&job.scope)? == serde_json::to_value(&input.scope)?,
            "agent_logs.delete_job_conflict"
        );
        tx.commit().await?;
        return Ok(job);
    }
    let active: bool=sqlx::query_scalar("select exists(select 1 from application_log_deletion_jobs where application_id=$1 and status in ('queued','running'))").bind(app).fetch_one(&mut *tx).await?;
    anyhow::ensure!(!active, "agent_logs.delete_job_active");
    let boundary: OffsetDateTime = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("insert into application_log_deletion_jobs(id,application_id,scope_id,requested_scope,ingested_at_before,status) values($1,$2,$3,$4,$5,'queued')").bind(input.job_id).bind(app).bind(scope_id).bind(serde_json::to_value(&input.scope)?).bind(boundary).execute(&mut *tx).await?;
    let (from, to) = input
        .scope
        .bounds()?
        .map_or((None, None), |(from, to)| (Some(from), Some(to)));
    let total=sqlx::query("insert into application_log_deletion_records(job_id,record_id) select $1,id from application_run_log_tasks where application_id=$2 and scope_id=$3 and source_kind='imported' and ($4::timestamptz is null or started_at >= $4) and ($5::timestamptz is null or started_at < $5) and ingested_at < $6").bind(input.job_id).bind(app).bind(scope_id).bind(from).bind(to).bind(boundary).execute(&mut *tx).await?.rows_affected();
    sqlx::query("update application_log_deletion_jobs set total_records=$2,status=case when $2=0 then 'succeeded' else 'queued' end where id=$1").bind(input.job_id).bind(i64::try_from(total)?).execute(&mut *tx).await?;
    let job = map_job(
        sqlx::query(&format!(
            "select {JOB_COLUMNS} from application_log_deletion_jobs j where j.id=$1"
        ))
        .bind(input.job_id)
        .fetch_one(&mut *tx)
        .await?,
    )?;
    tx.commit().await?;
    Ok(job)
}

pub(in crate::orchestration_runtime_repository) async fn get(
    store: &PgControlPlaneStore,
    app: Uuid,
    scope: Uuid,
    id: Option<Uuid>,
) -> Result<Option<AgentLogsDeleteJob>> {
    sqlx::query(&format!("select {JOB_COLUMNS} from application_log_deletion_jobs j where application_id=$1 and scope_id=$2 and ($3::uuid is null or id=$3) order by created_at desc,id desc limit 1"))
        .bind(app).bind(scope).bind(id).fetch_optional(store.pool()).await?.map(map_job).transpose()
}
pub(in crate::orchestration_runtime_repository) async fn stop(
    store: &PgControlPlaneStore,
    app: Uuid,
    scope: Uuid,
    id: Uuid,
) -> Result<Option<AgentLogsDeleteJob>> {
    // FK takes KEY SHARE, compatible with the batch's NO KEY UPDATE lock.
    sqlx::query("insert into application_log_deletion_stop_requests(job_id) select id from application_log_deletion_jobs where id=$1 and application_id=$2 and scope_id=$3 and status in ('queued','running') on conflict do nothing").bind(id).bind(app).bind(scope).execute(store.pool()).await?;
    get(store, app, scope, Some(id)).await
}
pub(in crate::orchestration_runtime_repository) async fn next(
    store: &PgControlPlaneStore,
) -> Result<Option<AgentLogsDeleteJob>> {
    sqlx::query(&format!("select {JOB_COLUMNS} from application_log_deletion_jobs j where status in ('queued','running') order by updated_at,id limit 1"))
        .fetch_optional(store.pool()).await?.map(map_job).transpose()
}
pub(in crate::orchestration_runtime_repository) async fn advance(
    store: &PgControlPlaneStore,
    id: Uuid,
) -> Result<()> {
    let mut tx = store.pool().begin().await?;
    let row=sqlx::query(&format!("select {JOB_COLUMNS} from application_log_deletion_jobs j where id=$1 and status in ('queued','running') for no key update of j skip locked")).bind(id).fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        return Ok(());
    };
    let job = map_job(row)?;
    // Try rather than wait prevents reverse lock-order deadlocks with legacy DELETE/start.
    let locked: bool =
        sqlx::query_scalar("select pg_try_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("agent-logs-application:{}", job.application_id))
            .fetch_one(&mut *tx)
            .await?;
    if !locked {
        return Ok(());
    }
    let count = if job.stop_requested {
        0
    } else {
        sqlx::query("with selected as (select record_id from application_log_deletion_records where job_id=$1 order by record_id limit $2) delete from application_run_log_tasks t using selected s where t.id=s.record_id")
            .bind(id).bind(i64::from(job.scope.batch_size().ok_or_else(||anyhow!("agent_logs.delete_job_batch_size"))?.get())).execute(&mut *tx).await?.rows_affected()
    };
    anyhow::ensure!(
        job.stop_requested || count > 0 || job.deleted_records == job.total_records,
        "agent_logs.delete_job_workset_changed"
    );
    // Read cancellation again after DELETE; stop arriving in this batch applies at commit.
    let stopped: bool = sqlx::query_scalar(
        "select exists(select 1 from application_log_deletion_stop_requests where job_id=$1)",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let deleted = job.deleted_records + count;
    let status = AgentLogsDeleteJobStatus::after_batch(deleted, job.total_records, stopped);
    sqlx::query("update application_log_deletion_jobs set deleted_records=$2,status=$3,updated_at=clock_timestamp() where id=$1").bind(id).bind(i64::try_from(deleted)?).bind(serde_json::to_value(status)?.as_str().unwrap()).execute(&mut *tx).await?;
    if !status.is_active() {
        sqlx::query("delete from application_log_deletion_records where job_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    // Includes deferred canonical-body reference reclamation; progress is visible only after it.
    tx.commit().await?;
    Ok(())
}
pub(in crate::orchestration_runtime_repository) async fn fail(
    store: &PgControlPlaneStore,
    id: Uuid,
    expected_deleted_records: u64,
) -> Result<()> {
    let mut tx = store.pool().begin().await?;
    let changed=sqlx::query("update application_log_deletion_jobs set status='failed',error_code='agent_logs_delete_batch_failed',updated_at=clock_timestamp() where id=$1 and status in ('queued','running') and deleted_records=$2").bind(id).bind(i64::try_from(expected_deleted_records)?).execute(&mut *tx).await?.rows_affected();
    if changed > 0 {
        sqlx::query("delete from application_log_deletion_records where job_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
