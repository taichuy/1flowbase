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
            ) values (
                $1,
                (
                    select applications.workspace_id
                    from flow_runs
                    join applications on applications.id = flow_runs.application_id
                    where flow_runs.id = $2
                ),
                $2,
                $3,
                $4,
                'processing',
                $5,
                $6
            )
            on conflict (callback_task_id) do nothing
            returning
                id,
                flow_run_id,
                callback_task_id,
                source,
                status,
                response_payload,
                idempotency_key,
                error_payload,
                created_at,
                updated_at,
                completed_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.callback_task_id)
        .bind(&input.source)
        .bind(&input.response_payload)
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
            .ok_or(ControlPlaneError::Conflict("callback_resume_attempt_missing"))?;
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
                response_payload,
                idempotency_key,
                error_payload,
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
                error_payload = case when status = 'processing' then $3 else error_payload end,
                completed_at = coalesce(completed_at, $4),
                updated_at = now()
            where id = $1
              and status in ('processing', 'cancelled')
            returning
                id,
                flow_run_id,
                callback_task_id,
                source,
                status,
                response_payload,
                idempotency_key,
                error_payload,
                created_at,
                updated_at,
                completed_at
            "#,
        )
        .bind(input.attempt_id)
        .bind(input.status.as_str())
        .bind(&input.error_payload)
        .bind(input.completed_at)
        .fetch_optional(self.pool())
        .await?;

        let Some(row) = row else {
            return Err(ControlPlaneError::Conflict("callback_resume_attempt_not_processing").into());
        };

        map_flow_run_callback_resume_attempt_record(&row)
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
                response_payload,
                idempotency_key,
                error_payload,
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
