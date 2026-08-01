impl PgControlPlaneStore {
    pub async fn insert_model_provider_request_logs_batch(
        &self,
        records: &[control_plane::ports::ProviderRequestLogTask],
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let mut query = QueryBuilder::<Postgres>::new(
            "insert into model_provider_request_logs (id, scope_id, attempt_id, flow_run_id, node_run_id, application_id, conversation_id, application_name, attempt_index, is_retry, retry_reason, provider_instance_id, provider_instance_display_name, provider_code, protocol, upstream_model_id, reasoning_effort, status, error_code, failed_after_first_token, input_tokens, output_tokens, total_tokens, started_at, first_token_at, finished_at, time_to_first_token_ms, total_duration_ms, created_at) ",
        );
        query.push_values(records, |mut row, record| {
            row.push_bind(Uuid::now_v7())
                .push_bind(record.scope_id)
                .push_bind(record.attempt_id)
                .push_bind(record.flow_run_id)
                .push_bind(record.node_run_id)
                .push_bind(record.application_id)
                .push_bind(&record.conversation_id)
                .push_bind(&record.application_name)
                .push_bind(record.attempt_index)
                .push_bind(record.is_retry)
                .push_bind(&record.retry_reason)
                .push_bind(record.provider_instance_id)
                .push_bind(&record.provider_instance_display_name)
                .push_bind(&record.provider_code)
                .push_bind(&record.protocol)
                .push_bind(&record.upstream_model_id)
                .push_bind(&record.reasoning_effort)
                .push_bind(&record.status)
                .push_bind(&record.error_code)
                .push_bind(record.failed_after_first_token)
                .push_bind(record.input_tokens)
                .push_bind(record.output_tokens)
                .push_bind(record.total_tokens)
                .push_bind(record.started_at)
                .push_bind(record.first_token_at)
                .push_bind(record.finished_at)
                .push_bind(record.time_to_first_token_ms)
                .push_bind(record.total_duration_ms)
                .push_bind(OffsetDateTime::now_utc());
        });
        query.push(" on conflict (attempt_id) do nothing");
        query.build().execute(self.pool()).await?;
        Ok(())
    }

    pub async fn delete_model_provider_request_logs(
        &self,
        input: control_plane::ports::DeleteModelProviderRequestLogsInput,
    ) -> Result<u64> {
        if input.attempt_ids.is_empty() {
            return Ok(0);
        }
        if input.attempt_ids.len()
            > control_plane::ports::MODEL_PROVIDER_REQUEST_LOG_DELETE_BATCH_LIMIT
        {
            return Err(
                control_plane::errors::ControlPlaneError::InvalidInput("attempt_ids").into(),
            );
        }

        let result = sqlx::query(
            "delete from model_provider_request_logs where scope_id = $1 and attempt_id = any($2)",
        )
        .bind(input.scope_id)
        .bind(input.attempt_ids)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn clear_model_provider_request_logs_batch(
        &self,
        input: control_plane::ports::ClearModelProviderRequestLogsBatchInput,
    ) -> Result<control_plane::ports::ClearModelProviderRequestLogsBatchResult> {
        let candidate_limit =
            (control_plane::ports::MODEL_PROVIDER_REQUEST_LOG_DELETE_BATCH_LIMIT + 1) as i64;
        let delete_limit =
            control_plane::ports::MODEL_PROVIDER_REQUEST_LOG_DELETE_BATCH_LIMIT as i64;
        let row = sqlx::query(
            r#"
            with snapshot as (
                select coalesce($2, statement_timestamp()) as created_before
            ), candidates as (
                select id
                from model_provider_request_logs, snapshot
                where scope_id = $1 and created_at <= snapshot.created_before
                order by created_at asc, id asc
                limit $3
            ), to_delete as (
                select id from candidates limit $4
            ), deleted as (
                delete from model_provider_request_logs logs
                using to_delete
                where logs.id = to_delete.id
                returning logs.id
            )
            select
                (select count(*) from deleted) as deleted_count,
                (select count(*) from candidates) > $4 as has_more,
                (select created_before from snapshot) as snapshot_created_before
            "#,
        )
        .bind(input.scope_id)
        .bind(input.snapshot_created_before)
        .bind(candidate_limit)
        .bind(delete_limit)
        .fetch_one(self.pool())
        .await?;

        Ok(
            control_plane::ports::ClearModelProviderRequestLogsBatchResult {
                deleted_count: row.try_get::<i64, _>("deleted_count")? as u64,
                has_more: row.try_get("has_more")?,
                snapshot_created_before: row.try_get("snapshot_created_before")?,
            },
        )
    }
}
