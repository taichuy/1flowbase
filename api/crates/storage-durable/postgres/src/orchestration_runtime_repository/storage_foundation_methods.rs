use sha2::{Digest, Sha256};

fn write_canonical_runtime_json(
    value: &serde_json::Value,
    output: &mut Vec<u8>,
) -> serde_json::Result<()> {
    match value {
        serde_json::Value::Object(object) => {
            output.push(b'{');
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key)?;
                output.push(b':');
                write_canonical_runtime_json(&object[key], output)?;
            }
            output.push(b'}');
        }
        serde_json::Value::Array(items) => {
            output.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical_runtime_json(item, output)?;
            }
            output.push(b']');
        }
        scalar => serde_json::to_writer(output, scalar)?,
    }
    Ok(())
}

impl PgControlPlaneStore {
    async fn put_canonical_runtime_content(
        &self,
        input: &PutCanonicalRuntimeContentInput,
    ) -> Result<domain::CanonicalRuntimeContentRecord> {
        let mut canonical = Vec::new();
        write_canonical_runtime_json(&input.content, &mut canonical)?;
        let content_hash = format!("sha256:{:x}", Sha256::digest(&canonical));
        let byte_size = i64::try_from(canonical.len())?;
        let row = sqlx::query(
            r#"
            insert into runtime_canonical_contents (
                id, scope_id, application_id, content_hash, content, byte_size
            )
            select $1, $2, applications.id, $3, $4, $5
              from applications
             where applications.id = $6 and applications.scope_id = $2
            on conflict (application_id, content_hash) do nothing
            returning id, scope_id, application_id, content_hash, content, byte_size, created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.scope_id)
        .bind(&content_hash)
        .bind(&input.content)
        .bind(byte_size)
        .bind(input.application_id)
        .fetch_optional(self.pool())
        .await?;

        let row = match row {
            Some(row) => row,
            None => {
                sqlx::query(
                    r#"
                select id, scope_id, application_id, content_hash, content, byte_size, created_at
                  from runtime_canonical_contents
                 where scope_id = $1 and application_id = $2 and content_hash = $3
                "#,
                )
                .bind(input.scope_id)
                .bind(input.application_id)
                .bind(&content_hash)
                .fetch_one(self.pool())
                .await?
            }
        };
        let record = map_canonical_runtime_content_record(row);
        if record.content != input.content {
            return Err(anyhow!(
                "canonical runtime content hash collision for application {}",
                input.application_id
            ));
        }
        Ok(record)
    }

    async fn append_context_version(
        &self,
        input: &AppendContextVersionInput,
    ) -> Result<domain::ContextVersionRecord> {
        let row = sqlx::query(
            r#"
            insert into runtime_context_projections (
                id, flow_run_id, projection_kind, source_item_refs, model_input_ref,
                model_input_hash, provider_continuation_metadata, previous_projection_id,
                scope_id, application_id, context_sequence, transition_kind, transition_actor,
                declared_compaction_provenance, actual_content_id
            )
            select $1, flow_runs.id, 'context_version', '[]'::jsonb,
                   'runtime_canonical_content:' || runtime_canonical_contents.id::text,
                   runtime_canonical_contents.content_hash, '{}'::jsonb, $2,
                   $3, $4, $5, $6, $7, $8, runtime_canonical_contents.id
              from flow_runs
              join runtime_canonical_contents
                on runtime_canonical_contents.id = $9
               and runtime_canonical_contents.scope_id = $3
               and runtime_canonical_contents.application_id = $4
             where flow_runs.id = $10
               and flow_runs.scope_id = $3
               and flow_runs.application_id = $4
            returning id, scope_id, application_id, flow_run_id, previous_projection_id,
                      context_sequence, transition_kind, transition_actor,
                      declared_compaction_provenance, actual_content_id, created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.parent_context_version_id)
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(input.sequence)
        .bind(input.transition_kind.as_str())
        .bind(input.transition_actor.as_str())
        .bind(&input.declared_compaction_provenance)
        .bind(input.actual_content_id)
        .bind(input.flow_run_id)
        .fetch_one(self.pool())
        .await?;
        map_context_version_record(row)
    }

    async fn bind_invocation_context(
        &self,
        input: &BindInvocationContextInput,
    ) -> Result<domain::InvocationContextBindingRecord> {
        let row = sqlx::query(
            r#"
            insert into runtime_invocation_context_bindings (
                invocation_span_id, scope_id, application_id, flow_run_id, context_version_id
            )
            select runtime_spans.id, $1, $2, flow_runs.id, runtime_context_projections.id
              from runtime_spans
              join flow_runs on flow_runs.id = runtime_spans.flow_run_id
              join runtime_context_projections
                on runtime_context_projections.id = $3
               and runtime_context_projections.flow_run_id = flow_runs.id
               and runtime_context_projections.scope_id = $1
               and runtime_context_projections.application_id = $2
             where runtime_spans.id = $4
               and flow_runs.id = $5
               and flow_runs.scope_id = $1
               and flow_runs.application_id = $2
            on conflict (invocation_span_id) do nothing
            returning invocation_span_id, scope_id, application_id, flow_run_id,
                      context_version_id, created_at
            "#,
        )
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(input.context_version_id)
        .bind(input.invocation_span_id)
        .bind(input.flow_run_id)
        .fetch_optional(self.pool())
        .await?;
        let row = match row {
            Some(row) => row,
            None => {
                sqlx::query(
                    r#"
                select invocation_span_id, scope_id, application_id, flow_run_id,
                       context_version_id, created_at
                  from runtime_invocation_context_bindings
                 where invocation_span_id = $1 and scope_id = $2 and application_id = $3
                   and flow_run_id = $4 and context_version_id = $5
                "#,
                )
                .bind(input.invocation_span_id)
                .bind(input.scope_id)
                .bind(input.application_id)
                .bind(input.flow_run_id)
                .bind(input.context_version_id)
                .fetch_one(self.pool())
                .await?
            }
        };
        Ok(map_invocation_context_binding_record(row))
    }

    async fn append_recovery_history(
        &self,
        input: &AppendRecoveryHistoryInput,
    ) -> Result<domain::RecoveryHistoryRecord> {
        let row = sqlx::query(
            r#"
            insert into flow_run_recovery_history (
                id, scope_id, application_id, flow_run_id, node_run_id, sequence, state_code,
                node_sequence, iteration_index, attempt_index, resume_sequence, event_sequence,
                context_version_id, recovery_content_id, idempotency_key
            )
            select $1, $2, $3, flow_runs.id, $4, $5, $6, $7, $8, $9, $10, $11,
                   runtime_context_projections.id, $12, $13
              from flow_runs
              join runtime_context_projections
                on runtime_context_projections.id = $14
               and runtime_context_projections.flow_run_id = flow_runs.id
               and runtime_context_projections.scope_id = $2
               and runtime_context_projections.application_id = $3
             where flow_runs.id = $15
               and flow_runs.scope_id = $2
               and flow_runs.application_id = $3
               and ($12::uuid is null or exists (
                   select 1 from runtime_canonical_contents
                    where id = $12 and scope_id = $2 and application_id = $3
               ))
            on conflict do nothing
            returning id, scope_id, application_id, flow_run_id, node_run_id, sequence,
                      state_code, node_sequence, iteration_index, attempt_index,
                      resume_sequence, event_sequence, context_version_id, recovery_content_id,
                      idempotency_key, created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(input.node_run_id)
        .bind(input.sequence)
        .bind(input.state_code.as_str())
        .bind(input.coordinate.node_sequence)
        .bind(input.coordinate.iteration_index)
        .bind(input.coordinate.attempt_index)
        .bind(input.coordinate.resume_sequence)
        .bind(input.coordinate.event_sequence)
        .bind(input.recovery_content_id)
        .bind(&input.idempotency_key)
        .bind(input.context_version_id)
        .bind(input.flow_run_id)
        .fetch_optional(self.pool())
        .await?;
        let row = match row {
            Some(row) => row,
            None => {
                sqlx::query(
                    r#"
                select id, scope_id, application_id, flow_run_id, node_run_id, sequence,
                       state_code, node_sequence, iteration_index, attempt_index,
                       resume_sequence, event_sequence, context_version_id, recovery_content_id,
                       idempotency_key, created_at
                  from flow_run_recovery_history
                 where flow_run_id = $1 and idempotency_key = $2 and scope_id = $3
                   and application_id = $4 and node_run_id is not distinct from $5
                   and sequence = $6 and state_code = $7 and node_sequence = $8
                   and iteration_index = $9 and attempt_index = $10
                   and resume_sequence = $11 and event_sequence = $12
                   and context_version_id = $13
                   and recovery_content_id is not distinct from $14
                "#,
                )
                .bind(input.flow_run_id)
                .bind(&input.idempotency_key)
                .bind(input.scope_id)
                .bind(input.application_id)
                .bind(input.node_run_id)
                .bind(input.sequence)
                .bind(input.state_code.as_str())
                .bind(input.coordinate.node_sequence)
                .bind(input.coordinate.iteration_index)
                .bind(input.coordinate.attempt_index)
                .bind(input.coordinate.resume_sequence)
                .bind(input.coordinate.event_sequence)
                .bind(input.context_version_id)
                .bind(input.recovery_content_id)
                .fetch_one(self.pool())
                .await?
            }
        };
        map_recovery_history_record(row)
    }
}
