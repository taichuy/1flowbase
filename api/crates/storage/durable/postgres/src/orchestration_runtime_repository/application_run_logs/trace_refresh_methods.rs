impl PgControlPlaneStore {
    async fn list_trace_enrichment_events(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                node_run_id,
                span_id,
                parent_span_id,
                sequence,
                event_type,
                layer,
                source,
                trust_level,
                item_id,
                ledger_ref,
                runtime_original_json(payload, runtime_events.raw_json_payloads, 'payload') as payload,
                visibility,
                durability,
                created_at
            from runtime_events
            where flow_run_id = $1
              and event_type like 'visible_internal_llm_tool_%'
            order by sequence asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_runtime_event_record).collect()
    }

    async fn claim_application_run_trace_refresh(
        &self,
    ) -> Result<Option<ApplicationRunTraceRefreshJob>> {
        let row = sqlx::query(
            r#"
            with candidate as (
                select flow_run_id from application_run_trace_refresh_queue
                where available_at <= now() and (lease_until is null or lease_until < now())
                order by available_at, flow_run_id for update skip locked limit 1
            ), claimed as (
                update application_run_trace_refresh_queue q
                set lease_until=now()+interval '5 minutes', attempts=attempts+1
                from candidate c where q.flow_run_id=c.flow_run_id
                returning q.flow_run_id, q.revision
            ) select c.*, f.application_id from claimed c join flow_runs f on f.id=c.flow_run_id
        "#,
        )
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|row| ApplicationRunTraceRefreshJob {
            flow_run_id: row.get("flow_run_id"),
            application_id: row.get("application_id"),
            revision: row.get("revision"),
        }))
    }

    async fn finish_application_run_trace_refresh(
        &self,
        job: &ApplicationRunTraceRefreshJob,
        succeeded: bool,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        if succeeded {
            sqlx::query("delete from application_run_trace_refresh_queue where flow_run_id=$1 and revision=$2")
                .bind(job.flow_run_id).bind(job.revision).execute(&mut *tx).await?;
        }
        // A concurrent source write keeps the row and schedules the next snapshot.
        sqlx::query("update application_run_trace_refresh_queue set lease_until=null, available_at=now()+case when $2 then interval '250 milliseconds' else interval '10 seconds' end where flow_run_id=$1")
            .bind(job.flow_run_id).bind(succeeded).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn get_application_run_trace_read_status(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        projection_version: i32,
    ) -> Result<Option<domain::ApplicationRunTraceProjectionStatusRecord>> {
        let row = sqlx::query(
            r#"
            select f.id as flow_run_id, $3::integer as projection_version,
                coalesce(s.status, 'pending') as status,
                coalesce(s.source_watermark, '') as source_watermark,
                coalesce(s.attempt_count, 0) as attempt_count,
                s.last_attempt_at, s.last_success_at, s.last_error_code, s.last_error_stage,
                s.last_error_source_kind, s.last_error_source_locator, s.last_error_message,
                s.last_error_ref, coalesce(s.retriable, true) as retriable,
                coalesce(s.created_at, f.created_at) as created_at,
                coalesce(s.updated_at, f.updated_at) as updated_at
            from flow_runs f left join application_run_trace_projection_statuses s
                on s.flow_run_id=f.id and s.projection_version=$3
            where f.application_id=$1 and f.id=$2
        "#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .bind(projection_version)
        .fetch_optional(self.pool())
        .await?;
        row.map(map_application_run_trace_projection_status_record)
            .transpose()
    }
}
