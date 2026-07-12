impl PgControlPlaneStore {
    async fn list_runtime_spans(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeSpanRecord>> {
        let rows = sqlx::query(
            r#"
            select
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
            from runtime_spans
            where flow_run_id = $1
            order by started_at asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_runtime_span_record).collect()
    }

    async fn list_runtime_events(
        &self,
        flow_run_id: Uuid,
        after_sequence: i64,
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        let rows = sqlx::query(
            r#"
            select
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
            from runtime_events
            where flow_run_id = $1
              and sequence > $2
            order by sequence asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .bind(after_sequence)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_runtime_event_record).collect()
    }

    async fn list_runtime_event_backfill_page(
        &self,
        flow_run_id: Uuid,
        after_stream_sequence: i64,
        limit: usize,
    ) -> Result<Vec<domain::RuntimeEventRecord>> {
        let limit = i64::try_from(limit.max(1)).unwrap_or(i64::MAX);
        let rows = sqlx::query(
            r#"
            select
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
            from runtime_events
            where flow_run_id = $1
              and coalesce(
                    case
                      when payload ->> 'sequence_end' ~ '^-?[0-9]+$'
                      then (payload ->> 'sequence_end')::bigint
                    end,
                    case
                      when payload ->> 'stream_sequence' ~ '^-?[0-9]+$'
                      then (payload ->> 'stream_sequence')::bigint
                    end,
                    sequence
                  ) > $2
            order by
                coalesce(
                    case
                      when payload ->> 'sequence_end' ~ '^-?[0-9]+$'
                      then (payload ->> 'sequence_end')::bigint
                    end,
                    case
                      when payload ->> 'stream_sequence' ~ '^-?[0-9]+$'
                      then (payload ->> 'stream_sequence')::bigint
                    end,
                    sequence
                ) asc,
                sequence asc,
                id asc
            limit $3
            "#,
        )
        .bind(flow_run_id)
        .bind(after_stream_sequence)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_runtime_event_record).collect()
    }

    async fn list_runtime_items(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::RuntimeItemRecord>> {
        let rows = sqlx::query(
            r#"
            select
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
            from runtime_items
            where flow_run_id = $1
            order by created_at asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_runtime_item_record).collect()
    }

    async fn list_context_projections(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ContextProjectionRecord>> {
        let rows = sqlx::query(
            r#"
            select
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
            from runtime_context_projections
            where flow_run_id = $1
            order by created_at asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(map_context_projection_record)
            .collect())
    }

    async fn list_usage_ledger(&self, flow_run_id: Uuid) -> Result<Vec<domain::UsageLedgerRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                node_run_id,
                span_id,
                failover_attempt_id,
                provider_instance_id,
                gateway_route_id,
                model_id,
                upstream_model_id,
                upstream_request_id,
                input_tokens,
                cached_input_tokens,
                output_tokens,
                reasoning_output_tokens,
                total_tokens,
                input_cache_hit_tokens,
                input_cache_miss_tokens,
                cache_read_tokens,
                cache_write_tokens,
                price_snapshot,
                cost_snapshot,
                usage_status,
                raw_usage,
                normalized_usage,
                created_at
            from runtime_usage_ledger
            where flow_run_id = $1
            order by created_at asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_usage_ledger_record).collect()
    }

    async fn list_model_failover_attempt_ledger(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ModelFailoverAttemptLedgerRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                queue_snapshot_id,
                attempt_index,
                provider_instance_id,
                provider_code,
                upstream_model_id,
                protocol,
                request_ref,
                request_hash,
                started_at,
                first_token_at,
                finished_at,
                status,
                failed_after_first_token,
                upstream_request_id,
                error_code,
                error_message_ref,
                usage_ledger_id,
                cost_ledger_id,
                response_ref
            from model_failover_attempt_ledger
            where flow_run_id = $1
            order by attempt_index asc, started_at asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(map_model_failover_attempt_ledger_record)
            .collect())
    }

    async fn list_model_provider_request_logs_page(
        &self,
        input: control_plane::ports::ListModelProviderRequestLogsPageInput,
    ) -> Result<control_plane::ports::ModelProviderRequestLogsPage> {
        let page = input.page.max(1);
        let page_size = input.page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;
        let mut count_query = QueryBuilder::<Postgres>::new(
            "select count(*) as total_count from model_provider_request_logs logs",
        );
        push_model_provider_request_log_filters(&mut count_query, &input);
        let total_count: i64 = count_query
            .build()
            .fetch_one(self.pool())
            .await?
            .get("total_count");

        let mut query = QueryBuilder::<Postgres>::new(
            "select attempt_id, flow_run_id, application_id, conversation_id, application_name, attempt_index, is_retry, retry_reason, provider_instance_id, provider_instance_display_name, provider_code, protocol, upstream_model_id, reasoning_effort, status, error_code, failed_after_first_token, input_tokens, output_tokens, total_tokens, started_at, first_token_at, finished_at, time_to_first_token_ms, total_duration_ms from model_provider_request_logs logs",
        );
        push_model_provider_request_log_filters(&mut query, &input);
        query
            .push(" order by logs.started_at desc, logs.id desc limit ")
            .push_bind(page_size)
            .push(" offset ")
            .push_bind(offset);
        let rows = query.build().fetch_all(self.pool()).await?;
        let items = rows
            .into_iter()
            .map(|row| control_plane::ports::ModelProviderRequestLogRecord {
                attempt_id: row.get("attempt_id"),
                flow_run_id: row.get("flow_run_id"),
                application_id: row.get("application_id"),
                conversation_id: row.get("conversation_id"),
                application_name: row.get("application_name"),
                attempt_index: row.get("attempt_index"),
                is_retry: row.get("is_retry"),
                retry_reason: row.get("retry_reason"),
                provider_instance_id: row.get("provider_instance_id"),
                provider_instance_display_name: row.get("provider_instance_display_name"),
                provider_code: row.get("provider_code"),
                protocol: row.get("protocol"),
                upstream_model_id: row.get("upstream_model_id"),
                reasoning_effort: row.get("reasoning_effort"),
                status: row.get("status"),
                error_code: row.get("error_code"),
                failed_after_first_token: row.get("failed_after_first_token"),
                input_tokens: row.get("input_tokens"),
                output_tokens: row.get("output_tokens"),
                total_tokens: row.get("total_tokens"),
                started_at: row.get("started_at"),
                first_token_at: row.get("first_token_at"),
                finished_at: row.get("finished_at"),
                time_to_first_token_ms: row.get("time_to_first_token_ms"),
                total_duration_ms: row.get("total_duration_ms"),
            })
            .collect();
        Ok(control_plane::ports::ModelProviderRequestLogsPage {
            items,
            total_count,
            page,
            page_size,
        })
    }

    async fn list_capability_invocations(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::CapabilityInvocationRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                flow_run_id,
                span_id,
                capability_id,
                requested_by_span_id,
                requester_kind,
                arguments_ref,
                authorization_status,
                authorization_reason,
                result_ref,
                normalized_result,
                started_at,
                finished_at,
                error_payload,
                created_at
            from capability_invocations
            where flow_run_id = $1
            order by created_at asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(map_capability_invocation_record)
            .collect())
    }

    async fn list_application_runs(
        &self,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationRunSummary>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                run_mode,
                status,
                target_node_id,
                title,
                '{}'::jsonb as input_payload,
                external_user,
                api_key_id,
                publication_version_id,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                (
                    select users.account
                    from users
                    where users.id = flow_runs.created_by
                ) as authorized_account,
                started_at,
                finished_at,
                created_at,
                updated_at
            from flow_runs
            where application_id = $1
              and (
                  flow_runs.import_job_id is null
                  or exists (
                      select 1
                      from run_archive_import_jobs import_jobs
                      where import_jobs.id = flow_runs.import_job_id
                        and import_jobs.status = 'succeeded'
                  )
              )
            order by created_at desc, id desc
            "#,
        )
        .bind(application_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_application_run_summary).collect()
    }

    async fn list_application_runs_page(
        &self,
        application_id: Uuid,
        input: control_plane::ports::ListApplicationRunsPageInput,
    ) -> Result<control_plane::ports::ApplicationRunSummaryPage> {
        let page = input.page.max(1);
        let page_size = input.page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;
        let created_after = input.created_after;
        let order_by = Self::application_runs_page_order_by(
            input.sort_by.as_deref(),
            input.sort_order.as_deref(),
        );

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)
            from flow_runs
            where application_id = $1
              and ($2::timestamptz is null or created_at >= $2)
              and (
                  flow_runs.import_job_id is null
                  or exists (
                      select 1
                      from run_archive_import_jobs import_jobs
                      where import_jobs.id = flow_runs.import_job_id
                        and import_jobs.status = 'succeeded'
                  )
              )
            "#,
        )
        .bind(application_id)
        .bind(created_after)
        .fetch_one(self.pool())
        .await?;

        let rows = sqlx::query(&format!(
            r#"
            select
                id,
                run_mode,
                status,
                target_node_id,
                title,
                '{{}}'::jsonb as input_payload,
                external_user,
                api_key_id,
                publication_version_id,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                (
                    select users.account
                    from users
                    where users.id = flow_runs.created_by
                ) as authorized_account,
                started_at,
                finished_at,
                created_at,
                updated_at
            from flow_runs
            where application_id = $1
              and ($2::timestamptz is null or created_at >= $2)
              and (
                  flow_runs.import_job_id is null
                  or exists (
                      select 1
                      from run_archive_import_jobs import_jobs
                      where import_jobs.id = flow_runs.import_job_id
                        and import_jobs.status = 'succeeded'
                  )
              )
            order by {}
            limit $3 offset $4
            "#,
            order_by
        ))
        .bind(application_id)
        .bind(created_after)
        .bind(page_size)
        .bind(offset)
        .fetch_all(self.pool())
        .await?;

        Ok(control_plane::ports::ApplicationRunSummaryPage {
            items: rows
                .into_iter()
                .map(map_application_run_summary)
                .collect::<Result<Vec<_>>>()?,
            total,
            page,
            page_size,
        })
    }

    async fn list_application_conversation_runs_page(
        &self,
        application_id: Uuid,
        input: control_plane::ports::ListApplicationConversationRunsPageInput,
    ) -> Result<control_plane::ports::ApplicationConversationRunsPage> {
        let limit = input.limit.clamp(1, 50);
        let (start_rn, end_rn, total) = if let Some(anchor_run_id) = input.before_run_id {
            let Some((anchor_rn, total)) = self
                .application_conversation_run_position(
                    application_id,
                    &input.external_conversation_id,
                    anchor_run_id,
                )
                .await?
            else {
                return Ok(empty_application_conversation_runs_page());
            };
            ((anchor_rn - limit).max(1), anchor_rn - 1, total)
        } else if let Some(anchor_run_id) = input.after_run_id {
            let Some((anchor_rn, total)) = self
                .application_conversation_run_position(
                    application_id,
                    &input.external_conversation_id,
                    anchor_run_id,
                )
                .await?
            else {
                return Ok(empty_application_conversation_runs_page());
            };
            (anchor_rn + 1, (anchor_rn + limit).min(total), total)
        } else {
            let Some(anchor_run_id) = input.around_run_id else {
                return Ok(empty_application_conversation_runs_page());
            };
            let Some((anchor_rn, total)) = self
                .application_conversation_run_position(
                    application_id,
                    &input.external_conversation_id,
                    anchor_run_id,
                )
                .await?
            else {
                return Ok(empty_application_conversation_runs_page());
            };
            let latest_start = (total - limit + 1).max(1);
            let centered_start = anchor_rn - (limit / 2);
            let start_rn = centered_start.max(1).min(latest_start);
            (start_rn, (start_rn + limit - 1).min(total), total)
        };

        if start_rn > end_rn || total == 0 {
            return Ok(empty_application_conversation_runs_page());
        }

        let rows = sqlx::query(
            r#"
            with grouped as (
                select
                    runs.id,
                    runs.status,
                    (
                        array_agg(messages.content order by messages.sequence asc)
                        filter (where messages.role = 'user')
                    )[1] as query,
                    null::text as model,
                    (
                        array_agg(messages.content order by messages.sequence asc)
                        filter (where messages.role = 'assistant')
                    )[1] as answer,
                    runs.started_at,
                    runs.finished_at,
                    min(messages.sequence) as order_sequence
                from application_conversation_messages messages
                join application_conversations conversations
                  on conversations.id = messages.conversation_id
                join flow_runs runs
                  on runs.id = messages.flow_run_id
                where messages.application_id = $1
                  and conversations.external_conversation_id = $2
                  and (
                      runs.import_job_id is null
                      or exists (
                          select 1
                          from run_archive_import_jobs import_jobs
                          where import_jobs.id = runs.import_job_id
                            and import_jobs.status = 'succeeded'
                      )
                  )
                group by runs.id, runs.status, runs.started_at, runs.finished_at
            ),
            ordered as (
                select
                    id,
                    status,
                    query,
                    model,
                    answer,
                    started_at,
                    finished_at,
                    row_number() over (order by order_sequence asc, id asc) as rn
                from grouped
            )
            select
                id,
                status,
                query,
                model,
                answer,
                started_at,
                finished_at
            from ordered
            where rn between $3 and $4
            order by rn asc
            "#
        )
        .bind(application_id)
        .bind(&input.external_conversation_id)
        .bind(start_rn)
        .bind(end_rn)
        .fetch_all(self.pool())
        .await?;

        let items = rows
            .into_iter()
            .map(map_application_conversation_run_summary)
            .collect::<Result<Vec<_>>>()?;
        let before_cursor = items.first().map(|run| run.id);
        let after_cursor = items.last().map(|run| run.id);

        Ok(control_plane::ports::ApplicationConversationRunsPage {
            items,
            has_before: start_rn > 1,
            has_after: end_rn < total,
            before_cursor,
            after_cursor,
        })
    }

    async fn application_conversation_run_position(
        &self,
        application_id: Uuid,
        external_conversation_id: &str,
        flow_run_id: Uuid,
    ) -> Result<Option<(i64, i64)>> {
        let row = sqlx::query(
            r#"
            with grouped as (
                select
                    runs.id,
                    min(messages.sequence) as order_sequence
                from application_conversation_messages messages
                join application_conversations conversations
                  on conversations.id = messages.conversation_id
                join flow_runs runs
                  on runs.id = messages.flow_run_id
                where messages.application_id = $1
                  and conversations.external_conversation_id = $2
                  and (
                      runs.import_job_id is null
                      or exists (
                          select 1
                          from run_archive_import_jobs import_jobs
                          where import_jobs.id = runs.import_job_id
                            and import_jobs.status = 'succeeded'
                      )
                  )
                group by runs.id
            ),
            ordered as (
                select
                    id,
                    row_number() over (order by order_sequence asc, id asc) as rn,
                    count(*) over () as total
                from grouped
            )
            select rn, total
            from ordered
            where id = $3
            "#
        )
        .bind(application_id)
        .bind(external_conversation_id)
        .bind(flow_run_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|row| (row.get("rn"), row.get("total"))))
    }

    fn application_runs_page_order_by(sort_by: Option<&str>, sort_order: Option<&str>) -> String {
        let sort_by = sort_by.unwrap_or("created_at").to_ascii_lowercase();
        let sort_order = sort_order.unwrap_or("desc").to_ascii_lowercase();
        let field = match sort_by.as_str() {
            "started_at" => "started_at",
            "finished_at" => "finished_at",
            "updated_at" => "updated_at",
            "created_at" => "created_at",
            _ => "created_at",
        };
        let direction = match sort_order.as_str() {
            "asc" => "asc",
            _ => "desc",
        };

        format!("{field} {direction}, id {direction}")
    }

    async fn get_application_run_detail(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunDetail>> {
        let Some(flow_run) =
            fetch_flow_run_for_application(self, application_id, flow_run_id).await?
        else {
            return Ok(None);
        };

        let callback_tasks = list_callback_tasks_for_flow_run(self, flow_run.id).await?;
        Ok(Some(domain::ApplicationRunDetail {
            node_runs: list_node_runs_for_flow_run(self, flow_run.id).await?,
            checkpoints: list_checkpoints_for_flow_run(self, flow_run.id).await?,
            events: list_events_for_flow_run(self, flow_run.id).await?,
            stitched_trace: list_stitched_trace_for_flow_run(self, &flow_run).await?,
            subagent_traces: list_subagent_traces_for_flow_run(self, &flow_run, &callback_tasks)
                .await?,
            flow_run,
            callback_tasks,
        }))
    }

    async fn get_application_run_trace_projection_source(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunDetail>> {
        let Some(flow_run) =
            fetch_flow_run_for_application(self, application_id, flow_run_id).await?
        else {
            return Ok(None);
        };

        let callback_tasks = list_callback_tasks_for_flow_run(self, flow_run.id).await?;
        Ok(Some(domain::ApplicationRunDetail {
            node_runs: list_node_runs_for_flow_run(self, flow_run.id).await?,
            checkpoints: list_checkpoints_for_flow_run(self, flow_run.id).await?,
            events: list_events_for_flow_run(self, flow_run.id).await?,
            stitched_trace: list_stitched_trace_for_flow_run(self, &flow_run).await?,
            subagent_traces: list_subagent_traces_for_flow_run(self, &flow_run, &callback_tasks)
                .await?,
            flow_run,
            callback_tasks,
        }))
    }

    async fn get_latest_node_run(
        &self,
        application_id: Uuid,
        node_id: &str,
    ) -> Result<Option<domain::NodeLastRun>> {
        let latest = sqlx::query(
            r#"
            select
                nr.id as node_run_id,
                fr.id as flow_run_id
            from node_runs nr
            join flow_runs fr on fr.id = nr.flow_run_id
            where fr.application_id = $1
              and nr.node_id = $2
              and (
                  fr.import_job_id is null
                  or exists (
                      select 1
                      from run_archive_import_jobs import_jobs
                      where import_jobs.id = fr.import_job_id
                        and import_jobs.status = 'succeeded'
                  )
              )
            order by nr.started_at desc, nr.id desc
            limit 1
            "#,
        )
        .bind(application_id)
        .bind(node_id)
        .fetch_optional(self.pool())
        .await?;

        let Some(latest) = latest else {
            return Ok(None);
        };
        let flow_run_id: Uuid = latest.get("flow_run_id");
        let node_run_id: Uuid = latest.get("node_run_id");
        let flow_run = fetch_flow_run_for_application(self, application_id, flow_run_id)
            .await?
            .expect("joined flow_run must exist");
        let node_run = fetch_node_run(self, node_run_id)
            .await?
            .expect("joined node_run must exist");

        Ok(Some(domain::NodeLastRun {
            checkpoints: list_checkpoints_for_node_run(self, node_run.id).await?,
            events: list_events_for_node_context(self, flow_run.id, node_run.id).await?,
            flow_run,
            node_run,
        }))
    }
}


fn push_model_provider_request_log_filters<'a>(
    query: &mut QueryBuilder<'a, Postgres>,
    input: &'a control_plane::ports::ListModelProviderRequestLogsPageInput,
) {
    query.push(" where logs.scope_id = ").push_bind(input.scope_id);
    if let Some(application_name) = input.application_name.as_deref() {
        query.push(" and logs.application_name = ").push_bind(application_name);
    }
    if let Some(provider_instance_id) = input.provider_instance_id {
        query.push(" and logs.provider_instance_id = ").push_bind(provider_instance_id);
    }
    if let Some(model_id) = input.model_id.as_deref() {
        query.push(" and logs.upstream_model_id = ").push_bind(model_id);
    }
    if let Some(status) = input.status.as_deref() {
        query.push(" and logs.status = ").push_bind(status);
    }
    if input.zero_output_only {
        query.push(" and coalesce(logs.output_tokens, 0) = 0");
    }
    if let Some(started_after) = input.started_after {
        query.push(" and logs.started_at >= ").push_bind(started_after);
    }
    if let Some(started_before) = input.started_before {
        query.push(" and logs.started_at <= ").push_bind(started_before);
    }
}

fn empty_application_conversation_runs_page(
) -> control_plane::ports::ApplicationConversationRunsPage {
    control_plane::ports::ApplicationConversationRunsPage {
        items: Vec::new(),
        has_before: false,
        has_after: false,
        before_cursor: None,
        after_cursor: None,
    }
}
