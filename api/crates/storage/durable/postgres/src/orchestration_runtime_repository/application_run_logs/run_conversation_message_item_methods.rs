// Reading and writing the run conversation message projection: lifecycle,
// paging, cursor probes and the row mapper. Derivation of the projected rows
// lives in run_conversation_projection_methods.rs.

// Bump when the projection derivation changes: the stored revision is prefixed
// with this version, so rows written by an older writer are never served as
// current.
const APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION: i32 = 5;

/// Roles that carry context rather than a conversation turn. They are projected
/// next to the paged items so the newest page never hides the context in force.
const APPLICATION_RUN_CONTEXT_ROLES: [&str; 2] = ["system", "developer"];

/// Context entries live outside the conversation sequence range. Adding or
/// removing a context entry therefore never shifts a conversation position, so
/// a cursor a client already holds keeps pointing at the same message.
const APPLICATION_RUN_CONTEXT_SEQUENCE_BASE: i64 = 1_000_000;

const APPLICATION_RUN_OUTPUT_SOURCE_PROVIDER_ITEM: &str = "provider_output_item";
const APPLICATION_RUN_OUTPUT_SOURCE_PERSISTED_ANSWER: &str = "persisted_answer";
const APPLICATION_RUN_OUTPUT_SOURCE_ERROR: &str = "error";
const APPLICATION_RUN_OUTPUT_SOURCE_NONE: &str = "none";

impl PgControlPlaneStore {
    /// Returns whether the stored rows were replaced.
    async fn ensure_application_run_conversation_message_items_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<bool> {
        if Self::application_run_conversation_message_items_projection_is_current(tx, flow_run.id)
            .await?
        {
            return Ok(false);
        }

        Self::replace_application_run_conversation_message_items_projection(tx, flow_run).await?;
        Ok(true)
    }

    /// A projection is current only when every row was written from the run
    /// watermark the retained facts currently produce. Rows written before the
    /// watermark existed carry none and are reprojected exactly once.
    async fn application_run_conversation_message_items_projection_is_current(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<bool> {
        let revision =
            Self::application_run_conversation_message_watermark(tx, flow_run_id).await?;
        let current: bool = sqlx::query_scalar(
            r#"
            select coalesce(bool_and(source_revision is not distinct from $3), false)
            from application_run_conversation_message_items
            where flow_run_id = $1
              and projection_version = $2
            "#,
        )
        .bind(flow_run_id)
        .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
        .bind(revision)
        .fetch_one(&mut **tx)
        .await?;
        Ok(current)
    }

    /// The revision a projection must carry to be current: the retained-fact
    /// watermark plus the writer version that produced the rows.
    async fn application_run_conversation_message_watermark(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<Option<String>> {
        let watermark: Option<String> =
            sqlx::query_scalar("select application_run_message_projection_watermark($1)")
                .bind(flow_run_id)
                .fetch_one(&mut **tx)
                .await?;
        Ok(watermark.map(|watermark| {
            format!(
                "v{}:{watermark}",
                APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION
            )
        }))
    }

    async fn replace_application_run_conversation_message_items_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<()> {
        Self::delete_application_run_conversation_message_items_projection(tx, flow_run.id).await?;

        let scope_id =
            sqlx::query_scalar::<_, Uuid>("select workspace_id from applications where id = $1")
                .bind(flow_run.application_id)
                .fetch_one(&mut **tx)
                .await?;
        let revision =
            Self::application_run_conversation_message_watermark(tx, flow_run.id).await?;
        if let Some(items) =
            Self::application_run_native_message_items(tx, flow_run, revision.clone()).await?
        {
            for (sequence, source_key, native) in items.into_iter() {
                let output_source = native
                    .get("_log_output_source")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty());
                let context_source = native
                    .get("_log_context_source")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty());
                sqlx::query(r#"insert into application_run_conversation_message_items(
                    id,scope_id,application_id,flow_run_id,display_sequence,source_kind,role,content,
                    native_message,detail_run_id,can_open_detail,is_current,status,started_at,finished_at,
                    projection_version,log_conversation_id,source_item_key,
                    output_source,context_source,source_revision)
                    select md5(coalesce(log_context->>'log_conversation_id',id::text)||':'||$5)::uuid,
                        $2,application_id,id,$3,'current_run',$6,$7,$8,
                        case when $10::text is null then id else null end,
                        $10::text is null,
                        $10::text is null,
                        case when $10::text is null then status else 'succeeded' end,
                        started_at,finished_at,
                        $4,(log_context->>'log_conversation_id')::uuid,
                        case when $8::jsonb ? '_source_item' then $5 else null end,
                        $9,$10,$11 from flow_runs where id=$1"#)
                    .bind(flow_run.id).bind(scope_id).bind(sequence)
                    .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION).bind(source_key)
                    .bind(native.get("role").and_then(Value::as_str)).bind(native.get("content").and_then(Value::as_str))
                    .bind(&native).bind(output_source).bind(context_source).bind(&revision)
                    .execute(&mut **tx).await?;
            }
            return Ok(());
        }
        let effective_system =
            Self::application_run_conversation_llm_effective_system(tx, flow_run.id).await?;
        let contexts = application_run_conversation_contexts(&flow_run.input_payload, effective_system);
        let llm_assistant_message =
            Self::application_run_conversation_llm_assistant_message(tx, flow_run.id).await?;
        let items = application_run_conversation_message_items_from_flow_run(
            flow_run,
            scope_id,
            contexts,
            llm_assistant_message,
        );

        for item in items {
            sqlx::query(
                r#"
                insert into application_run_conversation_message_items (
                    id,
                    scope_id,
                    application_id,
                    flow_run_id,
                    display_sequence,
                    source_kind,
                    role,
                    content,
                    query,
                    model,
                    answer,
                    native_message,
                    detail_run_id,
                    can_open_detail,
                    is_current,
                    status,
                    started_at,
                    finished_at,
                    projection_version,
                    created_at,
                    updated_at,
                    output_source,
                    context_source,
                    source_revision
                ) values (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                    $21, $22, $23, $24
                )
                "#,
            )
            .bind(
                sqlx::query_scalar::<_, Uuid>("select md5($1)::uuid")
                    .bind(format!(
                        "{}:legacy:{}",
                        item.flow_run_id, item.display_sequence
                    ))
                    .fetch_one(&mut **tx)
                    .await?,
            )
            .bind(item.scope_id)
            .bind(item.application_id)
            .bind(item.flow_run_id)
            .bind(item.display_sequence)
            .bind(item.source_kind)
            .bind(item.role)
            .bind(item.content)
            .bind(item.query)
            .bind(item.model)
            .bind(item.answer)
            .bind(item.native_message)
            .bind(item.detail_run_id)
            .bind(item.can_open_detail)
            .bind(item.is_current)
            .bind(item.status)
            .bind(item.started_at)
            .bind(item.finished_at)
            .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
            .bind(item.created_at)
            .bind(item.updated_at)
            .bind(item.output_source)
            .bind(item.context_source)
            .bind(&revision)
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

    /// The effective system prompt actually sent to the model node: the prompt
    /// messages the node ran with, or the resolved system of its LLM context.
    async fn application_run_conversation_llm_effective_system(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<Option<String>> {
        let rows = sqlx::query(
            r#"
            select input_payload, debug_payload
            from node_runs
            where flow_run_id = $1
              and node_type = 'llm'
            order by started_at asc, id asc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(&mut **tx)
        .await?;

        for row in rows {
            let input_payload: serde_json::Value = row.try_get("input_payload")?;
            if let Some(system) = llm_prompt_messages_system_content(&input_payload) {
                return Ok(Some(system));
            }

            let debug_payload: serde_json::Value = row.try_get("debug_payload")?;
            if let Some(system) = llm_effective_system_content(&debug_payload) {
                return Ok(Some(system));
            }
        }

        Ok(None)
    }

    async fn application_run_conversation_llm_assistant_message(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<Option<serde_json::Value>> {
        let rows = sqlx::query(
            r#"
            select debug_payload
            from node_runs
            where flow_run_id = $1
              and node_type = 'llm'
              and status = 'succeeded'
            order by finished_at desc nulls last, started_at desc, id desc
            "#,
        )
        .bind(flow_run_id)
        .fetch_all(&mut **tx)
        .await?;

        for row in rows {
            let debug_payload: serde_json::Value = row.try_get("debug_payload")?;
            if let Some(message) =
                canonical_assistant_message(debug_payload.get("assistant_message"))
            {
                return Ok(Some(message));
            }
        }

        Ok(None)
    }

    async fn delete_application_run_conversation_message_items_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            r#"
            delete from application_run_conversation_message_items
            where flow_run_id = $1
            "#,
        )
        .bind(flow_run_id)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    async fn list_application_run_conversation_message_items_page(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        input: ListApplicationRunConversationMessageItemsPageInput,
    ) -> Result<control_plane_contracts::ports::ApplicationRunConversationMessageItemsPage> {
        let limit = input.limit.clamp(1, 50);
        self.ensure_application_run_conversation_message_items_projection_for_read(
            application_id,
            flow_run_id,
        )
        .await?;
        let total_count = self
            .application_run_conversation_message_items_count(application_id, flow_run_id)
            .await?;
        let contexts = self
            .application_run_conversation_context_items(application_id, flow_run_id)
            .await?;
        let output_state = self
            .application_run_conversation_output_state(application_id, flow_run_id)
            .await?;

        let mut rows = if let Some(before_sequence) = input.before_sequence {
            let sql = run_conversation_message_items_select_sql(
                "and display_sequence < $4",
                "display_sequence desc, id desc",
                "$5",
            );
            sqlx::query(&sql)
                .bind(application_id)
                .bind(flow_run_id)
                .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
                .bind(before_sequence)
                .bind(limit)
                .fetch_all(self.pool())
                .await?
        } else if let Some(after_sequence) = input.after_sequence {
            let sql = run_conversation_message_items_select_sql(
                "and display_sequence > $4",
                "display_sequence asc, id asc",
                "$5",
            );
            sqlx::query(&sql)
                .bind(application_id)
                .bind(flow_run_id)
                .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
                .bind(after_sequence)
                .bind(limit)
                .fetch_all(self.pool())
                .await?
        } else {
            let sql = run_conversation_message_items_select_sql(
                "",
                "display_sequence desc, id desc",
                "$4",
            );
            sqlx::query(&sql)
                .bind(application_id)
                .bind(flow_run_id)
                .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
                .bind(limit)
                .fetch_all(self.pool())
                .await?
        };

        if input.before_sequence.is_some()
            || (input.before_sequence.is_none() && input.after_sequence.is_none())
        {
            rows.reverse();
        }

        let items = rows
            .into_iter()
            .map(map_application_run_conversation_message_item)
            .collect::<Result<Vec<_>>>()?;
        let first_sequence = items.first().map(|item| item.display_sequence);
        let last_sequence = items.last().map(|item| item.display_sequence);
        let has_before = match first_sequence {
            Some(sequence) => {
                self.application_run_conversation_message_item_sequence_exists(
                    application_id,
                    flow_run_id,
                    "display_sequence < $4",
                    sequence,
                )
                .await?
            }
            None => match input.after_sequence {
                Some(sequence) => {
                    self.application_run_conversation_message_item_sequence_exists(
                        application_id,
                        flow_run_id,
                        "display_sequence <= $4",
                        sequence,
                    )
                    .await?
                }
                None => false,
            },
        };
        let has_after = match last_sequence {
            Some(sequence) => {
                self.application_run_conversation_message_item_sequence_exists(
                    application_id,
                    flow_run_id,
                    "display_sequence > $4",
                    sequence,
                )
                .await?
            }
            None => match input.before_sequence {
                Some(sequence) => {
                    self.application_run_conversation_message_item_sequence_exists(
                        application_id,
                        flow_run_id,
                        "display_sequence >= $4",
                        sequence,
                    )
                    .await?
                }
                None => false,
            },
        };

        Ok(
            control_plane_contracts::ports::ApplicationRunConversationMessageItemsPage {
                items,
                contexts,
                output_state,
                total_count,
                has_before,
                has_after,
                before_cursor: has_before
                    .then(|| {
                        first_sequence.or_else(|| {
                            input
                                .after_sequence
                                .map(|sequence| sequence.saturating_add(1))
                        })
                    })
                    .flatten(),
                after_cursor: has_after
                    .then(|| {
                        last_sequence.or_else(|| {
                            input
                                .before_sequence
                                .map(|sequence| sequence.saturating_sub(1))
                        })
                    })
                    .flatten(),
                newest_sequence: last_sequence,
            },
        )
    }

    async fn application_run_conversation_message_items_count(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<i64> {
        sqlx::query_scalar::<_, i64>(&format!(
            "{} select count(*)::bigint from task_message_items",
            application_run_task_message_items_cte(),
        ))
        .bind(application_id)
        .bind(flow_run_id)
        .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
        .fetch_one(self.pool())
        .await
        .map_err(Into::into)
    }

    /// Context entries are served beside the page, never inside it, so a full
    /// page of new turns cannot push the context out of reach.
    async fn application_run_conversation_context_items(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Vec<domain::ApplicationRunConversationContextItem>> {
        let rows = sqlx::query(
            r#"
            select id, flow_run_id, role, context_source, content, display_sequence
            from application_run_conversation_message_items
            where application_id = $1
              and flow_run_id = $2
              and projection_version = $3
              and context_source is not null
            order by display_sequence asc, id asc
            "#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(domain::ApplicationRunConversationContextItem {
                    id: row.try_get("id")?,
                    flow_run_id: row.try_get("flow_run_id")?,
                    role: row.try_get("role")?,
                    context_source: row.try_get("context_source")?,
                    content: row.try_get("content")?,
                    display_sequence: row.try_get("display_sequence")?,
                })
            })
            .collect()
    }

    async fn application_run_conversation_output_state(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunConversationOutputState>> {
        let row = sqlx::query(
            r#"
            select
                f.id as flow_run_id,
                f.status,
                coalesce(s.call_kind, 'generate') as call_kind,
                f.log_context->>'request_kind' as request_kind,
                -- The state reports where the visible answer came from: a
                -- stored answer outranks tool-call items, which are output
                -- items but not an answer.
                coalesce(max(case m.output_source
                    when 'persisted_answer' then 3
                    when 'provider_output_item' then 2
                    when 'error' then 1
                    when 'none' then 0
                end), 0) as output_rank,
                count(*) filter (where m.output_source = 'provider_output_item')::bigint
                    as output_item_count
            from flow_runs f
            left join application_run_log_summaries s on s.flow_run_id = f.id
            left join application_run_conversation_message_items m
                on m.flow_run_id = f.id
               and m.projection_version = $3
            where f.application_id = $1
              and f.id = $2
            group by f.id, f.status, s.call_kind, f.log_context
            "#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
        .fetch_optional(self.pool())
        .await?;

        row.map(|row| {
            let output_rank: i32 = row.try_get("output_rank")?;
            Ok(domain::ApplicationRunConversationOutputState {
                flow_run_id: row.try_get("flow_run_id")?,
                status: row.try_get("status")?,
                call_kind: row.try_get("call_kind")?,
                request_kind: row.try_get("request_kind")?,
                output_source: application_run_output_source_from_rank(output_rank).to_string(),
                output_item_count: row.try_get("output_item_count")?,
            })
        })
        .transpose()
    }

    async fn ensure_application_run_conversation_message_items_projection_for_read(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<()> {
        let Some(flow_run) =
            fetch_flow_run_for_application(self, application_id, flow_run_id).await?
        else {
            return Ok(());
        };

        let mut tx = self.pool().begin().await?;
        let locked_flow_run_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            select id
            from flow_runs
            where application_id = $1
              and id = $2
            for update
            "#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .fetch_optional(&mut *tx)
        .await?;
        if locked_flow_run_id.is_some() {
            // A call that is still running already retains its request and every
            // completed provider output item, so the read model reflects those
            // facts instead of hiding the call until it reaches a terminal state.
            let replaced =
                Self::ensure_application_run_conversation_message_items_projection(&mut tx, &flow_run)
                    .await?;
            if replaced {
                // The task row derives from member projections; refresh it in the
                // same transaction so list and detail never disagree.
                Self::refresh_application_run_log_task_for_flow_run(&mut tx, flow_run_id).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    async fn get_application_run_conversation_current_item(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::ApplicationRunConversationMessageItem>> {
        let row = sqlx::query(
            r#"
            select
                flow_runs.id as id,
                applications.workspace_id as scope_id,
                flow_runs.application_id,
                flow_runs.id as flow_run_id,
                0::bigint as display_sequence,
                'current_run'::text as source_kind,
                null::text as role,
                null::text as content,
                coalesce(
                    nullif(btrim(flow_runs.input_payload #>> '{query}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{input_text}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{node-start,query}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{node-start,question}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{node-start,prompt}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{node-start,message}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{node-start,input}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{start,query}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{start,question}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{start,prompt}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{start,message}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{start,input}'), '')
                ) as query,
                coalesce(
                    nullif(btrim(flow_runs.input_payload #>> '{model}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{node-start,model}'), ''),
                    nullif(btrim(flow_runs.input_payload #>> '{start,model}'), '')
                ) as model,
                coalesce(
                    case when jsonb_typeof(flow_runs.output_payload -> 'answer') = 'string'
                        then nullif(btrim(flow_runs.output_payload #>> '{answer}'), '') end,
                    case when jsonb_typeof(flow_runs.output_payload -> 'answer') = 'object'
                        then nullif(btrim(flow_runs.output_payload #>> '{answer,preview}'), '') end,
                    case when jsonb_typeof(flow_runs.output_payload -> 'text') = 'string'
                        then nullif(btrim(flow_runs.output_payload #>> '{text}'), '') end,
                    case when jsonb_typeof(flow_runs.output_payload -> 'output') = 'string'
                        then nullif(btrim(flow_runs.output_payload #>> '{output}'), '') end,
                    case when jsonb_typeof(flow_runs.output_payload -> 'content') = 'string'
                        then nullif(btrim(flow_runs.output_payload #>> '{content}'), '') end,
                    case when jsonb_typeof(flow_runs.output_payload -> 'message') = 'string'
                        then nullif(btrim(flow_runs.output_payload #>> '{message}'), '') end,
                    nullif(btrim(flow_runs.error_payload #>> '{error,message}'), '')
                ) as answer,
                flow_runs.id as detail_run_id,
                true as can_open_detail,
                true as is_current,
                flow_runs.status,
                flow_runs.started_at,
                flow_runs.finished_at,
                $3::integer as projection_version,
                flow_runs.created_at,
                flow_runs.updated_at,
                case
                    when jsonb_typeof(flow_runs.output_payload -> 'answer') = 'string'
                      or jsonb_typeof(flow_runs.output_payload -> 'text') = 'string'
                      or jsonb_typeof(flow_runs.output_payload -> 'output') = 'string'
                      or jsonb_typeof(flow_runs.output_payload -> 'content') = 'string'
                      or jsonb_typeof(flow_runs.output_payload -> 'message') = 'string'
                    then 'persisted_answer'
                    when nullif(btrim(flow_runs.error_payload #>> '{error,message}'), '') is not null
                    then 'error'
                    else 'none'
                end as output_source
            from flow_runs
            join applications on applications.id = flow_runs.application_id
            where flow_runs.application_id = $1
              and flow_runs.id = $2
            "#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_application_run_conversation_message_item)
            .transpose()
    }

    async fn application_run_conversation_message_item_sequence_exists(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        display_sequence_filter: &'static str,
        display_sequence: i64,
    ) -> Result<bool> {
        let sql = format!(
            "{} select exists(select 1 from task_message_items where {})",
            application_run_task_message_items_cte(),
            display_sequence_filter.replace("display_sequence", "task_sequence"),
        );

        let exists = sqlx::query_scalar::<_, bool>(&sql)
            .bind(application_id)
            .bind(flow_run_id)
            .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
            .bind(display_sequence)
            .fetch_one(self.pool())
            .await?;
        Ok(exists)
    }
}

fn application_run_output_source_from_rank(rank: i32) -> &'static str {
    match rank {
        3 => APPLICATION_RUN_OUTPUT_SOURCE_PERSISTED_ANSWER,
        2 => APPLICATION_RUN_OUTPUT_SOURCE_PROVIDER_ITEM,
        1 => APPLICATION_RUN_OUTPUT_SOURCE_ERROR,
        _ => APPLICATION_RUN_OUTPUT_SOURCE_NONE,
    }
}

// The run message projection is single-run scoped; task-level convergence is
// served from the task row, never by joining member projections at read time.
// Context entries are not part of the paged stream, so a caller that only reads
// the newest page still sees every context entry.
fn application_run_task_message_items_cte() -> &'static str {
    r#"with task_message_items as (
        select m.*, m.display_sequence as task_sequence
        from application_run_conversation_message_items m
        where m.application_id=$1 and m.flow_run_id=$2 and m.projection_version=$3
          and m.context_source is null
    )"#
}

fn run_conversation_message_items_select_sql(
    cursor_filter: &str,
    order_by: &str,
    limit_placeholder: &str,
) -> String {
    format!(
        r#"{}
        select id,scope_id,application_id,flow_run_id,task_sequence as display_sequence,
            source_kind,role,content,query,model,answer,detail_run_id,can_open_detail,
            is_current,status,started_at,finished_at,projection_version,created_at,updated_at,
            output_source
        from task_message_items where true {}
        order by {} limit {limit_placeholder}"#,
        application_run_task_message_items_cte(),
        cursor_filter.replace("display_sequence", "task_sequence"),
        order_by.replace("display_sequence", "task_sequence"),
    )
}

fn map_application_run_conversation_message_item(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ApplicationRunConversationMessageItem> {
    Ok(domain::ApplicationRunConversationMessageItem {
        id: row.try_get("id")?,
        scope_id: row.try_get("scope_id")?,
        application_id: row.try_get("application_id")?,
        flow_run_id: row.try_get("flow_run_id")?,
        display_sequence: row.try_get("display_sequence")?,
        source_kind: row.try_get("source_kind")?,
        role: row.try_get("role")?,
        content: row.try_get("content")?,
        query: row.try_get("query")?,
        model: row.try_get("model")?,
        answer: row.try_get("answer")?,
        detail_run_id: row.try_get("detail_run_id")?,
        can_open_detail: row.try_get("can_open_detail")?,
        is_current: row.try_get("is_current")?,
        status: row.try_get("status")?,
        started_at: row.try_get("started_at")?,
        finished_at: row.try_get("finished_at")?,
        projection_version: row.try_get("projection_version")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        output_source: row.try_get("output_source")?,
    })
}
