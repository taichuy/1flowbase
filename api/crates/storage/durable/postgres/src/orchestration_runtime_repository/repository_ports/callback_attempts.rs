use super::*;

#[async_trait]
impl ApplicationPublishedCallbackAttemptRepository for PgControlPlaneStore {
    async fn record_published_callback_resume_attempt(
        &self,
        input: &RecordFlowRunCallbackResumeAttemptInput,
    ) -> Result<RecordFlowRunCallbackResumeAttemptOutput> {
        PgControlPlaneStore::record_flow_run_callback_resume_attempt(self, input).await
    }

    async fn get_published_callback_resume_attempt(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        PgControlPlaneStore::get_flow_run_callback_resume_attempt_by_callback_task(
            self,
            callback_task_id,
        )
        .await
    }

    async fn finish_published_callback_resume_attempt(
        &self,
        input: &FinishFlowRunCallbackResumeAttemptInput,
    ) -> Result<domain::FlowRunCallbackResumeAttemptRecord> {
        PgControlPlaneStore::finish_flow_run_callback_resume_attempt(self, input).await
    }

    async fn claim_published_callback_resume_attempt(
        &self,
        attempt_id: Uuid,
        response_payload: Value,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        PgControlPlaneStore::claim_flow_run_callback_resume_attempt(
            self,
            attempt_id,
            &response_payload,
        )
        .await
    }

    async fn has_active_published_callback_resume_claim(
        &self,
        callback_task_id: Uuid,
    ) -> Result<bool> {
        sqlx::query_scalar(
            "select exists (select 1 from flow_run_resume_claims where callback_task_id = $1 and status = 'processing' and lease_expires_at > now())",
        )
        .bind(callback_task_id)
        .fetch_one(self.pool())
        .await
        .map_err(Into::into)
    }

    async fn reclaim_expired_callback_resume_attempt(
        &self,
        attempt_id: Uuid,
        response_payload: Value,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        let mut tx = self.pool().begin().await?;
        lock_callback_attempt_mutation(&mut tx, attempt_id).await?;
        // Compare restored JSON while holding the attempt lock: PostgreSQL json has
        // no equality operator, and casting to jsonb rejects lossless U+0000 data.
        let stored: Option<Value> = sqlx::query_scalar(
            "select runtime_original_json(response_payload, raw_json_payloads, 'response_payload') from flow_run_callback_resume_attempts where id = $1 for update",
        )
        .bind(attempt_id)
        .fetch_optional(&mut *tx)
        .await?;
        if stored.as_ref() != Some(&response_payload) {
            tx.rollback().await?;
            return Ok(None);
        }
        let row = sqlx::query(r#"
            update flow_run_callback_resume_attempts a set status = 'processing', updated_at = greatest(clock_timestamp(), a.updated_at + interval '1 microsecond'), completed_at = null
            where a.id = $1
              and a.source = 'openai_responses'
              and (a.response_payload ? 'responses_continuation' or exists (
                  select 1 from flow_run_callback_tasks task
                  where task.id = a.callback_task_id
                    and task.callback_kind = 'llm_tool_calls'
                    and task.status = 'completed'
              ))
              and (a.status = 'received' or (a.status = 'processing' and a.updated_at <= now() - interval '5 minutes'))
              and not exists (select 1 from flow_run_resume_claims c where c.callback_task_id = a.callback_task_id
                              and c.status = 'processing' and c.lease_expires_at > now())
            returning a.id, a.flow_run_id, a.callback_task_id, a.source, a.status,
              runtime_original_json(a.response_payload, a.raw_json_payloads, 'response_payload') as response_payload,
              a.idempotency_key, runtime_original_json(a.error_payload, a.raw_json_payloads, 'error_payload') as error_payload,
              a.created_at, a.updated_at, a.completed_at
        "#).bind(attempt_id).fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        row.as_ref()
            .map(map_flow_run_callback_resume_attempt_record)
            .transpose()
    }

    async fn release_unstarted_published_callback_resume_attempt(
        &self,
        attempt_id: Uuid,
        expected_updated_at: OffsetDateTime,
    ) -> Result<bool> {
        self.release_unstarted_flow_run_callback_resume_attempt(attempt_id, expected_updated_at)
            .await
    }

    async fn park_published_callback_resume_attempt(
        &self,
        attempt_id: Uuid,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>> {
        PgControlPlaneStore::park_flow_run_callback_resume_attempt(self, attempt_id).await
    }

    async fn cancel_published_callback_resume_attempts_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<domain::FlowRunCallbackResumeAttemptRecord>> {
        PgControlPlaneStore::cancel_flow_run_callback_resume_attempts_for_run(
            self,
            flow_run_id,
            completed_at,
        )
        .await
    }

    async fn fail_waiting_callback_published_run(
        &self,
        flow_run_id: Uuid,
        error_payload: serde_json::Value,
        finished_at: OffsetDateTime,
    ) -> Result<Option<domain::FlowRunRecord>> {
        PgControlPlaneStore::update_flow_run_if_status(
            self,
            &UpdateFlowRunInput {
                flow_run_id,
                status: domain::FlowRunStatus::Failed,
                output_payload: serde_json::json!({}),
                error_payload: Some(error_payload),
                finished_at: Some(finished_at),
            },
            domain::FlowRunStatus::WaitingCallback,
        )
        .await
    }

    async fn complete_waiting_callback_published_internal_run(
        &self,
        flow_run_id: Uuid,
        output_payload: serde_json::Value,
        finished_at: OffsetDateTime,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let completed = PgControlPlaneStore::update_flow_run_if_status(
            self,
            &UpdateFlowRunInput {
                flow_run_id,
                status: domain::FlowRunStatus::Succeeded,
                output_payload: output_payload.clone(),
                error_payload: None,
                finished_at: Some(finished_at),
            },
            domain::FlowRunStatus::WaitingCallback,
        )
        .await?;

        if completed.is_some() {
            sqlx::query(
                r#"
                update node_run_records
                set status = 'succeeded',
                output_payload = ($2::jsonb -> 0),
                error_payload = null,
                finished_at = $3,
                raw_json_payloads = (node_run_records.raw_json_payloads - 'output_payload' - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('output_payload', ($2::jsonb -> 1), 'error_payload', null))
            where flow_run_id = $1
                  and status = 'waiting_callback'
                "#,
            )
            .bind(flow_run_id)
            .bind(lossless_json_parameter(&(&output_payload)))
            .bind(finished_at)
            .execute(self.pool())
            .await?;
        }

        Ok(completed)
    }
}
