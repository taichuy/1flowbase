impl PgControlPlaneStore {
    async fn append_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> Result<domain::RunEventRecord> {
        let mut tx = self.pool().begin().await?;
        lock_open_flow_run_for_event_append(&mut tx, input.flow_run_id).await?;
        let scope_id = flow_run_scope_id_for_update(&mut tx, input.flow_run_id).await?;
        lock_flow_run_event_sequence(&mut tx, input.flow_run_id).await?;
        let next_sequence = next_event_sequence(&mut tx, input.flow_run_id).await?;
        let resume_timeline_description = resume_timeline_description(&input.payload);
        let row = sqlx::query(
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
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, true)
            returning
                id,
                flow_run_id,
                node_run_id,
                sequence,
                event_type,
                payload,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(scope_id)
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(next_sequence)
        .bind(&input.event_type)
        .bind(&input.payload)
        .bind(resume_timeline_description)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(map_run_event_record(row))
    }

    async fn append_run_events(
        &self,
        inputs: &[AppendRunEventInput],
    ) -> Result<Vec<domain::RunEventRecord>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        if inputs
            .iter()
            .any(|input| input.flow_run_id != inputs[0].flow_run_id)
        {
            let mut records = Vec::with_capacity(inputs.len());
            for input in inputs {
                records.push(self.append_run_event(input).await?);
            }
            return Ok(records);
        }

        let mut tx = self.pool().begin().await?;
        lock_open_flow_run_for_event_append(&mut tx, inputs[0].flow_run_id).await?;
        lock_flow_run_event_sequence(&mut tx, inputs[0].flow_run_id).await?;
        let first_sequence = next_event_sequence(&mut tx, inputs[0].flow_run_id).await?;
        let scope_id = flow_run_scope_id_for_update(&mut tx, inputs[0].flow_run_id).await?;
        let mut builder = QueryBuilder::<Postgres>::new(
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
            ) "#,
        );
        builder.push_values(inputs.iter().enumerate(), |mut row, (index, input)| {
            row.push_bind(Uuid::now_v7())
                .push_bind(scope_id)
                .push_bind(input.flow_run_id)
                .push_bind(input.node_run_id)
                .push_bind(first_sequence + index as i64)
                .push_bind(&input.event_type)
                .push_bind(&input.payload)
                .push_bind(resume_timeline_description(&input.payload))
                .push_bind(true);
        });
        builder.push(
            r#"
            returning
                id,
                flow_run_id,
                node_run_id,
                sequence,
                event_type,
                payload,
                created_at
            "#,
        );
        let rows = builder.build().fetch_all(&mut *tx).await?;
        tx.commit().await?;

        let mut records = rows
            .into_iter()
            .map(map_run_event_record)
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.sequence);
        Ok(records)
    }

    async fn append_runtime_span(
        &self,
        input: &AppendRuntimeSpanInput,
    ) -> Result<domain::RuntimeSpanRecord> {
        let row = sqlx::query(
            r#"
            insert into runtime_spans (
                id,
                flow_run_id,
                node_run_id,
                parent_span_id,
                kind,
                name,
                status,
                capability_id,
                input_ref,
                output_ref,
                error_payload,
                metadata,
                started_at,
                finished_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            returning
                id,
                flow_run_id,
                node_run_id,
                parent_span_id,
                kind,
                name,
                status,
                capability_id,
                input_ref,
                output_ref,
                error_payload,
                metadata,
                started_at,
                finished_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(input.parent_span_id)
        .bind(input.kind.as_str())
        .bind(&input.name)
        .bind(input.status.as_str())
        .bind(input.capability_id.as_deref())
        .bind(input.input_ref.as_deref())
        .bind(input.output_ref.as_deref())
        .bind(&input.error_payload)
        .bind(&input.metadata)
        .bind(input.started_at)
        .bind(input.finished_at)
        .fetch_one(self.pool())
        .await?;

        map_runtime_span_record(row)
    }

    async fn append_runtime_event(
        &self,
        input: &AppendRuntimeEventInput,
    ) -> Result<domain::RuntimeEventRecord> {
        let mut tx = self.pool().begin().await?;
        lock_open_flow_run_for_event_append(&mut tx, input.flow_run_id).await?;
        lock_flow_run_event_sequence(&mut tx, input.flow_run_id).await?;
        let next_sequence = next_runtime_event_sequence(&mut tx, input.flow_run_id).await?;
        let row = sqlx::query(
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
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            returning
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
                durability,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(input.span_id)
        .bind(input.parent_span_id)
        .bind(next_sequence)
        .bind(&input.event_type)
        .bind(input.layer.as_str())
        .bind(input.source.as_str())
        .bind(input.trust_level.as_str())
        .bind(input.item_id)
        .bind(input.ledger_ref.as_deref())
        .bind(&input.payload)
        .bind(input.visibility.as_str())
        .bind(input.durability.as_str())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;

        map_runtime_event_record(row)
    }

    async fn append_runtime_events(
        &self,
        inputs: &[AppendRuntimeEventInput],
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        if inputs
            .iter()
            .any(|input| input.flow_run_id != inputs[0].flow_run_id)
        {
            let mut records = Vec::with_capacity(inputs.len());
            for input in inputs {
                records.push(self.append_runtime_event(input).await?);
            }
            return Ok(records);
        }

        let mut tx = self.pool().begin().await?;
        lock_open_flow_run_for_event_append(&mut tx, inputs[0].flow_run_id).await?;
        lock_flow_run_event_sequence(&mut tx, inputs[0].flow_run_id).await?;
        let first_sequence = next_runtime_event_sequence(&mut tx, inputs[0].flow_run_id).await?;
        let mut builder = QueryBuilder::<Postgres>::new(
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
            ) "#,
        );
        builder.push_values(inputs.iter().enumerate(), |mut row, (index, input)| {
            row.push_bind(Uuid::now_v7())
                .push_bind(input.flow_run_id)
                .push_bind(input.node_run_id)
                .push_bind(input.span_id)
                .push_bind(input.parent_span_id)
                .push_bind(first_sequence + index as i64)
                .push_bind(&input.event_type)
                .push_bind(input.layer.as_str())
                .push_bind(input.source.as_str())
                .push_bind(input.trust_level.as_str())
                .push_bind(input.item_id)
                .push_bind(input.ledger_ref.as_deref())
                .push_bind(&input.payload)
                .push_bind(input.visibility.as_str())
                .push_bind(input.durability.as_str());
        });
        builder.push(
            r#"
            returning
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
                durability,
                created_at
            "#,
        );
        let rows = builder.build().fetch_all(&mut *tx).await?;
        tx.commit().await?;

        let mut records = rows
            .into_iter()
            .map(map_runtime_event_record)
            .collect::<Result<Vec<_>>>()?;
        records.sort_by_key(|record| record.sequence);
        Ok(records)
    }

    async fn append_runtime_item(
        &self,
        input: &AppendRuntimeItemInput,
    ) -> Result<domain::RuntimeItemRecord> {
        let row = sqlx::query(
            r#"
            insert into runtime_items (
                id,
                flow_run_id,
                span_id,
                kind,
                status,
                source_event_id,
                input_ref,
                output_ref,
                usage_ledger_id,
                trust_level
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            returning
                id,
                flow_run_id,
                span_id,
                kind,
                status,
                source_event_id,
                input_ref,
                output_ref,
                usage_ledger_id,
                trust_level,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.span_id)
        .bind(input.kind.as_str())
        .bind(input.status.as_str())
        .bind(input.source_event_id)
        .bind(input.input_ref.as_deref())
        .bind(input.output_ref.as_deref())
        .bind(input.usage_ledger_id)
        .bind(input.trust_level.as_str())
        .fetch_one(self.pool())
        .await?;

        map_runtime_item_record(row)
    }

    async fn append_context_projection(
        &self,
        input: &AppendContextProjectionInput,
    ) -> Result<domain::ContextProjectionRecord> {
        let row = sqlx::query(
            r#"
            insert into runtime_context_projections (
                id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                projection_kind,
                merge_stage_ref,
                source_transcript_ref,
                source_item_refs,
                compaction_event_id,
                summary_version,
                model_input_ref,
                model_input_hash,
                compacted_summary_ref,
                previous_projection_id,
                token_estimate,
                provider_continuation_metadata
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            returning
                id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                projection_kind,
                merge_stage_ref,
                source_transcript_ref,
                source_item_refs,
                compaction_event_id,
                summary_version,
                model_input_ref,
                model_input_hash,
                compacted_summary_ref,
                previous_projection_id,
                token_estimate,
                provider_continuation_metadata,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(input.llm_turn_span_id)
        .bind(&input.projection_kind)
        .bind(input.merge_stage_ref.as_deref())
        .bind(input.source_transcript_ref.as_deref())
        .bind(&input.source_item_refs)
        .bind(input.compaction_event_id)
        .bind(input.summary_version.as_deref())
        .bind(&input.model_input_ref)
        .bind(&input.model_input_hash)
        .bind(input.compacted_summary_ref.as_deref())
        .bind(input.previous_projection_id)
        .bind(input.token_estimate)
        .bind(&input.provider_continuation_metadata)
        .fetch_one(self.pool())
        .await?;

        Ok(map_context_projection_record(row))
    }
}

async fn lock_open_flow_run_for_event_append(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    flow_run_id: Uuid,
) -> Result<()> {
    let status =
        sqlx::query_scalar::<_, String>("select status from flow_runs where id = $1 for update")
            .bind(flow_run_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    if matches!(
        status.as_str(),
        "succeeded" | "incomplete" | "failed" | "cancelled"
    ) {
        return Err(ControlPlaneError::Conflict("flow_run_terminal").into());
    }
    Ok(())
}

fn resume_timeline_description(payload: &serde_json::Value) -> Option<&str> {
    ["resume_request_id", "callback_task_id"]
        .into_iter()
        .find_map(|key| {
            payload
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
        })
}
