const APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION: i32 = 4;

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
            let revision =
                Self::application_run_native_message_source_revision(tx, flow_run.id).await?;
            let stale: bool = sqlx::query_scalar(
                "select exists(select 1 from application_run_conversation_message_items where flow_run_id=$1 and native_message->>'_log_source_revision' is distinct from $2)"
            ).bind(flow_run.id).bind(revision.map(|v| v.to_string())).fetch_one(&mut **tx).await?;
            if !stale {
                return Ok(());
            }
        }

        Self::replace_application_run_conversation_message_items_projection(tx, flow_run).await
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
        if let Some(items) = Self::application_run_native_message_items(tx, flow_run).await? {
            for (sequence, (source_key, native)) in items.into_iter().enumerate() {
                sqlx::query(r#"insert into application_run_conversation_message_items(
                    id,scope_id,application_id,flow_run_id,display_sequence,source_kind,role,content,
                    native_message,detail_run_id,can_open_detail,is_current,status,started_at,finished_at,
                    projection_version,log_conversation_id,source_item_key)
                    select md5(coalesce(log_context->>'log_conversation_id',id::text)||':'||$5)::uuid,
                        $2,application_id,id,$3,'current_run',$6,$7,$8,id,true,true,status,started_at,finished_at,
                        $4,(log_context->>'log_conversation_id')::uuid,$5 from flow_runs where id=$1"#)
                    .bind(flow_run.id).bind(scope_id).bind(sequence as i64)
                    .bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION).bind(source_key)
                    .bind(native.get("role").and_then(Value::as_str)).bind(native.get("content").and_then(Value::as_str))
                    .bind(&native).execute(&mut **tx).await?;
            }
            return Ok(());
        }
        let llm_system_content =
            Self::application_run_conversation_llm_system_content(tx, flow_run.id).await?;
        let llm_assistant_message =
            Self::application_run_conversation_llm_assistant_message(tx, flow_run.id).await?;
        let items = application_run_conversation_message_items_from_flow_run(
            flow_run,
            scope_id,
            llm_system_content,
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
                    updated_at
                ) values (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                    $21
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
        let related_runs = sqlx::query_scalar::<_, Uuid>(
            "select run_id from application_run_log_task_runs($1,$2) order by run_id",
        ).bind(application_id).bind(flow_run_id).fetch_all(self.pool()).await?;
        for related in related_runs {
            self.ensure_application_run_conversation_message_items_projection_for_read(
                application_id,
                related,
            )
            .await?;
        }
        let total_count = self
            .application_run_conversation_message_items_count(application_id, flow_run_id)
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
    native_message: Option<serde_json::Value>,
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
    llm_assistant_message: Option<serde_json::Value>,
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
            let Some(content) = application_run_conversation_history_message_content(message)
            else {
                continue;
            };
            if is_hidden_conversation_history_message(message) {
                continue;
            }

            match role {
                "system"
                    if !items
                        .iter()
                        .any(|item| item.role.as_deref() == Some("system")) =>
                {
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
        native_message: llm_assistant_message,
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
        native_message: None,
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

fn canonical_assistant_message(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let object = value?.as_object()?;
    if object.get("role").and_then(serde_json::Value::as_str) != Some("assistant") {
        return None;
    }
    let content = object.get("content")?.as_str()?;
    let mut message = serde_json::Map::new();
    message.insert(
        "role".to_string(),
        serde_json::Value::String("assistant".to_string()),
    );
    message.insert(
        "content".to_string(),
        serde_json::Value::String(content.to_string()),
    );
    for field in ["content_blocks", "tool_calls"] {
        if let Some(value) = object.get(field).filter(|value| value.is_array()) {
            message.insert(field.to_string(), value.clone());
        }
    }
    Some(serde_json::Value::Object(message))
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
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("system"))
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

// Original message projection, scoped by the same durable task identity as
// the original list. Single-run cursors retain their existing sequence values.
fn application_run_task_message_items_cte() -> &'static str {
    r#"with task_runs as (
        select s.* from application_run_log_task_runs($1,$2) member
        join application_run_log_summaries s on s.flow_run_id=member.run_id
    ), message_items as (
        select m.* from task_runs s
        join application_run_conversation_message_items m on m.flow_run_id=s.flow_run_id
            and m.application_id=s.application_id and m.scope_id=s.scope_id
        where m.projection_version=$3
        union all
        select live.* from task_runs s
        cross join lateral jsonb_populate_record(null::application_run_conversation_message_items,
            jsonb_build_object('id',s.flow_run_id,'scope_id',s.scope_id,'application_id',s.application_id,
                'flow_run_id',s.flow_run_id,'display_sequence',0,'source_kind','current_run',
                'detail_run_id',s.flow_run_id,'can_open_detail',true,'is_current',s.flow_run_id=$2,
                'status',s.status,'started_at',s.started_at,'finished_at',s.finished_at,
                'created_at',s.created_at,'updated_at',s.updated_at,'projection_version',$3)) live
        where s.log_task_run_id is not null
            and s.status in ('queued','running','waiting_callback','waiting_human','paused')
            and not exists(select 1 from application_run_conversation_message_items m
                where m.flow_run_id=s.flow_run_id and m.projection_version=$3)
    ), task_message_items as (
        select m.*,case when s.log_task_run_id is null then m.display_sequence
            else row_number() over(order by m.flow_run_id,m.display_sequence,m.id)-1 end as task_sequence
        from message_items m join task_runs s on s.flow_run_id=m.flow_run_id
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
            is_current,status,started_at,finished_at,projection_version,created_at,updated_at
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
    })
}

impl PgControlPlaneStore {
    // Called by the existing terminal message projection owner. Runtime events
    // and retained request facts remain the rebuild sources, never diagnostics.
    async fn application_run_native_message_items(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        run: &domain::FlowRunRecord,
    ) -> Result<Option<Vec<(String, Value)>>> {
        let context: Option<Value> =
            sqlx::query_scalar("select log_context from flow_runs where id=$1")
                .bind(run.id)
                .fetch_one(&mut **tx)
                .await?;
        let Some(context) = context else {
            return Ok(None);
        };
        // Capture before reading facts: a concurrent append can only leave an
        // older revision, forcing a fresh read instead of marking stale data current.
        let revision = Self::application_run_native_message_source_revision(tx, run.id).await?;
        let rows = sqlx::query(
            r#"with scope_runs as materialized (
                select id,log_context from flow_runs where id=$1
                union
                select f.id,f.log_context from application_run_log_conversation_runs($2,$3::uuid) member
                join flow_runs f on f.id=member.run_id
            ), facts as (
                select f.id as run_id,e.sequence,
                    'output:'||case when e.payload->'item'->>'call_id' is not null then 'tool'
                        else coalesce(e.payload->'item'->>'type','unknown') end||':'||
                        coalesce(e.payload->'item'->>'call_id',e.payload->'item'->>'id',e.id::text) as source_key,
                    e.payload->'item' as item
                from scope_runs f join runtime_events e on e.flow_run_id=f.id
                where e.event_type='provider_output_item_done' and e.payload->'item' is not null
                union all
                select f.id,-1000000+r.ordinality,
                    'result:'||coalesce(r.item->>'call_id',f.id::text||':'||r.ordinality),r.item
                from scope_runs f cross join lateral jsonb_array_elements(
                    coalesce(f.log_context->'tool_results','[]'::jsonb)) with ordinality r(item,ordinality)
            ), owners as (
                select distinct on(source_key) * from facts order by source_key,run_id,sequence
            )
            select o.source_key,o.item,
                exists(select 1 from facts f where f.source_key=o.source_key and f.item is distinct from o.item) as conflicting
            from owners o where run_id=$1 order by sequence,source_key"#,
        ).bind(run.id).bind(run.application_id)
            .bind(context.get("log_conversation_id").and_then(Value::as_str).and_then(|v| Uuid::parse_str(v).ok()))
            .fetch_all(&mut **tx).await?;
        let mut items = Vec::new();
        // A full request history does not create another user message. Only an
        // explicit task anchor owns that task's observed prompt.
        let task = context.get("log_task_run_id").and_then(Value::as_str);
        if task.is_none() || task == Some(run.id.to_string().as_str()) {
            if let Some(prompt) = context.get("prompt").filter(|v| v.is_object()) {
                items.push((format!("prompt:{}",run.id),json!({"role":"user","content":native_log_item_text(prompt),"_source_item":prompt})));
            }
        }
        if items.is_empty() && task.is_none() {
            if let Some(prompt) = application_conversation_user_text(&run.input_payload) {
                items.push((
                    format!("prompt:{}", run.id),
                    json!({"role":"user","content":prompt}),
                ));
            }
        }
        let has_formal_output = rows
            .iter()
            .any(|row| row.get::<String, _>("source_key").starts_with("output:"));
        if !has_formal_output && task.is_none() {
            if let Some(answer) = application_conversation_answer_text(&run.output_payload) {
                items.push((
                    format!("answer:{}", run.id),
                    json!({"role":"assistant","content":answer}),
                ));
            }
        }
        for row in rows {
            let key: String = row.try_get("source_key")?;
            let item: Value = row.try_get("item")?;
            let role = if key.starts_with("result:") {
                "tool"
            } else {
                "assistant"
            };
            let mut conflicting: bool = row.try_get("conflicting")?;
            // Historical conflict observations survive projection rebuilds.
            let retained = if key.starts_with("result:") {
                "conflicting_result_call_ids"
            } else {
                "conflicting_output_keys"
            };
            conflicting |= context
                .get(retained)
                .and_then(Value::as_array)
                .is_some_and(|keys| {
                    keys.iter()
                        .any(|v| v.as_str() == key.split_once(':').map(|(_, id)| id))
                });
            items.push((key,json!({"role":role,"content":native_log_item_text(&item),"_source_item":item,"_log_conflicting":conflicting})));
        }
        // Every real call keeps its original detail/trace link even if it only
        // retransmits facts owned by another call. This is not another message.
        if items.is_empty() {
            items.push((
                format!("run:{}", run.id),
                json!({"role":null,"content":null}),
            ));
        }
        for (_, message) in &mut items {
            message["_log_source_revision"] = json!(revision);
        }
        Ok(Some(items))
    }

    // Formal events and retained request contexts are append-only facts. Include
    // every scoped call so a later replay/conflict invalidates the original owner,
    // without introducing another projection writer into the event pipeline.
    async fn application_run_native_message_source_revision(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        flow_run_id: Uuid,
    ) -> Result<Option<i64>> {
        sqlx::query_scalar(r#"
            with anchor as (select * from flow_runs where id=$1 and log_context is not null),
            members as materialized (
                select id,log_context from anchor
                union
                select f.id,f.log_context from anchor a
                join application_run_log_conversation_runs(a.application_id,(a.log_context->>'log_conversation_id')::uuid) member on true
                join flow_runs f on f.id=member.run_id
            )
            select case when exists(select 1 from anchor) then
                (select count(*)+coalesce(sum(jsonb_array_length(coalesce(log_context->'tool_results','[]'::jsonb))),0)::bigint from members)
                +(select count(*) from runtime_events e join members m on m.id=e.flow_run_id where e.event_type='provider_output_item_done')
            end
        "#).bind(flow_run_id).fetch_one(&mut **tx).await.map_err(Into::into)
    }
}

fn native_log_item_text(item: &Value) -> String {
    if let Some(text) = item.as_str() {
        return text.to_owned();
    }
    for field in ["content", "output", "summary"] {
        if let Some(value) = item.get(field) {
            if let Some(text) = value.as_str() {
                return text.to_owned();
            }
            if let Some(parts) = value.as_array() {
                let text = parts
                    .iter()
                    .filter_map(|part| part.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    item.get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
