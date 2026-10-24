use super::*;
#[path = "responses_context.rs"]
mod responses_context;

#[async_trait]
impl ApplicationPublishedRunControlRepository for PgControlPlaneStore {
    async fn find_published_responses_contexts(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        api_key_id: Uuid,
        actor_user_id: Uuid,
        response_id: &str,
    ) -> Result<Vec<control_plane_contracts::application_public_runtime::PublishedResponsesContext>>
    {
        responses_context::find(
            self,
            workspace_id,
            application_id,
            api_key_id,
            actor_user_id,
            response_id,
        )
        .await
    }

    async fn get_published_flow_run(
        &self,
        flow_run_id: Uuid,
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
            where id = $1
              and run_mode = 'published_api_run'
            "#,
        )
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_flow_run_record).transpose()
    }

    async fn find_published_flow_run_by_provider_response_id(
        &self,
        application_id: Uuid,
        api_key_id: Uuid,
        provider_response_id: &str,
    ) -> Result<Option<domain::FlowRunRecord>> {
        let row = sqlx::query(
            r#"
            select
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
            from flow_runs
            join node_run_records on node_run_records.flow_run_id = flow_runs.id
            where flow_runs.application_id = $1
              and flow_runs.api_key_id = $2
              and flow_runs.run_mode = 'published_api_run'
              and (node_run_records.output_payload ->> 'response_id' = $3
                      or node_run_records.output_payload #>> '{provider_metadata,native_response,response_id}' = $3
                   or exists (select 1 from runtime_spans rs where rs.flow_run_id = flow_runs.id and rs.metadata #>> '{native_response,response_id}' = $3))
            order by node_run_records.finished_at desc nulls last, node_run_records.id desc
            limit 1
            "#,
        )
        .bind(application_id)
        .bind(api_key_id)
        .bind(provider_response_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_flow_run_record).transpose()
    }

    async fn cancel_published_flow_run(
        &self,
        input: &CancelPublishedFlowRunInput,
    ) -> Result<CommitFlowRunTerminalReceipt> {
        PgControlPlaneStore::commit_flow_run_terminal(
            self,
            &CommitFlowRunTerminalInput {
                flow_run_id: input.flow_run_id,
                expected_status: input.from_status,
                result: CommitFlowRunTerminalResult::Cancelled {
                    output_payload: input.output_payload.clone(),
                    error_payload: input.error_payload.clone(),
                },
                flow_run_event_payload: input.flow_run_event_payload.clone(),
                terminal_event_payload: input.terminal_event_payload.clone(),
                finished_at: input.finished_at,
            },
        )
        .await
    }

    async fn cancel_published_pending_callback_tasks_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<domain::CallbackTaskRecord>> {
        let rows = sqlx::query(
            r#"
            update flow_run_callback_tasks
            set status = 'cancelled',
                completed_at = $2
            where flow_run_id = $1
              and status = 'pending'
            returning
                id,
                flow_run_id,
                node_run_id,
                callback_kind,
                status,
                runtime_original_json(request_payload, flow_run_callback_tasks.raw_json_payloads, 'request_payload') as request_payload,
                runtime_original_json(response_payload, flow_run_callback_tasks.raw_json_payloads, 'response_payload') as response_payload,
                runtime_original_json(external_ref_payload, flow_run_callback_tasks.raw_json_payloads, 'external_ref_payload') as external_ref_payload,
                created_at,
                completed_at
            "#,
        )
        .bind(flow_run_id)
        .bind(completed_at)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_callback_task_record).collect()
    }

    async fn list_waiting_callback_published_flow_run_ids_for_conversation(
        &self,
        input: &ListWaitingCallbackPublishedRunsInput,
    ) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            r#"
            select id
            from flow_runs
            where application_id = $1
              and api_key_id = $2
              and external_user = $3
              and external_conversation_id = $4
              and run_mode = 'published_api_run'
              and status = 'waiting_callback'
            order by started_at asc, id asc
            "#,
        )
        .bind(input.application_id)
        .bind(input.api_key_id)
        .bind(&input.external_user)
        .bind(&input.external_conversation_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
    }

    async fn find_native_responses_callbacks_by_call_ids(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        api_key_id: Uuid,
        actor_user_id: Uuid,
        call_ids: &[String],
    ) -> Result<Vec<domain::CallbackTaskRecord>> {
        let rows = sqlx::query(
            r#"
            select c.id, c.flow_run_id, c.node_run_id, c.callback_kind, c.status,
                   runtime_original_json(c.request_payload, c.raw_json_payloads, 'request_payload') as request_payload, runtime_original_json(c.response_payload, c.raw_json_payloads, 'response_payload') as response_payload, runtime_original_json(c.external_ref_payload, c.raw_json_payloads, 'external_ref_payload') as external_ref_payload,
                   c.created_at, c.completed_at
            from flow_run_callback_tasks c
            join flow_runs f on f.id = c.flow_run_id
            join applications a on a.id = f.application_id
            where a.workspace_id = $1 and f.application_id = $2
              and f.api_key_id = $3 and f.created_by = $4
              and f.run_mode = 'published_api_run'
              and c.callback_kind = 'llm_tool_calls'
              and jsonb_typeof(c.request_payload #> '{provider_metadata,native_response}') = 'object'
              and (
                  jsonb_path_query_array(c.request_payload, '$.tool_calls[*].call_id')
                  || jsonb_path_query_array(c.request_payload, '$.tool_calls[*].id')
              ) ?| $5::text[]
              and exists (
                  select 1 from jsonb_array_elements(
                      case when jsonb_typeof(c.request_payload->'tool_calls') = 'array'
                           then c.request_payload->'tool_calls' else '[]'::jsonb end
                  ) tool_call(item)
                  where coalesce(item->>'call_id', item->>'id') = any($5)
              )
            order by c.created_at, c.id
            "#,
        )
        .bind(workspace_id)
        .bind(application_id)
        .bind(api_key_id)
        .bind(actor_user_id)
        .bind(call_ids)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(map_callback_task_record).collect()
    }

    async fn find_native_responses_callbacks_by_response_id(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        api_key_id: Uuid,
        actor_user_id: Uuid,
        provider_response_id: &str,
    ) -> Result<Vec<domain::CallbackTaskRecord>> {
        let rows = sqlx::query(
            r#"
            select c.id, c.flow_run_id, c.node_run_id, c.callback_kind, c.status,
                   runtime_original_json(c.request_payload, c.raw_json_payloads, 'request_payload') as request_payload, runtime_original_json(c.response_payload, c.raw_json_payloads, 'response_payload') as response_payload, runtime_original_json(c.external_ref_payload, c.raw_json_payloads, 'external_ref_payload') as external_ref_payload,
                   c.created_at, c.completed_at
            from flow_run_callback_tasks c
            join flow_runs f on f.id = c.flow_run_id
            join applications a on a.id = f.application_id
            where a.workspace_id = $1 and f.application_id = $2
              and f.api_key_id = $3 and f.created_by = $4
              and f.run_mode = 'published_api_run'
              and c.callback_kind = 'llm_tool_calls'
              and c.request_payload #>> '{provider_metadata,native_response,response_id}' = $5
            order by c.created_at, c.id
            "#,
        )
        .bind(workspace_id)
        .bind(application_id)
        .bind(api_key_id)
        .bind(actor_user_id)
        .bind(provider_response_id)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(map_callback_task_record).collect()
    }

    async fn find_semantic_responses_callbacks_by_call_ids(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        api_key_id: Uuid,
        actor_user_id: Uuid,
        call_ids: &[String],
    ) -> Result<Vec<domain::CallbackTaskRecord>> {
        let rows = sqlx::query(
            r#"
            select c.id, c.flow_run_id, c.node_run_id, c.callback_kind, c.status,
                   runtime_original_json(c.request_payload, c.raw_json_payloads, 'request_payload') as request_payload, runtime_original_json(c.response_payload, c.raw_json_payloads, 'response_payload') as response_payload, runtime_original_json(c.external_ref_payload, c.raw_json_payloads, 'external_ref_payload') as external_ref_payload,
                   c.created_at, c.completed_at
            from flow_run_callback_tasks c
            join flow_runs f on f.id = c.flow_run_id
            join applications a on a.id = f.application_id
            where a.workspace_id = $1 and f.application_id = $2
              and f.api_key_id = $3 and f.created_by = $4
              and f.run_mode = 'published_api_run'
              and c.callback_kind = 'llm_tool_calls'
              and jsonb_typeof(c.request_payload #> '{responses_round}') = 'object'
              and (
                  jsonb_path_query_array(c.request_payload, '$.tool_calls[*].call_id')
                  || jsonb_path_query_array(c.request_payload, '$.tool_calls[*].id')
              ) ?| $5::text[]
              and exists (
                  select 1 from jsonb_array_elements(
                      case when jsonb_typeof(c.request_payload->'tool_calls') = 'array'
                           then c.request_payload->'tool_calls' else '[]'::jsonb end
                  ) tool_call(item)
                  where coalesce(item->>'call_id', item->>'id') = any($5)
              )
            order by c.created_at, c.id
            "#,
        )
        .bind(workspace_id)
        .bind(application_id)
        .bind(api_key_id)
        .bind(actor_user_id)
        .bind(call_ids)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(map_callback_task_record).collect()
    }

    async fn find_semantic_responses_callbacks_by_response_id(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        api_key_id: Uuid,
        actor_user_id: Uuid,
        provider_response_id: &str,
    ) -> Result<Vec<domain::CallbackTaskRecord>> {
        let rows = sqlx::query(
            r#"
            select c.id, c.flow_run_id, c.node_run_id, c.callback_kind, c.status,
                   runtime_original_json(c.request_payload, c.raw_json_payloads, 'request_payload') as request_payload, runtime_original_json(c.response_payload, c.raw_json_payloads, 'response_payload') as response_payload, runtime_original_json(c.external_ref_payload, c.raw_json_payloads, 'external_ref_payload') as external_ref_payload,
                   c.created_at, c.completed_at
            from flow_run_callback_tasks c
            join flow_runs f on f.id = c.flow_run_id
            join applications a on a.id = f.application_id
            where a.workspace_id = $1 and f.application_id = $2
              and f.api_key_id = $3 and f.created_by = $4
              and f.run_mode = 'published_api_run'
              and c.callback_kind = 'llm_tool_calls'
              and c.request_payload #>> '{responses_round,response_id}' = $5
            order by c.created_at, c.id
            "#,
        )
        .bind(workspace_id)
        .bind(application_id)
        .bind(api_key_id)
        .bind(actor_user_id)
        .bind(provider_response_id)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(map_callback_task_record).collect()
    }

    async fn get_published_callback_task(
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
                runtime_original_json(request_payload, flow_run_callback_tasks.raw_json_payloads, 'request_payload') as request_payload,
                runtime_original_json(response_payload, flow_run_callback_tasks.raw_json_payloads, 'response_payload') as response_payload,
                case
                    when callback_kind = 'llm_tool_calls' then null
                    else runtime_original_json(external_ref_payload, flow_run_callback_tasks.raw_json_payloads, 'external_ref_payload')
                end as external_ref_payload,
                created_at,
                completed_at
            from flow_run_callback_tasks
            where id = $1
            "#,
        )
        .bind(callback_task_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_callback_task_tool_summary_record).transpose()
    }

    async fn get_published_run_stream_state(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<PublishedRunStreamState>> {
        let row = sqlx::query(
            r#"
            select
                status,
                runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload
            from flow_runs
            where application_id = $1
              and id = $2
              and run_mode = 'published_api_run'
              and (
                  import_job_id is null
                  or exists (
                      select 1
                      from run_archive_import_jobs import_jobs
                      where import_jobs.id = flow_runs.import_job_id
                        and import_jobs.status = 'succeeded'
                  )
              )
            "#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let node_usages = self.read_flow_run_node_usages(flow_run_id).await?;
        let latest_pending_callback = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                node_run_id,
                callback_kind,
                runtime_original_json(request_payload, flow_run_callback_tasks.raw_json_payloads, 'request_payload') as request_payload
            from flow_run_callback_tasks
            where flow_run_id = $1
              and status = 'pending'
            order by created_at desc, id desc
            limit 1
            "#,
        )
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?
        .map(|row| {
            let callback_kind: String = row.get("callback_kind");
            let original: Value = row.get("request_payload");
            let (request_payload, tool_calls) = if callback_kind == "llm_tool_calls" {
                (None, original.get("tool_calls").cloned())
            } else {
                (Some(original), None)
            };
            PublishedRunPendingCallback {
                id: row.get("id"),
                flow_run_id: row.get("flow_run_id"),
                node_run_id: row.get("node_run_id"),
                callback_kind,
                request_payload,
                tool_calls,
            }
        });

        Ok(Some(PublishedRunStreamState {
            status: crate::mappers::orchestration_runtime_mapper::parse_flow_run_status(
                row.get::<String, _>("status").as_str(),
            )?,
            output_payload: row.get("output_payload"),
            error_payload: row.get("error_payload"),
            node_usages,
            latest_pending_callback,
        }))
    }
}
