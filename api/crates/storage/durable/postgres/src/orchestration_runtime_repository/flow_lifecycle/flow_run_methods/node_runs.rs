use super::*;

impl PgControlPlaneStore {
    pub(super) async fn create_node_run(
        &self,
        input: &CreateNodeRunInput,
    ) -> Result<domain::NodeRunRecord> {
        let mut tx = self.pool().begin().await?;
        lock_application_run_native_projection(&mut tx, input.flow_run_id).await?;
        let row = sqlx::query(
            r#"
            with locked_flow as (
                select flow_runs.id, applications.workspace_id
                from flow_runs
                join applications on applications.id = flow_runs.application_id
                where flow_runs.id = $2
                  and flow_runs.status not in ('succeeded', 'incomplete', 'failed', 'cancelled')
                for no key update of flow_runs
            )
            insert into node_run_records (
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
            , raw_json_payloads)
            select $1, workspace_id, id, $3, $4, $5, $6, ($7::jsonb -> 0), ($8::jsonb -> 0), $9, jsonb_strip_nulls(jsonb_build_object('input_payload', ($7::jsonb -> 1), 'debug_payload', ($8::jsonb -> 1))) from locked_flow
            returning
                id,
                flow_run_id,
                node_id,
                node_type,
                node_alias,
                status,
                runtime_original_json(input_payload, node_run_records.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(output_payload, node_run_records.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(error_payload, node_run_records.raw_json_payloads, 'error_payload') as error_payload,
                runtime_original_json(metrics_payload, node_run_records.raw_json_payloads, 'metrics_payload') as metrics_payload,
                runtime_original_json(debug_payload, node_run_records.raw_json_payloads, 'debug_payload') as debug_payload,
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
        .bind(lossless_json_parameter(&(&input.input_payload)))
        .bind(lossless_json_parameter(&(&input.debug_payload)))
        .bind(input.started_at)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        row.map(map_node_run_record)
            .transpose()?
            .ok_or_else(|| ControlPlaneError::Conflict("flow_run_terminal").into())
    }

    pub(super) async fn update_node_run(
        &self,
        input: &UpdateNodeRunInput,
    ) -> Result<domain::NodeRunRecord> {
        let mut tx = self.pool().begin().await?;
        let source_run: Option<Uuid> =
            sqlx::query_scalar("select flow_run_id from node_runs where id=$1")
                .bind(input.node_run_id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(source_run) = source_run {
            lock_application_run_native_projection(&mut tx, source_run).await?;
        }
        // Lock in a separate statement: a view UPDATE cannot re-evaluate its
        // metadata WHERE clause through the base table's EvalPlanQual after a
        // concurrent terminal transition releases the flow lock.
        let locked_flow = sqlx::query_scalar::<_, Uuid>(
            r#"
            select flow_runs.id from flow_runs
            join node_runs candidate on candidate.flow_run_id=flow_runs.id
            where candidate.id=$1
                and flow_runs.status not in ('succeeded','incomplete','failed','cancelled')
            for no key update of flow_runs
            "#,
        )
        .bind(input.node_run_id)
        .fetch_optional(&mut *tx)
        .await?;
        if locked_flow.is_none() {
            tx.rollback().await?;
            return Err(ControlPlaneError::Conflict("flow_run_terminal").into());
        }
        let row = sqlx::query(
            r#"
            with locked_flow as (
                select flow_runs.id
                from flow_runs
                join node_runs candidate on candidate.flow_run_id = flow_runs.id
                where candidate.id = $1
                  and flow_runs.status not in ('succeeded', 'incomplete', 'failed', 'cancelled')
                for no key update of flow_runs
            )
            update node_run_records
            set status = $2,
                output_payload = ($3::jsonb -> 0),
                error_payload = case
                    when ($4::jsonb -> 0) is null
                        and node_run_records.status = 'failed'
                        and $2 <> 'retrying'
                    then node_run_records.error_payload
                    else ($4::jsonb -> 0)
                end,
                metrics_payload = ($5::jsonb -> 0),
                debug_payload = ($6::jsonb -> 0),
                finished_at = $7,
                raw_json_payloads = (node_run_records.raw_json_payloads - 'output_payload' - 'error_payload' - 'metrics_payload' - 'debug_payload') || jsonb_strip_nulls(jsonb_build_object('output_payload', ($3::jsonb -> 1), 'error_payload', case
                    when ($4::jsonb -> 1) is null
                        and node_run_records.status = 'failed'
                        and $2 <> 'retrying'
                    then node_run_records.raw_json_payloads -> 'error_payload'
                    else ($4::jsonb -> 1)
                end, 'metrics_payload', ($5::jsonb -> 1), 'debug_payload', ($6::jsonb -> 1)))
            from locked_flow
            where node_run_records.id = $1
              and node_run_records.flow_run_id = locked_flow.id
              and node_run_records.status <> 'cancelled'
            returning
                node_run_records.id,
                node_run_records.flow_run_id,
                node_run_records.node_id,
                node_run_records.node_type,
                node_run_records.node_alias,
                node_run_records.status,
                runtime_original_json(node_run_records.input_payload, node_run_records.raw_json_payloads, 'input_payload') as input_payload,
                runtime_original_json(node_run_records.output_payload, node_run_records.raw_json_payloads, 'output_payload') as output_payload,
                runtime_original_json(node_run_records.error_payload, node_run_records.raw_json_payloads, 'error_payload') as error_payload,
                runtime_original_json(node_run_records.metrics_payload, node_run_records.raw_json_payloads, 'metrics_payload') as metrics_payload,
                runtime_original_json(node_run_records.debug_payload, node_run_records.raw_json_payloads, 'debug_payload') as debug_payload,
                node_run_records.started_at,
                node_run_records.finished_at
            "#,
        )
        .bind(input.node_run_id)
        .bind(input.status.as_str())
        .bind(lossless_json_parameter(&(&input.output_payload)))
        .bind(lossless_json_parameter(&(&input.error_payload)))
        .bind(lossless_json_parameter(&(&input.metrics_payload)))
        .bind(lossless_json_parameter(&(&input.debug_payload)))
        .bind(input.finished_at)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.rollback().await?;
            return Err(ControlPlaneError::Conflict("flow_run_terminal").into());
        };
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
        if node_run.node_type == "llm" {
            // Effective prompt/debug facts are owned by the node writer. GET
            // must never rebuild the business conversation to discover them.
            Self::refresh_completed_output_projection(&mut tx, node_run.flow_run_id).await?;
        }
        tx.commit().await?;

        Ok(node_run)
    }

    pub(super) async fn complete_node_run(
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
}
