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

async fn put_canonical_runtime_content_in_transaction(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    scope_id: Uuid,
    application_id: Uuid,
    content: &Value,
) -> Result<(Uuid, String, i64)> {
    let mut canonical = Vec::new();
    write_canonical_runtime_json(content, &mut canonical)?;
    let content_hash = format!("sha256:{:x}", Sha256::digest(&canonical));
    let byte_size = i64::try_from(canonical.len())?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("canonical-runtime:{application_id}:{content_hash}"))
        .execute(&mut **tx)
        .await?;
    let inserted = sqlx::query_scalar::<_, Uuid>(
        r#"
        insert into runtime_canonical_contents (
            id, scope_id, application_id, content_hash, content, byte_size
        )
        select $1, $2, applications.id, $3, $4, $5
          from applications
         where applications.id = $6 and applications.scope_id = $2
        on conflict (application_id, content_hash) do nothing
        returning id
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(scope_id)
    .bind(&content_hash)
    .bind(content)
    .bind(byte_size)
    .bind(application_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (content_id, stored_content, stored_byte_size) = match inserted {
        Some(content_id) => (content_id, content.clone(), byte_size),
        None => {
            sqlx::query_as::<_, (Uuid, Value, i64)>(
                r#"
            select id, content, byte_size
              from runtime_canonical_contents
             where scope_id = $1 and application_id = $2 and content_hash = $3
            "#,
            )
            .bind(scope_id)
            .bind(application_id)
            .bind(&content_hash)
            .fetch_one(&mut **tx)
            .await?
        }
    };
    if stored_content != *content || stored_byte_size != byte_size {
        return Err(anyhow!(
            "canonical runtime content hash collision for application {application_id}"
        ));
    }
    Ok((content_id, content_hash, byte_size))
}

fn recovery_state_for_flow_status(
    status: domain::FlowRunStatus,
) -> Option<domain::RecoveryStateCode> {
    match status {
        domain::FlowRunStatus::Running => Some(domain::RecoveryStateCode::Running),
        domain::FlowRunStatus::WaitingCallback => Some(domain::RecoveryStateCode::WaitingCallback),
        domain::FlowRunStatus::WaitingHuman => Some(domain::RecoveryStateCode::WaitingHuman),
        domain::FlowRunStatus::Paused => Some(domain::RecoveryStateCode::Paused),
        domain::FlowRunStatus::Succeeded => Some(domain::RecoveryStateCode::Succeeded),
        domain::FlowRunStatus::Failed => Some(domain::RecoveryStateCode::Failed),
        domain::FlowRunStatus::Cancelled => Some(domain::RecoveryStateCode::Cancelled),
        domain::FlowRunStatus::Queued | domain::FlowRunStatus::Incomplete => None,
    }
}

async fn append_flow_run_recovery_state_in_transaction(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    flow_run: &domain::FlowRunRecord,
) -> Result<()> {
    let Some(state_code) = recovery_state_for_flow_status(flow_run.status) else {
        return Ok(());
    };
    let Some((context_version_id, recovery_content_id, node_run_id)) =
        sqlx::query_as::<_, (Uuid, Uuid, Option<Uuid>)>(
            r#"
            select id, actual_content_id, node_run_id
              from runtime_context_projections
             where flow_run_id = $1 and projection_kind = 'context_version'
             order by context_sequence desc nulls last, created_at desc, id desc
             limit 1
            "#,
        )
        .bind(flow_run.id)
        .fetch_optional(&mut **tx)
        .await?
    else {
        return Ok(());
    };
    let last_state = sqlx::query_scalar::<_, String>(
        "select state_code from flow_run_recovery_history where flow_run_id = $1 order by sequence desc, id desc limit 1",
    )
    .bind(flow_run.id)
    .fetch_optional(&mut **tx)
    .await?;
    if last_state.as_deref() == Some(state_code.as_str()) {
        return Ok(());
    }
    let (sequence, node_sequence, iteration_index, attempt_index, resume_sequence) =
        sqlx::query_as::<_, (i64, i64, i64, i32, i64)>(
            r#"
            select coalesce(max(sequence), -1) + 1,
                   coalesce(max(node_sequence), 0),
                   coalesce(max(iteration_index), 0),
                   coalesce(max(attempt_index), 0),
                   coalesce(max(resume_sequence), 0)
              from flow_run_recovery_history where flow_run_id = $1
            "#,
        )
        .bind(flow_run.id)
        .fetch_one(&mut **tx)
        .await?;
    let event_sequence = sqlx::query_scalar::<_, i64>(
        "select coalesce(max(sequence), 0) from runtime_events where flow_run_id = $1",
    )
    .bind(flow_run.id)
    .fetch_one(&mut **tx)
    .await?;
    let scope_id = sqlx::query_scalar::<_, Uuid>(
        "select scope_id from flow_runs where id = $1 and application_id = $2",
    )
    .bind(flow_run.id)
    .bind(flow_run.application_id)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        insert into flow_run_recovery_history (
            id, scope_id, application_id, flow_run_id, node_run_id, sequence, state_code,
            node_sequence, iteration_index, attempt_index, resume_sequence, event_sequence,
            context_version_id, recovery_content_id, idempotency_key
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        on conflict (flow_run_id, idempotency_key) do nothing
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(scope_id)
    .bind(flow_run.application_id)
    .bind(flow_run.id)
    .bind(node_run_id)
    .bind(sequence)
    .bind(state_code.as_str())
    .bind(node_sequence)
    .bind(iteration_index)
    .bind(attempt_index)
    .bind(resume_sequence)
    .bind(event_sequence)
    .bind(context_version_id)
    .bind(recovery_content_id)
    .bind(format!("flow-state:{}:{sequence}", state_code.as_str()))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl PgControlPlaneStore {
    async fn append_provider_invocation_context(
        &self,
        input: &AppendProviderInvocationContextInput,
    ) -> Result<domain::ContextVersionRecord> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(input.flow_run_id.to_string())
            .execute(&mut *tx)
            .await?;
        if let Some(row) = sqlx::query(
            r#"
            select p.id, p.scope_id, p.application_id, p.flow_run_id,
                   p.previous_projection_id, p.context_sequence, p.transition_kind,
                   p.transition_actor, p.declared_compaction_provenance,
                   p.actual_content_id, p.created_at
              from runtime_invocation_context_bindings b
              join runtime_context_projections p on p.id = b.context_version_id
             where b.invocation_span_id = $1
            "#,
        )
        .bind(input.invocation_span_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            tx.commit().await?;
            return map_context_version_record(row);
        }
        let previous = sqlx::query(
            r#"
            select p.id, c.content
              from runtime_invocation_context_bindings b
              join runtime_context_projections p on p.id = b.context_version_id
              join runtime_canonical_contents c on c.id = p.actual_content_id
             where b.flow_run_id = $1
             order by b.created_at desc, b.invocation_span_id desc
             limit 1
            "#,
        )
        .bind(input.flow_run_id)
        .fetch_optional(&mut *tx)
        .await?;
        let parent_context_version_id: Option<Uuid> = previous.as_ref().map(|row| row.get("id"));
        let explicit = input
            .context_epoch
            .get("declaration")
            .and_then(Value::as_str)
            == Some("explicit");
        let observed_replacement = !explicit
            && previous.as_ref().is_some_and(|row| {
                let previous: Value = row.get("content");
                let old = previous.get("provider_messages").and_then(Value::as_array);
                let new = input
                    .actual_context
                    .get("provider_messages")
                    .and_then(Value::as_array);
                matches!((old, new), (Some(old), Some(new)) if !new.starts_with(old))
            });
        let transition_kind = if explicit {
            domain::ContextTransitionKind::DeclaredCompaction
        } else if observed_replacement {
            domain::ContextTransitionKind::ObservedReplacement
        } else if parent_context_version_id.is_some() {
            domain::ContextTransitionKind::Append
        } else {
            domain::ContextTransitionKind::Initial
        };
        let transition_actor = if explicit {
            domain::ContextTransitionActor::Client
        } else {
            domain::ContextTransitionActor::Host
        };
        let declared_provenance = explicit.then(|| input.context_epoch.clone());
        let (content_id, content_hash, _) = put_canonical_runtime_content_in_transaction(
            &mut tx,
            input.scope_id,
            input.application_id,
            &input.actual_context,
        )
        .await?;
        let sequence = sqlx::query_scalar::<_, i64>(
            "select coalesce(max(context_sequence), -1) + 1 from runtime_context_projections where flow_run_id = $1",
        )
        .bind(input.flow_run_id)
        .fetch_one(&mut *tx)
        .await?;
        let version_id = Uuid::now_v7();
        let row = sqlx::query(
            r#"
            insert into runtime_context_projections (
                id, flow_run_id, projection_kind, source_item_refs, model_input_ref,
                model_input_hash, provider_continuation_metadata, previous_projection_id,
                scope_id, application_id, context_sequence, transition_kind, transition_actor,
                declared_compaction_provenance, actual_content_id
            ) values (
                $1, $2, 'context_version', '[]'::jsonb,
                'runtime_canonical_content:' || $3::text, $4, '{}'::jsonb, $5,
                $6, $7, $8, $9, $10, $11, $3
            )
            returning id, scope_id, application_id, flow_run_id, previous_projection_id,
                      context_sequence, transition_kind, transition_actor,
                      declared_compaction_provenance, actual_content_id, created_at
            "#,
        )
        .bind(version_id)
        .bind(input.flow_run_id)
        .bind(content_id)
        .bind(content_hash)
        .bind(parent_context_version_id)
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(sequence)
        .bind(transition_kind.as_str())
        .bind(transition_actor.as_str())
        .bind(&declared_provenance)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            insert into runtime_invocation_context_bindings (
                invocation_span_id, scope_id, application_id, flow_run_id, context_version_id
            ) values ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(input.invocation_span_id)
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(input.flow_run_id)
        .bind(version_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        map_context_version_record(row)
    }

    async fn put_canonical_runtime_content(
        &self,
        input: &PutCanonicalRuntimeContentInput,
    ) -> Result<domain::CanonicalRuntimeContentRecord> {
        let mut tx = self.pool().begin().await?;
        let (content_id, _, _) = put_canonical_runtime_content_in_transaction(
            &mut tx,
            input.scope_id,
            input.application_id,
            &input.content,
        )
        .await?;
        let row = sqlx::query(
            r#"
            select id, scope_id, application_id, content_hash, content, byte_size, created_at
              from runtime_canonical_contents where id = $1
            "#,
        )
        .bind(content_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        let record = map_canonical_runtime_content_record(row);
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
