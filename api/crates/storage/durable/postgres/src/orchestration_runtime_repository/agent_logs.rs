use super::*;
use control_plane_contracts::ports::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn ingest(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    scope_id: Uuid,
    api_key_id: Uuid,
    batch: &AgentLogsBatch,
    costs: &[Option<String>],
) -> Result<AgentLogsReceipt> {
    batch.validate()?;
    anyhow::ensure!(
        costs.len() == batch.events.len(),
        "agent_logs.cost_snapshot"
    );
    let mut tx = store.pool().begin().await?;
    // Serializes the entire identity namespace, including creation and parent binding.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("agent-logs:{application_id}:{}", batch.source_id))
        .execute(&mut *tx)
        .await?;
    let supported:bool=sqlx::query_scalar("select exists(select 1 from applications where id=$1 and scope_id=$2 and application_type='agent_logs')").bind(application_id).bind(scope_id).fetch_one(&mut *tx).await?;
    anyhow::ensure!(supported, "agent_logs.application_type");
    let mut receipt = AgentLogsReceipt {
        accepted_events: 0,
        duplicate_events: 0,
        record_ids: vec![],
    };
    let mut records = BTreeSet::new();
    for (event, cost) in batch.events.iter().zip(costs) {
        let payload = serde_json::to_value(event)?;
        let (hash, _) = canonical_runtime_json_identity(
            &json!({"source_client":batch.source_client,"event":payload}),
        )?;
        let existing:Option<(String,Uuid)>=sqlx::query_as("select payload_hash,record_id from application_log_upload_receipts where application_id=$1 and source_id=$2 and event_id=$3")
            .bind(application_id).bind(&batch.source_id).bind(&event.event_id).fetch_optional(&mut *tx).await?;
        if let Some((old, record)) = existing {
            anyhow::ensure!(old == hash, "agent_logs.event_conflict");
            receipt.duplicate_events += 1;
            records.insert(record);
            continue;
        }
        let record_id =
            ensure_task(&mut tx, application_id, scope_id, api_key_id, batch, event).await?;
        let request_id = record_id;
        sqlx::query("insert into client_trajectory_captures(request_id,record_id,event_sequence,status) values($1,$1,0,'complete') on conflict(request_id) do nothing").bind(record_id).execute(&mut *tx).await?;
        let step_id = Uuid::now_v7();
        let kind = serde_json::to_value(event.kind)?
            .as_str()
            .unwrap()
            .to_owned();
        let step = ClientTrajectoryStep {
            id: step_id,
            request_id,
            sequence: event.sequence,
            created_at: event.occurred_at.clone(),
            category: kind.clone(),
            name: event.name.clone().unwrap_or(kind),
            namespace: None,
            preview: event
                .content
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect(),
            parameters_preview: None,
            result_preview: None,
            status: if event.inherited {
                "inherited"
            } else {
                "observed"
            }
            .into(),
            origin: "imported".into(),
            protocol: batch.source_client.clone(),
            transport: ClientTrajectoryTransport::File,
            flow_run_id: None,
            node_run_id: None,
            parent_id: None,
            call_id: event.call_id.clone(),
            item_id: Some(event.event_id.clone()),
            response_id: event.usage.as_ref().and_then(|u| u.response_id.clone()),
            turn_id: Some(event.source_task_id.clone()),
            related_step_id: None,
            available_sections: vec!["overview".into(), "raw".into(), "result".into()],
        };
        let (metadata, originals) = lossless_json_columns("metadata", &serde_json::to_value(step)?);
        sqlx::query("insert into client_trajectory_steps(id,request_id,record_id,event_sequence,metadata,raw_json_payloads,observed_at,semantic_metadata_restored) values($1,$2,$3,$4,$5,$6,$7,true)")
            .bind(step_id).bind(request_id).bind(record_id).bind(event.sequence).bind(metadata).bind(originals).bind(&event.occurred_at).execute(&mut *tx).await?;
        let (content_id, _value_hash, _value_byte_size, created) =
            put_canonical_runtime_content_with_creation(
                &mut tx,
                scope_id,
                application_id,
                &payload,
            )
            .await?;
        if created {
            sqlx::query("insert into runtime_observation_body_ownership(content_id) values($1) on conflict do nothing").bind(content_id).execute(&mut *tx).await?;
        }
        // Body lives once. Selected sections reference the whole source fact or its content child.
        for section in ["overview", "raw", "result"] {
            let path: Vec<String> = if section == "result" && event.content.is_some() {
                vec!["content".into()]
            } else {
                vec![]
            };
            let selected_value = if path.is_empty() {
                &payload
            } else {
                payload.get("content").expect("source content exists")
            };
            let (value_hash, value_byte_size) = canonical_runtime_json_identity(selected_value)?;
            sqlx::query("insert into client_trajectory_sections(id,request_id,step_id,record_id,section,event_sequence,content_id,content_path,body_kind,observed_at,value_hash,value_byte_size) values($1,$2,$3,$4,$5,$6,$7,$8,'content',$9,$10,$11)")
                .bind(Uuid::now_v7()).bind(request_id).bind(step_id).bind(record_id).bind(section).bind(event.sequence).bind(content_id).bind(path).bind(&event.occurred_at).bind(&value_hash).bind(value_byte_size).execute(&mut *tx).await?;
        }
        sqlx::query("insert into application_log_upload_receipts(application_id,source_id,event_id,payload_hash,record_id,step_id,rated_cost) values($1,$2,$3,$4,$5,$6,$7::numeric)")
            .bind(application_id).bind(&batch.source_id).bind(&event.event_id).bind(hash).bind(record_id).bind(step_id).bind(cost).execute(&mut *tx).await?;
        records.insert(record_id);
        receipt.accepted_events += 1;
    }
    for record in &records {
        refresh_task(&mut tx, application_id, *record).await?;
    }
    sqlx::query("update application_run_log_tasks child set parent_task_run_id=parent.id from application_run_log_tasks parent where child.application_id=$1 and parent.application_id=$1 and child.source_id=$2 and parent.source_id=$2 and child.source_session_id=parent.source_session_id and child.parent_source_task_id=parent.source_task_id and child.id<>parent.id and child.parent_task_run_id is distinct from parent.id")
        .bind(application_id).bind(&batch.source_id).execute(&mut *tx).await?;
    receipt.record_ids = records.into_iter().collect();
    tx.commit().await?;
    Ok(receipt)
}

async fn ensure_task(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    application_id: Uuid,
    scope_id: Uuid,
    api_key_id: Uuid,
    batch: &AgentLogsBatch,
    event: &AgentLogEvent,
) -> Result<Uuid> {
    let id:Option<(Uuid,String)>=sqlx::query_as("select id,source_client from application_run_log_tasks where application_id=$1 and source_id=$2 and source_session_id=$3 and source_task_id=$4 and source_kind='imported'")
        .bind(application_id).bind(&batch.source_id).bind(&event.source_session_id).bind(&event.source_task_id).fetch_optional(&mut **tx).await?;
    if let Some((id, client)) = id {
        anyhow::ensure!(client == batch.source_client, "agent_logs.event_conflict");
        return Ok(id);
    }
    let id = Uuid::now_v7();
    sqlx::query("insert into application_run_log_tasks(id,application_id,scope_id,member_run_ids,run_mode,status,outcome,title,call_kind,api_key_id,started_at,created_at,updated_at,source_kind,source_id,source_client,source_session_id,source_task_id,total_cost) values($1,$2,$3,'{}','imported','running','in_progress',$4,'collected',$5,$6::timestamptz,$6::timestamptz,$6::timestamptz,'imported',$7,$8,$9,$10,0)")
        .bind(id).bind(application_id).bind(scope_id).bind(event.content.as_deref().unwrap_or(&event.source_task_id).chars().take(100).collect::<String>()).bind(api_key_id).bind(&event.occurred_at).bind(&batch.source_id).bind(&batch.source_client).bind(&event.source_session_id).bind(&event.source_task_id).execute(&mut **tx).await?;
    Ok(id)
}

async fn refresh_task(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    application_id: Uuid,
    record_id: Uuid,
) -> Result<()> {
    let rows=sqlx::query("select runtime_original_json(c.content,c.raw_json_payloads,'content') as body,r.rated_cost::text as cost from application_log_upload_receipts r join client_trajectory_sections s on s.step_id=r.step_id and s.section='overview' join runtime_canonical_contents c on c.id=s.content_id and c.application_id=r.application_id where r.application_id=$1 and r.record_id=$2 order by s.event_sequence,r.event_id")
        .bind(application_id).bind(record_id).fetch_all(&mut **tx).await?;
    let events = rows
        .into_iter()
        .map(|r| {
            Ok((
                serde_json::from_value::<AgentLogEvent>(r.get("body"))?,
                r.get::<Option<String>, _>("cost"),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut messages = vec![];
    let mut user = None;
    let mut final_output = None;
    let mut ended = false;
    let mut parent = None;
    let mut usage_groups: BTreeMap<String, Vec<&(AgentLogEvent, Option<String>)>> = BTreeMap::new();
    for item in &events {
        let e = &item.0;
        if e.parent_source_task_id.is_some() {
            parent = e.parent_source_task_id.clone();
        }
        if e.inherited && e.kind != AgentLogEventKind::System {
            continue;
        }
        if let Some(content) = &e.content {
            match e.kind {
                AgentLogEventKind::System => messages.push(AgentLogMessage {
                    role: "system".into(),
                    content: content.clone(),
                    sequence: e.sequence,
                }),
                AgentLogEventKind::User => {
                    user = Some(content.clone());
                }
                AgentLogEventKind::Assistant if e.phase.as_deref() == Some("final_answer") => {
                    final_output = Some((content.clone(), e.sequence));
                }
                _ => {}
            }
        }
        if e.kind == AgentLogEventKind::TaskEnd {
            ended = true;
        }
        if let Some(u) = &e.usage {
            if e.inherited {
                continue;
            }
            usage_groups
                .entry(u.response_id.clone().unwrap_or_default())
                .or_default()
                .push(item);
        }
    }
    if let Some(e) = events
        .iter()
        .rev()
        .find(|(e, _)| !e.inherited && e.kind == AgentLogEventKind::User)
    {
        if let Some(c) = &e.0.content {
            messages.push(AgentLogMessage {
                role: "user".into(),
                content: c.clone(),
                sequence: e.0.sequence,
            });
        }
    }
    if let Some((content, sequence)) = &final_output {
        messages.push(AgentLogMessage {
            role: "assistant".into(),
            content: content.clone(),
            sequence: *sequence,
        });
    }
    let mut totals = [None::<i64>; 4];
    let mut complete = [true; 4];
    let mut total_cost = rust_decimal::Decimal::ZERO;
    let mut cost_complete = !usage_groups.is_empty();
    for group in usage_groups.values() {
        let deltas = group
            .iter()
            .filter(|(e, _)| e.usage.as_ref().unwrap().basis == AgentLogUsageBasis::Delta)
            .copied()
            .collect::<Vec<_>>();
        let selected = if deltas.is_empty() {
            group.last().copied().into_iter().collect::<Vec<_>>()
        } else {
            deltas
        };
        for (e, cost) in selected {
            let u = e.usage.as_ref().unwrap();
            for (index, value) in [
                u.total_tokens,
                u.input_tokens,
                u.output_tokens,
                u.input_cache_hit_tokens,
            ]
            .into_iter()
            .enumerate()
            {
                if let Some(value) = value {
                    totals[index] = Some(
                        totals[index]
                            .unwrap_or(0)
                            .checked_add(value)
                            .ok_or_else(|| anyhow!("agent_logs.usage_overflow"))?,
                    );
                } else {
                    complete[index] = false;
                }
            }
            if let Some(cost) = cost {
                total_cost += cost.parse::<rust_decimal::Decimal>()?;
            } else {
                cost_complete = false;
            }
        }
    }
    for index in 0..4 {
        if !complete[index] {
            totals[index] = None;
        }
    }
    let last = events
        .last()
        .ok_or_else(|| anyhow!("agent_logs.empty_record"))?;
    let status = if ended || final_output.is_some() {
        "succeeded"
    } else {
        "running"
    };
    let outcome = if final_output.is_some() {
        "final_answer_observed"
    } else if ended {
        "no_final_answer"
    } else {
        "in_progress"
    };
    let final_text = final_output.as_ref().map(|(s, _)| s);
    sqlx::query("update application_run_log_tasks set user_input=$3,final_output=$4,title=coalesce(left($3,100),title),status=$5,outcome=$6,total_tokens=$7,input_tokens=$8,output_tokens=$9,input_cache_hit_tokens=$10,total_cost=$11::numeric,cost_breakdown=jsonb_build_object('total_cost',$11::text),finished_at=case when $5='succeeded' then $12::timestamptz end,updated_at=now(),parent_source_task_id=$13,parent_task_run_id=(select p.id from application_run_log_tasks p where p.application_id=$1 and p.source_id=application_run_log_tasks.source_id and p.source_session_id=application_run_log_tasks.source_session_id and p.source_task_id=$13 and p.id<>$2) where application_id=$1 and id=$2")
        .bind(application_id).bind(record_id).bind(user).bind(final_text).bind(status).bind(outcome).bind(totals[0]).bind(totals[1]).bind(totals[2]).bind(totals[3]).bind(if cost_complete {Some(total_cost.to_string())}else{None}).bind(&last.0.occurred_at).bind(parent).execute(&mut **tx).await?;
    sqlx::query("delete from application_run_conversation_message_items where application_id=$1 and record_id=$2").bind(application_id).bind(record_id).execute(&mut **tx).await?;
    for m in messages {
        sqlx::query("insert into application_run_conversation_message_items(id,scope_id,application_id,record_id,display_sequence,source_kind,role,content,can_open_detail,is_current,status,started_at) select $1,scope_id,application_id,id,$3,'current_run',$4,$5,false,true,status,started_at from application_run_log_tasks where id=$2")
            .bind(Uuid::now_v7()).bind(record_id).bind(m.sequence).bind(m.role).bind(m.content).execute(&mut **tx).await?;
    }
    Ok(())
}

pub(super) async fn overview(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    record_id: Uuid,
) -> Result<Option<ApplicationLogRecordOverview>> {
    let row=sqlx::query("select id,source_kind,source_client,source_session_id,source_task_id,case when source_kind='native' then id else native_run_id end as native_run_id,title,outcome,total_tokens,input_tokens,output_tokens,input_cache_hit_tokens,total_cost::text as total_cost,member_run_ids from application_run_log_tasks where application_id=$1 and id=$2")
        .bind(application_id).bind(record_id).fetch_optional(store.pool()).await?;
    let Some(row) = row else { return Ok(None) };
    let native: Option<Uuid> = row.get("native_run_id");
    let messages=if native.is_some() {
        sqlx::query("select role,coalesce(content,answer,query,'') as content,display_sequence from application_run_conversation_message_items where application_id=$1 and flow_run_id=any($2) and (role in ('system','user') or role='assistant' and native_message#>>'{_source_item,phase}'='final_answer') order by display_sequence")
            .bind(application_id).bind(row.get::<Vec<Uuid>,_>("member_run_ids")).fetch_all(store.pool()).await?
    } else {
        sqlx::query("select role,content,display_sequence from application_run_conversation_message_items where application_id=$1 and record_id=$2 order by display_sequence")
            .bind(application_id).bind(record_id).fetch_all(store.pool()).await?
    }.into_iter().map(|r|AgentLogMessage{role:r.get("role"),content:r.get("content"),sequence:r.get("display_sequence")}).collect();
    Ok(Some(ApplicationLogRecordOverview {
        record_id,
        source_kind: row.get("source_kind"),
        source_client: row.get("source_client"),
        source_session_id: row.get("source_session_id"),
        source_task_id: row.get("source_task_id"),
        native_run_id: native,
        title: row.get("title"),
        outcome: row.get("outcome"),
        messages,
        total_tokens: row.get("total_tokens"),
        input_tokens: row.get("input_tokens"),
        output_tokens: row.get("output_tokens"),
        input_cache_hit_tokens: row.get("input_cache_hit_tokens"),
        cost_breakdown: ApplicationLogCostBreakdown {
            total_cost: row.get("total_cost"),
        },
        available_views: vec![
            ApplicationLogView::Conversation,
            ApplicationLogView::ClientTrajectory,
        ],
    }))
}

pub(super) async fn page(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    record_id: Uuid,
    cursor: Option<i64>,
    limit: i64,
) -> Result<ClientTrajectoryPage> {
    let native = record_native_run(store, application_id, record_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("log_record"))?;
    if let Some(run) = native {
        return store
            .read_client_trajectory_page(run, None, cursor, limit)
            .await;
    }
    let limit = limit.clamp(1, 100);
    let rows=sqlx::query("select runtime_original_json(s.metadata,s.raw_json_payloads,'metadata') as body from client_trajectory_steps s join application_run_log_tasks t on t.id=s.record_id where t.application_id=$1 and t.id=$2 and ($3::bigint is null or s.event_sequence>$3) order by s.event_sequence,s.id limit $4")
        .bind(application_id).bind(record_id).bind(cursor).bind(limit+1).fetch_all(store.pool()).await?;
    let mut items = rows
        .into_iter()
        .map(|r| serde_json::from_value::<ClientTrajectoryStep>(r.get("body")).map_err(Into::into))
        .collect::<Result<Vec<_>>>()?;
    let more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next_cursor = if more {
        items.last().map(|i| i.sequence)
    } else {
        None
    };
    Ok(ClientTrajectoryPage {
        items,
        next_cursor,
        integrity: "complete".into(),
    })
}

pub(super) async fn section(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    record_id: Uuid,
    step_id: Uuid,
    section: &str,
    cursor: Option<i64>,
    limit: i64,
) -> Result<Option<ClientTrajectorySection>> {
    let Some(native) = record_native_run(store, application_id, record_id).await? else {
        return Ok(None);
    };
    if let Some(run) = native {
        return store
            .read_client_trajectory_section(run, None, step_id, section, cursor, limit)
            .await;
    }
    let row=sqlx::query("select s.request_id,s.event_sequence,s.content_path,runtime_original_json(c.content,c.raw_json_payloads,'content') as body from client_trajectory_sections s join application_run_log_tasks t on t.id=s.record_id join runtime_canonical_contents c on c.id=s.content_id and c.application_id=t.application_id and c.scope_id=t.scope_id where t.application_id=$1 and t.id=$2 and s.step_id=$3 and s.section=$4 and ($5::bigint is null or s.event_sequence>$5) order by s.event_sequence limit 1")
        .bind(application_id).bind(record_id).bind(step_id).bind(section).bind(cursor).fetch_optional(store.pool()).await?;
    let Some(row) = row else { return Ok(None) };
    let mut value: Value = row.get("body");
    for key in row.get::<Vec<String>, _>("content_path") {
        value = value
            .get(&key)
            .cloned()
            .ok_or_else(|| anyhow!("log_record.content_locator"))?;
    }
    Ok(Some(ClientTrajectorySection {
        step_id,
        request_id: row.get("request_id"),
        evidence_scope: "client".into(),
        section: section.into(),
        items: vec![ClientTrajectorySectionItem {
            sequence: row.get("event_sequence"),
            value,
        }],
        next_cursor: None,
    }))
}

async fn record_native_run(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    record_id: Uuid,
) -> Result<Option<Option<Uuid>>> {
    sqlx::query_scalar("select case when source_kind='native' then id else native_run_id end from application_run_log_tasks where application_id=$1 and id=$2")
        .bind(application_id).bind(record_id).fetch_optional(store.pool()).await.map_err(Into::into)
}
