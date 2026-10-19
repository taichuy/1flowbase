use super::*;

/// One transaction owns complete logical tasks, including every native call.
pub(super) async fn delete_batch(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    job: &AgentLogsDeleteJob,
) -> Result<u64> {
    let limit = i64::from(
        job.scope
            .batch_size()
            .ok_or_else(|| anyhow!("agent_logs.delete_job_batch_size"))?
            .get(),
    );
    let records = sqlx::query(
        "select record_id,native_run_ids from application_log_deletion_records
         where job_id=$1 order by record_id limit $2",
    )
    .bind(job.job_id)
    .bind(limit)
    .fetch_all(&mut **tx)
    .await?;
    let record_ids: Vec<Uuid> = records.iter().map(|row| row.get("record_id")).collect();
    let mut run_ids: Vec<Uuid> = records
        .iter()
        .flat_map(|row| row.get::<Vec<Uuid>, _>("native_run_ids"))
        .collect();
    run_ids.sort_unstable();
    run_ids.dedup();
    if !run_ids.is_empty() {
        // Native ingress chooses a task anchor under this same conversation row
        // lock. A concurrent later call either commits before our recheck, or
        // chooses a fresh anchor after this deletion commits.
        sqlx::query(
            "select id from application_conversations where id in
             (select log_conversation_id from application_run_log_tasks where id=any($1))
             order by id for update",
        )
        .bind(&record_ids)
        .fetch_all(&mut **tx)
        .await?;
        sqlx::query("select id from flow_runs where id=any($1) order by id for update")
            .bind(&run_ids)
            .fetch_all(&mut **tx)
            .await?;
        sqlx::query(
            "select id from application_run_log_tasks where id=any($1) order by id for update",
        )
        .bind(&record_ids)
        .fetch_all(&mut **tx)
        .await?;
        // SQL safety: compiled-in SQL only; application, scope and IDs are bound.
        let current = sqlx::query(sqlx::AssertSqlSafe(format!(
            "select * from ({CANDIDATES_SQL}) candidates where record_id=any($6)"
        )))
        .bind(job.application_id)
        .bind(
            sqlx::query_scalar::<_, Uuid>(
                "select scope_id from application_log_deletion_jobs where id=$1",
            )
            .bind(job.job_id)
            .fetch_one(&mut **tx)
            .await?,
        )
        .bind(Option::<OffsetDateTime>::None)
        .bind(Option::<OffsetDateTime>::None)
        .bind(Option::<OffsetDateTime>::None)
        .bind(&record_ids)
        .fetch_all(&mut **tx)
        .await?;
        anyhow::ensure!(
            current.len() == records.len(),
            "agent_logs.delete_job_workset_changed"
        );
        for record in &records {
            let id: Uuid = record.get("record_id");
            let expected: Vec<Uuid> = record.get("native_run_ids");
            anyhow::ensure!(
                current
                    .iter()
                    .any(|row| row.get::<Uuid, _>("record_id") == id
                        && row.get::<Vec<Uuid>, _>("native_run_ids") == expected),
                "agent_logs.delete_job_workset_changed"
            );
        }
        delete_native_runs(tx, job.application_id, &run_ids).await?;
    }
    // Native anchors cascade from flow_runs; imported tasks are the source facts.
    let imported_count = records
        .iter()
        .filter(|row| row.get::<Vec<Uuid>, _>("native_run_ids").is_empty())
        .count() as u64;
    let deleted = sqlx::query("delete from application_run_log_tasks where id=any($1)")
        .bind(&record_ids)
        .execute(&mut **tx)
        .await?
        .rows_affected();
    anyhow::ensure!(
        deleted == imported_count,
        "agent_logs.delete_job_workset_changed"
    );
    Ok(record_ids.len() as u64)
}

async fn delete_native_runs(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    application_id: Uuid,
    run_ids: &[Uuid],
) -> Result<()> {
    let contents: Vec<Uuid> = sqlx::query_scalar(
        "select actual_content_id from runtime_context_projections where flow_run_id=any($1) and actual_content_id is not null
         union select recovery_content_id from flow_run_recovery_history where flow_run_id=any($1) and recovery_content_id is not null
         union select canonical_content_id from runtime_legacy_shadow_rows where flow_run_id=any($1) and canonical_content_id is not null
         union select observation_body_content_id from runtime_events where flow_run_id=any($1) and observation_body_content_id is not null
         union select content_id from client_trajectory_sections where flow_run_id=any($1) and content_id is not null",
    ).bind(run_ids).fetch_all(&mut **tx).await?;
    // Release RESTRICT edges before deleting their context parents. Financial
    // rows retain their original run UUIDs under the formal retention migration.
    sqlx::query("delete from runtime_invocation_context_bindings where flow_run_id=any($1)")
        .bind(run_ids)
        .execute(&mut **tx)
        .await?;
    sqlx::query("delete from flow_run_recovery_history where flow_run_id=any($1)")
        .bind(run_ids)
        .execute(&mut **tx)
        .await?;
    sqlx::query("delete from runtime_legacy_shadow_rows where flow_run_id=any($1)")
        .bind(run_ids)
        .execute(&mut **tx)
        .await?;
    sqlx::query("delete from runtime_context_projections where flow_run_id=any($1)")
        .bind(run_ids)
        .execute(&mut **tx)
        .await?;
    // Span parents may cross run boundaries. Preserve descendants belonging to
    // retained tasks instead of letting their parent's CASCADE erase live facts.
    // FOR UPDATE also orders concurrent child FK insertions before the detach.
    sqlx::query("select id from runtime_spans where flow_run_id=any($1) order by id for update")
        .bind(run_ids)
        .fetch_all(&mut **tx)
        .await?;
    sqlx::query(
        "update runtime_spans set parent_span_id=null
         where not (flow_run_id=any($1)) and parent_span_id in
             (select id from runtime_spans where flow_run_id=any($1))",
    )
    .bind(run_ids)
    .execute(&mut **tx)
    .await?;
    // Request logs deliberately have no run FK; remove their application-owned
    // rows explicitly rather than leaving detached request bodies/metadata.
    sqlx::query(
        "delete from model_provider_request_logs where application_id=$1 and flow_run_id=any($2)",
    )
    .bind(application_id)
    .bind(run_ids)
    .execute(&mut **tx)
    .await?;
    let deleted = sqlx::query("delete from flow_runs where application_id=$1 and id=any($2)")
        .bind(application_id)
        .bind(run_ids)
        .execute(&mut **tx)
        .await?
        .rows_affected();
    anyhow::ensure!(
        deleted == run_ids.len() as u64,
        "agent_logs.delete_job_workset_changed"
    );

    // Generic context bodies have no observation ownership marker. Reclaim only
    // this batch's candidates, under the canonical writer's lock, after testing
    // every durable reference owner. Shared bodies remain readable.
    sqlx::query(
        "select pg_advisory_xact_lock(hashtextextended('canonical-runtime:'||application_id::text||':'||content_hash,0))
         from runtime_canonical_contents where application_id=$1 and id=any($2) order by content_hash",
    ).bind(application_id).bind(&contents).fetch_all(&mut **tx).await?;
    sqlx::query(include_str!("reclaim_contents.sql"))
        .bind(application_id)
        .bind(&contents)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
