impl PgControlPlaneStore {
    async fn upsert_compiled_plan(
        &self,
        input: &UpsertCompiledPlanInput,
    ) -> Result<domain::CompiledPlanRecord> {
        let row = sqlx::query(
            r#"
            insert into flow_compiled_plans (
                id,
                flow_id,
                flow_draft_id,
                schema_version,
                document_hash,
                document_updated_at,
                plan,
                scope_id,
                created_by,
                updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, (select scope_id from flows where id = $2), $8, $8)
            returning
                id,
                flow_id,
                flow_draft_id,
                schema_version,
                document_hash,
                document_updated_at,
                plan,
                created_by,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_id)
        .bind(input.flow_draft_id)
        .bind(&input.schema_version)
        .bind(&input.document_hash)
        .bind(input.document_updated_at)
        .bind(&input.plan)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_compiled_plan_record(row)
    }

    async fn get_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> Result<Option<domain::CompiledPlanRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                flow_id,
                flow_draft_id,
                schema_version,
                document_hash,
                document_updated_at,
                plan,
                created_by,
                created_at,
                updated_at
            from flow_compiled_plans
            where id = $1
            "#,
        )
        .bind(compiled_plan_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_compiled_plan_record).transpose()
    }

    async fn create_flow_run(&self, input: &CreateFlowRunInput) -> Result<domain::FlowRunRecord> {
        let row = sqlx::query(
            r#"
            insert into flow_runs (
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                input_payload,
                api_key_id,
                publication_version_id,
                assistant_conversation_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                created_by,
                started_at,
                updated_at
            , raw_json_payloads) values ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, ($13::jsonb -> 0), $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, jsonb_strip_nulls(jsonb_build_object('input_payload', ($13::jsonb -> 1))) )
            returning
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                runtime_original_json(input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,
                created_by,
                null::text as authorized_account,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.application_id)
        .bind(input.flow_id)
        .bind(input.flow_draft_id)
        .bind(input.compiled_plan_id)
        .bind(&input.debug_session_id)
        .bind(&input.flow_schema_version)
        .bind(&input.document_hash)
        .bind(input.run_mode.as_str())
        .bind(input.target_node_id.as_deref())
        .bind(&input.title)
        .bind(input.status.as_str())
        .bind(lossless_json_parameter(&(&input.input_payload)))
        .bind(input.api_key_id)
        .bind(input.publication_version_id)
        .bind(input.assistant_conversation_id)
        .bind(input.external_user.as_deref())
        .bind(input.external_conversation_id.as_deref())
        .bind(input.external_trace_id.as_deref())
        .bind(input.compatibility_mode.as_deref())
        .bind(input.idempotency_key.as_deref())
        .bind(input.actor_user_id)
        .bind(input.started_at)
        .bind(input.started_at)
        .fetch_one(self.pool())
        .await?;

        let flow_run = map_flow_run_record(row)?;
        if let Some(conversation_id) = input.assistant_conversation_id {
            sqlx::query(
                "update assistant_conversations set updated_at = $2 where conversation_id = $1",
            )
            .bind(conversation_id)
            .bind(flow_run.started_at)
            .execute(self.pool())
            .await?;
        }
        if matches!(
            flow_run.run_mode,
            domain::FlowRunMode::PublishedApiRun
                | domain::FlowRunMode::AssistantExecution
                | domain::FlowRunMode::WorkflowHttpRun
        ) {
            self.upsert_application_run_log_summary_for_flow_run(&flow_run)
                .await?;
        }
        Ok(flow_run)
    }

    async fn create_published_flow_run(
        &self,
        input: &CreateFlowRunInput,
    ) -> Result<CreatePublishedFlowRunResult> {
        self.create_published_flow_run_inner(input, false).await
    }

    async fn create_published_flow_run_superseding_callback_predecessors(
        &self,
        input: &CreateFlowRunInput,
    ) -> Result<CreatePublishedFlowRunResult> {
        self.create_published_flow_run_inner(input, true).await
    }

    async fn create_published_flow_run_inner(
        &self,
        input: &CreateFlowRunInput,
        supersede_callback_predecessors: bool,
    ) -> Result<CreatePublishedFlowRunResult> {
        let conflict_target = match input.run_mode {
            domain::FlowRunMode::WorkflowScheduleRun => {
                "(application_id, idempotency_key)\n                where run_mode = 'workflow_schedule_run'\n                  and idempotency_key is not null"
            }
            _ => {
                "(application_id, api_key_id, idempotency_key)\n                where run_mode = 'published_api_run'\n                  and api_key_id is not null\n                  and idempotency_key is not null"
            }
        };
        let query = format!(
            r#"
            insert into flow_runs (
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                input_payload,
                api_key_id,
                publication_version_id,
                assistant_conversation_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                created_by,
                started_at,
                updated_at
            , raw_json_payloads) values ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, ($13::jsonb -> 0), $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, jsonb_strip_nulls(jsonb_build_object('input_payload', ($13::jsonb -> 1))) )
            on conflict {conflict_target} do nothing
            returning
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                runtime_original_json(input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,
                created_by,
                null::text as authorized_account,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at
            "#,
        );
        let mut tx = self.pool().begin().await?;
        // SQL safety: The conflict target is selected from literal clauses by FlowRunMode; run data remain bound.
        let inserted = sqlx::query(sqlx::AssertSqlSafe(query.as_str()))
            .bind(Uuid::now_v7())
            .bind(input.application_id)
            .bind(input.flow_id)
            .bind(input.flow_draft_id)
            .bind(input.compiled_plan_id)
            .bind(&input.debug_session_id)
            .bind(&input.flow_schema_version)
            .bind(&input.document_hash)
            .bind(input.run_mode.as_str())
            .bind(input.target_node_id.as_deref())
            .bind(&input.title)
            .bind(input.status.as_str())
            .bind(lossless_json_parameter(&(&input.input_payload)))
            .bind(input.api_key_id)
            .bind(input.publication_version_id)
            .bind(input.assistant_conversation_id)
            .bind(input.external_user.as_deref())
            .bind(input.external_conversation_id.as_deref())
            .bind(input.external_trace_id.as_deref())
            .bind(input.compatibility_mode.as_deref())
            .bind(input.idempotency_key.as_deref())
            .bind(input.actor_user_id)
            .bind(input.started_at)
            .bind(input.started_at)
            .fetch_optional(&mut *tx)
            .await?;

        let (flow_run, created) = if let Some(row) = inserted {
            {
                let run = map_flow_run_record(row)?;
                if let Some(context) = &input.application_run_log_context {
                    Self::bind_application_run_log_context(&mut tx, &run, context).await?;
                    if supersede_callback_predecessors {
                        Self::supersede_callback_predecessors_in_transaction(
                            &mut tx,
                            &run,
                            context,
                            input.started_at,
                        )
                        .await?;
                    }
                }
                tx.commit().await?;
                (run, true)
            }
        } else {
            tx.commit().await?;
            let conflict_api_key = match input.run_mode {
                domain::FlowRunMode::WorkflowScheduleRun => None,
                _ => input.api_key_id,
            };
            let flow_run = self
                .find_published_flow_run_by_idempotency_key(
                    input.application_id,
                    conflict_api_key,
                    input.idempotency_key.as_deref().ok_or_else(|| {
                        anyhow!("idempotency conflict without an idempotency key")
                    })?,
                )
                .await?
                .ok_or_else(|| anyhow!("idempotency conflict canonical flow run is unavailable"))?;
            (flow_run, false)
        };
        if created {
            if let Some(conversation_id) = input.assistant_conversation_id {
                sqlx::query(
                    "update assistant_conversations set updated_at = $2 where conversation_id = $1",
                )
                .bind(conversation_id)
                .bind(flow_run.started_at)
                .execute(self.pool())
                .await?;
            }
            self.upsert_application_run_log_summary_for_flow_run(&flow_run)
                .await?;
        }
        Ok(CreatePublishedFlowRunResult { flow_run, created })
    }

    async fn list_local_summary_superseded_flow_run_ids(
        &self,
        successor_flow_run_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        sqlx::query_scalar(
            r#"
            select id
            from flow_runs
            where status='cancelled'
              and error_payload->>'reason'='local_summary_superseded'
              and error_payload->>'successor_flow_run_id'=$1
            order by id
            "#,
        )
        .bind(successor_flow_run_id.to_string())
        .fetch_all(self.pool())
        .await
        .map_err(Into::into)
    }

    async fn create_flow_run_shell(
        &self,
        input: &CreateFlowRunShellInput,
    ) -> Result<domain::FlowRunRecord> {
        let row = sqlx::query(
            r#"
            insert into flow_runs (
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                input_payload,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                created_by,
                started_at,
                updated_at
            , raw_json_payloads) values ( $1, $2, $3, $4, null, $5, $6, $7, $8, $9, $10, $11, ($12::jsonb -> 0), $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, jsonb_strip_nulls(jsonb_build_object('input_payload', ($12::jsonb -> 1))) )
            returning
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                runtime_original_json(input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,
                created_by,
                null::text as authorized_account,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.application_id)
        .bind(input.flow_id)
        .bind(input.flow_draft_id)
        .bind(&input.debug_session_id)
        .bind(&input.flow_schema_version)
        .bind(&input.document_hash)
        .bind(input.run_mode.as_str())
        .bind(input.target_node_id.as_deref())
        .bind(&input.title)
        .bind(input.status.as_str())
        .bind(lossless_json_parameter(&(&input.input_payload)))
        .bind(input.api_key_id)
        .bind(input.publication_version_id)
        .bind(input.external_user.as_deref())
        .bind(input.external_conversation_id.as_deref())
        .bind(input.external_trace_id.as_deref())
        .bind(input.compatibility_mode.as_deref())
        .bind(input.idempotency_key.as_deref())
        .bind(input.actor_user_id)
        .bind(input.started_at)
        .bind(input.started_at)
        .fetch_one(self.pool())
        .await?;

        let flow_run = map_flow_run_record(row)?;
        Ok(flow_run)
    }

    async fn attach_compiled_plan_to_flow_run(
        &self,
        input: &AttachCompiledPlanToFlowRunInput,
    ) -> Result<domain::FlowRunRecord> {
        let row = sqlx::query(
            r#"
            update flow_runs
            set compiled_plan_id = $2,
                status = $3,
                updated_at = now()
            from flow_compiled_plans compiled
            where flow_runs.id = $1
              and compiled.id = $2
              and flow_runs.status = 'queued'
              and flow_runs.compiled_plan_id is null
              and flow_runs.flow_schema_version = $4
              and flow_runs.document_hash = $5
              and compiled.flow_id = flow_runs.flow_id
              and compiled.flow_draft_id = flow_runs.flow_draft_id
              and compiled.schema_version = flow_runs.flow_schema_version
              and compiled.document_hash = flow_runs.document_hash
            returning
                flow_runs.id,
                flow_runs.application_id,
                flow_runs.flow_id,
                flow_runs.flow_draft_id,
                flow_runs.compiled_plan_id,
                flow_runs.debug_session_id,
                flow_runs.flow_schema_version,
                flow_runs.document_hash,
                flow_runs.run_mode,
                flow_runs.target_node_id,
                flow_runs.title,
                flow_runs.status,
                runtime_original_json(flow_runs.input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(flow_runs.output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(flow_runs.error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,
                flow_runs.created_by,
                null::text as authorized_account,
                flow_runs.api_key_id,
                flow_runs.publication_version_id,
                flow_runs.external_user,
                flow_runs.external_conversation_id,
                flow_runs.external_trace_id,
                flow_runs.compatibility_mode,
                flow_runs.idempotency_key,
                flow_runs.started_at,
                flow_runs.finished_at,
                flow_runs.created_at,
                flow_runs.updated_at
            "#,
        )
        .bind(input.flow_run_id)
        .bind(input.compiled_plan_id)
        .bind(input.status.as_str())
        .bind(&input.flow_schema_version)
        .bind(&input.document_hash)
        .fetch_optional(self.pool())
        .await?
        .ok_or_else(|| anyhow!("flow run compiled plan cannot be attached"))?;

        let flow_run = map_flow_run_record(row)?;
        Ok(flow_run)
    }

    async fn fail_queued_flow_run_shell(
        &self,
        input: &FailQueuedFlowRunShellInput,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let row = sqlx::query(
            r#"
            update flow_runs
            set status = 'failed',
                output_payload = ($2::jsonb -> 0),
                error_payload = ($3::jsonb -> 0),
                finished_at = $4,
                updated_at = $4,
                raw_json_payloads = (flow_runs.raw_json_payloads - 'output_payload' - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('output_payload', ($2::jsonb -> 1), 'error_payload', ($3::jsonb -> 1)))
            where id = $1
              and status = 'queued'
              and compiled_plan_id is null
            returning
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                runtime_original_json(input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,
                created_by,
                null::text as authorized_account,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at
            "#,
        )
        .bind(input.flow_run_id)
        .bind(lossless_json_parameter(&(&input.output_payload)))
        .bind(lossless_json_parameter(&(&input.error_payload)))
        .bind(input.finished_at)
        .fetch_optional(self.pool())
        .await?;

        let flow_run = row.map(map_flow_run_record).transpose()?;
        if let Some(flow_run) = &flow_run {
            self.upsert_application_run_log_summary_for_flow_run(flow_run)
                .await?;
        }
        Ok(flow_run)
    }

    async fn get_flow_run(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>> {
        fetch_flow_run_for_application(self, application_id, flow_run_id).await
    }

    async fn find_published_flow_run_by_idempotency_key(
        &self,
        application_id: Uuid,
        api_key_id: Option<Uuid>,
        idempotency_key: &str,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                runtime_original_json(input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,
                created_by,
                null::text as authorized_account,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at
            from flow_runs
            where application_id = $1
              and api_key_id is not distinct from $2
              and idempotency_key = $3
              and run_mode in ('published_api_run', 'workflow_schedule_run')
            order by created_at asc, id asc
            limit 1
            "#,
        )
        .bind(application_id)
        .bind(api_key_id)
        .bind(idempotency_key)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_flow_run_record).transpose()
    }

    async fn update_flow_run(&self, input: &UpdateFlowRunInput) -> Result<domain::FlowRunRecord> {
        let mut tx = self.pool().begin().await?;
        lock_application_run_native_projection(&mut tx, input.flow_run_id).await?;
        let row = sqlx::query(
            r#"
            update flow_runs
            set status = $2,
                output_payload = ($3::jsonb -> 0),
                error_payload = ($4::jsonb -> 0),
                finished_at = $5,
                updated_at = coalesce($5, now()),
                raw_json_payloads = (flow_runs.raw_json_payloads - 'output_payload' - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('output_payload', ($3::jsonb -> 1), 'error_payload', ($4::jsonb -> 1)))
            where id = $1
            returning
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                runtime_original_json(input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,
                created_by,
                null::text as authorized_account,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at
            "#,
        )
        .bind(input.flow_run_id)
        .bind(input.status.as_str())
        .bind(lossless_json_parameter(&(&input.output_payload)))
        .bind(lossless_json_parameter(&(&input.error_payload)))
        .bind(input.finished_at)
        .fetch_one(&mut *tx)
        .await?;

        let flow_run = map_flow_run_record(row)?;
        append_flow_run_recovery_state_in_transaction(&mut tx, &flow_run).await?;
        Self::upsert_application_run_log_summary_projection_for_flow_run(&mut tx, &flow_run)
            .await?;
        Self::ensure_application_run_conversation_message_items_projection(&mut tx, &flow_run)
            .await?;
        Self::refresh_application_run_log_task_for_flow_run(&mut tx, flow_run.id).await?;
        tx.commit().await?;

        if is_terminal_application_run_log_status(flow_run.status) {
            self.upsert_application_conversation_messages_for_flow_run(&flow_run)
                .await?;
        }

        Ok(flow_run)
    }

    async fn update_flow_run_if_status(
        &self,
        input: &UpdateFlowRunInput,
        expected_status: domain::FlowRunStatus,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let mut tx = self.pool().begin().await?;
        lock_application_run_native_projection(&mut tx, input.flow_run_id).await?;
        let row = sqlx::query(
            r#"
            update flow_runs
            set status = $2,
                output_payload = ($3::jsonb -> 0),
                error_payload = ($4::jsonb -> 0),
                finished_at = $5,
                updated_at = coalesce($5, now()),
                raw_json_payloads = (flow_runs.raw_json_payloads - 'output_payload' - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('output_payload', ($3::jsonb -> 1), 'error_payload', ($4::jsonb -> 1)))
            where id = $1
              and status = $6
            returning
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                runtime_original_json(input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,
                created_by,
                null::text as authorized_account,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at
            "#,
        )
        .bind(input.flow_run_id)
        .bind(input.status.as_str())
        .bind(lossless_json_parameter(&(&input.output_payload)))
        .bind(lossless_json_parameter(&(&input.error_payload)))
        .bind(input.finished_at)
        .bind(expected_status.as_str())
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = row {
            let flow_run = map_flow_run_record(row)?;
            append_flow_run_recovery_state_in_transaction(&mut tx, &flow_run).await?;
            Self::upsert_application_run_log_summary_projection_for_flow_run(&mut tx, &flow_run)
                .await?;
            Self::ensure_application_run_conversation_message_items_projection(&mut tx, &flow_run)
                .await?;
            Self::refresh_application_run_log_task_for_flow_run(&mut tx, flow_run.id).await?;
            tx.commit().await?;
            if is_terminal_application_run_log_status(flow_run.status) {
                self.upsert_application_conversation_messages_for_flow_run(&flow_run)
                    .await?;
            }
            return Ok(Some(flow_run));
        }

        tx.rollback().await?;
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            select exists(select 1 from flow_runs where id = $1)
            "#,
        )
        .bind(input.flow_run_id)
        .fetch_one(self.pool())
        .await?;
        if !exists {
            return Err(ControlPlaneError::NotFound("flow_run").into());
        }

        Ok(None)
    }

    async fn commit_flow_run_terminal(
        &self,
        input: &CommitFlowRunTerminalInput,
    ) -> Result<CommitFlowRunTerminalReceipt> {
        let mut tx = self.pool().begin().await?;
        lock_application_run_native_projection(&mut tx, input.flow_run_id).await?;
        let error_payload = input.result.error_payload().cloned();
        let row = sqlx::query(
            r#"
            update flow_runs
            set status = $2,
                output_payload = ($3::jsonb -> 0),
                error_payload = ($4::jsonb -> 0),
                finished_at = $5,
                updated_at = $5,
                raw_json_payloads = (flow_runs.raw_json_payloads - 'output_payload' - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('output_payload', ($3::jsonb -> 1), 'error_payload', ($4::jsonb -> 1)))
            where id = $1
              and status = $6
              and status not in ('succeeded', 'incomplete', 'failed', 'cancelled')
            returning
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                runtime_original_json(input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,
                created_by,
                null::text as authorized_account,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at
            "#,
        )
        .bind(input.flow_run_id)
        .bind(input.result.status().as_str())
        .bind(lossless_json_parameter(&(input.result.output_payload())))
        .bind(lossless_json_parameter(&(&error_payload)))
        .bind(input.finished_at)
        .bind(input.expected_status.as_str())
        .fetch_optional(&mut *tx)
        .await?;

        let Some(row) = row else {
            tx.rollback().await?;
            let exists = sqlx::query_scalar::<_, bool>(
                r#"
                select exists(select 1 from flow_runs where id = $1)
                "#,
            )
            .bind(input.flow_run_id)
            .fetch_one(self.pool())
            .await?;
            if !exists {
                return Err(ControlPlaneError::NotFound("flow_run").into());
            }
            return Ok(CommitFlowRunTerminalReceipt::Loser);
        };

        let flow_run = map_flow_run_record(row)?;
        if flow_run.status == domain::FlowRunStatus::Cancelled {
            sqlx::query(
                r#"
                update node_runs
                set status = 'cancelled',
                    finished_at = coalesce(finished_at, $2)
                where flow_run_id = $1
                  and status not in ('succeeded', 'failed', 'cancelled', 'skipped')
                "#,
            )
            .bind(flow_run.id)
            .bind(input.finished_at)
            .execute(&mut *tx)
            .await?;
        }
        Self::upsert_application_run_log_summary_projection_for_flow_run(&mut tx, &flow_run)
            .await?;
        // Copy the synchronous billing result once. Request-log retention and async
        // delivery must not affect the task's recorded cost.
        sqlx::query(include_str!("../application_run_logs/cost_snapshot.sql"))
            .bind(flow_run.id)
            .execute(&mut *tx)
            .await?;
        Self::replace_application_run_conversation_message_items_projection(&mut tx, &flow_run)
            .await?;
        Self::refresh_application_run_log_task_for_flow_run(&mut tx, flow_run.id).await?;

        let scope_id = flow_run_scope_id_for_update(&mut tx, flow_run.id).await?;
        let flow_event_sequence = next_event_sequence(&mut tx, flow_run.id).await?;
        let resume_timeline_description =
            resume_timeline_description(&input.flow_run_event_payload);
        sqlx::query(
            r#"
            insert into flow_run_events (
                id,
                scope_id,
                flow_run_id,
                node_run_id,
                sequence,
                event_type,
                payload,
                resume_timeline_description,
                resume_timeline_description_projected
            , raw_json_payloads) values ( $1, $2, $3, null, $4, $5, ($6::jsonb -> 0), $7, true, jsonb_strip_nulls(jsonb_build_object('payload', ($6::jsonb -> 1))) )
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(scope_id)
        .bind(flow_run.id)
        .bind(flow_event_sequence)
        .bind(input.result.flow_run_event_type())
        .bind(lossless_json_parameter(&(&input.flow_run_event_payload)))
        .bind(resume_timeline_description)
        .execute(&mut *tx)
        .await?;

        let runtime_event_sequence = next_runtime_event_sequence(&mut tx, flow_run.id).await?;
        sqlx::query(
            r#"
            insert into runtime_events (
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
                payload,
                visibility,
                durability
            , raw_json_payloads) values ( $1, $2, null, null, null, $3, $4, 'agent_transition', 'host', 'host_fact', null, null, ($5::jsonb -> 0), 'workspace', 'durable', jsonb_strip_nulls(jsonb_build_object('payload', ($5::jsonb -> 1))) )
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(flow_run.id)
        .bind(runtime_event_sequence)
        .bind(input.result.runtime_event_type())
        .bind(lossless_json_parameter(&(&input.terminal_event_payload)))
        .execute(&mut *tx)
        .await?;

        append_flow_run_recovery_state_in_transaction(&mut tx, &flow_run).await?;

        tx.commit().await?;
        match self
            .upsert_application_conversation_messages_for_flow_run(&flow_run)
            .await
        {
            Ok(()) => Ok(CommitFlowRunTerminalReceipt::Winner(flow_run)),
            Err(error) => {
                tracing::warn!(
                    flow_run_id = %flow_run.id,
                    application_id = %flow_run.application_id,
                    error = %error,
                    "flow run terminal commit won but its post-commit conversation projection failed"
                );
                Ok(CommitFlowRunTerminalReceipt::WinnerWithPostCommitProjectionWarning(flow_run))
            }
        }
    }

    async fn finalize_published_run_missing_stream_terminal(
        &self,
        input: &FinalizePublishedRunMissingStreamTerminalPersistenceInput,
    ) -> Result<FinalizePublishedRunMissingStreamTerminalPersistenceOutcome> {
        let receipt = self
            .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
                flow_run_id: input.flow_run_id,
                expected_status: input.expected_status,
                result: CommitFlowRunTerminalResult::Failed {
                    output_payload: input.output_payload.clone(),
                    error_payload: input.error_payload.clone(),
                },
                flow_run_event_payload: input.error_payload.clone(),
                terminal_event_payload: input.terminal_event_payload.clone(),
                finished_at: input.finished_at,
            })
            .await?;
        Ok(match receipt {
            CommitFlowRunTerminalReceipt::Winner(flow_run) => {
                FinalizePublishedRunMissingStreamTerminalPersistenceOutcome::Finalized(flow_run)
            }
            CommitFlowRunTerminalReceipt::WinnerWithPostCommitProjectionWarning(flow_run) => {
                FinalizePublishedRunMissingStreamTerminalPersistenceOutcome::FinalizedWithPostCommitProjectionWarning(flow_run)
            }
            CommitFlowRunTerminalReceipt::Loser => {
                FinalizePublishedRunMissingStreamTerminalPersistenceOutcome::CasMiss
            }
        })
    }

    async fn complete_flow_run(
        &self,
        input: &CompleteFlowRunInput,
    ) -> Result<domain::FlowRunRecord> {
        self.update_flow_run(&UpdateFlowRunInput {
            flow_run_id: input.flow_run_id,
            status: input.status,
            output_payload: input.output_payload.clone(),
            error_payload: input.error_payload.clone(),
            finished_at: Some(input.finished_at),
        })
        .await
    }
}

#[path = "flow_run_methods/node_runs.rs"]
mod node_runs;

#[path = "flow_run_methods/checkpoint_records.rs"]
mod checkpoint_records;
