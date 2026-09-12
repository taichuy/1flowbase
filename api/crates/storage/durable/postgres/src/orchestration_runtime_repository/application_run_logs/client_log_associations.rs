impl PgControlPlaneStore {
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
        sqlx::query("update flow_runs set log_context=$2 where id=$1")
            .bind(run.id)
            .bind(value)
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
        self.ensure_application_run_conversation_message_items_projection_for_read(
            application_id,
            flow_run_id,
        )
        .await?;
        // Reuse the original persisted native message projection. A result may
        // arrive in a later call, but only within the same authenticated log
        // conversation and with the exact call ID and result type.
        sqlx::query_scalar(r#"
            select m.native_message || jsonb_build_object('tool_result',result.native_message->'_source_item')
            from application_run_conversation_message_items m
            left join lateral (
                select r.native_message from application_run_conversation_message_items r
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
        "#).bind(application_id).bind(flow_run_id).fetch_all(self.pool()).await.map_err(Into::into)
    }
}

impl PgControlPlaneStore {
    async fn expand_application_run_log_tasks(
        &self,
        application_id: Uuid,
        flow_run_ids: &[Uuid],
    ) -> Result<Vec<Uuid>> {
        let selected = flow_run_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let existing:i64=sqlx::query_scalar("select count(*) from application_run_log_summaries where application_id=$1 and flow_run_id=any($2)")
            .bind(application_id).bind(flow_run_ids).fetch_one(self.pool()).await?;
        if existing as usize != selected.len() {
            return Err(ControlPlaneError::NotFound("flow_run").into());
        }
        sqlx::query_scalar(r#"select member.run_id
            from unnest($2::uuid[]) with ordinality requested(id,position)
            cross join lateral application_run_log_task_runs($1,requested.id) member
            group by member.run_id order by min(requested.position),member.run_id"#)
            .bind(application_id).bind(flow_run_ids).fetch_all(self.pool()).await.map_err(Into::into)
    }
}
