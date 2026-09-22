impl PgControlPlaneStore {
    async fn load_runtime_context_content_lineage(
        &self,
        context_version_id: Uuid,
    ) -> Result<Vec<RuntimeContextContentVersion>> {
        let rows = sqlx::query(
            r#"
            with recursive lineage as (
                select id, previous_projection_id, context_sequence, actual_content_id
                  from runtime_context_projections
                 where id = $1 and projection_kind = 'context_version'
                union all
                select parent.id, parent.previous_projection_id, parent.context_sequence,
                       parent.actual_content_id
                  from runtime_context_projections parent
                  join lineage child on child.previous_projection_id = parent.id
            )
            select lineage.id, lineage.context_sequence, runtime_original_json(runtime_canonical_contents.content, runtime_canonical_contents.raw_json_payloads, 'content') as content
              from lineage
              join runtime_canonical_contents on runtime_canonical_contents.id = lineage.actual_content_id
             order by lineage.context_sequence asc, lineage.id asc
            "#,
        )
        .bind(context_version_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| RuntimeContextContentVersion {
                context_version_id: row.get("id"),
                sequence: row.get("context_sequence"),
                content: row.get("content"),
            })
            .collect())
    }

    async fn persist_waiting_state(
        &self,
        input: &PersistWaitingStateInput,
    ) -> Result<Option<PersistedWaitingState>> {
        let target_status = match input.kind {
            PersistWaitingKind::Human => domain::FlowRunStatus::WaitingHuman,
            PersistWaitingKind::Callback(_) => domain::FlowRunStatus::WaitingCallback,
        };
        let mut tx = self.pool().begin().await?;
        let updated = sqlx::query_scalar::<_, Uuid>(
            r#"
            update flow_runs
               set status = $2,
                output_payload = ($3::jsonb -> 0),
                error_payload = null,
                finished_at = null,
                updated_at = now(),
                raw_json_payloads = (flow_runs.raw_json_payloads - 'output_payload' - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('output_payload', ($3::jsonb -> 1), 'error_payload', null))
            where id = $1 and application_id = $4 and scope_id = $5 and status = $6
             returning id
            "#,
        )
        .bind(input.flow_run_id)
        .bind(target_status.as_str())
        .bind(lossless_json_parameter(&(&input.output_payload)))
        .bind(input.application_id)
        .bind(input.scope_id)
        .bind(input.expected_status.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        if updated.is_none() {
            tx.rollback().await?;
            return Ok(None);
        }

        let (content_id, content_hash, _) = put_canonical_runtime_content_in_transaction(
            &mut tx,
            input.scope_id,
            input.application_id,
            &input.context_content,
        )
        .await?;
        let context_version_id = Uuid::now_v7();
        let context_sequence = sqlx::query_scalar::<_, i64>(
            "select coalesce(max(context_sequence), -1) + 1 from runtime_context_projections where flow_run_id = $1",
        )
        .bind(input.flow_run_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            insert into runtime_context_projections (
                id, flow_run_id, projection_kind, source_item_refs, model_input_ref,
                model_input_hash, provider_continuation_metadata, previous_projection_id,
                scope_id, application_id, context_sequence, transition_kind, transition_actor,
                declared_compaction_provenance, actual_content_id
            ) values (
                $1, $2, 'context_version', '[]'::jsonb,
                'runtime_canonical_content:' || $3::text, $4, '{}'::jsonb, $5,
                $6, $7, $8, $9, 'host', null, $3
            )
            "#,
        )
        .bind(context_version_id)
        .bind(input.flow_run_id)
        .bind(content_id)
        .bind(&content_hash)
        .bind(input.parent_context_version_id)
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(context_sequence)
        .bind(input.context_transition_kind.as_str())
        .execute(&mut *tx)
        .await?;
        let mut locator_payload = input.locator_payload.clone();
        locator_payload["context_version_id"] = json!(context_version_id);
        let mut variable_snapshot = input.variable_snapshot.clone();
        variable_snapshot["__runtime_recovery_context"]["context_version_id"] =
            json!(context_version_id);
        variable_snapshot["__runtime_recovery_context"]["sequence"] = json!(context_sequence);

        let checkpoint_row = sqlx::query(
            r#"
            insert into flow_run_checkpoints (
                id, scope_id, flow_run_id, node_run_id, status, reason,
                locator_payload, variable_snapshot, external_ref_payload
            , raw_json_payloads) values ( $1, $2, $3, $4, $5, $6, ($7::jsonb -> 0), ($8::jsonb -> 0), ($9::jsonb -> 0), jsonb_strip_nulls(jsonb_build_object('locator_payload', ($7::jsonb -> 1), 'variable_snapshot', ($8::jsonb -> 1), 'external_ref_payload', ($9::jsonb -> 1))) )
            returning id, flow_run_id, node_run_id, status, reason, runtime_original_json(locator_payload, flow_run_checkpoints.raw_json_payloads, 'locator_payload') as locator_payload,
                      runtime_original_json(variable_snapshot, flow_run_checkpoints.raw_json_payloads, 'variable_snapshot') as variable_snapshot, runtime_original_json(external_ref_payload, flow_run_checkpoints.raw_json_payloads, 'external_ref_payload') as external_ref_payload, created_at
            "#,
        )
        .bind(input.checkpoint_id)
        .bind(input.scope_id)
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(&input.checkpoint_status)
        .bind(&input.checkpoint_reason)
        .bind(lossless_json_parameter(&(&locator_payload)))
        .bind(lossless_json_parameter(&(&variable_snapshot)))
        .bind(lossless_json_parameter(&(&input.checkpoint_external_ref_payload)))
        .fetch_one(&mut *tx)
        .await?;
        let checkpoint = map_checkpoint_record(checkpoint_row);

        let callback_task = match &input.kind {
            PersistWaitingKind::Human => None,
            PersistWaitingKind::Callback(callback) => {
                let row = sqlx::query(
                    r#"
                    insert into flow_run_callback_tasks (
                        id, scope_id, flow_run_id, node_run_id, callback_kind, status,
                        request_payload, external_ref_payload
                    , raw_json_payloads) values ( $1, $2, $3, $4, $5, 'pending', ($6::jsonb -> 0), ($7::jsonb -> 0), jsonb_strip_nulls(jsonb_build_object('request_payload', ($6::jsonb -> 1), 'external_ref_payload', ($7::jsonb -> 1))) )
                    returning id, flow_run_id, node_run_id, callback_kind, status,
                              runtime_original_json(request_payload, flow_run_callback_tasks.raw_json_payloads, 'request_payload') as request_payload,
                              runtime_original_json(response_payload, flow_run_callback_tasks.raw_json_payloads, 'response_payload') as response_payload,
                              case when callback_kind = 'llm_tool_calls' then null
                                   else runtime_original_json(external_ref_payload, flow_run_callback_tasks.raw_json_payloads, 'external_ref_payload') end as external_ref_payload,
                              created_at, completed_at
                    "#,
                )
                .bind(callback.id)
                .bind(input.scope_id)
                .bind(input.flow_run_id)
                .bind(input.node_run_id)
                .bind(&callback.callback_kind)
                .bind(lossless_json_parameter(&(&callback.request_payload)))
                .bind(lossless_json_parameter(&(&callback.external_ref_payload)))
                .fetch_one(&mut *tx)
                .await?;
                let callback_task = map_callback_task_tool_summary_record(row)?;
                if callback.callback_kind == "llm_tool_calls" {
                    let tool_calls = callback
                        .request_payload
                        .get("tool_calls")
                        .and_then(Value::as_array)
                        .ok_or_else(|| {
                            anyhow!("llm tool callback request is missing tool_calls")
                        })?;
                    let mut ids = std::collections::BTreeSet::new();
                    for (ordinal, tool_call) in tool_calls.iter().enumerate() {
                        let tool_call_id = tool_call
                            .get("call_id")
                            .or_else(|| tool_call.get("id"))
                            .and_then(Value::as_str)
                            .filter(|id| !id.is_empty())
                            .ok_or_else(|| {
                                anyhow!("llm tool callback request has an unstable tool call id")
                            })?;
                        if !ids.insert(tool_call_id) {
                            return Err(anyhow!(
                                "llm tool callback request has duplicate tool call ids"
                            ));
                        }
                        sqlx::query(
                            r#"
                            insert into flow_run_tool_callback_inbox (
                                id, scope_id, application_id, flow_run_id, callback_task_id,
                                tool_call_id, tool_ordinal
                            ) values ($1, $2, $3, $4, $5, $6, $7)
                            "#,
                        )
                        .bind(Uuid::now_v7())
                        .bind(input.scope_id)
                        .bind(input.application_id)
                        .bind(input.flow_run_id)
                        .bind(callback.id)
                        .bind(tool_call_id)
                        .bind(i32::try_from(ordinal).map_err(|_| anyhow!("too many tool calls"))?)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                Some(callback_task)
            }
        };

        let mut tool_delivery_events = Vec::with_capacity(input.tool_delivery_events.len());
        for delivery in &input.tool_delivery_events {
            if delivery.flow_run_id != input.flow_run_id
                || delivery.event_type != "provider_output_item_done"
            {
                return Err(anyhow!(
                    "waiting transaction accepts only same-run provider_output_item_done deliveries"
                ));
            }
            let delivery_sequence = next_runtime_event_sequence(&mut tx, input.flow_run_id).await?;
            let row = sqlx::query(
                r#"
                insert into runtime_events (
                    id, scope_id, flow_run_id, node_run_id, span_id, parent_span_id, sequence,
                    event_type, layer, source, trust_level, item_id, ledger_ref, payload,
                    visibility, durability, delivery_status
                , raw_json_payloads) values ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, ($14::jsonb -> 0), $15, $16, 'pending', jsonb_strip_nulls(jsonb_build_object('payload', ($14::jsonb -> 1))) )
                returning id, flow_run_id, node_run_id, span_id, parent_span_id, sequence,
                          event_type, layer, source, trust_level, item_id, ledger_ref, runtime_original_json(payload, runtime_events.raw_json_payloads, 'payload') as payload,
                          visibility, durability, created_at
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(input.scope_id)
            .bind(input.flow_run_id)
            .bind(delivery.node_run_id)
            .bind(delivery.span_id)
            .bind(delivery.parent_span_id)
            .bind(delivery_sequence)
            .bind(&delivery.event_type)
            .bind(delivery.layer.as_str())
            .bind(delivery.source.as_str())
            .bind(delivery.trust_level.as_str())
            .bind(delivery.item_id)
            .bind(delivery.ledger_ref.as_deref())
            .bind(lossless_json_parameter(&(&delivery.payload)))
            .bind(delivery.visibility.as_str())
            .bind(delivery.durability.as_str())
            .fetch_one(&mut *tx)
            .await?;
            tool_delivery_events.push(map_runtime_event_record(row)?);
        }

        let event_sequence = next_runtime_event_sequence(&mut tx, input.flow_run_id).await?;
        let event_row = sqlx::query(
            r#"
            insert into runtime_events (
                id, scope_id, flow_run_id, node_run_id, span_id, parent_span_id, sequence,
                event_type, layer, source, trust_level, item_id, ledger_ref, payload,
                visibility, durability
            , raw_json_payloads) values ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, ($14::jsonb -> 0), $15, $16, jsonb_strip_nulls(jsonb_build_object('payload', ($14::jsonb -> 1))) )
            returning id, flow_run_id, node_run_id, span_id, parent_span_id, sequence,
                      event_type, layer, source, trust_level, item_id, ledger_ref, runtime_original_json(payload, runtime_events.raw_json_payloads, 'payload') as payload,
                      visibility, durability, created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.scope_id)
        .bind(input.flow_run_id)
        .bind(input.waiting_event.node_run_id)
        .bind(input.waiting_event.span_id)
        .bind(input.waiting_event.parent_span_id)
        .bind(event_sequence)
        .bind(&input.waiting_event.event_type)
        .bind(input.waiting_event.layer.as_str())
        .bind(input.waiting_event.source.as_str())
        .bind(input.waiting_event.trust_level.as_str())
        .bind(input.waiting_event.item_id)
        .bind(input.waiting_event.ledger_ref.as_deref())
        .bind(lossless_json_parameter(&(&input.waiting_event.payload)))
        .bind(input.waiting_event.visibility.as_str())
        .bind(input.waiting_event.durability.as_str())
        .fetch_one(&mut *tx)
        .await?;
        let waiting_event = map_runtime_event_record(event_row)?;

        let recovery_sequence = sqlx::query_scalar::<_, i64>(
            "select coalesce(max(sequence), -1) + 1 from flow_run_recovery_history where flow_run_id = $1",
        )
        .bind(input.flow_run_id)
        .fetch_one(&mut *tx)
        .await?;
        let resume_sequence = sqlx::query_scalar::<_, i64>(
            "select count(*) from flow_run_recovery_history where flow_run_id = $1 and state_code in ('waiting_callback', 'waiting_human')",
        )
        .bind(input.flow_run_id)
        .fetch_one(&mut *tx)
        .await?;
        let recovery_row = sqlx::query(
            r#"
            insert into flow_run_recovery_history (
                id, scope_id, application_id, flow_run_id, node_run_id, sequence, state_code,
                node_sequence, iteration_index, attempt_index, resume_sequence, event_sequence,
                context_version_id, recovery_content_id, idempotency_key
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, 0, 0, $9, $10, $11, $12, $13)
            returning id, scope_id, application_id, flow_run_id, node_run_id, sequence,
                      state_code, node_sequence, iteration_index, attempt_index,
                      resume_sequence, event_sequence, context_version_id, recovery_content_id,
                      idempotency_key, created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(recovery_sequence)
        .bind(target_status.as_str())
        .bind(
            input
                .locator_payload
                .get("next_node_index")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
        )
        .bind(resume_sequence)
        .bind(event_sequence)
        .bind(context_version_id)
        .bind(content_id)
        .bind(&input.recovery_idempotency_key)
        .fetch_one(&mut *tx)
        .await?;
        let recovery_history = map_recovery_history_record(recovery_row)?;
        match (input.resume_claim_id, input.resume_claim_token) {
            (Some(claim_id), Some(claim_token)) => {
                let updated = sqlx::query(
                    r#"
                    update flow_run_resume_claims
                       set status = 'succeeded', completed_at = now(), updated_at = now()
                     where id = $1 and claim_token = $2 and status = 'processing'
                    "#,
                )
                .bind(claim_id)
                .bind(claim_token)
                .execute(&mut *tx)
                .await?;
                if updated.rows_affected() != 1 {
                    return Err(ControlPlaneError::Conflict("resume_claim_not_owned").into());
                }
            }
            (None, None) => {}
            _ => {
                return Err(anyhow!(
                    "resume claim id and token must be provided together"
                ))
            }
        }
        Self::refresh_completed_output_projection(&mut tx, input.flow_run_id).await?;
        tx.commit().await?;

        let flow_run = self
            .get_flow_run(input.application_id, input.flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("persisted waiting flow run not found"))?;
        Ok(Some(PersistedWaitingState {
            flow_run,
            checkpoint,
            callback_task,
            waiting_event,
            tool_delivery_events,
            recovery_history,
        }))
    }
}
