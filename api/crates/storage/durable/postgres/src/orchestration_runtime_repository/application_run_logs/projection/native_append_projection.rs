// The bounded review proves ordered extension, not commutativity. This path
// admits only unchanged old representatives and strictly later new ranks. Any
// changed representative, ownership, ordering or run metadata uses the original
// complete repair before this function has written a row.
impl PgControlPlaneStore {
    async fn try_append_native_canonical_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        run: &domain::FlowRunRecord,
        members: &[Uuid],
        progress: &sqlx::postgres::PgRow,
    ) -> Result<bool> {
        use futures_util::TryStreamExt;
        let row = sqlx::query(r#"select message_projection_revision,
            raw_json_payloads ? 'log_context' as has_original,
            log_context->'prompt' as prompt,log_context->'log_task_run_id' as log_task_run_id,
            log_context->'conflicting_output_keys' as conflicting_output_keys,
            log_context->'conflicting_result_call_ids' as conflicting_result_call_ids,
            jsonb_array_length(coalesce(log_context->'tool_results','[]'::jsonb))::bigint as result_count
            from flow_runs where id=$1"#).bind(run.id).fetch_one(&mut **tx).await?;
        if row.try_get::<bool, _>("has_original")? {
            // The sidecar is one original JSON document, not independently
            // addressable result elements. Never slice its searchable surrogate.
            return Ok(false);
        }
        let source_revision: i64 = row.try_get("message_projection_revision")?;
        let previous_revision: i64 = progress.try_get("source_revision")?;
        let existing: bool = sqlx::query_scalar("select exists(select 1 from application_run_conversation_message_items where flow_run_id=$1)")
            .bind(run.id).fetch_one(&mut **tx).await?;
        if !existing {
            return Ok(false);
        }
        if source_revision == previous_revision {
            return Ok(true);
        }
        let previous_results: i64 = progress.try_get("result_count")?;
        if row.try_get::<i64, _>("result_count")? < previous_results
            || application_conversation_answer_text(&run.output_payload).is_some()
        {
            return Ok(false);
        }
        let mut context = serde_json::Map::new();
        for field in [
            "prompt",
            "log_task_run_id",
            "conflicting_output_keys",
            "conflicting_result_call_ids",
        ] {
            if let Some(value) = row.try_get::<Option<Value>, _>(field)? {
                context.insert(field.to_owned(), value);
            }
        }
        let context = Value::Object(context);
        let mut facts = std::collections::BTreeMap::new();
        // JSONPath addresses only the new suffix; the complete tool_results
        // array is never returned to Rust on this path.
        let results: Value = sqlx::query_scalar("select jsonb_path_query_array(log_context,'$.tool_results[$start to last]',jsonb_build_object('start',$2::bigint)) from flow_runs where id=$1")
            .bind(run.id).bind(previous_results).fetch_one(&mut **tx).await?;
        if let Value::Array(results) = results {
            for (offset, item) in results.into_iter().enumerate() {
                let fact = native_canonical_result_fact(
                    run.id,
                    previous_results as usize + offset,
                    item,
                    &context,
                );
                merge_native_canonical_fact(&mut facts, fact);
            }
        }
        let mut read_events = 0_i64;
        {
            let mut rows = sqlx::query("select flow_run_id,sequence,id,runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) as original from runtime_events where flow_run_id=$1 and event_type='provider_output_item_done' and payload->'item' is not null and sequence>$2 order by sequence")
                .bind(run.id).bind(progress.try_get::<i64,_>("output_sequence")?).fetch(&mut **tx);
            while let Some(row) = rows.try_next().await? {
                read_events += 1;
                merge_native_canonical_fact(&mut facts, native_canonical_output_fact(row)?);
            }
        }
        let sticky: Vec<String> = sqlx::query_scalar("select distinct unnest(conflicting_source_keys) from application_run_native_projection_progress where flow_run_id=any($1)")
            .bind(members).fetch_all(&mut **tx).await?;
        for fact in facts.values_mut() {
            fact.conflicting |= sticky.contains(&fact.key)
                || context
                    .get("conflicting_output_keys")
                    .and_then(Value::as_array)
                    .is_some_and(|keys| {
                        keys.iter()
                            .any(|key| key.as_str() == fact.key.strip_prefix("output:"))
                    });
        }
        let keys = facts.keys().cloned().collect::<Vec<_>>();
        if !keys.is_empty() {
            let mut rows = sqlx::query("select flow_run_id,source_item_key,source_occurrence_sequence,runtime_original_json(native_message,raw_json_payloads,'native_message') as original from application_run_conversation_message_items where flow_run_id=any($1) and source_occurrence_sequence is not null and source_item_key=any($2) order by flow_run_id,source_occurrence_sequence")
                .bind(members).bind(&keys).fetch(&mut **tx);
            while let Some(row) = rows.try_next().await? {
                let mut original: Value = row.try_get("original")?;
                let old = NativeCanonicalFact {
                    owner: row.try_get("flow_run_id")?,
                    sequence: row.try_get("source_occurrence_sequence")?,
                    key: row.try_get("source_item_key")?,
                    conflicting: original
                        .get("_log_conflicting")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    item: original
                        .as_object_mut()
                        .and_then(|object| object.remove("_source_item"))
                        .ok_or_else(|| {
                            anyhow!("canonical native projection missing original item")
                        })?,
                };
                // Preserve delta ⊕ old, including its same-rank left bias.
                merge_native_canonical_fact(&mut facts, old.clone());
                if facts.get(&old.key) != Some(&old) {
                    return Ok(false);
                }
                facts.remove(&old.key);
            }
        }
        let last_fact: Option<(i64, String)> = sqlx::query_as("select source_occurrence_sequence,source_item_key from application_run_conversation_message_items where flow_run_id=$1 and source_occurrence_sequence is not null order by source_occurrence_sequence desc,source_item_key desc limit 1")
            .bind(run.id).fetch_optional(&mut **tx).await?;
        if facts.values().any(|fact| {
            last_fact
                .as_ref()
                .is_some_and(|(sequence, key)| (fact.sequence, &fact.key) <= (*sequence, key))
        }) {
            return Ok(false);
        }
        let revision = Some(format!(
            "v{}:{source_revision}",
            APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION
        ));
        let mut specials = Self::application_run_native_message_items(
            run,
            revision.clone(),
            context.clone(),
            vec![],
        );
        if last_fact.is_some() || !facts.is_empty() {
            specials.retain(|(_, key, _)| key != &format!("run:{}", run.id));
        }
        let special_count: i64 = sqlx::query_scalar("select count(*) from application_run_conversation_message_items where flow_run_id=$1 and source_occurrence_sequence is null")
            .bind(run.id).fetch_one(&mut **tx).await?;
        if special_count != specials.len() as i64 {
            return Ok(false);
        }
        for (sequence, key, body) in specials {
            let packet = lossless_json_value(&body);
            if !packet[1].is_null() {
                return Ok(false);
            }
            let unchanged: bool = sqlx::query_scalar(r#"select exists(select 1 from application_run_conversation_message_items m
                join flow_runs f on f.id=m.flow_run_id
                where m.flow_run_id=$1 and m.id=md5(coalesce(f.log_context->>'log_conversation_id',f.id::text)||':'||$2)::uuid
                    and m.display_sequence=$3 and not(m.raw_json_payloads ? 'native_message')
                    and m.native_message-'_log_source_revision'=($4::jsonb->0)-'_log_source_revision')"#)
                .bind(run.id).bind(key).bind(sequence).bind(packet).fetch_one(&mut **tx).await?;
            if !unchanged {
                return Ok(false);
            }
        }
        let metadata_changed: bool = sqlx::query_scalar("select exists(select 1 from application_run_conversation_message_items where flow_run_id=$1 and (status is distinct from case when context_source is null then $2 else 'succeeded' end or started_at is distinct from $3 or finished_at is distinct from $4))")
            .bind(run.id).bind(run.status.as_str()).bind(run.started_at).bind(run.finished_at).fetch_one(&mut **tx).await?;
        if metadata_changed {
            return Ok(false);
        }
        // All eligibility checks precede writes. Existing rows (including their
        // TOAST originals) stay untouched; only new tail rows are inserted.
        let mut conflicting_keys: Vec<String> = sqlx::query_scalar("select source_item_key from application_run_conversation_message_items where flow_run_id=$1 and source_item_key=any($2) order by source_item_key")
            .bind(run.id).bind(progress.try_get::<Vec<String>,_>("conflicting_source_keys")?).fetch_all(&mut **tx).await?;
        conflicting_keys.extend(
            facts
                .values()
                .filter(|fact| fact.conflicting)
                .map(|fact| fact.key.clone()),
        );
        conflicting_keys.sort();
        if !facts.is_empty() {
            let tail: i64 = sqlx::query_scalar("select display_sequence from application_run_conversation_message_items where flow_run_id=$1 and context_source is null order by display_sequence desc limit 1")
                .bind(run.id).fetch_one(&mut **tx).await?;
            let scope: Uuid =
                sqlx::query_scalar("select workspace_id from applications where id=$1")
                    .bind(run.application_id)
                    .fetch_one(&mut **tx)
                    .await?;
            let items = Self::application_run_native_message_items(
                run,
                revision.clone(),
                context,
                facts.into_values().collect(),
            );
            for (index, (_, key, body)) in items
                .into_iter()
                .filter(|(_, _, body)| body.get("_log_source_sequence").is_some())
                .enumerate()
            {
                Self::insert_application_run_native_message_item(
                    tx,
                    run.id,
                    scope,
                    &revision,
                    tail + 1 + index as i64,
                    key,
                    body,
                )
                .await?;
            }
        }
        Self::publish_native_projection_progress(tx, run.id, 1, read_events, &conflicting_keys)
            .await?;
        // Every current-run caller already owns the summary before projection
        // and the task refresh afterwards. No historical owner changed here.
        Ok(true)
    }

    async fn publish_native_projection_progress(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        owner: Uuid,
        read_contexts: i64,
        read_events: i64,
        conflicting_keys: &[String],
    ) -> Result<()> {
        sqlx::query("insert into application_run_native_projection_progress(flow_run_id,projection_version,output_sequence,result_count,source_revision,invalidated,conflicting_source_keys,last_read_context_count,last_read_event_count) select f.id,$2,coalesce((select max(sequence) from runtime_events where flow_run_id=f.id and event_type='provider_output_item_done'),'-9223372036854775808'::bigint),jsonb_array_length(coalesce(f.log_context->'tool_results','[]'::jsonb)),f.message_projection_revision,false,$5,$3,$4 from flow_runs f where id=$1 on conflict(flow_run_id) do update set projection_version=excluded.projection_version,output_sequence=excluded.output_sequence,result_count=excluded.result_count,source_revision=excluded.source_revision,invalidated=false,conflicting_source_keys=excluded.conflicting_source_keys,last_read_context_count=excluded.last_read_context_count,last_read_event_count=excluded.last_read_event_count")
            .bind(owner).bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION).bind(read_contexts).bind(read_events).bind(conflicting_keys).execute(&mut **tx).await?;
        Ok(())
    }
}

fn native_canonical_result_fact(
    owner: Uuid,
    index: usize,
    item: Value,
    context: &Value,
) -> NativeCanonicalFact {
    let key = format!(
        "result:{}",
        item.get("call_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{owner}:{}", index + 1))
    );
    let conflicting = context
        .get("conflicting_result_call_ids")
        .and_then(Value::as_array)
        .is_some_and(|keys| {
            keys.iter()
                .any(|value| value.as_str() == key.strip_prefix("result:"))
        });
    NativeCanonicalFact {
        owner,
        sequence: -1_000_000 + index as i64 + 1,
        key,
        item,
        conflicting,
    }
}

fn native_canonical_output_fact(row: sqlx::postgres::PgRow) -> Result<NativeCanonicalFact> {
    let owner = row.try_get("flow_run_id")?;
    let sequence = row.try_get("sequence")?;
    let event: Uuid = row.try_get("id")?;
    let mut original: Value = row.try_get("original")?;
    let item = original
        .as_object_mut()
        .and_then(|object| object.remove("item"))
        .ok_or_else(|| anyhow!("native log fact projection does not match its original"))?;
    let kind = if item.get("call_id").and_then(Value::as_str).is_some() {
        "tool"
    } else {
        item.get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    };
    let id = item
        .get("call_id")
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| event.to_string());
    Ok(NativeCanonicalFact {
        owner,
        sequence,
        key: format!("output:{kind}:{id}"),
        item,
        conflicting: false,
    })
}

fn apply_published_native_revision(message: &mut Value, revision: Option<String>) {
    if let (Some(object), Some(revision)) = (message.as_object_mut(), revision) {
        if object.contains_key("_log_source_revision") {
            object.insert("_log_source_revision".into(), Value::String(revision));
        }
    }
}
