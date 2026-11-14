//! Native source adapter: complete archives and canonical messages, never UI summaries.
use super::*;
use crate::routes::application_runtime::archive::document::build_run_archive_v1_entry;
use control_plane_contracts::ports::{
    AgentLogEvent, AgentLogEventKind as Kind, AgentLogUsage, AgentLogUsageBasis, AgentLogsBatch,
    AgentLogsExportFacts, AGENT_LOGS_SCHEMA_VERSION,
};

pub(super) async fn read(
    store: storage_durable_postgres::MainDurableStore,
    files: std::sync::Arc<storage_object::FileStorageDriverRegistry>,
    actor: &ActorContext,
    app: &ApplicationRecord,
    selector: &AgentLogsExportSelector,
) -> Result<AgentLogsExportFacts, ApiError> {
    let (field, selected) = match selector {
        AgentLogsExportSelector::Records(ids) => ("id", ids),
        AgentLogsExportSelector::Conversations(ids) => ("log_conversation_id", ids),
    };
    let requested: BTreeSet<_> = selected.iter().copied().collect();
    let mut query = ApplicationLogRecordsQuery {
        filter: Filter::Field {
            field: field.into(),
            operator: Operator::In,
            value: json!(selected),
        },
        sort_field: "started_at".into(),
        descending: false,
        cursor: None,
        limit: 100,
    };
    let mut records = Vec::new();
    loop {
        let page = store
            .query_application_log_records(actor.current_workspace_id, &[app.id], &query)
            .await?;
        records.extend(page.items);
        match page.next_cursor {
            Some(cursor) if Some(&cursor) != query.cursor.as_ref() => query.cursor = Some(cursor),
            Some(_) => return Err(ControlPlaneError::Conflict("agent_logs_cursor_stalled").into()),
            None => break,
        }
    }
    let present: BTreeSet<_> = records
        .iter()
        .filter_map(|record| {
            if field == "id" {
                Some(record.record_id)
            } else {
                record.log_conversation_id
            }
        })
        .collect();
    if present != requested || records.iter().any(|record| record.native_run_id.is_none()) {
        return Err(ControlPlaneError::NotFound("agent_logs_record").into());
    }
    let record_ids = records.iter().map(|record| record.record_id).collect();
    let anchors: Vec<_> = records
        .iter()
        .filter_map(|record| record.native_run_id)
        .collect();
    let runs = store
        .expand_application_run_log_tasks(app.id, &anchors)
        .await?;
    let mut batch = AgentLogsBatch {
        schema_version: AGENT_LOGS_SCHEMA_VERSION.into(),
        source_id: format!("1flowbase:{}:{}", actor.current_workspace_id, app.id),
        source_client: "1flowbase".into(),
        events: Vec::new(),
    };
    let mut owners = std::collections::BTreeMap::new();
    for anchor in &anchors {
        let task = store
            .get_application_run_log_task(app.id, *anchor)
            .await?
            .ok_or(ControlPlaneError::Conflict(
                "agent_logs_native_task_missing",
            ))?;
        for member in &task.member_run_ids {
            owners.insert(*member, task.clone());
        }
    }
    for run_id in runs {
        let run = store
            .get_application_run_detail(app.id, run_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?
            .flow_run;
        // Use the real source time, never a generated export time in a fact.
        let archive = build_run_archive_v1_entry(
            store.clone(),
            files.clone(),
            actor.current_workspace_id,
            app,
            app.id,
            run_id,
            run.started_at,
        )
        .await?;
        let task = owners.get(&run_id).ok_or(ControlPlaneError::Conflict(
            "agent_logs_native_task_membership",
        ))?;
        let session = task
            .log_conversation_id
            .map(|id| id.to_string())
            .or_else(|| run.external_conversation_id.clone())
            .unwrap_or_else(|| task.id.to_string());
        let messages = store
            .read_native_agent_log_messages(actor.current_workspace_id, app.id, run_id)
            .await?;
        let usage = store.list_usage_ledger(run_id).await?;
        let mut archive = serde_json::to_value(archive)?;
        archive["native_source_relationships"] = json!({"task_id":task.id,"log_conversation_id":task.log_conversation_id,"parent_task_run_id":task.parent_task_run_id});
        if task.log_conversation_id.is_none() && run.external_conversation_id.is_none() {
            diagnostic(&mut archive, "conversation_relation_missing");
        }
        let mut events = normalize(&run, &session, archive, messages, usage)?;
        let parent = if run_id != task.id {
            Some(task.id)
        } else {
            task.parent_task_run_id
        };
        for event in &mut events {
            event.parent_source_task_id = parent.map(|id| id.to_string());
            if let Some(parent) = parent {
                event.event_id.push_str(&format!(":parent:{parent}"));
            }
        }
        batch.events.extend(events);
    }
    batch
        .validate()
        .map_err(|_| ControlPlaneError::Conflict("agent_logs_native_facts"))?;
    Ok(AgentLogsExportFacts {
        batches: vec![batch],
        record_ids,
    })
}
fn event(
    run: &domain::FlowRunRecord,
    session: &str,
    id: String,
    kind: Kind,
    time: OffsetDateTime,
) -> Result<AgentLogEvent, ApiError> {
    Ok(AgentLogEvent {
        event_id: format!("native:{session}:{}:{id}", run.id),
        source_session_id: session.into(),
        source_task_id: run.id.to_string(),
        parent_source_task_id: None,
        sequence: 0,
        occurred_at: time.format(&Rfc3339)?,
        kind,
        content: None,
        phase: None,
        name: None,
        call_id: None,
        model_id: None,
        reasoning_effort: None,
        provider_code: None,
        usage: None,
        inherited: false,
        raw: Value::Null,
    })
}
fn text(value: Option<&Value>) -> Option<String> {
    value.filter(|value| !value.is_null()).map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string())
    })
}
fn diagnostic(raw: &mut Value, code: &str) {
    let items = raw
        .as_object_mut()
        .expect("native archive is an object")
        .entry("exchange_diagnostics")
        .or_insert_with(|| json!([]));
    items
        .as_array_mut()
        .expect("exchange diagnostics is an array")
        .push(json!(code));
}
pub(super) fn normalize(
    run: &domain::FlowRunRecord,
    session: &str,
    archive: Value,
    mut messages: Vec<Value>,
    usage: Vec<domain::UsageLedgerRecord>,
) -> Result<Vec<AgentLogEvent>, ApiError> {
    let mut result = Vec::new();
    let mut source = event(run, session, "source".into(), Kind::Context, run.started_at)?;
    source.name = Some("native.source_facts".into());
    source.raw = archive;
    if messages.is_empty() {
        diagnostic(&mut source.raw, "canonical_messages_missing");
    }
    result.push(source);
    // The persisted sequence, including negative context positions, is the
    // canonical message order. Time and UUID/hash order cannot stand in for it.
    let mut ordered = Vec::with_capacity(messages.len());
    for unit in messages.drain(..) {
        let sequence = unit.get("display_sequence").and_then(Value::as_i64).ok_or(
            ControlPlaneError::Conflict("agent_logs_native_message_sequence"),
        )?;
        ordered.push((sequence, unit));
    }
    ordered.sort_by_key(|(sequence, _)| *sequence);
    if ordered.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(
            ControlPlaneError::Conflict("agent_logs_native_message_sequence_conflict").into(),
        );
    }
    for (_, unit) in ordered {
        let id = unit
            .get("id")
            .and_then(Value::as_str)
            .ok_or(ControlPlaneError::Conflict(
                "agent_logs_native_message_identity",
            ))?;
        let message = unit
            .get("native_message")
            .filter(|value| value.is_object())
            .ok_or(ControlPlaneError::Conflict(
                "agent_logs_native_message_missing",
            ))?;
        let original = message.get("_source_item").unwrap_or(message);
        let conflict = message.get("_log_conflicting").and_then(Value::as_bool) == Some(true);
        let kind = if conflict {
            Kind::Context
        } else {
            match original.get("type").and_then(Value::as_str) {
                Some("function_call" | "custom_tool_call") => Kind::ToolCall,
                Some("function_call_output" | "custom_tool_call_output") => Kind::ToolResult,
                _ => match message.get("role").and_then(Value::as_str) {
                    Some("system" | "developer") => Kind::System,
                    Some("user") => Kind::User,
                    Some("assistant") => Kind::Assistant,
                    Some("tool") => Kind::ToolResult,
                    _ => Kind::Context,
                },
            }
        };
        let timestamp = unit
            .get("started_at")
            .and_then(Value::as_str)
            .map(|time| OffsetDateTime::parse(time, &Rfc3339))
            .transpose()?
            .unwrap_or(run.started_at);
        let mut fact = event(run, session, format!("message:{id}"), kind, timestamp)?;
        // The original item carries complete arguments/results/parts. The projection's
        // display content may summarize it and is never the authority for these facts.
        fact.content = text(
            original
                .get("arguments")
                .or_else(|| original.get("input"))
                .or_else(|| original.get("output"))
                .or_else(|| original.get("content")),
        );
        fact.name = original
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_owned);
        fact.call_id = original
            .get("call_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        fact.phase = if conflict {
            Some("source_conflict".into())
        } else {
            original
                .get("phase")
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        fact.raw = unit;
        result.push(fact);
    }
    for ledger in usage {
        let mut fact = event(
            run,
            session,
            format!("usage:{}", ledger.id),
            Kind::Usage,
            ledger.created_at,
        )?;
        fact.model_id = ledger.model_id.clone();
        fact.usage = Some(AgentLogUsage {
            basis: AgentLogUsageBasis::Delta,
            response_id: ledger
                .span_id
                .map(|id| id.to_string())
                .or_else(|| Some(ledger.id.to_string())),
            input_tokens: ledger.input_tokens,
            output_tokens: ledger.output_tokens,
            input_cache_hit_tokens: ledger.input_cache_hit_tokens.or(ledger.cache_read_tokens),
            cache_write_tokens: ledger.cache_write_tokens,
            total_tokens: ledger.total_tokens,
        });
        fact.raw = json!({"native_source_fact":"usage_ledger","id":ledger.id});
        result.push(fact);
    }
    if run.status.is_terminal() {
        if let Some(finished) = run.finished_at {
            let mut fact = event(run, session, "terminal".into(), Kind::TaskEnd, finished)?;
            fact.phase = Some(
                if run.status == domain::FlowRunStatus::Succeeded {
                    "completed"
                } else {
                    run.status.as_str()
                }
                .into(),
            );
            fact.raw = json!({"native_source_fact":"flow_run_fact","id":run.id,"status":run.status.as_str()});
            result.push(fact);
        } else {
            diagnostic(&mut result[0].raw, "terminal_finished_at_missing");
        }
    }
    // Source facts are immutable. A corrected source unit gets a new version;
    // repeated unchanged exports keep byte-identical event and span identities.
    for (ordinal, fact) in result.iter_mut().enumerate() {
        // Dense ordinals have no reserved numeric capacity or timestamp packing.
        // The signed original message sequence remains in its single raw unit.
        fact.sequence = i64::try_from(ordinal)
            .map_err(|_| ControlPlaneError::Conflict("agent_logs_native_sequence_range"))?;
        use sha2::{Digest, Sha256};
        // Hash the complete exported fact, with the stable source locator before
        // appending its version. This includes occurred_at, sequence and provenance.
        let identity = serde_json::to_vec(&*fact)?;
        fact.event_id.push(':');
        fact.event_id.push_str(&format!(
            "{:x}",
            base16ct::HexDisplay(&Sha256::digest(identity))
        ));
    }
    Ok(result)
}
