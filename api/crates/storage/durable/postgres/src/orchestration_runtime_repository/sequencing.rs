use anyhow::Result;
use control_plane_contracts::ControlPlaneContractError as ControlPlaneError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(super) async fn lock_flow_run_event_sequence(
    tx: &mut Transaction<'_, Postgres>,
    flow_run_id: Uuid,
) -> Result<()> {
    // Serialize sequence writers without excluding FK readers of the unchanged run key.
    sqlx::query("select id from flow_runs where id = $1 for no key update")
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
    reserve_runtime_event_sequences(tx, flow_run_id, 1).await
}

pub(super) async fn reserve_runtime_event_sequences(
    tx: &mut Transaction<'_, Postgres>,
    flow_run_id: Uuid,
    count: i64,
) -> Result<i64> {
    anyhow::ensure!(count > 0, "runtime sequence reservation must be positive");
    let last = sqlx::query_scalar::<_, i64>(
        // The durable high-water also includes direct client directory writes.
        // Runtime insert's statement trigger advances it for every row of batches,
        // including callers which reserve one first_sequence then add offsets.
        // Later physical rows follow previous reserved ranges. Read only the indexed
        // tail for legacy/import counter repair; max(sequence + count) scans the run.
        r#"
        update flow_runs set runtime_event_sequence_high_water = greatest(
            runtime_event_sequence_high_water,
            coalesce((select sequence + reserved_sequence_count
                from runtime_events where flow_run_id = $1
                order by sequence desc limit 1), 0)
        ) + $2
        where id = $1 returning runtime_event_sequence_high_water
        "#,
    )
    .bind(flow_run_id)
    .bind(count)
    .fetch_one(&mut **tx)
    .await?;
    Ok(last - count + 1)
}

#[cfg(test)]
#[path = "_tests/sequencing.rs"]
mod tests;
