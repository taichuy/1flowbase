use super::*;
pub(in crate::orchestration_runtime_repository) async fn overview(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    record_id: Uuid,
) -> Result<Option<ApplicationLogRecordOverview>> {
    let row=sqlx::query("select id,status,source_id,source_kind,source_client,source_session_id,source_task_id,case when source_kind='native' then id else native_run_id end as native_run_id,title,outcome,total_tokens,input_tokens,output_tokens,input_cache_hit_tokens,total_cost::text as total_cost,member_run_ids from application_run_log_tasks where application_id=$1 and id=$2")
        .bind(application_id).bind(record_id).fetch_optional(store.pool()).await?;
    let Some(row) = row else { return Ok(None) };
    let native: Option<Uuid> = row.get("native_run_id");
    let (messages, output_state, projection_output) = if let Some(run) = native {
        // This is the same business-turn reader used by the chat. Its task CTE
        // selects final_output / projection_output and never provider fragments.
        let page = store
            .list_application_run_conversation_message_items_page(
                application_id,
                run,
                ListApplicationRunConversationMessageItemsPageInput {
                    before_sequence: None,
                    after_sequence: None,
                    limit: 1,
                },
            )
            .await?;
        let mut messages: Vec<_> = page
            .contexts
            .into_iter()
            .map(|c| AgentLogMessage {
                role: c.role,
                content: c.content,
                sequence: c.display_sequence,
            })
            .collect();
        let mut projection_output = None;
        for item in page.items {
            if let Some(content) = item.query {
                messages.push(AgentLogMessage {
                    role: "user".into(),
                    content,
                    sequence: item.display_sequence,
                });
            }
            if let Some(content) = item.answer {
                if item.output_source == "persisted_answer" {
                    messages.push(AgentLogMessage {
                        role: "assistant".into(),
                        content,
                        sequence: item.display_sequence,
                    });
                } else {
                    projection_output = Some(content);
                }
            }
        }
        (messages, page.output_state, projection_output)
    } else {
        let rows = sqlx::query("select role,content,raw_json_payloads->>'content' as content_original,display_sequence from application_run_conversation_message_items where application_id=$1 and record_id=$2 and role in ('system','user','assistant') order by display_sequence,id")
            .bind(application_id).bind(record_id).fetch_all(store.pool()).await?;
        let messages = rows
            .into_iter()
            .map(|r| {
                Ok(AgentLogMessage {
                    role: r.get("role"),
                    content: super::super::json_storage::original_required_text(&r, "content")?,
                    sequence: r.get("display_sequence"),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        (messages, None, None)
    };
    Ok(Some(ApplicationLogRecordOverview {
        record_id,
        status: row.get("status"),
        output_state,
        projection_output,
        source_id: row.get("source_id"),
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

pub(in crate::orchestration_runtime_repository) async fn page(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    record_id: Uuid,
    cursor: Option<String>,
    limit: i64,
) -> Result<RecordClientTrajectoryPage> {
    let native = record_native_run(store, application_id, record_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("log_record"))?;
    if let Some(run) = native {
        let cursor = cursor
            .map(|c| c.parse::<i64>())
            .transpose()
            .map_err(|_| ControlPlaneError::InvalidInput("record_trajectory_cursor"))?;
        let page = store
            .read_client_trajectory_page(run, None, cursor, limit)
            .await?;
        return Ok(RecordClientTrajectoryPage {
            items: page.items,
            next_cursor: page.next_cursor.map(RecordClientTrajectoryCursor::Native),
            integrity: page.integrity,
        });
    }
    let position = cursor
        .as_deref()
        .map(RecordClientTrajectoryCursor::imported_position)
        .transpose()
        .map_err(|_| ControlPlaneError::InvalidInput("record_trajectory_cursor"))?;
    let sequence = position.map(|p| p.0);
    let step_id = position.map(|p| p.1);
    let limit = limit.clamp(1, 100);
    let rows=sqlx::query("select runtime_original_json(s.metadata,s.raw_json_payloads,'metadata') as body from client_trajectory_steps s join application_run_log_tasks t on t.id=s.record_id where t.application_id=$1 and t.id=$2 and ($3::bigint is null or (s.event_sequence,s.id)>($3,$4::uuid)) order by s.event_sequence,s.id limit $5")
        .bind(application_id).bind(record_id).bind(sequence).bind(step_id).bind(limit+1).fetch_all(store.pool()).await?;
    let mut items = rows
        .into_iter()
        .map(|r| serde_json::from_value::<ClientTrajectoryStep>(r.get("body")).map_err(Into::into))
        .collect::<Result<Vec<_>>>()?;
    let more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next_cursor = if more {
        items
            .last()
            .map(|s| RecordClientTrajectoryCursor::imported(s.sequence, s.id))
    } else {
        None
    };
    Ok(RecordClientTrajectoryPage {
        items,
        next_cursor,
        integrity: "complete".into(),
    })
}

pub(in crate::orchestration_runtime_repository) async fn section(
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
    let limit = limit.clamp(1, 32);
    let mut rows=sqlx::query("select s.request_id,s.event_sequence,s.content_path,runtime_original_json(c.content,c.raw_json_payloads,'content') as body from client_trajectory_sections s join application_run_log_tasks t on t.id=s.record_id join runtime_canonical_contents c on c.id=s.content_id and c.application_id=t.application_id and c.scope_id=t.scope_id where t.application_id=$1 and t.id=$2 and s.step_id=$3 and s.section=$4 and ($5::bigint is null or s.event_sequence>$5) order by s.event_sequence limit $6")
        .bind(application_id).bind(record_id).bind(step_id).bind(section).bind(cursor).bind(limit+1).fetch_all(store.pool()).await?;
    if rows.is_empty() {
        return Ok(None);
    }
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let request_id = rows[0].get("request_id");
    let items = rows
        .into_iter()
        .map(|row| {
            let mut value: Value = row.get("body");
            for key in row.get::<Vec<String>, _>("content_path") {
                value = value
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| anyhow!("log_record.content_locator"))?;
            }
            Ok(ClientTrajectorySectionItem {
                sequence: row.get("event_sequence"),
                value,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let next_cursor = if more {
        items.last().map(|i| i.sequence)
    } else {
        None
    };
    Ok(Some(ClientTrajectorySection {
        step_id,
        request_id,
        evidence_scope: "client".into(),
        section: section.into(),
        items,
        next_cursor,
    }))
}

pub(in crate::orchestration_runtime_repository) async fn record_native_run(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    record_id: Uuid,
) -> Result<Option<Option<Uuid>>> {
    sqlx::query_scalar("select case when source_kind='native' then id else native_run_id end from application_run_log_tasks where application_id=$1 and id=$2")
        .bind(application_id).bind(record_id).fetch_optional(store.pool()).await.map_err(Into::into)
}
