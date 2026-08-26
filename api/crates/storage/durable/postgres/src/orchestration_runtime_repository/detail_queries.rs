use anyhow::{anyhow, Result};
use sqlx::Row;
use uuid::Uuid;

use control_plane_contracts::ports::{
    ApplicationRunResumeCallbackSummary, ApplicationRunResumeEventSummary,
};

use crate::repositories::PgControlPlaneStore;

use super::record_mappers::{
    map_callback_task_record, map_checkpoint_record, map_flow_run_record, map_node_run_record,
    map_run_event_record, map_runtime_event_record,
};

pub(super) async fn fetch_flow_run_for_application(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    flow_run_id: Uuid,
) -> Result<Option<domain::FlowRunRecord>> {
    let row = sqlx::query(
        r#"
        select
            id,
            application_id,
            flow_id,
            flow_draft_id,
            compiled_plan_id,
            debug_session_id,
            flow_schema_version,
            document_hash,
            run_mode,
            target_node_id,
            title,
            status,
            input_payload,
            output_payload,
            error_payload,
            created_by,
            (
                select users.account
                from users
                where users.id = flow_runs.created_by
            ) as authorized_account,
            api_key_id,
            publication_version_id,
            external_user,
            external_conversation_id,
            external_trace_id,
            compatibility_mode,
            idempotency_key,
            started_at,
            finished_at,
            created_at,
            updated_at
        from flow_runs
        where application_id = $1
          and id = $2
          and (
              flow_runs.import_job_id is null
              or exists (
                  select 1
                  from run_archive_import_jobs import_jobs
                  where import_jobs.id = flow_runs.import_job_id
                    and import_jobs.status = 'succeeded'
              )
          )
        "#,
    )
    .bind(application_id)
    .bind(flow_run_id)
    .fetch_optional(store.pool())
    .await?;

    row.map(map_flow_run_record).transpose()
}

pub(super) async fn fetch_node_run(
    store: &PgControlPlaneStore,
    node_run_id: Uuid,
) -> Result<Option<domain::NodeRunRecord>> {
    let row = sqlx::query(
        r#"
        select
            id,
            flow_run_id,
            node_id,
            node_type,
            node_alias,
            status,
            input_payload,
            output_payload,
            error_payload,
            metrics_payload,
            debug_payload,
            started_at,
            finished_at
        from node_runs
        where id = $1
        "#,
    )
    .bind(node_run_id)
    .fetch_optional(store.pool())
    .await?;

    row.map(map_node_run_record).transpose()
}

pub(super) async fn list_node_runs_for_flow_run(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<Vec<domain::NodeRunRecord>> {
    let rows = sqlx::query(
        r#"
        select
            id,
            flow_run_id,
            node_id,
            node_type,
            node_alias,
            status,
            input_payload,
            output_payload,
            error_payload,
            metrics_payload,
            debug_payload,
            started_at,
            finished_at
        from node_runs
        where flow_run_id = $1
        order by started_at asc, id asc
        "#,
    )
    .bind(flow_run_id)
    .fetch_all(store.pool())
    .await?;

    rows.into_iter().map(map_node_run_record).collect()
}

pub(super) async fn list_checkpoints_for_flow_run(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<Vec<domain::CheckpointRecord>> {
    let rows = sqlx::query(
        r#"
        select
            checkpoints.id, checkpoints.flow_run_id, checkpoints.node_run_id,
            checkpoints.status, checkpoints.reason, checkpoints.locator_payload,
            coalesce(contents.content, checkpoints.variable_snapshot) as variable_snapshot,
            checkpoints.external_ref_payload, checkpoints.created_at
        from flow_run_checkpoints checkpoints
        left join runtime_legacy_shadow_rows shadow_rows
          on shadow_rows.source_table = 'flow_run_checkpoints'
         and shadow_rows.source_column = 'variable_snapshot'
         and shadow_rows.source_row_id = checkpoints.id
        left join runtime_canonical_contents contents
          on contents.id = shadow_rows.canonical_content_id
         and contents.content = checkpoints.variable_snapshot
        where checkpoints.flow_run_id = $1
        order by checkpoints.created_at asc, checkpoints.id asc
        "#,
    )
    .bind(flow_run_id)
    .fetch_all(store.pool())
    .await?;

    Ok(rows.into_iter().map(map_checkpoint_record).collect())
}

pub(super) async fn list_checkpoints_for_node_run(
    store: &PgControlPlaneStore,
    node_run_id: Uuid,
) -> Result<Vec<domain::CheckpointRecord>> {
    let rows = sqlx::query(
        r#"
        select
            checkpoints.id, checkpoints.flow_run_id, checkpoints.node_run_id,
            checkpoints.status, checkpoints.reason, checkpoints.locator_payload,
            coalesce(contents.content, checkpoints.variable_snapshot) as variable_snapshot,
            checkpoints.external_ref_payload, checkpoints.created_at
        from flow_run_checkpoints checkpoints
        left join runtime_legacy_shadow_rows shadow_rows
          on shadow_rows.source_table = 'flow_run_checkpoints'
         and shadow_rows.source_column = 'variable_snapshot'
         and shadow_rows.source_row_id = checkpoints.id
        left join runtime_canonical_contents contents
          on contents.id = shadow_rows.canonical_content_id
         and contents.content = checkpoints.variable_snapshot
        where checkpoints.node_run_id = $1
        order by checkpoints.created_at asc, checkpoints.id asc
        "#,
    )
    .bind(node_run_id)
    .fetch_all(store.pool())
    .await?;

    Ok(rows.into_iter().map(map_checkpoint_record).collect())
}

pub(super) async fn list_events_for_flow_run(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<Vec<domain::RunEventRecord>> {
    let rows = sqlx::query(
        r#"
        select
            id,
            flow_run_id,
            node_run_id,
            sequence,
            event_type,
            payload,
            created_at
        from flow_run_events
        where flow_run_id = $1
        order by sequence asc, id asc
        "#,
    )
    .bind(flow_run_id)
    .fetch_all(store.pool())
    .await?;

    Ok(rows.into_iter().map(map_run_event_record).collect())
}

pub(super) async fn list_callback_tasks_for_flow_run(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<Vec<domain::CallbackTaskRecord>> {
    let rows = sqlx::query(
        r#"
        select
            id,
            flow_run_id,
            node_run_id,
            callback_kind,
            status,
            request_payload,
            response_payload,
            external_ref_payload,
            created_at,
            completed_at
        from flow_run_callback_tasks
        where flow_run_id = $1
        order by created_at asc, id asc
        "#,
    )
    .bind(flow_run_id)
    .fetch_all(store.pool())
    .await?;

    rows.into_iter().map(map_callback_task_record).collect()
}

pub(super) async fn overview_tool_callback_count(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<i64> {
    sqlx::query_scalar(
        r#"
        select coalesce(summaries.tool_callback_count, 0)::bigint
        from flow_runs runs
        left join application_run_log_summaries summaries
          on summaries.flow_run_id = runs.id
        where runs.id = $1
        "#,
    )
    .bind(flow_run_id)
    .fetch_one(store.pool())
    .await
    .map_err(Into::into)
}

pub(super) async fn latest_overview_waiting_node(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<(Option<String>, Option<Uuid>)> {
    let checkpoint = sqlx::query(
        r#"
        select locator_payload ->> 'node_id' as node_id, node_run_id
        from flow_run_checkpoints
        where flow_run_id = $1
          and status like 'waiting%'
        order by created_at desc, id desc
        limit 1
        "#,
    )
    .bind(flow_run_id)
    .fetch_optional(store.pool())
    .await?;
    if let Some(checkpoint) = checkpoint {
        return Ok((checkpoint.get("node_id"), checkpoint.get("node_run_id")));
    }

    let callback = sqlx::query(
        r#"
        select node_runs.node_id, tasks.node_run_id
        from flow_run_callback_tasks tasks
        left join node_runs on node_runs.id = tasks.node_run_id
        where tasks.flow_run_id = $1
          and tasks.status = 'pending'
        order by tasks.created_at desc, tasks.id desc
        limit 1
        "#,
    )
    .bind(flow_run_id)
    .fetch_optional(store.pool())
    .await?;

    Ok(callback
        .map(|row| (row.get("node_id"), row.get("node_run_id")))
        .unwrap_or((None, None)))
}

pub(super) async fn list_resume_timeline_events_for_flow_run(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<Vec<domain::RunEventRecord>> {
    let rows = sqlx::query(
        r#"
        select id, flow_run_id, node_run_id, sequence, event_type, payload, created_at
        from flow_run_events
        where flow_run_id = $1
          and event_type in (
              'public_run_resume_requested',
              'public_run_resume_succeeded',
              'public_run_resume_failed',
              'public_run_resume_cancelled',
              'flow_run_resumed'
          )
        order by sequence asc, id asc
        "#,
    )
    .bind(flow_run_id)
    .fetch_all(store.pool())
    .await?;

    Ok(rows.into_iter().map(map_run_event_record).collect())
}

pub(super) async fn list_resume_timeline_callback_summaries(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<Vec<ApplicationRunResumeCallbackSummary>> {
    let rows = sqlx::query(
        r#"
        select id, callback_kind, status, created_at, completed_at
        from flow_run_callback_tasks
        where flow_run_id = $1
        order by created_at asc, id asc
        "#,
    )
    .bind(flow_run_id)
    .fetch_all(store.pool())
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(ApplicationRunResumeCallbackSummary {
                id: row.get("id"),
                callback_kind: row.get("callback_kind"),
                status: crate::mappers::orchestration_runtime_mapper::parse_callback_task_status(
                    row.get::<String, _>("status").as_str(),
                )?,
                created_at: row.get("created_at"),
                completed_at: row.get("completed_at"),
            })
        })
        .collect()
}

pub(super) async fn list_resume_timeline_event_summaries(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<Vec<ApplicationRunResumeEventSummary>> {
    let rows = sqlx::query(
        r#"
        with projected_events as (
            select events.id, events.event_type,
                   events.resume_timeline_description as description,
                   events.created_at, events.sequence
            from flow_run_events events
            where events.flow_run_id = $1
              and events.resume_timeline_description_projected
              and events.event_type in (
                  'public_run_resume_requested',
                  'public_run_resume_succeeded',
                  'public_run_resume_failed',
                  'public_run_resume_cancelled',
                  'flow_run_resumed'
              )
        ),
        legacy_events as (
            select events.id, events.event_type,
                   case
                       when jsonb_typeof(events.payload -> 'resume_request_id') = 'string'
                            and btrim(events.payload ->> 'resume_request_id') <> ''
                           then events.payload ->> 'resume_request_id'
                       when jsonb_typeof(events.payload -> 'callback_task_id') = 'string'
                            and btrim(events.payload ->> 'callback_task_id') <> ''
                           then events.payload ->> 'callback_task_id'
                       else null
                   end as description,
                   events.created_at, events.sequence
            from flow_run_events events
            where events.flow_run_id = $1
              and not events.resume_timeline_description_projected
              and events.event_type in (
                  'public_run_resume_requested',
                  'public_run_resume_succeeded',
                  'public_run_resume_failed',
                  'public_run_resume_cancelled',
                  'flow_run_resumed'
              )
        )
        select * from projected_events
        union all
        select * from legacy_events
        order by sequence asc, id asc
        "#,
    )
    .bind(flow_run_id)
    .fetch_all(store.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ApplicationRunResumeEventSummary {
            id: row.get("id"),
            event_type: row.get("event_type"),
            description: row.get("description"),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub(super) async fn list_trace_checkpoints_for_node_runs(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    flow_run_id: Uuid,
    node_run_ids: Vec<Uuid>,
) -> Result<Vec<domain::CheckpointRecord>> {
    let rows = sqlx::query(
        r#"
        select checkpoints.id, checkpoints.flow_run_id, checkpoints.node_run_id,
               checkpoints.status, checkpoints.reason, checkpoints.locator_payload,
               coalesce(contents.content, checkpoints.variable_snapshot) as variable_snapshot,
               checkpoints.external_ref_payload,
               checkpoints.created_at
        from flow_run_checkpoints checkpoints
        join flow_runs runs on runs.id = checkpoints.flow_run_id
        left join runtime_legacy_shadow_rows shadow_rows
          on shadow_rows.source_table = 'flow_run_checkpoints'
         and shadow_rows.source_column = 'variable_snapshot'
         and shadow_rows.source_row_id = checkpoints.id
        left join runtime_canonical_contents contents
          on contents.id = shadow_rows.canonical_content_id
         and contents.content = checkpoints.variable_snapshot
        where runs.application_id = $1
          and checkpoints.flow_run_id = $2
          and checkpoints.node_run_id = any($3)
        order by checkpoints.created_at asc, checkpoints.id asc
        "#,
    )
    .bind(application_id)
    .bind(flow_run_id)
    .bind(node_run_ids)
    .fetch_all(store.pool())
    .await?;

    Ok(rows.into_iter().map(map_checkpoint_record).collect())
}

pub(super) async fn list_trace_events_for_node_runs(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    flow_run_id: Uuid,
    node_run_ids: Vec<Uuid>,
) -> Result<Vec<domain::RunEventRecord>> {
    let rows = sqlx::query(
        r#"
        select events.id, events.flow_run_id, events.node_run_id, events.sequence,
               events.event_type, events.payload, events.created_at
        from flow_run_events events
        join flow_runs runs on runs.id = events.flow_run_id
        where runs.application_id = $1
          and events.flow_run_id = $2
          and events.node_run_id = any($3)
        order by events.sequence asc, events.id asc
        "#,
    )
    .bind(application_id)
    .bind(flow_run_id)
    .bind(node_run_ids)
    .fetch_all(store.pool())
    .await?;

    Ok(rows.into_iter().map(map_run_event_record).collect())
}

pub(super) async fn list_stitched_trace_source_runs_for_flow_run(
    store: &PgControlPlaneStore,
    current_run: &domain::FlowRunRecord,
) -> Result<Vec<domain::FlowRunRecord>> {
    let Some(external_conversation_id) = current_run
        .external_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(Vec::new());
    };
    let Some(external_user) = current_run
        .external_user
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query(
        r#"
        select
            prior.id,
            prior.application_id,
            prior.flow_id,
            prior.flow_draft_id,
            prior.compiled_plan_id,
            prior.debug_session_id,
            prior.flow_schema_version,
            prior.document_hash,
            prior.run_mode,
            prior.target_node_id,
            prior.title,
            prior.status,
            prior.input_payload,
            prior.output_payload,
            prior.error_payload,
            prior.created_by,
            (
                select users.account
                from users
                where users.id = prior.created_by
            ) as authorized_account,
            prior.api_key_id,
            prior.publication_version_id,
            prior.external_user,
            prior.external_conversation_id,
            prior.external_trace_id,
            prior.compatibility_mode,
            prior.idempotency_key,
            prior.started_at,
            prior.finished_at,
            prior.created_at,
            prior.updated_at
        from flow_runs prior
        where prior.application_id = $1
          and prior.external_conversation_id = $2
          and prior.id <> $3
          and prior.started_at < $4
          and prior.external_user = $5
          and prior.api_key_id is not distinct from $6
          and prior.compatibility_mode is not distinct from $7
          and prior.status in ('cancelled', 'waiting_callback')
          and (
              prior.import_job_id is null
              or exists (
                  select 1
                  from run_archive_import_jobs prior_import_jobs
                  where prior_import_jobs.id = prior.import_job_id
                    and prior_import_jobs.status = 'succeeded'
              )
          )
          and not exists (
              select 1
              from flow_runs boundary
              where boundary.application_id = prior.application_id
                and boundary.external_conversation_id = prior.external_conversation_id
                and boundary.external_user = prior.external_user
                and boundary.api_key_id is not distinct from prior.api_key_id
                and boundary.compatibility_mode is not distinct from prior.compatibility_mode
                and boundary.id <> $3
                and boundary.started_at > prior.started_at
                and boundary.started_at < $4
                and boundary.status in ('succeeded', 'failed')
                and (
                    boundary.import_job_id is null
                    or exists (
                        select 1
                        from run_archive_import_jobs boundary_import_jobs
                        where boundary_import_jobs.id = boundary.import_job_id
                          and boundary_import_jobs.status = 'succeeded'
                    )
                )
          )
        order by prior.started_at asc, prior.id asc
        limit 12
        "#,
    )
    .bind(current_run.application_id)
    .bind(external_conversation_id)
    .bind(current_run.id)
    .bind(current_run.started_at)
    .bind(external_user)
    .bind(current_run.api_key_id)
    .bind(current_run.compatibility_mode.as_deref())
    .fetch_all(store.pool())
    .await?;

    rows.into_iter().map(map_flow_run_record).collect()
}

pub(super) async fn list_runtime_events_for_flow_run(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
) -> Result<Vec<domain::RuntimeEventRecord>> {
    let rows = sqlx::query(
        r#"
        select
            id,
            flow_run_id,
            node_run_id,
            span_id,
            parent_span_id,
            sequence,
            event_type,
            layer,
            source,
            trust_level,
            item_id,
            ledger_ref,
            payload,
            visibility,
            durability,
            created_at
        from runtime_events
        where flow_run_id = $1
        order by sequence asc, id asc
        "#,
    )
    .bind(flow_run_id)
    .fetch_all(store.pool())
    .await?;

    rows.into_iter().map(map_runtime_event_record).collect()
}

pub(super) async fn list_stitched_trace_for_flow_run(
    store: &PgControlPlaneStore,
    flow_run: &domain::FlowRunRecord,
) -> Result<Vec<domain::ApplicationRunStitchedTrace>> {
    let source_runs = list_stitched_trace_source_runs_for_flow_run(store, flow_run).await?;
    let mut stitched_trace = Vec::with_capacity(source_runs.len());

    for source_flow_run in source_runs {
        stitched_trace.push(domain::ApplicationRunStitchedTrace {
            node_runs: list_node_runs_for_flow_run(store, source_flow_run.id).await?,
            callback_tasks: list_callback_tasks_for_flow_run(store, source_flow_run.id).await?,
            events: list_events_for_flow_run(store, source_flow_run.id).await?,
            runtime_events: list_runtime_events_for_flow_run(store, source_flow_run.id).await?,
            source_flow_run,
        });
    }

    Ok(stitched_trace)
}

pub(super) async fn list_subagent_traces_for_flow_run(
    store: &PgControlPlaneStore,
    current_run: &domain::FlowRunRecord,
    callback_tasks: &[domain::CallbackTaskRecord],
) -> Result<Vec<domain::ApplicationRunSubagentTrace>> {
    if current_run.compatibility_mode.as_deref() != Some("anthropic-messages-v1") {
        return Ok(Vec::new());
    }
    let Some(external_conversation_id) = current_run
        .external_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(Vec::new());
    };

    let mut subagent_traces = Vec::new();
    for task in callback_tasks
        .iter()
        .filter(|task| task.callback_kind == "llm_tool_calls")
    {
        for tool_call in agent_tool_calls_from_callback_task(task) {
            let Some(tool_call_id) = tool_call
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
            else {
                continue;
            };
            let Some(prompt) = tool_call
                .get("arguments")
                .and_then(|arguments| arguments.get("prompt"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };

            let candidates = find_subagent_flow_run_candidates(
                store,
                current_run,
                external_conversation_id,
                task,
                prompt,
            )
            .await?;
            if candidates.len() != 1 {
                continue;
            }
            let source_flow_run = candidates
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("subagent candidate count should be checked"))?;
            let source_flow_run_id = source_flow_run.id;
            subagent_traces.push(domain::ApplicationRunSubagentTrace {
                parent_tool_call_id: tool_call_id,
                parent_callback_task_id: task.id,
                node_runs: list_node_runs_for_flow_run(store, source_flow_run_id).await?,
                callback_tasks: list_callback_tasks_for_flow_run(store, source_flow_run_id).await?,
                events: list_events_for_flow_run(store, source_flow_run_id).await?,
                runtime_events: list_runtime_events_for_flow_run(store, source_flow_run_id).await?,
                source_flow_run,
            });
        }
    }

    Ok(subagent_traces)
}

fn agent_tool_calls_from_callback_task(
    task: &domain::CallbackTaskRecord,
) -> Vec<serde_json::Value> {
    task.request_payload
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|tool_call| {
            tool_call.get("name").and_then(serde_json::Value::as_str) == Some("Agent")
        })
        .cloned()
        .collect()
}

async fn find_subagent_flow_run_candidates(
    store: &PgControlPlaneStore,
    current_run: &domain::FlowRunRecord,
    external_conversation_id: &str,
    task: &domain::CallbackTaskRecord,
    prompt: &str,
) -> Result<Vec<domain::FlowRunRecord>> {
    let rows = sqlx::query(
        r#"
        select
            candidate.id,
            candidate.application_id,
            candidate.flow_id,
            candidate.flow_draft_id,
            candidate.compiled_plan_id,
            candidate.debug_session_id,
            candidate.flow_schema_version,
            candidate.document_hash,
            candidate.run_mode,
            candidate.target_node_id,
            candidate.title,
            candidate.status,
            candidate.input_payload,
            candidate.output_payload,
            candidate.error_payload,
            candidate.created_by,
            (
                select users.account
                from users
                where users.id = candidate.created_by
            ) as authorized_account,
            candidate.api_key_id,
            candidate.publication_version_id,
            candidate.external_user,
            candidate.external_conversation_id,
            candidate.external_trace_id,
            candidate.compatibility_mode,
            candidate.idempotency_key,
            candidate.started_at,
            candidate.finished_at,
            candidate.created_at,
            candidate.updated_at
        from flow_runs candidate
        where candidate.application_id = $1
          and candidate.id <> $2
          and candidate.external_conversation_id = $3
          and candidate.external_user is not distinct from $4
          and candidate.api_key_id is not distinct from $5
          and candidate.compatibility_mode is not distinct from $6
          and candidate.compatibility_mode = 'anthropic-messages-v1'
          and candidate.started_at >= $7
          and ($8::timestamptz is null or candidate.started_at <= $8)
          and coalesce(
              candidate.input_payload #>> '{node-start,query}',
              candidate.input_payload #>> '{start,query}',
              candidate.input_payload #>> '{query}',
              ''
          ) = $9
          and (
              position('cc_is_subagent=true' in coalesce(
                  candidate.input_payload #>> '{node-start,system}',
                  candidate.input_payload #>> '{start,system}',
                  candidate.input_payload #>> '{system}',
                  ''
              )) > 0
              or (
                  position('Agent threads always have their cwd reset between bash calls' in coalesce(
                      candidate.input_payload #>> '{node-start,system}',
                      candidate.input_payload #>> '{start,system}',
                      candidate.input_payload #>> '{system}',
                      ''
                  )) > 0
                  and position('the parent agent reads your text output' in coalesce(
                      candidate.input_payload #>> '{node-start,system}',
                      candidate.input_payload #>> '{start,system}',
                      candidate.input_payload #>> '{system}',
                      ''
                  )) > 0
              )
          )
          and (
              candidate.import_job_id is null
              or exists (
                  select 1
                  from run_archive_import_jobs import_jobs
                  where import_jobs.id = candidate.import_job_id
                    and import_jobs.status = 'succeeded'
              )
          )
        order by candidate.started_at asc, candidate.id asc
        limit 2
        "#,
    )
    .bind(current_run.application_id)
    .bind(current_run.id)
    .bind(external_conversation_id)
    .bind(current_run.external_user.as_deref())
    .bind(current_run.api_key_id)
    .bind(current_run.compatibility_mode.as_deref())
    .bind(task.created_at)
    .bind(task.completed_at)
    .bind(prompt)
    .fetch_all(store.pool())
    .await?;

    rows.into_iter().map(map_flow_run_record).collect()
}

pub(super) async fn list_events_for_node_context(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
    node_run_id: Uuid,
) -> Result<Vec<domain::RunEventRecord>> {
    let rows = sqlx::query(
        r#"
        select
            id,
            flow_run_id,
            node_run_id,
            sequence,
            event_type,
            payload,
            created_at
        from flow_run_events
        where flow_run_id = $1
          and (node_run_id is null or node_run_id = $2)
        order by sequence asc, id asc
        "#,
    )
    .bind(flow_run_id)
    .bind(node_run_id)
    .fetch_all(store.pool())
    .await?;

    Ok(rows.into_iter().map(map_run_event_record).collect())
}
