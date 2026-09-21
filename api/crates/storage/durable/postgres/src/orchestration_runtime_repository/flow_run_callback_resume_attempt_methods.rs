impl PgControlPlaneStore {
    pub async fn record_flow_run_callback_resume_attempt(
        &self,
        input: &RecordFlowRunCallbackResumeAttemptInput,
    ) -> Result<RecordFlowRunCallbackResumeAttemptOutput> {
        let inserted = sqlx::query(
            r#"
            insert into flow_run_callback_resume_attempts (
                id,
                scope_id,
                flow_run_id,
                callback_task_id,
                source,
                status,
                response_payload,
                idempotency_key
            , raw_json_payloads) values ( $1, (
                    select applications.workspace_id
                    from flow_runs
                    join applications on applications.id = flow_runs.application_id
                    where flow_runs.id = $2
                ), $2, $3, $4, 'processing', ($5::jsonb -> 0), $6, jsonb_strip_nulls(jsonb_build_object('response_payload', ($5::jsonb -> 1))) )
            on conflict (callback_task_id) do nothing
            returning
                id,
                flow_run_id,
                callback_task_id,
                source,
                status,
                runtime_original_json(response_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'response_payload') as response_payload,
                idempotency_key,
                runtime_original_json(error_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'error_payload') as error_payload,
                created_at,
                updated_at,
                completed_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.callback_task_id)
        .bind(&input.source)
        .bind(lossless_json_parameter(&(&input.response_payload)))
        .bind(&input.idempotency_key)
        .fetch_optional(self.pool())
        .await?;

        if let Some(row) = inserted {
            return Ok(RecordFlowRunCallbackResumeAttemptOutput {
                attempt: map_flow_run_callback_resume_attempt_record(&row)?,
                inserted: true,
            });
        }

        let existing = self
            .get_flow_run_callback_resume_attempt_by_callback_task(input.callback_task_id)
            .await?
            .ok_or(ControlPlaneError::Conflict(
                "callback_resume_attempt_missing",
            ))?;
        Ok(RecordFlowRunCallbackResumeAttemptOutput {
            attempt: existing,
            inserted: false,
        })
    }

    pub async fn get_flow_run_callback_resume_attempt_by_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                callback_task_id,
                source,
                status,
                runtime_original_json(response_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'response_payload') as response_payload,
                idempotency_key,
                runtime_original_json(error_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'error_payload') as error_payload,
                created_at,
                updated_at,
                completed_at
            from flow_run_callback_resume_attempts
            where callback_task_id = $1
            "#,
        )
        .bind(callback_task_id)
        .fetch_optional(self.pool())
        .await?;

        row.as_ref()
            .map(map_flow_run_callback_resume_attempt_record)
            .transpose()
    }

    pub async fn finish_flow_run_callback_resume_attempt(
        &self,
        input: &FinishFlowRunCallbackResumeAttemptInput,
    ) -> Result<domain::FlowRunCallbackResumeAttemptRecord> {
        let row = sqlx::query(
            r#"
            update flow_run_callback_resume_attempts
            set status = case when status = 'processing' then $2 else status end,
                error_payload = case when status = 'processing' then ($3::jsonb -> 0) else error_payload end,
                completed_at = coalesce(completed_at, $4),
                updated_at = now(),
                raw_json_payloads = (flow_run_callback_resume_attempts.raw_json_payloads - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('error_payload', case when status = 'processing' then ($3::jsonb -> 1) else flow_run_callback_resume_attempts.raw_json_payloads -> 'error_payload' end))
            where id = $1
              and (status in ('processing', 'cancelled') or status = $2)
            returning
                id,
                flow_run_id,
                callback_task_id,
                source,
                status,
                runtime_original_json(response_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'response_payload') as response_payload,
                idempotency_key,
                runtime_original_json(error_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'error_payload') as error_payload,
                created_at,
                updated_at,
                completed_at
            "#,
        )
        .bind(input.attempt_id)
        .bind(input.status.as_str())
        .bind(lossless_json_parameter(&(&input.error_payload)))
        .bind(input.completed_at)
        .fetch_optional(self.pool())
        .await?;

        let Some(row) = row else {
            return Err(
                ControlPlaneError::Conflict("callback_resume_attempt_not_processing").into(),
            );
        };

        map_flow_run_callback_resume_attempt_record(&row)
    }

    async fn transition_flow_run_callback_resume_attempt(
        &self,
        attempt_id: Uuid,
        from_status: &str,
        to_status: &str,
        response_payload: Option<&Value>,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        let row = sqlx::query(
            r#"
            update flow_run_callback_resume_attempts
            set status = $3,
                response_payload = coalesce(($4::jsonb -> 0), response_payload),
                updated_at = now(),
                raw_json_payloads = (flow_run_callback_resume_attempts.raw_json_payloads - 'response_payload') || jsonb_strip_nulls(jsonb_build_object('response_payload', coalesce(($4::jsonb -> 1), flow_run_callback_resume_attempts.raw_json_payloads -> 'response_payload')))
            where id = $1
              and status = $2
            returning
                id,
                flow_run_id,
                callback_task_id,
                source,
                status,
                runtime_original_json(response_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'response_payload') as response_payload,
                idempotency_key,
                runtime_original_json(error_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'error_payload') as error_payload,
                created_at,
                updated_at,
                completed_at
            "#,
        )
        .bind(attempt_id)
        .bind(from_status)
        .bind(to_status)
        .bind(lossless_json_parameter(&(response_payload)))
        .fetch_optional(self.pool())
        .await?;
        row.as_ref()
            .map(map_flow_run_callback_resume_attempt_record)
            .transpose()
    }

    pub async fn claim_flow_run_callback_resume_attempt(
        &self,
        attempt_id: Uuid,
        response_payload: &Value,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        self.transition_flow_run_callback_resume_attempt(
            attempt_id,
            "received",
            "processing",
            Some(response_payload),
        )
        .await
    }

    pub async fn park_flow_run_callback_resume_attempt(
        &self,
        attempt_id: Uuid,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        self.transition_flow_run_callback_resume_attempt(attempt_id, "processing", "received", None)
            .await
    }

    pub async fn cancel_flow_run_callback_resume_attempts_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<domain::FlowRunCallbackResumeAttemptRecord>> {
        let rows = sqlx::query(
            r#"
            update flow_run_callback_resume_attempts
            set status = 'cancelled',
                completed_at = $2,
                updated_at = now()
            where flow_run_id = $1
              and status in ('received', 'processing')
            returning
                id,
                flow_run_id,
                callback_task_id,
                source,
                status,
                runtime_original_json(response_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'response_payload') as response_payload,
                idempotency_key,
                runtime_original_json(error_payload, flow_run_callback_resume_attempts.raw_json_payloads, 'error_payload') as error_payload,
                created_at,
                updated_at,
                completed_at
            "#,
        )
        .bind(flow_run_id)
        .bind(completed_at)
        .fetch_all(self.pool())
        .await?;

        rows.iter()
            .map(map_flow_run_callback_resume_attempt_record)
            .collect()
    }
}
