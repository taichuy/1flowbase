impl PgControlPlaneStore {
    async fn create_runtime_debug_artifact(
        &self,
        input: &CreateRuntimeDebugArtifactInput,
    ) -> Result<domain::RuntimeDebugArtifactRecord> {
        let row = sqlx::query(
            r#"
            insert into runtime_debug_artifacts (
                id,
                workspace_id,
                application_id,
                flow_run_id,
                node_run_id,
                run_event_id,
                artifact_kind,
                content_type,
                original_size_bytes,
                preview_size_bytes,
                storage_id,
                storage_ref,
                retention_state
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            returning
                id,
                workspace_id,
                application_id,
                flow_run_id,
                node_run_id,
                run_event_id,
                artifact_kind,
                content_type,
                original_size_bytes,
                preview_size_bytes,
                storage_id,
                storage_ref,
                retention_state,
                created_at
            "#,
        )
        .bind(input.artifact_id)
        .bind(input.workspace_id)
        .bind(input.application_id)
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(input.run_event_id)
        .bind(&input.artifact_kind)
        .bind(&input.content_type)
        .bind(input.original_size_bytes)
        .bind(input.preview_size_bytes)
        .bind(input.storage_id)
        .bind(&input.storage_ref)
        .bind(&input.retention_state)
        .fetch_one(self.pool())
        .await?;

        Ok(map_runtime_debug_artifact_record(row))
    }

    async fn get_runtime_debug_artifact(
        &self,
        input: &GetRuntimeDebugArtifactInput,
    ) -> Result<Option<domain::RuntimeDebugArtifactRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                workspace_id,
                application_id,
                flow_run_id,
                node_run_id,
                run_event_id,
                artifact_kind,
                content_type,
                original_size_bytes,
                preview_size_bytes,
                storage_id,
                storage_ref,
                retention_state,
                created_at
            from runtime_debug_artifacts
            where id = $1
              and workspace_id = $2
              and application_id = $3
              and retention_state = 'active'
            "#,
        )
        .bind(input.artifact_id)
        .bind(input.workspace_id)
        .bind(input.application_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_runtime_debug_artifact_record))
    }

    async fn update_flow_run_payloads(
        &self,
        input: &UpdateFlowRunPayloadsInput,
    ) -> Result<domain::FlowRunRecord> {
        let row = sqlx::query(
            r#"
            update flow_runs
            set input_payload = ($2::jsonb -> 0),
                output_payload = ($3::jsonb -> 0),
                error_payload = ($4::jsonb -> 0),
                updated_at = now(),
                raw_json_payloads = (flow_runs.raw_json_payloads - 'input_payload' - 'output_payload' - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('input_payload', ($2::jsonb -> 1), 'output_payload', ($3::jsonb -> 1), 'error_payload', ($4::jsonb -> 1)))
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
        .bind(lossless_json_parameter(&(&input.input_payload)))
        .bind(lossless_json_parameter(&(&input.output_payload)))
        .bind(lossless_json_parameter(&(&input.error_payload)))
        .fetch_one(self.pool())
        .await?;

        map_flow_run_record(row)
    }

    async fn update_node_run_payloads(
        &self,
        input: &UpdateNodeRunPayloadsInput,
    ) -> Result<domain::NodeRunRecord> {
        let row = sqlx::query(
            r#"
            update node_runs
            set input_payload = ($2::jsonb -> 0),
                output_payload = ($3::jsonb -> 0),
                error_payload = case
                    when ($4::jsonb -> 0) is null
                        and node_runs.status = 'failed'
                    then node_runs.error_payload
                    else ($4::jsonb -> 0)
                end,
                metrics_payload = ($5::jsonb -> 0),
                debug_payload = ($6::jsonb -> 0),
                raw_json_payloads = (node_runs.raw_json_payloads - 'input_payload' - 'output_payload' - 'error_payload' - 'metrics_payload' - 'debug_payload') || jsonb_strip_nulls(jsonb_build_object('input_payload', ($2::jsonb -> 1), 'output_payload', ($3::jsonb -> 1), 'error_payload', case
                    when ($4::jsonb -> 1) is null
                        and node_runs.status = 'failed'
                    then node_runs.raw_json_payloads -> 'error_payload'
                    else ($4::jsonb -> 1)
                end, 'metrics_payload', ($5::jsonb -> 1), 'debug_payload', ($6::jsonb -> 1)))
            where id = $1
            returning
                id,
                flow_run_id,
                node_id,
                node_type,
                node_alias,
                status,
                runtime_original_json(input_payload, node_runs.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, node_runs.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, node_runs.raw_json_payloads, 'error_payload') as error_payload,
                runtime_original_json(metrics_payload, node_runs.raw_json_payloads, 'metrics_payload') as metrics_payload,
                runtime_original_json(debug_payload, node_runs.raw_json_payloads, 'debug_payload') as debug_payload,
                started_at,
                finished_at
            "#,
        )
        .bind(input.node_run_id)
        .bind(lossless_json_parameter(&(&input.input_payload)))
        .bind(lossless_json_parameter(&(&input.output_payload)))
        .bind(lossless_json_parameter(&(&input.error_payload)))
        .bind(lossless_json_parameter(&(&input.metrics_payload)))
        .bind(lossless_json_parameter(&(&input.debug_payload)))
        .fetch_one(self.pool())
        .await?;

        map_node_run_record(row)
    }

    async fn update_run_event_payload(
        &self,
        input: &UpdateRunEventPayloadInput,
    ) -> Result<domain::RunEventRecord> {
        let row = sqlx::query(
            r#"
            update flow_run_events
            set payload = ($2::jsonb -> 0),
                resume_timeline_description = $3,
                resume_timeline_description_projected = true,
                raw_json_payloads = (flow_run_events.raw_json_payloads - 'payload') || jsonb_strip_nulls(jsonb_build_object('payload', ($2::jsonb -> 1)))
            where id = $1
            returning
                id,
                flow_run_id,
                node_run_id,
                sequence,
                event_type,
                runtime_original_json(payload, flow_run_events.raw_json_payloads, 'payload') as payload,
                created_at
            "#,
        )
        .bind(input.run_event_id)
        .bind(lossless_json_parameter(&(&input.payload)))
        .bind(resume_timeline_description(&input.payload))
        .fetch_one(self.pool())
        .await?;

        Ok(map_run_event_record(row))
    }

    async fn update_checkpoint_payloads(
        &self,
        input: &UpdateCheckpointPayloadsInput,
    ) -> Result<domain::CheckpointRecord> {
        let row = sqlx::query(
            r#"
            update flow_run_checkpoints
            set locator_payload = ($2::jsonb -> 0),
                variable_snapshot = ($3::jsonb -> 0),
                external_ref_payload = ($4::jsonb -> 0),
                raw_json_payloads = (flow_run_checkpoints.raw_json_payloads - 'locator_payload' - 'variable_snapshot' - 'external_ref_payload') || jsonb_strip_nulls(jsonb_build_object('locator_payload', ($2::jsonb -> 1), 'variable_snapshot', ($3::jsonb -> 1), 'external_ref_payload', ($4::jsonb -> 1)))
            where id = $1
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
        .bind(input.checkpoint_id)
        .bind(lossless_json_parameter(&(&input.locator_payload)))
        .bind(lossless_json_parameter(&(&input.variable_snapshot)))
        .bind(lossless_json_parameter(&(&input.external_ref_payload)))
        .fetch_one(self.pool())
        .await?;

        Ok(map_checkpoint_record(row))
    }

    async fn update_callback_task_payloads(
        &self,
        input: &UpdateCallbackTaskPayloadsInput,
    ) -> Result<domain::CallbackTaskRecord> {
        let row = sqlx::query(
            r#"
            update flow_run_callback_tasks
            set request_payload = ($2::jsonb -> 0),
                response_payload = ($3::jsonb -> 0),
                external_ref_payload = ($4::jsonb -> 0),
                raw_json_payloads = (flow_run_callback_tasks.raw_json_payloads - 'request_payload' - 'response_payload' - 'external_ref_payload') || jsonb_strip_nulls(jsonb_build_object('request_payload', ($2::jsonb -> 1), 'response_payload', ($3::jsonb -> 1), 'external_ref_payload', ($4::jsonb -> 1)))
            where id = $1
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
        .bind(input.callback_task_id)
        .bind(lossless_json_parameter(&(&input.request_payload)))
        .bind(lossless_json_parameter(&(&input.response_payload)))
        .bind(lossless_json_parameter(&(&input.external_ref_payload)))
        .fetch_one(self.pool())
        .await?;

        map_callback_task_record(row)
    }
}

fn map_runtime_debug_artifact_record(
    row: sqlx::postgres::PgRow,
) -> domain::RuntimeDebugArtifactRecord {
    domain::RuntimeDebugArtifactRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        application_id: row.get("application_id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        run_event_id: row.get("run_event_id"),
        artifact_kind: row.get("artifact_kind"),
        content_type: row.get("content_type"),
        original_size_bytes: row.get("original_size_bytes"),
        preview_size_bytes: row.get("preview_size_bytes"),
        storage_id: row.get("storage_id"),
        storage_ref: row.get("storage_ref"),
        retention_state: row.get("retention_state"),
        created_at: row.get("created_at"),
    }
}
