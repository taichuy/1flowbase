//! Projection-only task sources: never hydrate historical flow or node bodies.
use super::*;
use futures_util::TryStreamExt;

impl PgControlPlaneStore {
    /// Later calls of the task anchored by `flow_run`, with their own facts so
    /// the anchor trace can nest them as rounds. Empty for non-anchor runs.
    pub(super) async fn list_task_round_projection_sources_for_flow_run(
        &self,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<Vec<domain::ApplicationRunTaskRoundProjectionSource>> {
        let member_ids: Option<Vec<Uuid>> = sqlx::query_scalar(
            "select member_run_ids from application_run_log_tasks where application_id=$1 and id=$2",
        )
        .bind(flow_run.application_id)
        .bind(flow_run.id)
        .fetch_optional(self.pool())
        .await?;
        let mut rounds = Vec::new();
        for member_id in member_ids.unwrap_or_default().into_iter().skip(1) {
            let Some(source_flow_run) =
                fetch_source_run_metadata(self, flow_run.application_id, member_id).await?
            else {
                continue;
            };
            let call_kind: String = sqlx::query_scalar(
                "select call_kind from application_run_log_summaries where flow_run_id=$1",
            )
            .bind(member_id)
            .fetch_one(self.pool())
            .await?;
            let callback_tasks = list_callback_tasks_for_flow_run(self, member_id).await?;
            rounds.push(domain::ApplicationRunTaskRoundProjectionSource {
                call_kind,
                node_runs: list_source_node_metadata(self, member_id, &callback_tasks).await?,
                callback_tasks,
                native_messages: self
                    .application_run_native_trace_messages(flow_run.application_id, member_id)
                    .await?,
                source_flow_run,
            });
        }
        Ok(rounds)
    }

    /// Tasks whose client declared `flow_run`'s task as their parent.
    pub(super) async fn list_child_task_projection_sources_for_flow_run(
        &self,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<Vec<domain::ApplicationRunChildTaskProjectionSource>> {
        let children: Vec<(Uuid, Option<String>)> = sqlx::query_as(
            "select id, subagent_kind from application_run_log_tasks where application_id=$1 and parent_task_run_id=$2 order by started_at, id",
        )
        .bind(flow_run.application_id)
        .bind(flow_run.id)
        .fetch_all(self.pool())
        .await?;
        let mut traces = Vec::new();
        for (child_id, subagent_kind) in children {
            let Some(source_flow_run) =
                fetch_source_run_metadata(self, flow_run.application_id, child_id).await?
            else {
                continue;
            };
            let callback_tasks = list_callback_tasks_for_flow_run(self, child_id).await?;
            traces.push(domain::ApplicationRunChildTaskProjectionSource {
                subagent_kind,
                node_runs: list_source_node_metadata(self, child_id, &callback_tasks).await?,
                callback_tasks,
                source_flow_run,
            });
        }
        Ok(traces)
    }
}

async fn fetch_source_run_metadata(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    flow_run_id: Uuid,
) -> Result<Option<domain::ApplicationRunSourceRunMetadata>> {
    let row = sqlx::query(
        r#"
        select id, title, status, started_at, finished_at from flow_runs
        where application_id=$1 and id=$2 and (
            import_job_id is null or exists (
                select 1 from run_archive_import_jobs jobs
                where jobs.id=flow_runs.import_job_id and jobs.status='succeeded'
            )
        )
    "#,
    )
    .bind(application_id)
    .bind(flow_run_id)
    .fetch_optional(store.pool())
    .await?;
    row.map(|row| {
        Ok(domain::ApplicationRunSourceRunMetadata {
            id: row.get("id"),
            title: row.get("title"),
            status: crate::mappers::orchestration_runtime_mapper::parse_flow_run_status(
                &row.get::<String, _>("status"),
            )?,
            started_at: row.get("started_at"),
            finished_at: row.get("finished_at"),
        })
    })
    .transpose()
}

async fn list_source_node_metadata(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
    callback_tasks: &[domain::CallbackTaskRecord],
) -> Result<Vec<domain::ApplicationRunSourceNodeMetadata>> {
    // The accepted ASCII keys/markers are unchanged by the lossless JSONB
    // escaping. NUL-containing or literal-backslash markers cannot become either
    // accepted marker. Inspecting raw JSON with PostgreSQL -> would reject NUL
    // even in unrelated fields, so restore route originals selectively in Rust.
    let tool_node_ids = callback_tasks
        .iter()
        .filter(|task| task.callback_kind == "llm_tool_calls")
        .map(|task| task.node_run_id)
        .collect::<Vec<_>>();
    // Read physical sections separately: the full operational view concatenates
    // all raw sidecars and would deTOAST unused historical input/output bodies.
    let mut rows = sqlx::query(r#"
        select n.id, n.node_id, n.node_type, n.node_alias, n.status, n.started_at, n.finished_at,
            runtime_original_json(coalesce(m.payload,'{}'::jsonb), m.raw_json_payloads, 'metrics_payload') as metrics_payload,
            case when n.node_type='answer' then coalesce(
                i.payload #>> '{presentation,materialized_from}' in ('waiting_prefix','canonical_stream_state')
                or d.payload #>> '{answer_presentation,materialized_from}' in ('waiting_prefix','canonical_stream_state'), false)
                else false end as legacy_answer_snapshot,
            case when n.id=any($2) then
                case when jsonb_typeof(d.payload -> 'visible_internal_llm_tool_trace')='array'
                    then d.payload -> 'visible_internal_llm_tool_trace' end
                end as tool_route_traces,
            case when n.id=any($2) then
                case when jsonb_typeof(d.payload -> 'visible_internal_llm_tool_trace')='array'
                    then d.raw_json_payloads ->> 'debug_payload' end
                end as tool_route_original
        from node_runs n
        left join node_run_details m on m.node_run_id=n.id and m.section='metrics_payload'
        left join node_run_details i on i.node_run_id=n.id and i.section='input_payload' and n.node_type='answer'
        left join node_run_details d on d.node_run_id=n.id and d.section='debug_payload'
            and (n.node_type='answer' or n.id=any($2))
        where n.flow_run_id=$1 order by n.started_at asc, n.id asc
    "#).bind(flow_run_id).bind(&tool_node_ids).fetch(store.pool());
    let mut nodes = Vec::new();
    // Consume one row at a time: a lossless historical debug body is never held
    // for the whole task, and serde skips every unneeded field without Values.
    while let Some(row) = rows.try_next().await? {
        let tool_route_traces = match row.get::<Option<&str>, _>("tool_route_original") {
            Some(raw) => {
                serde_json::from_str::<SourceDebugToolRoutes>(raw)?.visible_internal_llm_tool_trace
            }
            None => row
                .get::<Option<serde_json::Value>, _>("tool_route_traces")
                .and_then(|value| value.as_array().cloned())
                .unwrap_or_default(),
        };
        nodes.push(domain::ApplicationRunSourceNodeMetadata {
            id: row.get("id"),
            node_id: row.get("node_id"),
            node_type: row.get("node_type"),
            node_alias: row.get("node_alias"),
            status: crate::mappers::orchestration_runtime_mapper::parse_node_run_status(
                &row.get::<String, _>("status"),
            )?,
            metrics_payload: row.get("metrics_payload"),
            legacy_answer_snapshot: row.get("legacy_answer_snapshot"),
            tool_route_traces,
            started_at: row.get("started_at"),
            finished_at: row.get("finished_at"),
        });
    }
    Ok(nodes)
}

#[derive(serde::Deserialize)]
struct SourceDebugToolRoutes {
    visible_internal_llm_tool_trace: Vec<serde_json::Value>,
}
