// Derivation of the run conversation message projection from retained facts:
// context entries, conversation turns, formal provider output items and the
// persisted answer. Included into the same module as the projection writer, so
// the writer stays the only component that writes these rows.

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
    output_source: Option<&'static str>,
    context_source: Option<&'static str>,
}

/// One system/developer context entry in force for a call, with the layer it
/// came from: the client request, the application configuration, or the
/// effective prompt the model node actually ran with.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ApplicationRunConversationContext {
    role: String,
    context_source: &'static str,
    content: String,
}

fn application_run_conversation_contexts(
    input_payload: &serde_json::Value,
    effective_system: Option<String>,
) -> Vec<ApplicationRunConversationContext> {
    let mut contexts = Vec::new();
    let native_context = input_payload
        .get(extension_contracts::provider_contract::NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY);

    if let Some(system) = native_context
        .and_then(|value| value.get("system"))
        .and_then(native_model_system_text)
    {
        push_application_run_conversation_context(
            &mut contexts,
            "system",
            "client_request",
            system,
        );
    }
    if let Some(messages) = native_context
        .and_then(|value| value.get("messages"))
        .and_then(serde_json::Value::as_array)
    {
        for message in messages {
            let Some(role) = message.get("role").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if !APPLICATION_RUN_CONTEXT_ROLES.contains(&role) {
                continue;
            }
            if let Some(content) = application_run_conversation_history_message_content(message) {
                push_application_run_conversation_context(
                    &mut contexts,
                    role,
                    "client_request",
                    content,
                );
            }
        }
    }
    if let Some(system) = application_conversation_system_text(input_payload) {
        push_application_run_conversation_context(
            &mut contexts,
            "system",
            "application_config",
            system,
        );
    }
    if let Some(system) = effective_system {
        push_application_run_conversation_context(
            &mut contexts,
            "system",
            "effective_prompt",
            system,
        );
    }

    contexts
}

/// Context entries are deduplicated by content so the same effective prompt is
/// reported once, from its earliest source.
fn push_application_run_conversation_context(
    contexts: &mut Vec<ApplicationRunConversationContext>,
    role: &str,
    context_source: &'static str,
    content: String,
) {
    let Some(content) = trimmed_text(&content) else {
        return;
    };
    if contexts.iter().any(|context| context.content == content) {
        return;
    }
    contexts.push(ApplicationRunConversationContext {
        role: role.to_string(),
        context_source,
        content,
    });
}

fn native_model_system_text(value: &serde_json::Value) -> Option<String> {
    let blocks = value.as_array()?;
    let text = blocks
        .iter()
        .filter_map(conversation_text_value)
        .collect::<Vec<_>>()
        .join("\n\n");
    trimmed_text(&text)
}

fn application_run_conversation_message_items_from_flow_run(
    flow_run: &domain::FlowRunRecord,
    scope_id: Uuid,
    contexts: Vec<ApplicationRunConversationContext>,
    llm_assistant_message: Option<serde_json::Value>,
) -> Vec<ApplicationRunConversationMessageItemProjection> {
    let mut items = Vec::new();
    let model = application_conversation_model_text(&flow_run.input_payload);

    let mut conversation_sequence = 0_i64;
    for (index, context) in contexts.into_iter().enumerate() {
        push_application_run_conversation_context_item(
            &mut items,
            flow_run,
            scope_id,
            context,
            model.clone(),
            APPLICATION_RUN_CONTEXT_SEQUENCE_BASE + index as i64,
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
            // Context roles are served from the context projection, never as a
            // conversation turn.
            if APPLICATION_RUN_CONTEXT_ROLES.contains(&role) {
                continue;
            }
            let Some(content) = application_run_conversation_history_message_content(message)
            else {
                continue;
            };
            if is_hidden_conversation_history_message(message) {
                continue;
            }

            match role {
                "user" | "assistant" => {
                    push_application_run_conversation_imported_item(
                        &mut items,
                        flow_run,
                        scope_id,
                        role,
                        content,
                        model.clone(),
                        conversation_sequence,
                    );
                    conversation_sequence += 1;
                }
                _ => {}
            }
        }
    }

    let output_answer = application_conversation_answer_text(&flow_run.output_payload);
    let error_answer = flow_run
        .error_payload
        .as_ref()
        .and_then(application_conversation_answer_text);
    let output_source = match (&output_answer, &error_answer) {
        (Some(_), _) => APPLICATION_RUN_OUTPUT_SOURCE_PERSISTED_ANSWER,
        (None, Some(_)) => APPLICATION_RUN_OUTPUT_SOURCE_ERROR,
        (None, None) => APPLICATION_RUN_OUTPUT_SOURCE_NONE,
    };
    items.push(ApplicationRunConversationMessageItemProjection {
        scope_id,
        application_id: flow_run.application_id,
        flow_run_id: flow_run.id,
        display_sequence: conversation_sequence,
        source_kind: "current_run",
        role: None,
        content: None,
        query: application_conversation_user_text(&flow_run.input_payload),
        model,
        answer: output_answer.or(error_answer),
        native_message: llm_assistant_message,
        detail_run_id: Some(flow_run.id),
        can_open_detail: true,
        is_current: true,
        status: flow_run.status.as_str().to_string(),
        started_at: flow_run.started_at,
        finished_at: flow_run.finished_at,
        created_at: flow_run.created_at,
        updated_at: flow_run.updated_at,
        output_source: Some(output_source),
        context_source: None,
    });

    items
}

fn push_application_run_conversation_context_item(
    items: &mut Vec<ApplicationRunConversationMessageItemProjection>,
    flow_run: &domain::FlowRunRecord,
    scope_id: Uuid,
    context: ApplicationRunConversationContext,
    model: Option<String>,
    display_sequence: i64,
) {
    items.push(ApplicationRunConversationMessageItemProjection {
        scope_id,
        application_id: flow_run.application_id,
        flow_run_id: flow_run.id,
        display_sequence,
        source_kind: "imported_context",
        role: Some(context.role),
        content: Some(context.content),
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
        output_source: None,
        context_source: Some(context.context_source),
    });
}

fn push_application_run_conversation_imported_item(
    items: &mut Vec<ApplicationRunConversationMessageItemProjection>,
    flow_run: &domain::FlowRunRecord,
    scope_id: Uuid,
    role: &str,
    content: String,
    model: Option<String>,
    display_sequence: i64,
) {
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
        output_source: None,
        context_source: None,
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

impl PgControlPlaneStore {
    // Called by the existing message projection owner. Runtime events and
    // retained request facts remain the rebuild sources, never diagnostics.
    async fn application_run_native_message_items(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        run: &domain::FlowRunRecord,
        revision: Option<String>,
    ) -> Result<Option<Vec<(i64, String, Value)>>> {
        let context: Option<Value> =
            sqlx::query_scalar("select runtime_original_json(log_context, raw_json_payloads, 'log_context') from flow_runs where id=$1")
                .bind(run.id)
                .fetch_one(&mut **tx)
                .await?;
        let Some(context) = context else {
            return Ok(None);
        };
        let rows = sqlx::query(
            r#"with scope_ids as materialized (
                select id from flow_runs where id=$1
                union
                select run_id as id from application_run_log_conversation_runs($2,$3::uuid)
            ), scope_runs as materialized (
                select f.id,f.log_context
                from scope_ids member join flow_runs f on f.id=member.id
            ), facts as (
                select f.id as run_id,e.sequence,
                    'output:'||case when e.payload->'item'->>'call_id' is not null then 'tool'
                        else coalesce(e.payload->'item'->>'type','unknown') end||':'||
                        coalesce(e.payload->'item'->>'call_id',e.payload->'item'->>'id',e.id::text) as source_key,
                    runtime_original_json(e.payload, e.raw_json_payloads, 'payload') as original,
                    null::bigint as result_ordinal
                from scope_runs f join runtime_events e on e.flow_run_id=f.id
                where e.event_type='provider_output_item_done' and e.payload->'item' is not null
                union all
                select f.id,-1000000+r.ordinality,
                    'result:'||coalesce(r.item->>'call_id',f.id::text||':'||r.ordinality),
                    null::json as original, r.ordinality as result_ordinal
                from scope_runs f cross join lateral jsonb_array_elements(
                    coalesce(f.log_context->'tool_results','[]'::jsonb)) with ordinality r(item,ordinality)
            )
            select run_id, sequence, source_key, original, result_ordinal from facts order by source_key, run_id, sequence"#,
        ).bind(run.id).bind(run.application_id)
            .bind(context.get("log_conversation_id").and_then(Value::as_str).and_then(|v| Uuid::parse_str(v).ok()))
            .fetch_all(&mut **tx).await?;

        // JSONB is a searchable projection. Compare restored values here so a
        // real NUL and a literal escape remain different facts.
        let mut groups = std::collections::BTreeMap::<String, Vec<(Uuid, i64, Value)>>::new();
        let mut result_rows = Vec::new();
        for row in rows {
            let run_id = row.try_get("run_id")?;
            let sequence = row.try_get("sequence")?;
            let source_key: String = row.try_get("source_key")?;
            let ordinal: Option<i64> = row.try_get("result_ordinal")?;
            if let Some(ordinal) = ordinal {
                result_rows.push((run_id, sequence, source_key, ordinal));
            } else {
                let original: Value = row.try_get("original")?;
                let item = original.get("item").cloned().ok_or_else(|| {
                    anyhow!("native log fact projection does not match its original")
                })?;
                groups
                    .entry(source_key)
                    .or_default()
                    .push((run_id, sequence, item));
            }
        }
        if !result_rows.is_empty() {
            let run_ids = result_rows
                .iter()
                .map(|(run_id, _, _, _)| *run_id)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            // PostgreSQL JSON cannot extract a member containing U+0000. Transfer
            // each original context once, then select its results in serde_json.
            let contexts = sqlx::query(
                "select id,runtime_original_json(log_context,raw_json_payloads,'log_context') as original from flow_runs where id=any($1)",
            )
            .bind(&run_ids)
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .map(|row| Ok((row.try_get("id")?, row.try_get("original")?)))
            .collect::<Result<std::collections::BTreeMap<Uuid, Value>>>()?;
            for (run_id, sequence, source_key, ordinal) in result_rows {
                let item = ordinal
                    .checked_sub(1)
                    .and_then(|index| usize::try_from(index).ok())
                    .and_then(|index| {
                        contexts
                            .get(&run_id)?
                            .get("tool_results")?
                            .as_array()?
                            .get(index)
                    })
                    .cloned()
                    .ok_or_else(|| {
                        anyhow!("native log fact projection does not match its original")
                    })?;
                groups
                    .entry(source_key)
                    .or_default()
                    .push((run_id, sequence, item));
            }
        }
        let mut facts = Vec::new();
        for (key, candidates) in groups {
            let (owner, sequence, item) = &candidates[0];
            let conflicting = candidates.iter().any(|(_, _, other)| other != item);
            if *owner == run.id {
                facts.push((*sequence, key, item.clone(), conflicting));
            } else if conflicting {
                // This derivation is called only by the projection writer. A later
                // member can contradict a fact owned by an earlier run; update that
                // derived marker in the same transaction without changing its evidence.
                let original: Option<Value> = sqlx::query_scalar(
                    r#"
                    select runtime_original_json(native_message,raw_json_payloads,'native_message')
                    from application_run_conversation_message_items
                    where flow_run_id=$1 and source_item_key=$2
                      and native_message->>'_log_conflicting' is distinct from 'true'
                    for update
                "#,
                )
                .bind(owner)
                .bind(&key)
                .fetch_optional(&mut **tx)
                .await?;
                if let Some(mut original) = original {
                    original["_log_conflicting"] = Value::Bool(true);
                    sqlx::query(r#"
                        update application_run_conversation_message_items
                        set native_message=($3::jsonb -> 0),
                            raw_json_payloads=(raw_json_payloads - 'native_message') ||
                                jsonb_strip_nulls(jsonb_build_object('native_message', $3::jsonb -> 1))
                        where flow_run_id=$1 and source_item_key=$2
                    "#).bind(owner).bind(&key).bind(lossless_json_parameter(&original))
                        .execute(&mut **tx).await?;
                }
            }
        }
        facts.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
        let mut items: Vec<(i64, String, Value)> = Vec::new();
        // Context is not a conversation turn: it is projected in its own
        // sequence range and served beside the page so it stays discoverable
        // regardless of pagination.
        let effective_system =
            Self::application_run_conversation_llm_effective_system(tx, run.id).await?;
        for (index, entry) in
            application_run_conversation_contexts(&run.input_payload, effective_system)
                .into_iter()
                .enumerate()
        {
            items.push((
                APPLICATION_RUN_CONTEXT_SEQUENCE_BASE + index as i64,
                format!("context:{}:{}:{}", run.id, entry.context_source, index),
                json!({
                    "role": entry.role,
                    "content": entry.content,
                    "_log_context_source": entry.context_source,
                }),
            ));
        }
        let mut conversation_sequence = 0_i64;
        // A full request history does not create another user message. Only an
        // explicit task anchor owns that task's observed prompt.
        let task = context.get("log_task_run_id").and_then(Value::as_str);
        if task.is_none() || task == Some(run.id.to_string().as_str()) {
            if let Some(prompt) = context.get("prompt").filter(|v| v.is_object()) {
                items.push((conversation_sequence,format!("prompt:{}",run.id),json!({"role":"user","content":native_log_item_text(prompt),"_source_item":prompt})));
                conversation_sequence += 1;
            }
        }
        if conversation_sequence == 0 && task.is_none() {
            if let Some(prompt) = application_conversation_user_text(&run.input_payload) {
                items.push((
                    conversation_sequence,
                    format!("prompt:{}", run.id),
                    json!({"role":"user","content":prompt}),
                ));
                conversation_sequence += 1;
            }
        }
        for (_, key, item, observed_conflict) in &facts {
            let key = key.clone();
            let item = item.clone();
            let role = if key.starts_with("result:") {
                "tool"
            } else {
                "assistant"
            };
            let mut conflicting = *observed_conflict;
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
            items.push((conversation_sequence,key,json!({"role":role,"content":native_log_item_text(&item),"_source_item":item,"_log_conflicting":conflicting,"_log_output_source":APPLICATION_RUN_OUTPUT_SOURCE_PROVIDER_ITEM})));
            conversation_sequence += 1;
        }
        // Tool calls are output items but they are not the answer. A call whose
        // only formal items are tool calls still shows the answer it stored,
        // marked as persisted evidence rather than as a provider message. A
        // formal answer item arriving later replaces it on the next
        // reprojection, so the answer is never counted twice.
        let has_formal_answer = facts.iter().any(|(_, key, item, _)| {
            key.starts_with("output:") && native_output_item_is_answer(item)
        });
        if !has_formal_answer {
            if let Some(answer) = application_conversation_answer_text(&run.output_payload) {
                items.push((
                    conversation_sequence,
                    format!("answer:{}", run.id),
                    json!({
                        "role":"assistant",
                        "content":answer,
                        "_log_output_source":APPLICATION_RUN_OUTPUT_SOURCE_PERSISTED_ANSWER,
                    }),
                ));
                conversation_sequence += 1;
            }
        }
        // Every real call keeps its original detail/trace link even if it only
        // retransmits facts owned by another call. This is not another message.
        if conversation_sequence == 0 {
            conversation_sequence += 1;
            items.push((
                conversation_sequence - 1,
                format!("run:{}", run.id),
                json!({
                    "role":null,
                    "content":null,
                    "_log_output_source":APPLICATION_RUN_OUTPUT_SOURCE_NONE,
                }),
            ));
        }
        for (_, _, message) in &mut items {
            message["_log_source_revision"] = json!(revision);
        }
        Ok(Some(items))
    }
}

/// A formal provider output item answers the call when it carries a message
/// payload; tool calls and tool results do not.
fn native_output_item_is_answer(item: &Value) -> bool {
    if item.get("type").and_then(Value::as_str) == Some("message") {
        return true;
    }
    if item.get("phase").and_then(Value::as_str) == Some("final_answer") {
        return true;
    }
    item.get("content").is_some() && item.get("role").and_then(Value::as_str) == Some("assistant")
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
