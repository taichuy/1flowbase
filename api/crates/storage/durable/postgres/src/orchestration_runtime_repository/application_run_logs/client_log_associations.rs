impl PgControlPlaneStore {
    async fn supersede_callback_predecessors_in_transaction(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        successor: &domain::FlowRunRecord,
        context: &control_plane_contracts::ports::ApplicationRunLogContext,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<Uuid>> {
        if context.identity_status != "identified" {
            return Ok(Vec::new());
        }
        let (Some(protocol), Some(thread_id), Some(turn_id), Some(api_key_id)) = (
            context.protocol.as_deref(),
            context.thread_id.as_deref(),
            context.turn_id.as_deref(),
            successor.api_key_id,
        ) else {
            return Ok(Vec::new());
        };
        let task_run_id: Option<String> =
            sqlx::query_scalar("select log_context->>'log_task_run_id' from flow_runs where id=$1")
                .bind(successor.id)
                .fetch_optional(&mut **tx)
                .await?
                .flatten();
        let Some(task_run_id) = task_run_id else {
            return Ok(Vec::new());
        };
        let error_payload = json!({
            "code": "cancelled",
            "message": "published run superseded by local summary compaction",
            "reason": "local_summary_superseded",
            "successor_flow_run_id": successor.id,
        });
        let rows = sqlx::query(
            r#"
            update flow_runs
            set status='cancelled',
                error_payload = ($9::jsonb -> 0),
                finished_at=$8,
                updated_at=$8,
                raw_json_payloads = (flow_runs.raw_json_payloads - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('error_payload', ($9::jsonb -> 1)))
            where id<>$1
              and application_id=$2
              and api_key_id=$3
              and created_by=$4
              and coalesce(external_user,'')=coalesce($5::text,'')
              and compatibility_mode='openai-responses-v1'
              and status='waiting_callback'
              and log_context->>'protocol'=$6
              and log_context->>'thread_id'=$7
              and log_context->>'turn_id'=$10
              and log_context->>'log_task_run_id'=$11
            returning id,application_id,flow_id,flow_draft_id,compiled_plan_id,
              debug_session_id,flow_schema_version,document_hash,run_mode,target_node_id,
              title,status,runtime_original_json(input_payload, flow_runs.raw_json_payloads, 'input_payload') as input_payload,runtime_original_json(output_payload, flow_runs.raw_json_payloads, 'output_payload') as output_payload,runtime_original_json(error_payload, flow_runs.raw_json_payloads, 'error_payload') as error_payload,created_by,
              null::text as authorized_account,api_key_id,publication_version_id,
              external_user,external_conversation_id,external_trace_id,compatibility_mode,
              idempotency_key,started_at,finished_at,created_at,updated_at
            "#,
        )
        .bind(successor.id)
        .bind(successor.application_id)
        .bind(api_key_id)
        .bind(successor.created_by)
        .bind(successor.external_user.as_deref())
        .bind(protocol)
        .bind(thread_id)
        .bind(completed_at)
        .bind(lossless_json_parameter(&(&error_payload)))
        .bind(turn_id)
        .bind(&task_run_id)
        .fetch_all(&mut **tx)
        .await?;

        let mut superseded_ids = Vec::with_capacity(rows.len());
        for row in rows {
            let predecessor = map_flow_run_record(row)?;
            superseded_ids.push(predecessor.id);
            sqlx::query(
                "update node_runs set status='cancelled', finished_at=coalesce(finished_at,$2) where flow_run_id=$1 and status not in ('succeeded','failed','cancelled','skipped')",
            )
            .bind(predecessor.id)
            .bind(completed_at)
            .execute(&mut **tx)
            .await?;
            let callbacks = sqlx::query(
                "update flow_run_callback_tasks set status='cancelled', completed_at=$2 where flow_run_id=$1 and status='pending' returning id,node_run_id,callback_kind",
            )
            .bind(predecessor.id)
            .bind(completed_at)
            .fetch_all(&mut **tx)
            .await?;
            sqlx::query(
                "update flow_run_callback_resume_attempts set status='cancelled',\n                completed_at=$2,\n                updated_at=$2,\n                error_payload = ($3::jsonb -> 0),\n                raw_json_payloads = (flow_run_callback_resume_attempts.raw_json_payloads - 'error_payload') || jsonb_strip_nulls(jsonb_build_object('error_payload', ($3::jsonb -> 1)))\n            where flow_run_id=$1 and status in ('received','processing')",
            )
            .bind(predecessor.id)
            .bind(completed_at)
            .bind(lossless_json_parameter(&(&error_payload)))
            .execute(&mut **tx)
            .await?;

            Self::upsert_application_run_log_summary_projection_for_flow_run(tx, &predecessor)
                .await?;
            // Supersession is a terminal writer too: retain the cost before ledger cleanup.
            sqlx::query(include_str!("cost_snapshot.sql"))
                .bind(predecessor.id)
                .execute(&mut **tx)
                .await?;
            Self::replace_application_run_conversation_message_items_projection(tx, &predecessor)
                .await?;
            let scope_id = flow_run_scope_id_for_update(tx, predecessor.id).await?;
            let sequence = next_event_sequence(tx, predecessor.id).await?;
            sqlx::query(
                "insert into flow_run_events(id,scope_id,flow_run_id,node_run_id,sequence,event_type,payload,resume_timeline_description,resume_timeline_description_projected, raw_json_payloads) values( $1, $2, $3, null, $4, 'flow_run_cancelled', ($5::jsonb -> 0), null, true, jsonb_strip_nulls(jsonb_build_object('payload', ($5::jsonb -> 1))) )",
            )
            .bind(Uuid::now_v7())
            .bind(scope_id)
            .bind(predecessor.id)
            .bind(sequence)
            .bind(lossless_json_parameter(&(&error_payload)))
            .execute(&mut **tx)
            .await?;
            for callback in callbacks {
                let callback_id: Uuid = callback.get("id");
                let node_run_id: Uuid = callback.get("node_run_id");
                let callback_kind: String = callback.get("callback_kind");
                let sequence = next_event_sequence(tx, predecessor.id).await?;
                sqlx::query(
                    "insert into flow_run_events(id,scope_id,flow_run_id,node_run_id,sequence,event_type,payload,resume_timeline_description,resume_timeline_description_projected, raw_json_payloads) values( $1, $2, $3, $4, $5, 'public_run_callback_cancelled', ($6::jsonb -> 0), null, true, jsonb_strip_nulls(jsonb_build_object('payload', ($6::jsonb -> 1))) )",
                )
                .bind(Uuid::now_v7())
                .bind(scope_id)
                .bind(predecessor.id)
                .bind(node_run_id)
                .bind(sequence)
                .bind(lossless_json_parameter(&(json!({
                    "callback_task_id": callback_id,
                    "callback_kind": callback_kind,
                    "reason": "local_summary_superseded",
                    "successor_flow_run_id": successor.id,
                }))))
                .execute(&mut **tx)
                .await?;
            }
            let runtime_sequence = next_runtime_event_sequence(tx, predecessor.id).await?;
            sqlx::query(
                "insert into runtime_events(id,flow_run_id,node_run_id,span_id,parent_span_id,sequence,event_type,layer,source,trust_level,item_id,ledger_ref,payload,visibility,durability, raw_json_payloads) values( $1, $2, null, null, null, $3, 'flow_cancelled', 'agent_transition', 'host', 'host_fact', null, null, ($4::jsonb -> 0), 'workspace', 'durable', jsonb_strip_nulls(jsonb_build_object('payload', ($4::jsonb -> 1))) )",
            )
            .bind(Uuid::now_v7())
            .bind(predecessor.id)
            .bind(runtime_sequence)
            .bind(lossless_json_parameter(&(&error_payload)))
            .execute(&mut **tx)
            .await?;
            append_flow_run_recovery_state_in_transaction(tx, &predecessor).await?;
            Self::refresh_application_run_log_task_for_flow_run(tx, predecessor.id).await?;
        }
        Ok(superseded_ids)
    }

    async fn bind_application_run_log_context(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        run: &domain::FlowRunRecord,
        context: &control_plane_contracts::ports::ApplicationRunLogContext,
    ) -> Result<()> {
        let key = run
            .api_key_id
            .ok_or_else(|| anyhow!("client log identity requires authenticated API key"))?;
        let value_protocol = context
            .protocol
            .as_deref()
            .ok_or_else(|| anyhow!("client log identity requires the declaring protocol"))?
            .to_owned();
        let mut value = serde_json::to_value(context)?;
        if matches!(
            context.identity_status.as_str(),
            "identified" | "unknown_turn"
        ) {
            if let Some(thread) = context.thread_id.as_deref() {
                let candidate = Uuid::now_v7();
                // This is the pre-existing LOG conversation, never the public
                // conversation whose messages participate in model inference.
                let conversation:Uuid=sqlx::query_scalar("insert into application_conversations(id,scope_id,application_id,api_key_id,external_user,external_conversation_id,client_protocol,client_thread_id,created_at,updated_at) select $1,workspace_id,id,$3,$4,$5,$8,$6,$7,$7 from applications where id=$2 on conflict(application_id,api_key_id,(coalesce(external_user,'')),client_protocol,client_thread_id) where client_thread_id is not null do update set updated_at=greatest(application_conversations.updated_at,excluded.updated_at) returning id")
                    .bind(candidate).bind(run.application_id).bind(key).bind(run.external_user.as_deref())
                    .bind(format!("log:{value_protocol}:{candidate}")).bind(thread).bind(run.started_at).bind(&value_protocol).fetch_one(&mut **tx).await?;
                value["log_conversation_id"] = json!(conversation);
                // The conversation upsert holds its row lock through commit.
                // Concurrent first calls in a task cannot choose different anchors.
                if let Some(turn) = context.turn_id.as_deref() {
                    let anchor:Option<Uuid>=sqlx::query_scalar("select (log_context->>'log_task_run_id')::uuid from flow_runs where application_id=$1 and api_key_id=$2 and log_context->>'log_conversation_id'=$3 and log_context->>'turn_id'=$4 and log_context->>'log_task_run_id' is not null order by id limit 1")
                        .bind(run.application_id).bind(key).bind(conversation.to_string()).bind(turn).fetch_optional(&mut **tx).await?;
                    value["log_task_run_id"] = json!(anchor.unwrap_or(run.id));
                    if anchor.is_none() {
                        if let Some(parent_thread) = context
                            .parent_thread_id
                            .as_deref()
                            .or(context.forked_from_thread_id.as_deref())
                            .or_else(|| context.parent_turn_id.as_ref().map(|_| thread))
                        {
                            let parent:Option<Uuid>=sqlx::query_scalar("select f.id from flow_runs f join application_conversations c on c.id=(f.log_context->>'log_conversation_id')::uuid where c.application_id=$1 and c.api_key_id=$2 and coalesce(c.external_user,'')=coalesce($3::text,'') and c.client_protocol=$6 and c.client_thread_id=$4 and ($5::text is null or f.log_context->>'turn_id'=$5) order by f.id limit 1")
                                .bind(run.application_id).bind(key).bind(run.external_user.as_deref()).bind(parent_thread).bind(context.parent_turn_id.as_deref()).bind(&value_protocol).fetch_optional(&mut **tx).await?;
                            value["parent_run_id"] = json!(parent);
                            value["relation_status"] = json!(if parent.is_some() {
                                "resolved_parent"
                            } else {
                                "declared_parent_unresolved"
                            });
                        }
                    }
                }
            }
        }
        if let Some(previous) = context
            .previous_response_id
            .as_deref()
            .and_then(|v| v.strip_prefix("resp_"))
            .and_then(|v| Uuid::parse_str(v).ok())
        {
            let cause:Option<Uuid>=sqlx::query_scalar("select id from flow_runs where id=$1 and id<>$2 and application_id=$3 and api_key_id=$4 and coalesce(external_user,'')=coalesce($5::text,'')")
                .bind(previous).bind(run.id).bind(run.application_id).bind(key).bind(run.external_user.as_deref()).fetch_optional(&mut **tx).await?;
            value["caused_by_run_id"] = json!(cause);
        }
        sqlx::query("update flow_runs set log_context=($2::jsonb -> 0), raw_json_payloads=(raw_json_payloads - 'log_context') || jsonb_strip_nulls(jsonb_build_object('log_context', $2::jsonb -> 1)) where id=$1")
            .bind(run.id)
            .bind(lossless_json_parameter(&value))
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

impl PgControlPlaneStore {
    async fn application_run_native_trace_messages(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Vec<Value>> {
        // Reuse the original persisted native message projection. A result may
        // arrive in a later call, but only within the same authenticated log
        // conversation and with the exact call ID and result type.
        let rows = sqlx::query(r#"
            select runtime_original_json(m.native_message, m.raw_json_payloads, 'native_message') as native_message, result.native_message as result_message
            from application_run_conversation_message_items m
            left join lateral (
                select runtime_original_json(r.native_message, r.raw_json_payloads, 'native_message') as native_message from application_run_conversation_message_items r
                where r.application_id=m.application_id and r.scope_id=m.scope_id
                  and r.log_conversation_id=m.log_conversation_id
                  and r.source_item_key='result:'||(m.native_message#>>'{_source_item,call_id}')
                  and r.native_message#>>'{_source_item,type}'=case m.native_message#>>'{_source_item,type}'
                      when 'custom_tool_call' then 'custom_tool_call_output'
                      when 'function_call' then 'function_call_output' end
                  and not coalesce((r.native_message->>'_log_conflicting')::boolean,false)
                limit 1
            ) result on not coalesce((m.native_message->>'_log_conflicting')::boolean,false)
            where m.application_id=$1 and m.flow_run_id=$2 and m.source_item_key like 'output:%'
            order by m.display_sequence,m.id
        "#).bind(application_id).bind(flow_run_id).fetch_all(self.pool()).await?;
        rows.into_iter()
            .map(|row| {
                let mut message: Value = row.try_get("native_message")?;
                let result: Option<Value> = row.try_get("result_message")?;
                message["tool_result"] = result
                    .as_ref()
                    .and_then(|value| value.get("_source_item"))
                    .cloned()
                    .unwrap_or(Value::Null);
                Ok(message)
            })
            .collect()
    }
}
