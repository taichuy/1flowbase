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
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19,
                $20, $21, $22, $23, $24
            )
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
                input_payload,
                output_payload,
                error_payload,
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
        .bind(&input.input_payload)
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
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19,
                $20, $21, $22, $23, $24
            )
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
                input_payload,
                output_payload,
                error_payload,
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
        let inserted = sqlx::query(&query)
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
            .bind(&input.input_payload)
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
            .fetch_optional(self.pool())
            .await?;

        let (flow_run, created) = if let Some(row) = inserted {
            (map_flow_run_record(row)?, true)
        } else {
            let conflict_api_key = match input.run_mode {
                domain::FlowRunMode::WorkflowScheduleRun => None,
                _ => input.api_key_id,
            };
            let flow_run = self
                .find_published_flow_run_by_idempotency_key(
                    input.application_id,
                    conflict_api_key,
                    input
                        .idempotency_key
                        .as_deref()
                        .ok_or_else(|| anyhow!("idempotency conflict without an idempotency key"))?,
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
            ) values (
                $1, $2, $3, $4, null, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16, $17, $18,
                $19, $20, $21, $22
            )
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
                input_payload,
                output_payload,
                error_payload,
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
        .bind(&input.input_payload)
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
                flow_runs.input_payload,
                flow_runs.output_payload,
                flow_runs.error_payload,
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
                output_payload = $2,
                error_payload = $3,
                finished_at = $4,
                updated_at = $4
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
                input_payload,
                output_payload,
                error_payload,
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
        .bind(&input.output_payload)
        .bind(&input.error_payload)
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
                input_payload,
                output_payload,
                error_payload,
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

    async fn create_node_run(&self, input: &CreateNodeRunInput) -> Result<domain::NodeRunRecord> {
        let row = sqlx::query(
            r#"
            insert into node_runs (
                id,
                scope_id,
                flow_run_id,
                node_id,
                node_type,
                node_alias,
                status,
                input_payload,
                debug_payload,
                started_at
            ) values (
                $1,
                (
                    select applications.workspace_id
                    from flow_runs
                    join applications on applications.id = flow_runs.application_id
                    where flow_runs.id = $2
                ),
                $2, $3, $4, $5, $6, $7, $8, $9
            )
            returning
                id,
                flow_run_id,
                node_id,
                node_type,
                node_alias,
                status,
                input_payload,
                output_payload,
                error_payload,
                metrics_payload,
                debug_payload,
                started_at,
                finished_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(&input.node_id)
        .bind(&input.node_type)
        .bind(&input.node_alias)
        .bind(input.status.as_str())
        .bind(&input.input_payload)
        .bind(&input.debug_payload)
        .bind(input.started_at)
        .fetch_one(self.pool())
        .await?;

        map_node_run_record(row)
    }

    async fn update_node_run(&self, input: &UpdateNodeRunInput) -> Result<domain::NodeRunRecord> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            r#"
            update node_runs
            set status = $2,
                output_payload = $3,
                error_payload = case
                    when $4::jsonb is null
                        and node_runs.status = 'failed'
                        and $2 <> 'retrying'
                    then node_runs.error_payload
                    else $4
                end,
                metrics_payload = $5,
                debug_payload = $6,
                finished_at = $7
            where id = $1
            returning
                id,
                flow_run_id,
                node_id,
                node_type,
                node_alias,
                status,
                input_payload,
                output_payload,
                error_payload,
                metrics_payload,
                debug_payload,
                started_at,
                finished_at
            "#,
        )
        .bind(input.node_run_id)
        .bind(input.status.as_str())
        .bind(&input.output_payload)
        .bind(&input.error_payload)
        .bind(&input.metrics_payload)
        .bind(&input.debug_payload)
        .bind(input.finished_at)
        .fetch_one(&mut *tx)
        .await?;

        let node_run = map_node_run_record(row)?;
        sqlx::query(
            r#"
            update flow_runs
            set updated_at = coalesce($2, now())
            where id = $1
            "#,
        )
        .bind(node_run.flow_run_id)
        .bind(input.finished_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(node_run)
    }

    async fn complete_node_run(
        &self,
        input: &CompleteNodeRunInput,
    ) -> Result<domain::NodeRunRecord> {
        self.update_node_run(&UpdateNodeRunInput {
            node_run_id: input.node_run_id,
            status: input.status,
            output_payload: input.output_payload.clone(),
            error_payload: input.error_payload.clone(),
            metrics_payload: input.metrics_payload.clone(),
            debug_payload: input.debug_payload.clone(),
            finished_at: Some(input.finished_at),
        })
        .await
    }

    async fn update_flow_run(&self, input: &UpdateFlowRunInput) -> Result<domain::FlowRunRecord> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            r#"
            update flow_runs
            set status = $2,
                output_payload = $3,
                error_payload = $4,
                finished_at = $5,
                updated_at = coalesce($5, now())
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
                input_payload,
                output_payload,
                error_payload,
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
        .bind(&input.output_payload)
        .bind(&input.error_payload)
        .bind(input.finished_at)
        .fetch_one(&mut *tx)
        .await?;

        let flow_run = map_flow_run_record(row)?;
        Self::upsert_application_run_log_summary_projection_for_flow_run(&mut tx, &flow_run)
            .await?;
        if is_terminal_application_run_log_status(flow_run.status) {
            Self::replace_application_run_conversation_message_items_projection(&mut tx, &flow_run)
                .await?;
        } else {
            Self::delete_application_run_conversation_message_items_projection(
                &mut tx,
                flow_run.id,
            )
            .await?;
        }
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
        let row = sqlx::query(
            r#"
            update flow_runs
            set status = $2,
                output_payload = $3,
                error_payload = $4,
                finished_at = $5,
                updated_at = coalesce($5, now())
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
                input_payload,
                output_payload,
                error_payload,
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
        .bind(&input.output_payload)
        .bind(&input.error_payload)
        .bind(input.finished_at)
        .bind(expected_status.as_str())
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = row {
            let flow_run = map_flow_run_record(row)?;
            Self::upsert_application_run_log_summary_projection_for_flow_run(&mut tx, &flow_run)
                .await?;
            if is_terminal_application_run_log_status(flow_run.status) {
                Self::replace_application_run_conversation_message_items_projection(
                    &mut tx, &flow_run,
                )
                .await?;
            } else {
                Self::delete_application_run_conversation_message_items_projection(
                    &mut tx,
                    flow_run.id,
                )
                .await?;
            }
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
        let error_payload = input.result.error_payload().cloned();
        let row = sqlx::query(
            r#"
            update flow_runs
            set status = $2,
                output_payload = $3,
                error_payload = $4,
                finished_at = $5,
                updated_at = $5
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
                input_payload,
                output_payload,
                error_payload,
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
        .bind(input.result.output_payload())
        .bind(&error_payload)
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
        Self::upsert_application_run_log_summary_projection_for_flow_run(&mut tx, &flow_run)
            .await?;
        Self::replace_application_run_conversation_message_items_projection(&mut tx, &flow_run)
            .await?;

        let scope_id = flow_run_scope_id_for_update(&mut tx, flow_run.id).await?;
        let flow_event_sequence = next_event_sequence(&mut tx, flow_run.id).await?;
        sqlx::query(
            r#"
            insert into flow_run_events (
                id,
                scope_id,
                flow_run_id,
                node_run_id,
                sequence,
                event_type,
                payload
            ) values ($1, $2, $3, null, $4, $5, $6)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(scope_id)
        .bind(flow_run.id)
        .bind(flow_event_sequence)
        .bind(input.result.flow_run_event_type())
        .bind(&input.flow_run_event_payload)
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
            ) values (
                $1, $2, null, null, null, $3, $4, 'agent_transition',
                'host', 'host_fact', null, null, $5, 'workspace', 'durable'
            )
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(flow_run.id)
        .bind(runtime_event_sequence)
        .bind(input.result.runtime_event_type())
        .bind(&input.terminal_event_payload)
        .execute(&mut *tx)
        .await?;

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

    async fn get_checkpoint(
        &self,
        flow_run_id: Uuid,
        checkpoint_id: Uuid,
    ) -> Result<Option<domain::CheckpointRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                node_run_id,
                status,
                reason,
                locator_payload,
                variable_snapshot,
                external_ref_payload,
                created_at
            from flow_run_checkpoints
            where flow_run_id = $1
              and id = $2
            "#,
        )
        .bind(flow_run_id)
        .bind(checkpoint_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(fetch_checkpoint_record))
    }

    async fn create_checkpoint(
        &self,
        input: &CreateCheckpointInput,
    ) -> Result<domain::CheckpointRecord> {
        let row = sqlx::query(
            r#"
            insert into flow_run_checkpoints (
                id,
                scope_id,
                flow_run_id,
                node_run_id,
                status,
                reason,
                locator_payload,
                variable_snapshot,
                external_ref_payload
            ) values (
                $1,
                (
                    select applications.workspace_id
                    from flow_runs
                    join applications on applications.id = flow_runs.application_id
                    where flow_runs.id = $2
                ),
                $2, $3, $4, $5, $6, $7, $8
            )
            returning
                id,
                flow_run_id,
                node_run_id,
                status,
                reason,
                locator_payload,
                variable_snapshot,
                external_ref_payload,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(&input.status)
        .bind(&input.reason)
        .bind(&input.locator_payload)
        .bind(&input.variable_snapshot)
        .bind(&input.external_ref_payload)
        .fetch_one(self.pool())
        .await?;

        Ok(map_checkpoint_record(row))
    }

    async fn create_callback_task(
        &self,
        input: &CreateCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        let row = sqlx::query(
            r#"
            insert into flow_run_callback_tasks (
                id,
                scope_id,
                flow_run_id,
                node_run_id,
                callback_kind,
                status,
                request_payload,
                external_ref_payload
            ) values (
                $1,
                (
                    select applications.workspace_id
                    from flow_runs
                    join applications on applications.id = flow_runs.application_id
                    where flow_runs.id = $2
                ),
                $2, $3, $4, 'pending', $5, $6
            )
            returning
                id,
                flow_run_id,
                node_run_id,
                callback_kind,
                status,
                case
                    when callback_kind = 'llm_tool_calls'
                    then jsonb_build_object('tool_calls', request_payload -> 'tool_calls')
                    else request_payload
                end as request_payload,
                response_payload,
                case
                    when callback_kind = 'llm_tool_calls' then null
                    else external_ref_payload
                end as external_ref_payload,
                created_at,
                completed_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(&input.callback_kind)
        .bind(&input.request_payload)
        .bind(&input.external_ref_payload)
        .fetch_one(self.pool())
        .await?;

        map_callback_task_record(row)
    }

    async fn get_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::CallbackTaskRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                node_run_id,
                callback_kind,
                status,
                request_payload,
                response_payload,
                external_ref_payload,
                created_at,
                completed_at
            from flow_run_callback_tasks
            where id = $1
            "#,
        )
        .bind(callback_task_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_callback_task_record).transpose()
    }

    async fn complete_callback_task(
        &self,
        input: &CompleteCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        let row = sqlx::query(
            r#"
            update flow_run_callback_tasks
            set status = 'completed',
                response_payload = $2,
                completed_at = $3
            where id = $1 and status = 'pending'
            returning
                id,
                flow_run_id,
                node_run_id,
                callback_kind,
                status,
                case
                    when callback_kind = 'llm_tool_calls'
                    then jsonb_build_object('tool_calls', request_payload -> 'tool_calls')
                    else request_payload
                end as request_payload,
                response_payload,
                case
                    when callback_kind = 'llm_tool_calls' then null
                    else external_ref_payload
                end as external_ref_payload,
                created_at,
                completed_at
            "#,
        )
        .bind(input.callback_task_id)
        .bind(&input.response_payload)
        .bind(input.completed_at)
        .fetch_optional(self.pool())
        .await?;

        let Some(row) = row else {
            if self
                .get_callback_task(input.callback_task_id)
                .await?
                .is_some()
            {
                return Err(ControlPlaneError::Conflict("callback_task_not_pending").into());
            }
            return Err(ControlPlaneError::NotFound("callback_task").into());
        };

        map_callback_task_record(row)
    }
}
