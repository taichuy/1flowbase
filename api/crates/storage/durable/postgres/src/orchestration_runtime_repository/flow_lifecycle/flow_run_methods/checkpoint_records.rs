use super::*;

impl PgControlPlaneStore {
    pub(super) async fn get_checkpoint(
        &self,
        flow_run_id: Uuid,
        checkpoint_id: Uuid,
    ) -> Result<Option<domain::CheckpointRecord>> {
        let row = sqlx::query(
            r#"
            select
                checkpoints.id,
                checkpoints.flow_run_id,
                checkpoints.node_run_id,
                checkpoints.status,
                checkpoints.reason,
                runtime_original_json(checkpoints.locator_payload, checkpoints.raw_json_payloads, 'locator_payload') as locator_payload,
                coalesce(runtime_original_json(contents.content, contents.raw_json_payloads, 'content'), runtime_original_json(checkpoints.variable_snapshot, checkpoints.raw_json_payloads, 'variable_snapshot')) as variable_snapshot,
                runtime_original_json(checkpoints.external_ref_payload, checkpoints.raw_json_payloads, 'external_ref_payload') as external_ref_payload,
                checkpoints.created_at
            from flow_run_checkpoints checkpoints
            left join runtime_legacy_shadow_rows shadow_rows
              on shadow_rows.source_table = 'flow_run_checkpoints'
             and shadow_rows.source_column = 'variable_snapshot'
             and shadow_rows.source_row_id = checkpoints.id
            left join runtime_canonical_contents contents
              on contents.id = shadow_rows.canonical_content_id
             and contents.content = checkpoints.variable_snapshot
             and (contents.raw_json_payloads -> 'content') is not distinct from (checkpoints.raw_json_payloads -> 'variable_snapshot')
            where checkpoints.flow_run_id = $1
              and checkpoints.id = $2
            "#,
        )
        .bind(flow_run_id)
        .bind(checkpoint_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(fetch_checkpoint_record))
    }

    pub(super) async fn create_checkpoint(
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
            , raw_json_payloads) values ( $1, (
                    select applications.workspace_id
                    from flow_runs
                    join applications on applications.id = flow_runs.application_id
                    where flow_runs.id = $2
                ), $2, $3, $4, $5, ($6::jsonb -> 0), ($7::jsonb -> 0), ($8::jsonb -> 0), jsonb_strip_nulls(jsonb_build_object('locator_payload', ($6::jsonb -> 1), 'variable_snapshot', ($7::jsonb -> 1), 'external_ref_payload', ($8::jsonb -> 1))) )
            returning
                id,
                flow_run_id,
                node_run_id,
                status,
                reason,
                runtime_original_json(locator_payload, flow_run_checkpoints.raw_json_payloads, 'locator_payload') as locator_payload,
                runtime_original_json(variable_snapshot, flow_run_checkpoints.raw_json_payloads, 'variable_snapshot') as variable_snapshot,
                runtime_original_json(external_ref_payload, flow_run_checkpoints.raw_json_payloads, 'external_ref_payload') as external_ref_payload,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(&input.status)
        .bind(&input.reason)
        .bind(lossless_json_parameter(&(&input.locator_payload)))
        .bind(lossless_json_parameter(&(&input.variable_snapshot)))
        .bind(lossless_json_parameter(&(&input.external_ref_payload)))
        .fetch_one(self.pool())
        .await?;

        Ok(map_checkpoint_record(row))
    }

    pub(super) async fn create_callback_task(
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
            , raw_json_payloads) values ( $1, (
                    select applications.workspace_id
                    from flow_runs
                    join applications on applications.id = flow_runs.application_id
                    where flow_runs.id = $2
                ), $2, $3, $4, 'pending', ($5::jsonb -> 0), ($6::jsonb -> 0), jsonb_strip_nulls(jsonb_build_object('request_payload', ($5::jsonb -> 1), 'external_ref_payload', ($6::jsonb -> 1))) )
            returning
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
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(&input.callback_kind)
        .bind(lossless_json_parameter(&(&input.request_payload)))
        .bind(lossless_json_parameter(&(&input.external_ref_payload)))
        .fetch_one(self.pool())
        .await?;

        map_callback_task_tool_summary_record(row)
    }

    pub(super) async fn get_callback_task(
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
                runtime_original_json(external_ref_payload, flow_run_callback_tasks.raw_json_payloads, 'external_ref_payload') as external_ref_payload,
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

    pub(super) async fn complete_callback_task(
        &self,
        input: &CompleteCallbackTaskInput,
    ) -> Result<domain::CallbackTaskRecord> {
        let row = sqlx::query(
            r#"
            update flow_run_callback_tasks
            set status = 'completed',
                response_payload = ($2::jsonb -> 0),
                completed_at = $3,
                raw_json_payloads = (flow_run_callback_tasks.raw_json_payloads - 'response_payload') || jsonb_strip_nulls(jsonb_build_object('response_payload', ($2::jsonb -> 1)))
            where id = $1 and status = 'pending'
            returning
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
            "#,
        )
        .bind(input.callback_task_id)
        .bind(lossless_json_parameter(&(&input.response_payload)))
        .bind(input.completed_at)
        .fetch_optional(self.pool())
        .await?;

        let row = match row {
            Some(row) => row,
            None => {
                let replay = sqlx::query(
                    r#"
                    select id, flow_run_id, node_run_id, callback_kind, status,
                           runtime_original_json(request_payload, flow_run_callback_tasks.raw_json_payloads, 'request_payload') as request_payload,
                           runtime_original_json(response_payload, flow_run_callback_tasks.raw_json_payloads, 'response_payload') as response_payload,
                           case when callback_kind = 'llm_tool_calls' then null
                                else runtime_original_json(external_ref_payload, flow_run_callback_tasks.raw_json_payloads, 'external_ref_payload') end as external_ref_payload,
                           created_at, completed_at
                      from flow_run_callback_tasks
                     where id = $1 and status = 'completed' and response_payload = ($2::jsonb -> 0)
                       and (raw_json_payloads -> 'response_payload') is not distinct from nullif($2::jsonb -> 1, 'null'::jsonb)
                    "#,
                )
                .bind(input.callback_task_id)
                .bind(lossless_json_parameter(&input.response_payload))
                .fetch_optional(self.pool())
                .await?;
                if let Some(replay) = replay {
                    replay
                } else if self
                    .get_callback_task(input.callback_task_id)
                    .await?
                    .is_some()
                {
                    return Err(ControlPlaneError::Conflict("callback_task_not_pending").into());
                } else {
                    return Err(ControlPlaneError::NotFound("callback_task").into());
                }
            }
        };

        map_callback_task_tool_summary_record(row)
    }
}
