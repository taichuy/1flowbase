// Canonical bodies live only in the existing message projection. Progress is
// metadata; a correction discards it and streams the original retained facts.
#[derive(Debug, Clone, PartialEq)]
struct NativeCanonicalFact {
    owner: Uuid,
    sequence: i64,
    key: String,
    item: Value,
    conflicting: bool,
}

fn merge_native_canonical_fact(
    facts: &mut std::collections::BTreeMap<String, NativeCanonicalFact>,
    mut candidate: NativeCanonicalFact,
) {
    use std::collections::btree_map::Entry;
    match facts.entry(candidate.key.clone()) {
        Entry::Vacant(entry) => {
            entry.insert(candidate);
        }
        Entry::Occupied(mut entry) => {
            let first = entry.get_mut();
            let conflict =
                first.conflicting || candidate.conflicting || first.item != candidate.item;
            if (candidate.owner, candidate.sequence) < (first.owner, first.sequence) {
                candidate.conflicting = conflict;
                *first = candidate;
            } else {
                first.conflicting = conflict;
            }
        }
    }
}

// Called at the transaction entrance, before any source/queue/projection row.
async fn lock_application_run_native_projection(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    flow_run_id: Uuid,
) -> Result<Option<Uuid>> {
    Ok(
        sqlx::query_scalar("select application_run_lock_native_projection($1)")
            .bind(flow_run_id)
            .fetch_one(&mut **tx)
            .await?,
    )
}

impl PgControlPlaneStore {
    async fn refresh_native_canonical_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        run: &domain::FlowRunRecord,
    ) -> Result<bool> {
        use futures_util::TryStreamExt;
        let identity = sqlx::query("select log_context is not null as native, message_projection_revision from flow_runs where id=$1")
            .bind(run.id).fetch_one(&mut **tx).await?;
        if !identity.try_get::<bool, _>("native")? {
            return Ok(false);
        }
        let conversation = lock_application_run_native_projection(tx, run.id).await?;
        let members: Vec<Uuid> = sqlx::query_scalar("select id from flow_runs where id=$1 union select run_id from application_run_log_conversation_runs($2,$3)")
            .bind(run.id).bind(run.application_id).bind(conversation).fetch_all(&mut **tx).await?;
        let rebuild: bool = sqlx::query_scalar("select exists(select 1 from unnest($1::uuid[]) m(id) left join application_run_native_projection_progress p on p.flow_run_id=m.id where p.invalidated or p.projection_version is distinct from $2 and m.id<>$3) or exists(select 1 from application_run_conversation_message_items where flow_run_id=$3 and projection_version<>$2)")
            .bind(&members).bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION).bind(run.id).fetch_one(&mut **tx).await?;
        let progress = sqlx::query("select output_sequence,result_count,source_revision,conflicting_source_keys from application_run_native_projection_progress where flow_run_id=$1 and projection_version=$2 and not invalidated")
            .bind(run.id).bind(APPLICATION_RUN_CONVERSATION_MESSAGE_ITEM_PROJECTION_VERSION).fetch_optional(&mut **tx).await?;
        let output_sequence: i64 = progress
            .as_ref()
            .map(|p| p.try_get("output_sequence"))
            .transpose()?
            .unwrap_or(i64::MIN);
        let result_count: usize = progress
            .as_ref()
            .map(|p| p.try_get::<i64, _>("result_count"))
            .transpose()?
            .unwrap_or(0)
            .try_into()?;
        if !rebuild {
            if let Some(progress) = progress.as_ref() {
                if progress.try_get::<i64, _>("source_revision")?
                    == identity.try_get::<i64, _>("message_projection_revision")?
                    && sqlx::query_scalar::<_, bool>("select exists(select 1 from application_run_conversation_message_items where flow_run_id=$1)")
                        .bind(run.id).fetch_one(&mut **tx).await?
                {
                    return Ok(true);
                }
                if Self::try_append_native_canonical_projection(tx, run, &members, progress).await?
                {
                    return Ok(true);
                }
            }
        }
        let source_runs = if rebuild {
            members.clone()
        } else {
            vec![run.id]
        };
        let mut sticky_keys = sqlx::query_scalar::<_, String>("select distinct unnest(conflicting_source_keys) from application_run_native_projection_progress where flow_run_id=any($1)")
            .bind(&members).fetch_all(&mut **tx).await?.into_iter().collect::<std::collections::BTreeSet<_>>();
        let mut facts = std::collections::BTreeMap::new();
        let mut changed = std::collections::BTreeSet::from([run.id]);
        let mut read_events = 0_i64;
        let mut read_contexts = 0_i64;
        let mut current_context = None;
        // Stream one context at a time and MOVE results out of it. There is no
        // simultaneous historical context map and duplicate candidate vector.
        {
            let mut rows = sqlx::query("select id,runtime_original_json(log_context,raw_json_payloads,'log_context') as original from flow_runs where id=any($1) order by id")
                .bind(&source_runs).fetch(&mut **tx);
            while let Some(row) = rows.try_next().await? {
                read_contexts += 1;
                let owner: Uuid = row.try_get("id")?;
                let mut context: Option<Value> = row.try_get("original")?;
                if let Some(keys) = context
                    .as_ref()
                    .and_then(|c| c.get("conflicting_output_keys"))
                    .and_then(Value::as_array)
                {
                    sticky_keys.extend(
                        keys.iter()
                            .filter_map(Value::as_str)
                            .map(|key| format!("output:{key}")),
                    );
                }
                let results = context
                    .as_mut()
                    .and_then(Value::as_object_mut)
                    .and_then(|c| c.remove("tool_results"));
                if let Some(Value::Array(results)) = results {
                    for (index, item) in
                        results
                            .into_iter()
                            .enumerate()
                            .skip(if rebuild { 0 } else { result_count })
                    {
                        let fact = native_canonical_result_fact(
                            owner,
                            index,
                            item,
                            context.as_ref().unwrap_or(&Value::Null),
                        );
                        merge_native_canonical_fact(&mut facts, fact);
                    }
                }
                if owner == run.id {
                    current_context = context;
                }
            }
        }
        {
            let mut rows = sqlx::query("select flow_run_id,sequence,id,runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) as original from runtime_events where flow_run_id=any($1) and event_type='provider_output_item_done' and payload->'item' is not null and ($2 or sequence>$3) order by flow_run_id,sequence")
                .bind(&source_runs).bind(rebuild).bind(output_sequence).fetch(&mut **tx);
            while let Some(row) = rows.try_next().await? {
                read_events += 1;
                merge_native_canonical_fact(&mut facts, native_canonical_output_fact(row)?);
            }
        }
        for fact in facts.values_mut() {
            fact.conflicting |= sticky_keys.contains(&fact.key);
        }
        let keys = facts.keys().cloned().collect::<Vec<_>>();
        // Only current bodies and candidate canonical keys are decoded normally.
        // Rebuild retains sticky conflict observations but never trusts old bodies.
        {
            let mut rows = sqlx::query("select flow_run_id,source_item_key,source_occurrence_sequence,runtime_original_json(native_message,raw_json_payloads,'native_message') as original from application_run_conversation_message_items where flow_run_id=any($1) and source_item_key is not null and ($2 or (source_occurrence_sequence is not null and (flow_run_id=$3 or source_item_key=any($4)))) order by flow_run_id,source_occurrence_sequence")
                .bind(&members).bind(rebuild).bind(run.id).bind(&keys).fetch(&mut **tx);
            while let Some(row) = rows.try_next().await? {
                let owner: Uuid = row.try_get("flow_run_id")?;
                let key: String = row.try_get("source_item_key")?;
                let mut original: Value = row.try_get("original")?;
                let conflicting = original
                    .get("_log_conflicting")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if rebuild {
                    if let Some(fact) = facts.get_mut(&key) {
                        fact.conflicting |= conflicting;
                    }
                    continue;
                }
                let item = original
                    .as_object_mut()
                    .and_then(|o| o.remove("_source_item"))
                    .ok_or_else(|| anyhow!("canonical native projection missing original item"))?;
                let sequence = row.try_get("source_occurrence_sequence")?;
                let candidate = NativeCanonicalFact {
                    owner,
                    sequence,
                    key: key.clone(),
                    item,
                    conflicting,
                };
                merge_native_canonical_fact(&mut facts, candidate);
                if let Some(fact) = facts.get(&key) {
                    if fact.owner != owner || fact.conflicting != conflicting {
                        changed.insert(owner);
                        changed.insert(fact.owner);
                    }
                }
            }
        }
        if rebuild {
            changed.extend(members.iter().copied());
        }
        // A late smaller owner can take an existing unique key. Remove all
        // affected rows before inserting any replacement, inside the same lock.
        let changed = changed.into_iter().collect::<Vec<_>>();
        // Fetch the remaining canonical facts of affected historical owners for
        // their display/answer rebuild, never the unrelated conversation bodies.
        if !rebuild {
            let owners = changed
                .iter()
                .copied()
                .filter(|id| *id != run.id)
                .collect::<Vec<_>>();
            let mut rows = sqlx::query("select flow_run_id,source_item_key,source_occurrence_sequence,runtime_original_json(native_message,raw_json_payloads,'native_message') as original from application_run_conversation_message_items where flow_run_id=any($1) and source_occurrence_sequence is not null and not(source_item_key=any($2))")
                .bind(&owners).bind(&keys).fetch(&mut **tx);
            while let Some(row) = rows.try_next().await? {
                let mut original: Value = row.try_get("original")?;
                let conflicting = original
                    .get("_log_conflicting")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let item = original
                    .as_object_mut()
                    .and_then(|o| o.remove("_source_item"))
                    .ok_or_else(|| anyhow!("canonical native projection missing original item"))?;
                merge_native_canonical_fact(
                    &mut facts,
                    NativeCanonicalFact {
                        owner: row.try_get("flow_run_id")?,
                        sequence: row.try_get("source_occurrence_sequence")?,
                        key: row.try_get("source_item_key")?,
                        item,
                        conflicting,
                    },
                );
            }
        }
        sqlx::query(
            "delete from application_run_conversation_message_items where flow_run_id=any($1)",
        )
        .bind(&changed)
        .execute(&mut **tx)
        .await?;
        let orphaned_conflicts = sticky_keys
            .into_iter()
            .filter(|key| !facts.contains_key(key))
            .collect::<Vec<_>>();
        let conflict_carrier = members.iter().min().copied();
        let mut by_owner = std::collections::BTreeMap::<Uuid, Vec<NativeCanonicalFact>>::new();
        for fact in facts.into_values() {
            by_owner.entry(fact.owner).or_default().push(fact);
        }
        read_contexts += changed.iter().filter(|id| **id != run.id).count() as i64;
        for owner in changed {
            let member = Self::fetch_application_run_projection_flow_run(tx, owner).await?;
            let mut context: Option<Value> = if owner == run.id {
                current_context.take()
            } else {
                // A present sidecar, including JSON null, owns restoration and
                // its errors. Only ordinary objects can omit results on the wire.
                sqlx::query_scalar("select case when jsonb_typeof(log_context)='object' and not coalesce(raw_json_payloads ? 'log_context',false) then (log_context-'tool_results')::json else runtime_original_json(log_context,raw_json_payloads,'log_context') end from flow_runs where id=$1").bind(owner).fetch_one(&mut **tx).await?
            };
            // Results have already been consumed; retain only request metadata
            // while constructing the run's output rows.
            if let Some(context) = context.as_mut().and_then(Value::as_object_mut) {
                context.remove("tool_results");
            }
            let revision = Self::application_run_conversation_message_watermark(tx, owner).await?;
            let owned_facts = by_owner.remove(&owner).unwrap_or_default();
            let mut conflicting_keys = owned_facts
                .iter()
                .filter(|fact| fact.conflicting)
                .map(|fact| fact.key.clone())
                .collect::<Vec<_>>();
            if rebuild && Some(owner) == conflict_carrier {
                conflicting_keys.extend(orphaned_conflicts.iter().cloned());
            }
            let native = context.map(|context| {
                Self::application_run_native_message_items(&member, revision, context, owned_facts)
            });
            Self::write_application_run_conversation_projection(tx, &member, native).await?;
            Self::upsert_application_run_log_summary_projection_for_flow_run(tx, &member).await?;
            Self::refresh_application_run_log_task_for_flow_run(tx, owner).await?;
            // Progress publication is part of the same transaction as every
            // affected owner, summary and task projection.
            Self::publish_native_projection_progress(
                tx,
                owner,
                read_contexts,
                read_events,
                &conflicting_keys,
            )
            .await?;
        }
        Ok(true)
    }
}

include!("native_append_projection.rs");
