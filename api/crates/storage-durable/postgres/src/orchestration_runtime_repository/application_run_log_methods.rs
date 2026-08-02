impl PgControlPlaneStore {
    async fn list_application_run_count_tokens_results(
        &self,
        flow_run_ids: &[Uuid],
    ) -> Result<Vec<control_plane::ports::ApplicationRunCountTokensResult>> {
        if flow_run_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            select id, output_payload
            from flow_runs
            where id = any($1)
              and output_payload ->> 'semantic_terminal' = 'count_tokens'
            "#,
        )
        .bind(flow_run_ids)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                let flow_run_id: Uuid = row.get("id");
                let output_payload: serde_json::Value = row.get("output_payload");
                let input_tokens =
                    control_plane::orchestration_runtime::count_tokens_input_tokens_from_output_payload(
                        &output_payload,
                    )?
                    .ok_or_else(|| anyhow!("CountTokens terminal did not contain a typed result"))?;
                Ok(control_plane::ports::ApplicationRunCountTokensResult {
                    flow_run_id,
                    input_tokens,
                })
            })
            .collect()
    }

    async fn upsert_application_run_log_summary_for_flow_run(
        &self,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<()> {
        let is_terminal = is_terminal_application_run_log_status(flow_run.status);
        let mut tx = self.pool().begin().await?;

        Self::upsert_visible_application_run_log_summary_projection(&mut tx, flow_run).await?;
        if is_terminal {
            Self::ensure_application_run_conversation_message_items_projection(&mut tx, flow_run)
                .await?;
        } else {
            Self::delete_application_run_conversation_message_items_projection(&mut tx, flow_run.id)
                .await?;
        }
        tx.commit().await?;

        if is_terminal {
            self.upsert_application_conversation_messages_for_flow_run(flow_run)
                .await?;
        }

        Ok(())
    }

    async fn upsert_visible_application_run_log_summary_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<()> {
        if is_anthropic_claude_code_internal_run(flow_run) {
            Self::delete_application_run_log_summary_projection(tx, flow_run.id).await?;
            return Ok(());
        }

        let display_title = control_plane::flow_run_title::display_flow_run_title(
            &flow_run.title,
            &flow_run.input_payload,
        );
        Self::upsert_application_run_log_summary_projection(tx, flow_run, &display_title).await
    }

    async fn delete_application_run_log_summary_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            r#"
            delete from application_run_log_summaries
            where flow_run_id = $1
            "#,
        )
        .bind(flow_run_id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn upsert_application_run_log_summary_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run: &domain::FlowRunRecord,
        display_title: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into application_run_log_summaries (
                flow_run_id,
                scope_id,
                application_id,
                run_mode,
                status,
                target_node_id,
                title,
                input_payload,
                external_user,
                created_by,
                authorized_account,
                api_key_id,
                api_key_name_snapshot,
                publication_version_id,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                total_tokens,
                input_tokens, output_tokens, input_cache_hit_tokens,
                unique_node_count,
                tool_callback_count,
                started_at,
                finished_at,
                created_at,
                updated_at
            ) values (
                $1, (select workspace_id from applications where id = $2), $2, $3, $4,
                $5, $6, $7, $8, $20,
                coalesce($9, (select users.account from users where users.id = $20)),
                $10, (select name from api_keys where id = $10),
                $11, $12, $13, $14, $15,
                coalesce(
                    (
                        select sum(
                            coalesce(
                                runtime_usage_ledger.total_tokens,
                                coalesce(runtime_usage_ledger.input_tokens, 0)
                                + coalesce(runtime_usage_ledger.output_tokens, 0)
                                + coalesce(runtime_usage_ledger.reasoning_output_tokens, 0)
                            )
                        )::bigint
                        from runtime_usage_ledger
                        where runtime_usage_ledger.flow_run_id = $1
                    ),
                    (
                        select sum(
                            coalesce(
                                case
                                    when node_runs.metrics_payload #>> '{usage,total_tokens}' ~ '^-?[0-9]+$'
                                    then (node_runs.metrics_payload #>> '{usage,total_tokens}')::bigint
                                end,
                                case
                                    when node_runs.metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                                      or node_runs.metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                                      or node_runs.metrics_payload #>> '{usage,reasoning_tokens}' ~ '^-?[0-9]+$'
                                    then coalesce(
                                        case
                                            when node_runs.metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                                            then (node_runs.metrics_payload #>> '{usage,input_tokens}')::bigint
                                        end,
                                        0
                                    )
                                    + coalesce(
                                        case
                                            when node_runs.metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                                            then (node_runs.metrics_payload #>> '{usage,output_tokens}')::bigint
                                        end,
                                        0
                                    )
                                    + coalesce(
                                        case
                                            when node_runs.metrics_payload #>> '{usage,reasoning_tokens}' ~ '^-?[0-9]+$'
                                            then (node_runs.metrics_payload #>> '{usage,reasoning_tokens}')::bigint
                                        end,
                                        0
                                    )
                                end,
                                0
                            )
                        )::bigint
                        from node_runs
                        where node_runs.flow_run_id = $1
                    )
                ),
                coalesce(
                    (
                        select sum(runtime_usage_ledger.input_tokens)::bigint
                        from runtime_usage_ledger
                        where runtime_usage_ledger.flow_run_id = $1
                    ),
                    (
                        select sum((node_runs.metrics_payload #>> '{usage,input_tokens}')::bigint)::bigint
                        from node_runs
                        where node_runs.flow_run_id = $1
                          and node_runs.metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                    )
                ),
                coalesce(
                    (
                        select sum(runtime_usage_ledger.output_tokens)::bigint
                        from runtime_usage_ledger
                        where runtime_usage_ledger.flow_run_id = $1
                    ),
                    (
                        select sum((node_runs.metrics_payload #>> '{usage,output_tokens}')::bigint)::bigint
                        from node_runs
                        where node_runs.flow_run_id = $1
                          and node_runs.metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                    )
                ),
                coalesce(
                    (
                        select sum(coalesce(
                            runtime_usage_ledger.input_cache_hit_tokens,
                            runtime_usage_ledger.cache_read_tokens,
                            runtime_usage_ledger.cached_input_tokens
                        ))::bigint
                        from runtime_usage_ledger
                        where runtime_usage_ledger.flow_run_id = $1
                    ),
                    (
                        select sum(
                            coalesce(
                                case
                                    when node_runs.metrics_payload #>> '{usage,input_cache_hit_tokens}' ~ '^-?[0-9]+$'
                                    then (node_runs.metrics_payload #>> '{usage,input_cache_hit_tokens}')::bigint
                                end,
                                case
                                    when node_runs.metrics_payload #>> '{usage,cache_read_tokens}' ~ '^-?[0-9]+$'
                                    then (node_runs.metrics_payload #>> '{usage,cache_read_tokens}')::bigint
                                end,
                                case
                                    when node_runs.metrics_payload #>> '{usage,cached_input_tokens}' ~ '^-?[0-9]+$'
                                    then (node_runs.metrics_payload #>> '{usage,cached_input_tokens}')::bigint
                                end
                            )
                        )::bigint
                        from node_runs
                        where node_runs.flow_run_id = $1
                    )
                ),
                coalesce(
                    (
                        select count(distinct node_runs.node_id)::bigint
                        from node_runs
                        where node_runs.flow_run_id = $1
                    ),
                    0
                ),
                coalesce(
                    (
                        select sum(
                            case
                                when jsonb_typeof(flow_run_callback_tasks.request_payload -> 'tool_calls') = 'array'
                                then jsonb_array_length(flow_run_callback_tasks.request_payload -> 'tool_calls')::bigint
                                else 0
                            end
                        )::bigint
                        from flow_run_callback_tasks
                        where flow_run_callback_tasks.flow_run_id = $1
                          and flow_run_callback_tasks.callback_kind = 'llm_tool_calls'
                    ),
                    0
                ),
                $16, $17, $18, $19
            )
            on conflict (flow_run_id) do update
            set application_id = excluded.application_id,
                scope_id = excluded.scope_id,
                run_mode = excluded.run_mode,
                status = excluded.status,
                target_node_id = excluded.target_node_id,
                title = excluded.title,
                input_payload = excluded.input_payload,
                external_user = excluded.external_user,
                created_by = excluded.created_by,
                authorized_account = excluded.authorized_account,
                api_key_id = excluded.api_key_id,
                api_key_name_snapshot = coalesce(
                    excluded.api_key_name_snapshot,
                    application_run_log_summaries.api_key_name_snapshot
                ),
                publication_version_id = excluded.publication_version_id,
                external_conversation_id = excluded.external_conversation_id,
                external_trace_id = excluded.external_trace_id,
                compatibility_mode = excluded.compatibility_mode,
                idempotency_key = excluded.idempotency_key,
                total_tokens = excluded.total_tokens,
                input_tokens = excluded.input_tokens,
                output_tokens = excluded.output_tokens,
                input_cache_hit_tokens = excluded.input_cache_hit_tokens,
                unique_node_count = excluded.unique_node_count,
                tool_callback_count = excluded.tool_callback_count,
                started_at = excluded.started_at,
                finished_at = excluded.finished_at,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                log_updated_at = now()
            "#,
        )
        .bind(flow_run.id)
        .bind(flow_run.application_id)
        .bind(flow_run.run_mode.as_str())
        .bind(flow_run.status.as_str())
        .bind(&flow_run.target_node_id)
        .bind(display_title)
        .bind(serde_json::json!({}))
        .bind(&flow_run.external_user)
        .bind(&flow_run.authorized_account)
        .bind(flow_run.api_key_id)
        .bind(flow_run.publication_version_id)
        .bind(&flow_run.external_conversation_id)
        .bind(&flow_run.external_trace_id)
        .bind(&flow_run.compatibility_mode)
        .bind(&flow_run.idempotency_key)
        .bind(flow_run.started_at)
        .bind(flow_run.finished_at)
        .bind(flow_run.created_at)
        .bind(flow_run.updated_at)
        .bind(flow_run.created_by)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            update application_run_log_summaries
            set input_cache_hit_rate = case
                    when input_cache_hit_tokens is not null
                     and coalesce(input_tokens, 0) + input_cache_hit_tokens > 0
                    then input_cache_hit_tokens::double precision
                       / (coalesce(input_tokens, 0) + input_cache_hit_tokens)::double precision
                    else null
                end,
                log_updated_at = now()
            where flow_run_id = $1
              and input_cache_hit_rate is distinct from case
                    when input_cache_hit_tokens is not null
                     and coalesce(input_tokens, 0) + input_cache_hit_tokens > 0
                    then input_cache_hit_tokens::double precision
                       / (coalesce(input_tokens, 0) + input_cache_hit_tokens)::double precision
                    else null
                end
            "#,
        )
        .bind(flow_run.id)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    async fn upsert_application_conversation_messages_for_flow_run(
        &self,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<()> {
        let messages = application_conversation_messages_from_flow_run(flow_run);
        if messages.is_empty() {
            sqlx::query(
                r#"
                delete from application_conversation_messages
                where flow_run_id = $1
                "#,
            )
            .bind(flow_run.id)
            .execute(self.pool())
            .await?;
            return Ok(());
        }
        let conversation_key = application_conversation_key(flow_run);

        let Some(conversation) = sqlx::query(
            r#"
            with existing as (
                select id, scope_id, 0 as source_order
                from application_conversations
                where application_id = $1
                  and external_conversation_id = $2
                  and api_key_id is not distinct from (select id from api_keys where id = $3)
                  and external_user is not distinct from $4
                order by updated_at desc, id desc
                limit 1
            ),
            inserted as (
                insert into application_conversations (
                    id,
                    scope_id,
                    application_id,
                    api_key_id,
                    external_user,
                    external_conversation_id,
                    created_at,
                    updated_at
                )
                select
                    $5,
                    applications.workspace_id,
                    applications.id,
                    (select id from api_keys where id = $3),
                    $4,
                    $2,
                    $6,
                    $7
                from applications
                where applications.id = $1
                  and not exists (select 1 from existing)
                returning id, scope_id, 1 as source_order
            )
            select id, scope_id, source_order from existing
            union all
            select id, scope_id, source_order from inserted
            order by source_order asc
            limit 1
            "#,
        )
        .bind(flow_run.application_id)
        .bind(&conversation_key)
        .bind(flow_run.api_key_id)
        .bind(&flow_run.external_user)
        .bind(Uuid::now_v7())
        .bind(flow_run.started_at)
        .bind(flow_run.updated_at)
        .fetch_optional(self.pool())
        .await?
        else {
            return Ok(());
        };

        let conversation_id: Uuid = conversation.get("id");
        let scope_id: Uuid = conversation.get("scope_id");
        let sequences = messages
            .iter()
            .map(|message| message.sequence)
            .collect::<Vec<_>>();
        let mut tx = self.pool().begin().await?;

        sqlx::query(
            r#"
            delete from application_conversation_messages
            where conversation_id = $1
              and flow_run_id = $2
              and not (sequence = any($3))
            "#,
        )
        .bind(conversation_id)
        .bind(flow_run.id)
        .bind(&sequences)
        .execute(&mut *tx)
        .await?;

        if let Some(title) = messages
            .iter()
            .find(|message| message.role == "user")
            .map(|message| message.content.as_str())
        {
            sqlx::query(
                r#"
                update application_conversations
                set title = coalesce(nullif(title, ''), $2),
                    updated_at = greatest(updated_at, $3)
                where id = $1
                "#,
            )
            .bind(conversation_id)
            .bind(title)
            .bind(flow_run.updated_at)
            .execute(&mut *tx)
            .await?;
        }

        for message in messages {
            sqlx::query(
                r#"
                insert into application_conversation_messages (
                    id,
                    scope_id,
                    conversation_id,
                    application_id,
                    flow_run_id,
                    node_run_id,
                    role,
                    content,
                    sequence,
                    status,
                    started_at,
                    finished_at,
                    created_at,
                    updated_at
                ) values (
                    $1, $2, $3, $4, $5, null, $6, $7, $8, $9, $10, $11, $12, $13
                )
                on conflict (conversation_id, flow_run_id, sequence) do update
                set role = excluded.role,
                    content = excluded.content,
                    status = excluded.status,
                    started_at = excluded.started_at,
                    finished_at = excluded.finished_at,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(scope_id)
            .bind(conversation_id)
            .bind(flow_run.application_id)
            .bind(flow_run.id)
            .bind(message.role)
            .bind(&message.content)
            .bind(message.sequence)
            .bind(flow_run.status.as_str())
            .bind(flow_run.started_at)
            .bind(flow_run.finished_at)
            .bind(flow_run.started_at)
            .bind(flow_run.updated_at)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }

    async fn list_application_run_logs_page(
        &self,
        application_id: Uuid,
        input: ListApplicationRunsPageInput,
    ) -> Result<control_plane::ports::ApplicationRunLogSummaryPage> {
        let page = input.page.max(1);
        let page_size = input.page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;
        let created_after = input.created_after;
        let order_by = Self::application_runs_page_order_by(
            input.sort_by.as_deref(),
            input.sort_order.as_deref(),
        );
        let visible_filter =
            visible_application_run_log_summary_filter_sql("application_run_log_summaries");

        let total = sqlx::query_scalar::<_, i64>(&format!(
            r#"
            select count(*)::bigint
            from application_run_log_summaries
            where application_id = $1
              and ($2::timestamptz is null or created_at >= $2)
              and {visible_filter}
            "#,
        ))
        .bind(application_id)
        .bind(created_after)
        .fetch_one(self.pool())
        .await?;

        let rows = sqlx::query(&format!(
            r#"
            select
                flow_run_id as id,
                run_mode,
                status,
                target_node_id,
                title,
                '{{}}'::jsonb as input_payload,
                external_user,
                created_by,
                authorized_account,
                api_key_id,
                publication_version_id,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                total_tokens,
                input_tokens, output_tokens, input_cache_hit_tokens, input_cache_hit_rate,
                unique_node_count,
                tool_callback_count,
                started_at,
                finished_at,
                created_at,
                updated_at
            from application_run_log_summaries
            where application_id = $1
              and ($2::timestamptz is null or created_at >= $2)
              and {visible_filter}
            order by {order_by}
            limit $3 offset $4
            "#,
            order_by = order_by,
            visible_filter = visible_filter
        ))
        .bind(application_id)
        .bind(created_after)
        .bind(page_size)
        .bind(offset)
        .fetch_all(self.pool())
        .await?;

        let mut items = rows
                .into_iter()
                .map(map_application_run_log_summary)
                .collect::<Result<Vec<_>>>()?;
        let flow_run_ids = items.iter().map(|item| item.run.id).collect::<Vec<_>>();
        let count_tokens_results = self
            .list_application_run_count_tokens_results(&flow_run_ids)
            .await?
            .into_iter()
            .map(|result| (result.flow_run_id, result.input_tokens))
            .collect::<std::collections::HashMap<_, _>>();
        for item in &mut items {
            item.count_tokens_input_tokens = count_tokens_results.get(&item.run.id).copied();
        }

        Ok(control_plane::ports::ApplicationRunLogSummaryPage {
            items,
            total,
            page,
            page_size,
        })
    }

}

fn visible_application_run_log_summary_filter_sql(summary_table: &str) -> String {
    let hidden_internal_run_filter = hidden_anthropic_claude_code_internal_run_sql("runs");
    format!(
        r#"
        not exists (
            select 1
            from flow_runs runs
            where runs.id = {summary_table}.flow_run_id
              and ({hidden_internal_run_filter})
        )
        and exists (
            select 1
            from flow_runs runs
            left join run_archive_import_jobs import_jobs on import_jobs.id = runs.import_job_id
            where runs.id = {summary_table}.flow_run_id
              and (runs.import_job_id is null or import_jobs.status = 'succeeded')
        )
        "#,
        hidden_internal_run_filter = hidden_internal_run_filter
    )
}

#[derive(Debug)]
struct ApplicationConversationMessageProjection {
    role: &'static str,
    content: String,
    sequence: i64,
}

const APPLICATION_CONVERSATION_INPUT_KEYS: &[&str] =
    &["query", "question", "prompt", "message", "input", "input_text"];

fn application_conversation_key(flow_run: &domain::FlowRunRecord) -> String {
    flow_run
        .external_conversation_id
        .clone()
        .unwrap_or_else(|| format!("flow-run:{}", flow_run.id))
}

fn application_conversation_messages_from_flow_run(
    flow_run: &domain::FlowRunRecord,
) -> Vec<ApplicationConversationMessageProjection> {
    if is_anthropic_claude_code_internal_run(flow_run) {
        return Vec::new();
    }

    let mut messages = Vec::new();
    let mut ordinal = 1_i64;

    if let Some(system) = application_conversation_system_text(&flow_run.input_payload) {
        push_application_conversation_message(
            &mut messages,
            flow_run.started_at,
            &mut ordinal,
            "system",
            system,
        );
    }

    if let Some(query) = application_conversation_user_text(&flow_run.input_payload) {
        push_application_conversation_message(
            &mut messages,
            flow_run.started_at,
            &mut ordinal,
            "user",
            query,
        );
    }

    let answer = application_conversation_answer_text(&flow_run.output_payload).or_else(|| {
        flow_run
            .error_payload
            .as_ref()
            .and_then(application_conversation_answer_text)
    });
    if let Some(answer) = answer {
        push_application_conversation_message(
            &mut messages,
            flow_run.started_at,
            &mut ordinal,
            "assistant",
            answer,
        );
    }

    messages
}

fn is_anthropic_claude_code_internal_run(flow_run: &domain::FlowRunRecord) -> bool {
    if flow_run.compatibility_mode.as_deref() != Some("anthropic-messages-v1") {
        return false;
    }

    is_anthropic_claude_code_control_run(flow_run) || is_anthropic_claude_code_subagent_run(flow_run)
}

fn is_anthropic_claude_code_control_run(flow_run: &domain::FlowRunRecord) -> bool {
    if flow_run.compatibility_mode.as_deref() != Some("anthropic-messages-v1") {
        return false;
    }

    application_conversation_start_payload(&flow_run.input_payload)
        .get("compatibility")
        .and_then(|value| value.get("claude_code_control"))
        .and_then(serde_json::Value::as_str)
        .is_some()
        || application_conversation_user_text(&flow_run.input_payload)
            .as_deref()
            .and_then(
                control_plane::application_public_api::compat::anthropic::claude_code_control_kind,
            )
            .is_some()
}

fn is_anthropic_claude_code_subagent_run(flow_run: &domain::FlowRunRecord) -> bool {
    if flow_run.compatibility_mode.as_deref() != Some("anthropic-messages-v1") {
        return false;
    }

    application_conversation_system_text(&flow_run.input_payload)
        .as_deref()
        .is_some_and(is_claude_code_subagent_system)
}

fn is_claude_code_subagent_system(system: &str) -> bool {
    system.contains("cc_is_subagent=true")
        || (system.contains("Agent threads always have their cwd reset between bash calls")
            && system.contains("the parent agent reads your text output"))
}

fn hidden_anthropic_claude_code_internal_run_sql(run_table: &str) -> String {
    let query_text = anthropic_claude_code_query_sql(run_table);
    let system_text = anthropic_claude_code_system_sql(run_table);
    format!(
        r#"
        {run_table}.compatibility_mode = 'anthropic-messages-v1'
        and (
            {run_table}.input_payload #>> '{{node-start,compatibility,claude_code_control}}' is not null
            or {run_table}.input_payload #>> '{{start,compatibility,claude_code_control}}' is not null
            or position('Your task is to create a detailed summary of the conversation so far' in {query_text}) > 0
            or position('Your task is to create a detailed summary of the RECENT portion of the conversation' in {query_text}) > 0
            or position('Your task is to create a detailed summary of this conversation. This summary will be placed at the start of a continuing session' in {query_text}) > 0
            or (
                position('The user stepped away and is coming back. Write exactly 1-3 short sentences.' in {query_text}) > 0
                and position('Next: the concrete next step.' in {query_text}) > 0
            )
            or (
                position('This session is being continued from a previous conversation that ran out of context.' in {query_text}) > 0
                and (
                    position('The summary below covers the earlier portion of the conversation.' in {query_text}) > 0
                    or position('If you need specific details from before compaction' in {query_text}) > 0
                )
            )
            or position('cc_is_subagent=true' in {system_text}) > 0
            or (
                position('Agent threads always have their cwd reset between bash calls' in {system_text}) > 0
                and position('the parent agent reads your text output' in {system_text}) > 0
            )
        )
        "#
    )
}

fn anthropic_claude_code_query_sql(run_table: &str) -> String {
    format!(
        r#"
        coalesce(
            {run_table}.input_payload #>> '{{node-start,query}}',
            {run_table}.input_payload #>> '{{start,query}}',
            {run_table}.input_payload #>> '{{query}}',
            ''
        )
        "#
    )
}

fn anthropic_claude_code_system_sql(run_table: &str) -> String {
    format!(
        r#"
        coalesce(
            {run_table}.input_payload #>> '{{node-start,system}}',
            {run_table}.input_payload #>> '{{start,system}}',
            {run_table}.input_payload #>> '{{system}}',
            ''
        )
        "#
    )
}

fn push_application_conversation_message(
    messages: &mut Vec<ApplicationConversationMessageProjection>,
    started_at: OffsetDateTime,
    ordinal: &mut i64,
    role: &'static str,
    content: String,
) {
    messages.push(ApplicationConversationMessageProjection {
        role,
        content,
        sequence: application_conversation_message_sequence(started_at, *ordinal),
    });
    *ordinal += 1;
}

fn application_conversation_message_sequence(started_at: OffsetDateTime, ordinal: i64) -> i64 {
    started_at
        .unix_timestamp()
        .saturating_mul(1_000_000)
        .saturating_add(ordinal)
}

fn application_conversation_system_text(payload: &serde_json::Value) -> Option<String> {
    let start = application_conversation_start_payload(payload);
    string_field_value(start, "system").or_else(|| string_field_value(payload, "system"))
}

fn application_conversation_user_text(payload: &serde_json::Value) -> Option<String> {
    for source in [payload, application_conversation_start_payload(payload)] {
        for key in APPLICATION_CONVERSATION_INPUT_KEYS {
            if let Some(value) = string_field_value(source, key) {
                return Some(value);
            }
        }
    }

    None
}

fn application_conversation_answer_text(payload: &serde_json::Value) -> Option<String> {
    for key in ["answer", "text", "output", "content", "message"] {
        if let Some(value) = string_field_value(payload, key) {
            return Some(value);
        }
    }

    payload
        .get("error")
        .and_then(|value| value.get("message"))
        .and_then(trimmed_string)
}

fn application_conversation_start_payload(payload: &serde_json::Value) -> &serde_json::Value {
    payload
        .get("node-start")
        .or_else(|| payload.get("start"))
        .unwrap_or(payload)
}

fn string_field_value(value: &serde_json::Value, field: &str) -> Option<String> {
    value.get(field).and_then(conversation_text_value)
}

fn conversation_text_value(value: &serde_json::Value) -> Option<String> {
    trimmed_string(value).or_else(|| {
        value.as_object()?;

        artifact_preview_text(value).or_else(|| {
            value
                .get("text")
                .or_else(|| value.get("content"))
                .and_then(trimmed_string)
        })
    })
}

fn artifact_preview_text(value: &serde_json::Value) -> Option<String> {
    let preview = value.get("preview").and_then(trimmed_string)?;
    decode_artifact_preview_text(&preview).or(Some(preview))
}

fn decode_artifact_preview_text(preview: &str) -> Option<String> {
    if let Ok(decoded) = serde_json::from_str::<String>(preview) {
        return trimmed_text(&decoded);
    }

    let stripped = preview.strip_prefix('"')?;
    let completed = if stripped.ends_with('"') {
        preview.to_owned()
    } else {
        format!("{preview}\"")
    };
    if let Ok(decoded) = serde_json::from_str::<String>(&completed) {
        return trimmed_text(&decoded);
    }

    trimmed_text(stripped.trim_end_matches('"'))
        .map(|value| {
            value
                .replace("\\n", "\n")
                .replace("\\r", "\r")
                .replace("\\t", "\t")
                .replace("\\\"", "\"")
        })
        .and_then(|value| trimmed_text(&value))
}

fn trimmed_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .and_then(trimmed_text)
}

fn trimmed_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}
fn is_terminal_application_run_log_status(status: domain::FlowRunStatus) -> bool {
    matches!(
        status,
        domain::FlowRunStatus::Succeeded
            | domain::FlowRunStatus::Incomplete
            | domain::FlowRunStatus::Failed
            | domain::FlowRunStatus::Cancelled
    )
}
