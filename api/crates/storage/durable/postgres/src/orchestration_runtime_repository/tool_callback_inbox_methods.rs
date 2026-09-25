impl PgControlPlaneStore {
    async fn commit_tool_callback_results(
        &self,
        input: &CommitToolCallbackResultsInput,
    ) -> Result<CommitToolCallbackResultsOutput> {
        if input.results.is_empty() {
            return Err(ControlPlaneError::InvalidInput("tool_results").into());
        }
        let mut submitted = std::collections::BTreeSet::new();
        if input
            .results
            .iter()
            .any(|result| !submitted.insert(result.tool_call_id.as_str()))
        {
            return Err(ControlPlaneError::Conflict("tool_callback_result_duplicate").into());
        }

        let mut tx = self.pool().begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(input.callback_task_id.to_string())
            .execute(&mut *tx)
            .await?;
        let callback_row = sqlx::query(
            r#"
            select task.id, task.flow_run_id, task.node_run_id, task.callback_kind, task.status,
                   runtime_original_json(task.request_payload, task.raw_json_payloads, 'request_payload') as request_payload, runtime_original_json(task.response_payload, task.raw_json_payloads, 'response_payload') as response_payload, runtime_original_json(task.external_ref_payload, task.raw_json_payloads, 'external_ref_payload') as external_ref_payload,
                   task.created_at, task.completed_at, run.status as flow_run_status
              from flow_run_callback_tasks task
              join flow_runs run on run.id = task.flow_run_id
              join flow_run_checkpoints checkpoint
                on checkpoint.id = $5 and checkpoint.flow_run_id = run.id
             where task.id = $1 and task.flow_run_id = $2
               and run.application_id = $3 and run.scope_id = $4
             for update of task
            "#,
        )
        .bind(input.callback_task_id)
        .bind(input.flow_run_id)
        .bind(input.application_id)
        .bind(input.scope_id)
        .bind(input.checkpoint_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::Conflict("tool_callback_round_not_owned"))?;
        let flow_run_status: String = callback_row.get("flow_run_status");
        let callback_task = map_callback_task_record(callback_row)?;
        if callback_task.callback_kind != "llm_tool_calls" {
            return Err(ControlPlaneError::Conflict("tool_callback_round_invalid").into());
        }

        if callback_task.status == domain::CallbackTaskStatus::Completed {
            let submitted_continuation = input
                .responses_continuation
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?;
            if callback_task
                .response_payload
                .as_ref()
                .and_then(|payload| payload.get("responses_continuation"))
                != submitted_continuation.as_ref()
            {
                return Err(ControlPlaneError::Conflict("responses_continuation_conflict").into());
            }
            let claim_row = sqlx::query(&format!(
                "select {RESUME_CLAIM_COLUMNS} from flow_run_resume_claims where callback_task_id = $1"
            ))
            .bind(input.callback_task_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ControlPlaneError::Conflict("tool_callback_round_missing_claim"))?;
            let claim = map_resume_claim(&claim_row)?;
            for result in &input.results {
                let fingerprint = sqlx::query_scalar::<_, String>(
                    "select result_fingerprint from flow_run_tool_callback_inbox where callback_task_id = $1 and tool_call_id = $2",
                )
                .bind(input.callback_task_id)
                .bind(&result.tool_call_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(ControlPlaneError::Conflict("tool_callback_result_unknown"))?;
                if fingerprint != result.result_fingerprint {
                    return Err(ControlPlaneError::Conflict("tool_callback_result_conflict").into());
                }
            }
            tx.commit().await?;
            return Ok(CommitToolCallbackResultsOutput {
                callback_task,
                claim: Some(claim),
                disposition: ToolCallbackRoundDisposition::Completed,
            });
        }
        if callback_task.status != domain::CallbackTaskStatus::Pending {
            return Err(ControlPlaneError::Conflict("tool_callback_round_not_pending").into());
        }
        if flow_run_status != "waiting_callback" {
            return Err(ControlPlaneError::Conflict("flow_run_not_waiting_callback").into());
        }

        for result in &input.results {
            let row = sqlx::query(
                r#"
                select status, result_fingerprint
                  from flow_run_tool_callback_inbox
                 where scope_id = $1 and application_id = $2 and flow_run_id = $3
                   and callback_task_id = $4 and tool_call_id = $5
                 for update
                "#,
            )
            .bind(input.scope_id)
            .bind(input.application_id)
            .bind(input.flow_run_id)
            .bind(input.callback_task_id)
            .bind(&result.tool_call_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ControlPlaneError::Conflict("tool_callback_result_unknown"))?;
            let status: String = row.get("status");
            let existing: Option<String> = row.get("result_fingerprint");
            if status == "received" {
                if existing.as_deref() != Some(result.result_fingerprint.as_str()) {
                    return Err(ControlPlaneError::Conflict("tool_callback_result_conflict").into());
                }
                continue;
            }
            sqlx::query(
                r#"
                update flow_run_tool_callback_inbox
                   set status = 'received',
                result_payload = ($6::jsonb -> 0),
                result_fingerprint = $7,
                received_at = now(),
                updated_at = now(),
                raw_json_payloads = (flow_run_tool_callback_inbox.raw_json_payloads - 'result_payload') || jsonb_strip_nulls(jsonb_build_object('result_payload', ($6::jsonb -> 1)))
            where scope_id = $1 and application_id = $2 and flow_run_id = $3
                   and callback_task_id = $4 and tool_call_id = $5 and status = 'pending'
                "#,
            )
            .bind(input.scope_id)
            .bind(input.application_id)
            .bind(input.flow_run_id)
            .bind(input.callback_task_id)
            .bind(&result.tool_call_id)
            .bind(lossless_json_parameter(&(&result.result_payload)))
            .bind(&result.result_fingerprint)
            .execute(&mut *tx)
            .await?;
        }

        let remaining: i64 = sqlx::query_scalar(
            "select count(*) from flow_run_tool_callback_inbox where callback_task_id = $1 and status = 'pending'",
        )
        .bind(input.callback_task_id)
        .fetch_one(&mut *tx)
        .await?;
        if remaining != 0 {
            if input.responses_continuation.is_some() {
                return Err(
                    ControlPlaneError::Conflict("responses_tool_output_incomplete_round").into(),
                );
            }
            tx.commit().await?;
            return Ok(CommitToolCallbackResultsOutput {
                callback_task,
                claim: None,
                disposition: ToolCallbackRoundDisposition::WaitingForResults,
            });
        }

        let result_payloads = sqlx::query_scalar::<_, Value>(
            "select runtime_original_json(result_payload, flow_run_tool_callback_inbox.raw_json_payloads, 'result_payload') as result_payload from flow_run_tool_callback_inbox where callback_task_id = $1 order by tool_ordinal",
        )
        .bind(input.callback_task_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut response_payload = json!({ "tool_results": result_payloads });
        if let Some(continuation) = &input.responses_continuation {
            response_payload["responses_continuation"] = serde_json::to_value(continuation)?;
        }
        let callback_row = sqlx::query(
            r#"
            update flow_run_callback_tasks
               set status = 'completed',
                response_payload = ($2::jsonb -> 0),
                completed_at = now(),
                raw_json_payloads = (flow_run_callback_tasks.raw_json_payloads - 'response_payload') || jsonb_strip_nulls(jsonb_build_object('response_payload', ($2::jsonb -> 1)))
            where id = $1 and status = 'pending'
            returning id, flow_run_id, node_run_id, callback_kind, status, runtime_original_json(request_payload, flow_run_callback_tasks.raw_json_payloads, 'request_payload') as request_payload,
                      runtime_original_json(response_payload, flow_run_callback_tasks.raw_json_payloads, 'response_payload') as response_payload, runtime_original_json(external_ref_payload, flow_run_callback_tasks.raw_json_payloads, 'external_ref_payload') as external_ref_payload, created_at, completed_at
            "#,
        )
        .bind(input.callback_task_id)
        .bind(lossless_json_parameter(&(&response_payload)))
        .fetch_one(&mut *tx)
        .await?;
        let callback_task = map_callback_task_record(callback_row)?;

        let claim_token = Uuid::now_v7();
        let claim_row = sqlx::query(&format!(
            "insert into flow_run_resume_claims (id, scope_id, application_id, flow_run_id, checkpoint_id, callback_task_id, resume_kind, status, request_payload, claim_token, lease_expires_at, raw_json_payloads) values ( $1, $2, $3, $4, $5, $6, 'callback', 'processing', ($7::jsonb -> 0), $8, now() + interval '5 minutes', jsonb_strip_nulls(jsonb_build_object('request_payload', ($7::jsonb -> 1))) ) returning {RESUME_CLAIM_COLUMNS}"
        ))
        .bind(Uuid::now_v7())
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(input.flow_run_id)
        .bind(input.checkpoint_id)
        .bind(input.callback_task_id)
        .bind(lossless_json_parameter(&(&response_payload)))
        .bind(claim_token)
        .fetch_one(&mut *tx)
        .await?;
        let claim = map_resume_claim(&claim_row)?;
        append_resume_claim_running_recovery(
            &mut tx,
            &AcquireResumeClaimInput {
                scope_id: input.scope_id,
                application_id: input.application_id,
                flow_run_id: input.flow_run_id,
                checkpoint_id: input.checkpoint_id,
                callback_task_id: Some(input.callback_task_id),
                kind: ResumeClaimKind::Callback,
                request_payload: response_payload,
            },
            &claim,
        )
        .await?;
        tx.commit().await?;
        Ok(CommitToolCallbackResultsOutput {
            callback_task,
            claim: Some(claim),
            disposition: ToolCallbackRoundDisposition::Acquired,
        })
    }
}
