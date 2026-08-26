impl PgControlPlaneStore {
    async fn get_application_run_trace_projection_source_watermark(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<String>> {
        let Some(flow_run) = sqlx::query(
            r#"
            select
                id,
                application_id,
                external_conversation_id,
                external_user,
                api_key_id,
                compatibility_mode,
                started_at,
                updated_at
            from flow_runs
            where application_id = $1
              and id = $2
            "#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?
        else {
            return Ok(None);
        };

        let flow_run_updated_at: OffsetDateTime = flow_run.get("updated_at");
        let node_run_count = sqlx::query_scalar::<_, i64>(
            "select count(*) from node_runs where flow_run_id = $1",
        )
        .bind(flow_run_id)
        .fetch_one(self.pool())
        .await?;
        let callback_task_count = sqlx::query_scalar::<_, i64>(
            "select count(*) from flow_run_callback_tasks where flow_run_id = $1",
        )
        .bind(flow_run_id)
        .fetch_one(self.pool())
        .await?;
        let event_count = sqlx::query_scalar::<_, i64>(
            "select count(*) from flow_run_events where flow_run_id = $1",
        )
        .bind(flow_run_id)
        .fetch_one(self.pool())
        .await?;
        let external_conversation_id: Option<String> = flow_run.get("external_conversation_id");
        let external_user: Option<String> = flow_run.get("external_user");
        let api_key_id: Option<Uuid> = flow_run.get("api_key_id");
        let compatibility_mode: Option<String> = flow_run.get("compatibility_mode");
        let stitched_trace_count = if let (Some(external_conversation_id), Some(external_user)) = (
            external_conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            external_user
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        ) {
            let current_started_at: OffsetDateTime = flow_run.get("started_at");

            sqlx::query_scalar::<_, i64>(
                r#"
                select count(*)
                from flow_runs prior
                where prior.application_id = $1
                  and prior.external_conversation_id = $2
                  and prior.id <> $3
                  and prior.started_at < $4
                  and prior.external_user = $5
                  and prior.api_key_id is not distinct from $6
                  and prior.compatibility_mode is not distinct from $7
                  and prior.status in ('cancelled', 'waiting_callback')
                  and (
                      prior.import_job_id is null
                      or exists (
                          select 1
                          from run_archive_import_jobs prior_import_jobs
                          where prior_import_jobs.id = prior.import_job_id
                            and prior_import_jobs.status = 'succeeded'
                      )
                  )
                  and not exists (
                      select 1
                      from flow_runs boundary
                      where boundary.application_id = prior.application_id
                        and boundary.external_conversation_id = prior.external_conversation_id
                        and boundary.external_user = prior.external_user
                        and boundary.api_key_id is not distinct from prior.api_key_id
                        and boundary.compatibility_mode is not distinct from prior.compatibility_mode
                        and boundary.id <> $3
                        and boundary.started_at > prior.started_at
                        and boundary.started_at < $4
                        and boundary.status in ('succeeded', 'failed')
                        and (
                            boundary.import_job_id is null
                            or exists (
                                select 1
                                from run_archive_import_jobs boundary_import_jobs
                                where boundary_import_jobs.id = boundary.import_job_id
                                  and boundary_import_jobs.status = 'succeeded'
                            )
                        )
                  )
                "#,
            )
            .bind(application_id)
            .bind(external_conversation_id)
            .bind(flow_run_id)
            .bind(current_started_at)
            .bind(external_user)
            .bind(api_key_id)
            .bind(compatibility_mode.as_deref())
            .fetch_one(self.pool())
            .await?
        } else {
            0
        };
        let subagent_trace_count = if let Some(external_conversation_id) = external_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter(|_| compatibility_mode.as_deref() == Some("anthropic-messages-v1"))
        {
            sqlx::query_scalar::<_, i64>(
                r#"
                with agent_calls as (
                    select
                        callback_tasks.id as callback_task_id,
                        callback_tasks.created_at,
                        callback_tasks.completed_at,
                        coalesce(
                            agent_tool_calls.tool_call ->> 'id',
                            agent_tool_calls.tool_call ->> 'tool_call_id',
                            agent_tool_calls.tool_call ->> 'call_id',
                            md5(agent_tool_calls.tool_call::text)
                        ) as tool_call_key,
                        agent_tool_calls.tool_call #>> '{arguments,prompt}' as prompt
                    from flow_run_callback_tasks callback_tasks
                    cross join lateral jsonb_array_elements(
                        coalesce(callback_tasks.request_payload -> 'tool_calls', '[]'::jsonb)
                    ) as agent_tool_calls(tool_call)
                    where callback_tasks.flow_run_id = $3
                      and callback_tasks.callback_kind = 'llm_tool_calls'
                      and agent_tool_calls.tool_call ->> 'name' = 'Agent'
                      and nullif(trim(agent_tool_calls.tool_call #>> '{arguments,prompt}'), '') is not null
                ),
                matched_agent_calls as (
                    select
                        agent_calls.callback_task_id,
                        agent_calls.tool_call_key,
                        count(candidate.id)::bigint as candidate_count
                    from agent_calls
                    left join flow_runs candidate
                      on candidate.application_id = $1
                     and candidate.id <> $3
                     and candidate.external_conversation_id = $2
                     and candidate.external_user is not distinct from $4
                     and candidate.api_key_id is not distinct from $5
                     and candidate.compatibility_mode is not distinct from $6
                     and candidate.compatibility_mode = 'anthropic-messages-v1'
                     and candidate.started_at >= agent_calls.created_at
                     and (
                         agent_calls.completed_at is null
                         or candidate.started_at <= agent_calls.completed_at
                     )
                     and coalesce(
                         candidate.input_payload #>> '{node-start,query}',
                         candidate.input_payload #>> '{start,query}',
                         candidate.input_payload #>> '{query}',
                         ''
                     ) = agent_calls.prompt
                     and (
                         position('cc_is_subagent=true' in coalesce(
                             candidate.input_payload #>> '{node-start,system}',
                             candidate.input_payload #>> '{start,system}',
                             candidate.input_payload #>> '{system}',
                             ''
                         )) > 0
                         or (
                             position('Agent threads always have their cwd reset between bash calls' in coalesce(
                                 candidate.input_payload #>> '{node-start,system}',
                                 candidate.input_payload #>> '{start,system}',
                                 candidate.input_payload #>> '{system}',
                                 ''
                             )) > 0
                             and position('the parent agent reads your text output' in coalesce(
                                 candidate.input_payload #>> '{node-start,system}',
                                 candidate.input_payload #>> '{start,system}',
                                 candidate.input_payload #>> '{system}',
                                 ''
                             )) > 0
                         )
                     )
                     and (
                         candidate.import_job_id is null
                         or exists (
                             select 1
                             from run_archive_import_jobs import_jobs
                             where import_jobs.id = candidate.import_job_id
                               and import_jobs.status = 'succeeded'
                         )
                     )
                    group by agent_calls.callback_task_id, agent_calls.tool_call_key
                )
                select count(*)::bigint
                from matched_agent_calls
                where candidate_count = 1
                "#,
            )
            .bind(application_id)
            .bind(external_conversation_id)
            .bind(flow_run_id)
            .bind(external_user.as_deref())
            .bind(api_key_id)
            .bind(compatibility_mode.as_deref())
            .fetch_one(self.pool())
            .await?
        } else {
            0
        };

        Ok(Some(
            control_plane::orchestration_runtime::trace_projection::trace_projection_source_watermark_from_counts(
                flow_run_updated_at,
                usize::try_from(node_run_count)
                    .map_err(|_| anyhow!("node_run_count must fit usize"))?,
                usize::try_from(callback_task_count)
                    .map_err(|_| anyhow!("callback_task_count must fit usize"))?,
                usize::try_from(event_count).map_err(|_| anyhow!("event_count must fit usize"))?,
                usize::try_from(stitched_trace_count)
                    .map_err(|_| anyhow!("stitched_trace_count must fit usize"))?,
                usize::try_from(subagent_trace_count)
                    .map_err(|_| anyhow!("subagent_trace_count must fit usize"))?,
            ),
        ))
    }

    async fn replace_application_run_trace_projection(
        &self,
        input: &ReplaceApplicationRunTraceProjectionInput,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let scope_id = trace_projection_flow_run_scope_id_for_update(&mut tx, input.flow_run_id)
            .await?;

        sqlx::query(
            r#"
            delete from application_run_trace_node_contents
            where trace_node_id in (
                select trace_node_id
                from application_run_trace_nodes
                where flow_run_id = $1
            )
            "#,
        )
        .bind(input.flow_run_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "delete from application_run_trace_nodes where flow_run_id = $1",
        )
        .bind(input.flow_run_id)
        .execute(&mut *tx)
        .await?;

        for node in &input.nodes {
            sqlx::query(
                r#"
                insert into application_run_trace_nodes (
                    id,
                    scope_id,
                    trace_node_id,
                    flow_run_id,
                    parent_trace_node_id,
                    stable_locator,
                    node_kind,
                    owner_kind,
                    owner_id,
                    order_key,
                    node_id,
                    node_type,
                    node_mode,
                    node_alias,
                    status,
                    started_at,
                    finished_at,
                    duration_ms,
                    metrics_payload,
                    has_children,
                    child_count,
                    has_content,
                    content_ref,
                    source_flow_run_id,
                    source_trace_node_id,
                    parent_callback_task_id,
                    parent_tool_call_id,
                    trace_relation_kind,
                    projection_version,
                    source_watermark
                ) values (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                    $13, $14, $15, $16, $17, $18, $19, $20, $21, $22,
                    $23, $24, $25, $26, $27, $28, $29, $30
                )
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(scope_id)
            .bind(node.trace_node_id)
            .bind(input.flow_run_id)
            .bind(node.parent_trace_node_id)
            .bind(&node.stable_locator)
            .bind(&node.node_kind)
            .bind(&node.owner_kind)
            .bind(&node.owner_id)
            .bind(&node.order_key)
            .bind(&node.node_id)
            .bind(&node.node_type)
            .bind(&node.node_mode)
            .bind(&node.node_alias)
            .bind(&node.status)
            .bind(node.started_at)
            .bind(node.finished_at)
            .bind(node.duration_ms)
            .bind(&node.metrics_payload)
            .bind(node.has_children)
            .bind(node.child_count)
            .bind(node.has_content)
            .bind(&node.content_ref)
            .bind(node.source_flow_run_id)
            .bind(node.source_trace_node_id)
            .bind(node.parent_callback_task_id)
            .bind(&node.parent_tool_call_id)
            .bind(&node.trace_relation_kind)
            .bind(input.projection_version)
            .bind(&input.source_watermark)
            .execute(&mut *tx)
            .await?;
        }

        for content in &input.contents {
            sqlx::query(
                r#"
                insert into application_run_trace_node_contents (
                    id,
                    flow_run_id,
                    scope_id,
                    trace_node_id,
                    content_kind,
                    payload,
                    source_refs
                ) values ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(input.flow_run_id)
            .bind(scope_id)
            .bind(content.trace_node_id)
            .bind(&content.content_kind)
            .bind(&content.payload)
            .bind(&content.source_refs)
            .execute(&mut *tx)
            .await?;
        }

        upsert_application_run_trace_projection_status_in_tx(
            &mut tx,
            &UpsertApplicationRunTraceProjectionStatusInput {
                flow_run_id: input.flow_run_id,
                projection_version: input.projection_version,
                status: domain::ApplicationRunTraceProjectionStatus::Succeeded,
                source_watermark: input.source_watermark.clone(),
                attempt_count: 1,
                last_attempt_at: Some(OffsetDateTime::now_utc()),
                last_success_at: Some(OffsetDateTime::now_utc()),
                diagnostic: None,
            },
            scope_id,
        )
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn upsert_application_run_trace_projection_status(
        &self,
        input: &UpsertApplicationRunTraceProjectionStatusInput,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let scope_id = trace_projection_flow_run_scope_id_for_update(&mut tx, input.flow_run_id)
            .await?;
        upsert_application_run_trace_projection_status_in_tx(&mut tx, input, scope_id).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn get_application_run_trace_projection_status(
        &self,
        flow_run_id: Uuid,
        projection_version: i32,
    ) -> Result<Option<domain::ApplicationRunTraceProjectionStatusRecord>> {
        let row = sqlx::query(
            r#"
            select
                flow_run_id,
                projection_version,
                status,
                source_watermark,
                attempt_count,
                last_attempt_at,
                last_success_at,
                last_error_code,
                last_error_stage,
                last_error_source_kind,
                last_error_source_locator,
                last_error_message,
                last_error_ref,
                retriable,
                created_at,
                updated_at
            from application_run_trace_projection_statuses
            where flow_run_id = $1
              and projection_version = $2
            "#,
        )
        .bind(flow_run_id)
        .bind(projection_version)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_application_run_trace_projection_status_record)
            .transpose()
    }

    async fn list_application_run_trace_roots(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ApplicationRunTraceNodeRecord>> {
        let sql = trace_node_select_sql(
            "where flow_run_id = $1 and parent_trace_node_id is null order by order_key asc, trace_node_id asc",
        );
        let rows = sqlx::query(&sql)
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(map_application_run_trace_node_record)
            .collect()
    }

    async fn get_application_run_trace_statistics(
        &self,
        flow_run_id: Uuid,
    ) -> Result<ApplicationRunTraceProjectionStatistics> {
        let row = sqlx::query(
            r#"
            select
                sum(
                    case
                        when metrics_payload #>> '{usage,total_tokens}' ~ '^-?[0-9]+$'
                        then (metrics_payload #>> '{usage,total_tokens}')::bigint
                        when metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                          or metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                          or metrics_payload #>> '{usage,reasoning_tokens}' ~ '^-?[0-9]+$'
                        then
                            coalesce(
                                case
                                    when metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                                    then (metrics_payload #>> '{usage,input_tokens}')::bigint
                                end,
                                0
                            )
                            + coalesce(
                                case
                                    when metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                                    then (metrics_payload #>> '{usage,output_tokens}')::bigint
                                end,
                                0
                            )
                            + coalesce(
                                case
                                    when metrics_payload #>> '{usage,reasoning_tokens}' ~ '^-?[0-9]+$'
                                    then (metrics_payload #>> '{usage,reasoning_tokens}')::bigint
                                end,
                                0
                            )
                    end
                )::bigint as total_tokens,
                sum(
                    case
                        when metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                        then (metrics_payload #>> '{usage,input_tokens}')::bigint
                    end
                )::bigint as input_tokens,
                sum(
                    case
                        when metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                        then (metrics_payload #>> '{usage,output_tokens}')::bigint
                    end
                )::bigint as output_tokens,
                sum(
                    case
                        when metrics_payload #>> '{usage,input_cache_hit_tokens}' ~ '^-?[0-9]+$'
                        then (metrics_payload #>> '{usage,input_cache_hit_tokens}')::bigint
                        when metrics_payload #>> '{usage,cache_read_tokens}' ~ '^-?[0-9]+$'
                        then (metrics_payload #>> '{usage,cache_read_tokens}')::bigint
                    end
                )::bigint as input_cache_hit_tokens,
                count(distinct node_id) filter (where node_id is not null)::bigint as unique_node_count,
                count(*) filter (where node_kind = 'tool_callback')::bigint as tool_callback_count
            from application_run_trace_nodes
            where flow_run_id = $1
            "#,
        );
        let row = row
            .bind(flow_run_id)
            .fetch_one(self.pool())
            .await?;

        Ok(ApplicationRunTraceProjectionStatistics {
            total_tokens: row.get("total_tokens"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            input_cache_hit_tokens: row.get("input_cache_hit_tokens"),
            unique_node_count: row.get("unique_node_count"),
            tool_callback_count: row.get("tool_callback_count"),
        })
    }

    async fn list_application_run_trace_children_page(
        &self,
        input: ListApplicationRunTraceChildrenPageInput,
    ) -> Result<ListApplicationRunTraceChildrenPage> {
        let sql = trace_node_select_sql(
            r#"
            where flow_run_id = $1
              and parent_trace_node_id = $2
              and (
                $3::text is null
                or order_key > $3
                or (order_key = $3 and trace_node_id > $4)
              )
            order by order_key asc, trace_node_id asc
            limit $5
            "#,
        );
        let cursor_order_key = input.cursor.as_ref().map(|cursor| cursor.order_key.as_str());
        let cursor_trace_node_id = input.cursor.as_ref().map(|cursor| cursor.trace_node_id);
        let rows = sqlx::query(&sql)
            .bind(input.flow_run_id)
            .bind(input.parent_trace_node_id)
            .bind(cursor_order_key)
            .bind(cursor_trace_node_id)
            .bind(input.page_size + 1)
            .fetch_all(self.pool())
            .await?;

        let mut items: Vec<domain::ApplicationRunTraceNodeRecord> = rows
            .into_iter()
            .map(map_application_run_trace_node_record)
            .collect::<Result<Vec<_>>>()?;
        let has_more = items.len() > input.page_size as usize;
        if has_more {
            items.truncate(input.page_size as usize);
        }
        let next_cursor = if has_more {
            items
                .last()
                .map(|node| ApplicationRunTraceChildrenCursor {
                    order_key: node.order_key.clone(),
                    trace_node_id: node.trace_node_id,
                })
        } else {
            None
        };

        Ok(ListApplicationRunTraceChildrenPage {
            items,
            has_more,
            next_cursor,
            page_size: input.page_size,
        })
    }

    async fn get_application_run_trace_node(
        &self,
        flow_run_id: Uuid,
        trace_node_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunTraceNodeRecord>> {
        let sql = trace_node_select_sql("where flow_run_id = $1 and trace_node_id = $2");
        let row = sqlx::query(&sql)
            .bind(flow_run_id)
            .bind(trace_node_id)
            .fetch_optional(self.pool())
            .await?;

        row.map(map_application_run_trace_node_record).transpose()
    }

    async fn get_application_run_trace_node_by_locator(
        &self,
        flow_run_id: Uuid,
        stable_locator: &str,
    ) -> Result<Option<domain::ApplicationRunTraceNodeRecord>> {
        let sql = trace_node_select_sql("where flow_run_id = $1 and stable_locator = $2");
        let row = sqlx::query(&sql)
        .bind(flow_run_id)
        .bind(stable_locator)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_application_run_trace_node_record).transpose()
    }

    async fn get_application_run_trace_node_content(
        &self,
        flow_run_id: Uuid,
        trace_node_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunTraceNodeContentRecord>> {
        let row = sqlx::query(
            r#"
            select
                contents.trace_node_id,
                contents.content_kind,
                contents.payload,
                contents.source_refs,
                contents.created_at,
                contents.updated_at
            from application_run_trace_node_contents contents
            join application_run_trace_nodes nodes
              on nodes.trace_node_id = contents.trace_node_id
            where nodes.flow_run_id = $1
              and contents.flow_run_id = $1
              and contents.trace_node_id = $2
            "#,
        )
        .bind(flow_run_id)
        .bind(trace_node_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_application_run_trace_node_content_record)
            .transpose()
    }

    async fn list_application_run_trace_node_run_details(
        &self,
        flow_run_id: Uuid,
        node_run_ids: Vec<Uuid>,
    ) -> Result<Vec<domain::NodeRunRecord>> {
        let mut node_runs = Vec::with_capacity(node_run_ids.len());

        for node_run_id in node_run_ids {
            let Some(node_run) = fetch_node_run(self, node_run_id).await? else {
                continue;
            };
            if node_run.flow_run_id == flow_run_id {
                node_runs.push(node_run);
            }
        }

        Ok(node_runs)
    }
}

async fn trace_projection_flow_run_scope_id_for_update(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    flow_run_id: Uuid,
) -> Result<Uuid> {
    sqlx::query_scalar(
        r#"
        select flow_runs.scope_id
        from flow_runs
        join applications on applications.id = flow_runs.application_id
        where flow_runs.id = $1
        for update of flow_runs
        "#,
    )
    .bind(flow_run_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ControlPlaneError::NotFound("flow_run").into())
}

async fn upsert_application_run_trace_projection_status_in_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    input: &UpsertApplicationRunTraceProjectionStatusInput,
    scope_id: Uuid,
) -> Result<()> {
    let diagnostic = input.diagnostic.clone();

    sqlx::query(
        r#"
        insert into application_run_trace_projection_statuses (
            id,
            scope_id,
            flow_run_id,
            projection_version,
            status,
            source_watermark,
            attempt_count,
            last_attempt_at,
            last_success_at,
            last_error_code,
            last_error_stage,
            last_error_source_kind,
            last_error_source_locator,
            last_error_message,
            last_error_ref,
            retriable
        ) values (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16
        )
        on conflict (flow_run_id, projection_version) do update
        set status = excluded.status,
            scope_id = excluded.scope_id,
            source_watermark = excluded.source_watermark,
            attempt_count = excluded.attempt_count,
            last_attempt_at = excluded.last_attempt_at,
            last_success_at = excluded.last_success_at,
            last_error_code = excluded.last_error_code,
            last_error_stage = excluded.last_error_stage,
            last_error_source_kind = excluded.last_error_source_kind,
            last_error_source_locator = excluded.last_error_source_locator,
            last_error_message = excluded.last_error_message,
            last_error_ref = excluded.last_error_ref,
            retriable = excluded.retriable,
            updated_at = now()
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(scope_id)
    .bind(input.flow_run_id)
    .bind(input.projection_version)
    .bind(input.status.as_str())
    .bind(&input.source_watermark)
    .bind(input.attempt_count)
    .bind(input.last_attempt_at)
    .bind(input.last_success_at)
    .bind(diagnostic.as_ref().and_then(|value| value.last_error_code.as_deref()))
    .bind(diagnostic.as_ref().and_then(|value| value.last_error_stage.as_deref()))
    .bind(
        diagnostic
            .as_ref()
            .and_then(|value| value.last_error_source_kind.as_deref()),
    )
    .bind(
        diagnostic
            .as_ref()
            .and_then(|value| value.last_error_source_locator.as_deref()),
    )
    .bind(
        diagnostic
            .as_ref()
            .and_then(|value| value.last_error_message.as_deref()),
    )
    .bind(diagnostic.as_ref().and_then(|value| value.last_error_ref.as_deref()))
    .bind(diagnostic.as_ref().is_some_and(|value| value.retriable))
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn trace_node_select_sql(predicate: &str) -> String {
    format!(
        r#"
        select
            trace_node_id,
            flow_run_id,
            parent_trace_node_id,
            stable_locator,
            node_kind,
            owner_kind,
            owner_id,
            order_key,
            node_id,
            node_type,
            node_mode,
            node_alias,
            status,
            started_at,
            finished_at,
            duration_ms,
            metrics_payload,
            has_children,
            child_count,
            has_content,
            content_ref,
            source_flow_run_id,
            source_trace_node_id,
            parent_callback_task_id,
            parent_tool_call_id,
            trace_relation_kind,
            projection_version,
            source_watermark,
            created_at,
            updated_at
        from application_run_trace_nodes
        {predicate}
        "#
    )
}

fn map_application_run_trace_projection_status_record(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ApplicationRunTraceProjectionStatusRecord> {
    let status = match row.get::<String, _>("status").as_str() {
        "pending" => domain::ApplicationRunTraceProjectionStatus::Pending,
        "running" => domain::ApplicationRunTraceProjectionStatus::Running,
        "succeeded" => domain::ApplicationRunTraceProjectionStatus::Succeeded,
        "failed" => domain::ApplicationRunTraceProjectionStatus::Failed,
        "stale" => domain::ApplicationRunTraceProjectionStatus::Stale,
        "partial" => domain::ApplicationRunTraceProjectionStatus::Partial,
        value => return Err(anyhow!("unknown trace projection status: {value}")),
    };

    Ok(domain::ApplicationRunTraceProjectionStatusRecord {
        flow_run_id: row.get("flow_run_id"),
        projection_version: row.get("projection_version"),
        status,
        source_watermark: row.get("source_watermark"),
        attempt_count: row.get("attempt_count"),
        last_attempt_at: row.get("last_attempt_at"),
        last_success_at: row.get("last_success_at"),
        last_error_code: row.get("last_error_code"),
        last_error_stage: row.get("last_error_stage"),
        last_error_source_kind: row.get("last_error_source_kind"),
        last_error_source_locator: row.get("last_error_source_locator"),
        last_error_message: row.get("last_error_message"),
        last_error_ref: row.get("last_error_ref"),
        retriable: row.get("retriable"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_application_run_trace_node_record(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ApplicationRunTraceNodeRecord> {
    Ok(domain::ApplicationRunTraceNodeRecord {
        trace_node_id: row.get("trace_node_id"),
        flow_run_id: row.get("flow_run_id"),
        parent_trace_node_id: row.get("parent_trace_node_id"),
        stable_locator: row.get("stable_locator"),
        node_kind: row.get("node_kind"),
        owner_kind: row.get("owner_kind"),
        owner_id: row.get("owner_id"),
        order_key: row.get("order_key"),
        node_id: row.get("node_id"),
        node_type: row.get("node_type"),
        node_mode: row.get("node_mode"),
        node_alias: row.get("node_alias"),
        status: row.get("status"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        duration_ms: row.get("duration_ms"),
        metrics_payload: row.get("metrics_payload"),
        has_children: row.get("has_children"),
        child_count: row.get("child_count"),
        has_content: row.get("has_content"),
        content_ref: row.get("content_ref"),
        source_flow_run_id: row.get("source_flow_run_id"),
        source_trace_node_id: row.get("source_trace_node_id"),
        parent_callback_task_id: row.get("parent_callback_task_id"),
        parent_tool_call_id: row.get("parent_tool_call_id"),
        trace_relation_kind: row.get("trace_relation_kind"),
        projection_version: row.get("projection_version"),
        source_watermark: row.get("source_watermark"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_application_run_trace_node_content_record(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ApplicationRunTraceNodeContentRecord> {
    Ok(domain::ApplicationRunTraceNodeContentRecord {
        trace_node_id: row.get("trace_node_id"),
        content_kind: row.get("content_kind"),
        payload: row.get("payload"),
        source_refs: row.get("source_refs"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
