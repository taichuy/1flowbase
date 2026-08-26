use anyhow::Result;
use control_plane::errors::ControlPlaneError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(super) async fn lock_flow_run_event_sequence(
    tx: &mut Transaction<'_, Postgres>,
    flow_run_id: Uuid,
) -> Result<()> {
    sqlx::query("select id from flow_runs where id = $1 for update")
        .bind(flow_run_id)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn flow_run_scope_id_for_update(
    tx: &mut Transaction<'_, Postgres>,
    flow_run_id: Uuid,
) -> Result<Uuid> {
    sqlx::query_scalar(
        r#"
        select applications.workspace_id
        from flow_runs
        join applications on applications.id = flow_runs.application_id
        where flow_runs.id = $1
        for update of flow_runs
        "#,
    )
    .bind(flow_run_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ControlPlaneError::NotFound("flow_run").into())
}

pub(super) async fn next_event_sequence(
    tx: &mut Transaction<'_, Postgres>,
    flow_run_id: Uuid,
) -> Result<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        "select coalesce(max(sequence), 0) + 1 from flow_run_events where flow_run_id = $1",
    )
    .bind(flow_run_id)
    .fetch_one(&mut **tx)
    .await?)
}

pub(super) async fn next_runtime_event_sequence(
    tx: &mut Transaction<'_, Postgres>,
    flow_run_id: Uuid,
) -> Result<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        "select coalesce(max(sequence), 0) + 1 from runtime_events where flow_run_id = $1",
    )
    .bind(flow_run_id)
    .fetch_one(&mut **tx)
    .await?)
}
