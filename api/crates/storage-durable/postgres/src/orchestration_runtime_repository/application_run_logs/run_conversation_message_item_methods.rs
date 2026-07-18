const APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION: i32 = 1;

impl PgControlPlaneStore {
    async fn ensure_application_run_conversation_message_items_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<()> {
        let projected_count = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)::bigint
            from application_run_conversation_message_items
            where flow_run_id = $1
              and projection_version = $2
            "#,
        )
        .bind(flow_run.id)
        .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
        .fetch_one(&mut **tx)
        .await?;
        if projected_count > 0 {
            return Ok(());
        }

        Self::replace_application_run_conversation_message_items_projection(tx, flow_run).await
    }

    async fn replace_application_run_conversation_message_items_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<()> {
        Self::delete_application_run_conversation_message_items_projection(tx, flow_run.id).await?;

        let scope_id = sqlx::query_scalar::<_, Uuid>(
            "select workspace_id from applications where id = $1",
        )
        .bind(flow_run.application_id)
        .fetch_one(&mut **tx)
        .await?;
        let llm_system_content =
            Self::application_run_conversation_llm_system_content(tx, flow_run.id).await?;
        let items = application_run_conversation_message_items_from_flow_run(
            flow_run,
            scope_id,
            llm_system_content,
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
                    detail_run_id,
                    can_open_detail,
                    is_current,
                    status,
                    started_at,
                    finished_at,
                    projection_version,
                    created_at,
                    updated_at
                ) values (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20
                )
                "#,
            )
            .bind(Uuid::now_v7())
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
            .bind(item.detail_run_id)
            .bind(item.can_open_detail)
            .bind(item.is_current)
            .bind(item.status)
            .bind(item.started_at)
            .bind(item.finished_at)
            .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
            .bind(item.created_at)
            .bind(item.updated_at)
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

    async fn application_run_conversation_llm_system_content(
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

    async fn delete_application_run_conversation_message_items_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            r#"
            delete from application_run_conversation_message_items
            where flow_run_id = $1
              and projection_version = $2
            "#,
        )
        .bind(flow_run_id)
        .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    async fn list_application_run_conversation_message_items_page(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        input: ListApplicationRunConversationMessageItemsPageInput,
    ) -> Result<control_plane::ports::ApplicationRunConversationMessageItemsPage> {
        let limit = input.limit.clamp(1, 50);
        let mut total_count = self
            .application_run_conversation_message_items_count(application_id, flow_run_id)
            .await?;
        if total_count == 0 {
            self.ensure_application_run_conversation_message_items_projection_for_read(
                application_id,
                flow_run_id,
            )
            .await?;
            total_count = self
                .application_run_conversation_message_items_count(application_id, flow_run_id)
                .await?;
        }

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
                Some(sequence) => self
                    .application_run_conversation_message_item_sequence_exists(
                        application_id,
                        flow_run_id,
                        "display_sequence <= $4",
                        sequence,
                    )
                    .await?,
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
                Some(sequence) => self
                    .application_run_conversation_message_item_sequence_exists(
                        application_id,
                        flow_run_id,
                        "display_sequence >= $4",
                        sequence,
                    )
                    .await?,
                None => false,
            },
        };

        Ok(control_plane::ports::ApplicationRunConversationMessageItemsPage {
            items,
            total_count,
            has_before,
            has_after,
            before_cursor: has_before
                .then(|| {
                    first_sequence
                        .or_else(|| input.after_sequence.map(|sequence| sequence.saturating_add(1)))
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
        })
    }

    async fn application_run_conversation_message_items_count(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<i64> {
        sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)::bigint
            from application_run_conversation_message_items
            where application_id = $1
              and flow_run_id = $2
              and projection_version = $3
            "#,
        )
        .bind(application_id)
        .bind(flow_run_id)
        .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION)
        .fetch_one(self.pool())
        .await
        .map_err(Into::into)
    }

    async fn ensure_application_run_conversation_message_items_projection_for_read(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<()> {
        let Some(flow_run) = fetch_flow_run_for_application(self, application_id, flow_run_id).await?
        else {
            return Ok(());
        };
        if !is_terminal_application_run_log_status(flow_run.status) {
            return Ok(());
        }

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
            Self::ensure_application_run_conversation_message_items_projection(&mut tx, &flow_run)
                .await?;
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
                flow_runs.updated_at
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
            r#"
            select exists(
                select 1
                from application_run_conversation_message_items
                where application_id = $1
                  and flow_run_id = $2
                  and projection_version = $3
                  and {display_sequence_filter}
            )
            "#
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

#[derive(Debug)]
struct ApplicationRunConversationMessageItemProjection {
    scope_id: Uuid,
    application_id: Uuid,
    flow_run_id: Uuid,
    display_sequence: i64,
    source_kind: &'static str,
    role: Option<String>,
    content: Option<String>,
    query: Option<String>,
    model: Option<String>,
    answer: Option<String>,
    detail_run_id: Option<Uuid>,
    can_open_detail: bool,
    is_current: bool,
    status: String,
    started_at: OffsetDateTime,
    finished_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

fn application_run_conversation_message_items_from_flow_run(
    flow_run: &domain::FlowRunRecord,
    scope_id: Uuid,
    llm_system_content: Option<String>,
) -> Vec<ApplicationRunConversationMessageItemProjection> {
    let mut items = Vec::new();
    let model = application_conversation_model_text(&flow_run.input_payload);

    if let Some(system) = application_conversation_system_text(&flow_run.input_payload) {
        push_application_run_conversation_imported_item(
            &mut items,
            flow_run,
            scope_id,
            "system",
            system,
            model.clone(),
        );
    } else if let Some(system) = llm_system_content {
        push_application_run_conversation_imported_item(
            &mut items,
            flow_run,
            scope_id,
            "system",
            system,
            model.clone(),
        );
    }

    let start_payload = application_conversation_start_payload(&flow_run.input_payload);
    if let Some(history) = start_payload
        .get("history")
        .or_else(|| start_payload.get("messages"))
        .and_then(serde_json::Value::as_array)
    {
        for message in history {
            let role = message
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let Some(content) = application_run_conversation_history_message_content(message) else {
                continue;
            };
            if is_hidden_conversation_history_message(message) {
                continue;
            }

            match role {
                "system" if !items.iter().any(|item| item.role.as_deref() == Some("system")) => {
                    push_application_run_conversation_imported_item(
                        &mut items,
                        flow_run,
                        scope_id,
                        role,
                        content,
                        model.clone(),
                    );
                }
                "user" | "assistant" => push_application_run_conversation_imported_item(
                    &mut items,
                    flow_run,
                    scope_id,
                    role,
                    content,
                    model.clone(),
                ),
                _ => {}
            }
        }
    }

    let display_sequence = items.len() as i64;
    items.push(ApplicationRunConversationMessageItemProjection {
        scope_id,
        application_id: flow_run.application_id,
        flow_run_id: flow_run.id,
        display_sequence,
        source_kind: "current_run",
        role: None,
        content: None,
        query: application_conversation_user_text(&flow_run.input_payload),
        model,
        answer: application_conversation_answer_text(&flow_run.output_payload).or_else(|| {
            flow_run
                .error_payload
                .as_ref()
                .and_then(application_conversation_answer_text)
        }),
        detail_run_id: Some(flow_run.id),
        can_open_detail: true,
        is_current: true,
        status: flow_run.status.as_str().to_string(),
        started_at: flow_run.started_at,
        finished_at: flow_run.finished_at,
        created_at: flow_run.created_at,
        updated_at: flow_run.updated_at,
    });

    items
}

fn push_application_run_conversation_imported_item(
    items: &mut Vec<ApplicationRunConversationMessageItemProjection>,
    flow_run: &domain::FlowRunRecord,
    scope_id: Uuid,
    role: &str,
    content: String,
    model: Option<String>,
) {
    let display_sequence = items.len() as i64;
    items.push(ApplicationRunConversationMessageItemProjection {
        scope_id,
        application_id: flow_run.application_id,
        flow_run_id: flow_run.id,
        display_sequence,
        source_kind: "imported_context",
        role: Some(role.to_string()),
        content: Some(content),
        query: None,
        model,
        answer: None,
        detail_run_id: None,
        can_open_detail: false,
        is_current: false,
        status: "succeeded".to_string(),
        started_at: flow_run.started_at,
        finished_at: flow_run.finished_at,
        created_at: flow_run.created_at,
        updated_at: flow_run.updated_at,
    });
}

fn application_run_conversation_history_message_content(
    message: &serde_json::Value,
) -> Option<String> {
    let content = message.get("content")?;
    if let Some(text) = conversation_text_value(content) {
        return Some(text);
    }

    let parts = content.as_array()?;
    let text = parts
        .iter()
        .filter_map(conversation_text_value)
        .collect::<Vec<_>>()
        .join("");
    trimmed_text(&text)
}

fn llm_prompt_messages_system_content(payload: &serde_json::Value) -> Option<String> {
    let prompt_messages_value = payload.get("prompt_messages")?;
    let resolved_prompt_messages = runtime_debug_artifact_preview_value(prompt_messages_value);
    let messages = resolved_prompt_messages
        .as_ref()
        .unwrap_or(prompt_messages_value)
        .as_array()?;
    let system = messages
        .iter()
        .filter(|message| {
            message
                .get("role")
                .and_then(serde_json::Value::as_str)
                == Some("system")
        })
        .filter_map(application_run_conversation_history_message_content)
        .collect::<Vec<_>>()
        .join("\n\n");

    trimmed_text(&system)
}

fn llm_effective_system_content(payload: &serde_json::Value) -> Option<String> {
    let effective_system = payload
        .get("llm_context")
        .and_then(|context| context.get("effective_system"))?;
    let resolved_system = runtime_debug_artifact_preview_value(effective_system);

    resolved_system
        .as_ref()
        .and_then(conversation_prompt_text)
        .or_else(|| conversation_prompt_text(effective_system))
}

fn conversation_prompt_text(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = conversation_text_value(value) {
        return Some(text);
    }

    let parts = value.as_array()?;
    let text = parts
        .iter()
        .filter_map(conversation_text_value)
        .collect::<Vec<_>>()
        .join("");
    trimmed_text(&text)
}

fn runtime_debug_artifact_preview_value(value: &serde_json::Value) -> Option<serde_json::Value> {
    if !value
        .get("__runtime_debug_artifact")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    value
        .get("preview")
        .and_then(serde_json::Value::as_str)
        .and_then(|preview| serde_json::from_str(preview).ok())
}

fn application_conversation_model_text(payload: &serde_json::Value) -> Option<String> {
    string_field_value(payload, "model").or_else(|| {
        let start = application_conversation_start_payload(payload);
        string_field_value(start, "model")
    })
}

fn is_hidden_conversation_history_message(message: &serde_json::Value) -> bool {
    message
        .get("metadata")
        .and_then(|metadata| metadata.get("hidden_from_conversation"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn run_conversation_message_items_select_sql(
    cursor_filter: &str,
    order_by: &str,
    limit_placeholder: &str,
) -> String {
    format!(
        r#"
        select
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
            detail_run_id,
            can_open_detail,
            is_current,
            status,
            started_at,
            finished_at,
            projection_version,
            created_at,
            updated_at
        from application_run_conversation_message_items
        where application_id = $1
          and flow_run_id = $2
          and projection_version = $3
          {cursor_filter}
        order by {order_by}
        limit {limit_placeholder}
        "#
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
    })
}
