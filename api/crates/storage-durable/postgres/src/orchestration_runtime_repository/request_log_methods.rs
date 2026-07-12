impl PgControlPlaneStore {
    pub async fn insert_model_provider_request_logs_batch(
        &self,
        records: &[control_plane::ports::ProviderRequestLogTask],
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let mut query = QueryBuilder::<Postgres>::new(
            "insert into model_provider_request_logs (id, scope_id, attempt_id, flow_run_id, application_id, conversation_id, application_name, attempt_index, is_retry, retry_reason, provider_instance_id, provider_instance_display_name, provider_code, protocol, upstream_model_id, reasoning_effort, status, error_code, failed_after_first_token, input_tokens, output_tokens, total_tokens, started_at, first_token_at, finished_at, time_to_first_token_ms, total_duration_ms, created_at) ",
        );
        query.push_values(records, |mut row, record| {
            row.push_bind(Uuid::now_v7())
                .push_bind(record.scope_id)
                .push_bind(record.attempt_id)
                .push_bind(record.flow_run_id)
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
}
