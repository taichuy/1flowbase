use super::*;
use control_plane_contracts::ports::{
    InvalidWorkflowTrajectoryQuery, TrajectoryOverview, WorkflowEventSection,
    WorkflowTrajectoryBody, WorkflowTrajectoryEvent, WorkflowTrajectoryIndexItem,
    WorkflowTrajectoryNode, WorkflowTrajectoryPage, WorkflowTrajectoryQuery,
};
use time::format_description::well_known::Rfc3339;

fn scope_sql() -> &'static str {
    include_str!("scope.sql")
}
fn event_sql() -> String {
    format!("{}{}", scope_sql(), include_str!("events.sql"))
}
fn source_rank(id: &str) -> i32 {
    match id.split(':').next().unwrap_or_default() {
        "node_started" | "round_started" => 0,
        "workflow" => 1,
        "native" | "runtime" => 2,
        _ => 4,
    }
}
fn timestamp(value: Option<&str>) -> Result<Option<OffsetDateTime>> {
    value
        .map(|v| {
            OffsetDateTime::parse(v, &Rfc3339)
                .map_err(|_| InvalidWorkflowTrajectoryQuery("invalid timestamp".into()).into())
        })
        .transpose()
}
fn formatted(value: Option<OffsetDateTime>) -> Result<Option<String>> {
    value
        .map(|v| v.format(&Rfc3339).map_err(Into::into))
        .transpose()
}

impl PgControlPlaneStore {
    pub(super) async fn read_workflow_trajectory(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        query: WorkflowTrajectoryQuery,
    ) -> Result<WorkflowTrajectoryPage> {
        let from = timestamp(query.from.as_deref())?;
        let to = timestamp(query.to.as_deref())?;
        if from.zip(to).is_some_and(|(a, b)| a > b) {
            return Err(
                InvalidWorkflowTrajectoryQuery("invalid trajectory time range".into()).into(),
            );
        }
        // Opaque wire value encodes the immutable chronological key; no ordinal is invented.
        let previous = query
            .cursor
            .as_deref()
            .is_some_and(|v| v.starts_with("previous:"));
        let mut cursor: Option<(String, Uuid, i32, i64, String)> = query
            .cursor
            .as_deref()
            .map(|v| {
                serde_json::from_str(v.strip_prefix("previous:").unwrap_or(v))
                    .map_err(|_| InvalidWorkflowTrajectoryQuery("invalid trajectory cursor".into()))
            })
            .transpose()?;
        if let Some((_, _, rank, sequence, id)) = &cursor {
            let valid_id = id.split_once(':').is_some_and(|(prefix, id)| {
                matches!(
                    prefix,
                    "native"
                        | "workflow"
                        | "runtime"
                        | "node_started"
                        | "node_finished"
                        | "round_started"
                        | "round_finished"
                ) && Uuid::parse_str(id).is_ok()
            });
            if !valid_id || *rank != source_rank(id) || *sequence < 0 {
                return Err(
                    InvalidWorkflowTrajectoryQuery("invalid trajectory cursor".into()).into(),
                );
            }
        }
        let initial_focus = query.cursor.is_none() && query.focus_event_id.is_some();
        if let Some(id) = query.focus_event_id.as_deref() {
            let sql=format!("{} select event_id,created_at,flow_run_id,source_rank,event_sequence from enriched where event_id=$3 or source_id::text=$3 order by created_at,flow_run_id,source_rank,coalesce(event_sequence,0),event_id limit 1",event_sql());
            // SQL safety: CTEs/links are embedded SQL, paging clauses are closed choices and body tables/resolvers are literal selections.
            let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(application_id)
                .bind(flow_run_id)
                .bind(id)
                .fetch_optional(self.pool())
                .await?
                .ok_or_else(|| {
                    InvalidWorkflowTrajectoryQuery("trajectory focus not found".into())
                })?;
            if initial_focus {
                cursor = Some((
                    row.get::<OffsetDateTime, _>("created_at")
                        .format(&Rfc3339)?,
                    row.get("flow_run_id"),
                    row.get("source_rank"),
                    row.get::<Option<i64>, _>("event_sequence").unwrap_or(0),
                    row.get("event_id"),
                ));
            }
        }
        if let Some((time, run, rank, sequence, id)) = &cursor {
            let sql=format!("{} select exists(select 1 from enriched where created_at=$3 and flow_run_id=$4 and source_rank=$5 and coalesce(event_sequence,0)=$6 and event_id=$7)",event_sql());
            let valid: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(application_id)
                .bind(flow_run_id)
                .bind(timestamp(Some(time))?)
                .bind(run)
                .bind(rank)
                .bind(sequence)
                .bind(id)
                .fetch_one(self.pool())
                .await?;
            if !valid {
                return Err(InvalidWorkflowTrajectoryQuery(
                    "invalid trajectory cursor scope".into(),
                )
                .into());
            }
        }
        let cursor_time = timestamp(cursor.as_ref().map(|v| v.0.as_str()))?;
        let cursor_id = cursor.as_ref().map(|v| v.4.as_str());
        let limit = query.limit.unwrap_or(50).clamp(1, 100);
        let links = include_str!("links.sql")
            .replace("$2", "e.flow_run_id")
            .replace("$3", "(e.metadata->>'trigger_request_id')")
            .replace("$4", "(e.metadata->>'context_flow_run_id')")
            .replace("$5", "(e.metadata->>'context_response_id')");
        let comparison = if previous {
            "<"
        } else if initial_focus {
            ">="
        } else {
            ">"
        };
        let direction = if previous { "desc" } else { "asc" };
        let sql = format!("{} select e.*,({links}) links from enriched e
            where ($3='all' or e.category=$3 or ($3='agents' and e.depth>0) or ($3='rounds' and e.is_round))
            and ($4::uuid is null or e.node_run_id=$4)
            and ($5::text is null or e.metadata->>'trigger_request_id'=$5)
            and ($6::timestamptz is null or e.created_at >= $6)
            and ($7::timestamptz is null or e.created_at <= $7)
            and ($8::timestamptz is null or (e.created_at,e.flow_run_id,e.source_rank,coalesce(e.event_sequence,0),e.event_id){comparison}($8,$9::uuid,$10::integer,$11::bigint,$12::text))
            order by e.created_at {direction},e.flow_run_id {direction},e.source_rank {direction},coalesce(e.event_sequence,0) {direction},e.event_id {direction} limit $13",event_sql());
        let rows = if query.index_only {
            Vec::new()
        } else {
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(application_id)
                .bind(flow_run_id)
                .bind(query.category.as_str())
                .bind(query.node_run_id)
                .bind(query.request_id.map(|v| v.to_string()))
                .bind(from)
                .bind(to)
                .bind(cursor_time)
                .bind(cursor.as_ref().map(|v| v.1))
                .bind(cursor.as_ref().map(|v| v.2))
                .bind(cursor.as_ref().map(|v| v.3))
                .bind(cursor_id)
                .bind(limit + 1)
                .fetch_all(self.pool())
                .await?
        };
        let more = rows.len() > limit as usize;
        let mut items = Vec::new();
        for row in rows.into_iter().take(limit as usize) {
            let event_id: String = row.get("event_id");
            let created_at = row
                .get::<OffsetDateTime, _>("created_at")
                .format(&Rfc3339)?;
            let native_step = if event_id.starts_with("native:") {
                Some(ProviderTrajectoryStep {
                    event_id: row.get("source_id"),
                    event_sequence: row.get("event_sequence"),
                    event_type: row.get("event_type"),
                    metadata: row.get("metadata"),
                    created_at: created_at.clone(),
                    links: serde_json::from_value(row.get("links"))?,
                })
            } else {
                None
            };
            items.push(WorkflowTrajectoryEvent {
                execution_identity: decode_execution_identity(&row)?,
                event_id,
                event_sequence: row.get("event_sequence"),
                event_type: row.get("event_type"),
                created_at,
                category: row.get("category"),
                flow_run_id: row.get("flow_run_id"),
                task_run_id: row.get("task_run_id"),
                parent_task_run_id: row.get("parent_task_run_id"),
                node_run_id: row.get("node_run_id"),
                node_id: row.get("node_id"),
                node_alias: row.get("node_alias"),
                node_type: row.get("node_type"),
                status: row.get("status"),
                preview: row.get("preview"),
                native_step,
            });
        }
        if previous {
            items.reverse();
        }
        let cursor_for = |e: &WorkflowTrajectoryEvent| {
            serde_json::to_string(&(
                &e.created_at,
                e.flow_run_id,
                source_rank(&e.event_id),
                e.event_sequence.unwrap_or(0),
                &e.event_id,
            ))
        };
        let before = if let Some(first) = items.first() {
            let sql=format!("{} select exists(select 1 from enriched e where ($3='all' or e.category=$3 or ($3='agents' and e.depth>0) or ($3='rounds' and e.is_round)) and ($4::uuid is null or e.node_run_id=$4) and ($5::text is null or e.metadata->>'trigger_request_id'=$5) and ($6::timestamptz is null or e.created_at >= $6) and ($7::timestamptz is null or e.created_at <= $7) and (e.created_at,e.flow_run_id,e.source_rank,coalesce(e.event_sequence,0),e.event_id)<($8,$9::uuid,$10::integer,$11::bigint,$12::text))",event_sql());
            sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(application_id)
                .bind(flow_run_id)
                .bind(query.category.as_str())
                .bind(query.node_run_id)
                .bind(query.request_id.map(|id| id.to_string()))
                .bind(from)
                .bind(to)
                .bind(timestamp(Some(&first.created_at))?)
                .bind(first.flow_run_id)
                .bind(source_rank(&first.event_id))
                .bind(first.event_sequence.unwrap_or(0))
                .bind(&first.event_id)
                .fetch_one(self.pool())
                .await?
        } else {
            false
        };
        let previous_cursor = if before {
            items
                .first()
                .map(|e| cursor_for(e).map(|c| format!("previous:{c}")))
                .transpose()?
        } else {
            None
        };
        let next_cursor = if more && !previous || previous && query.cursor.is_some() {
            items
                .last()
                .map(|e| {
                    serde_json::to_string(&(
                        &e.created_at,
                        e.flow_run_id,
                        source_rank(&e.event_id),
                        e.event_sequence.unwrap_or(0),
                        &e.event_id,
                    ))
                })
                .transpose()?
        } else {
            None
        };
        // Catalogue and extent deliberately precede all interactive filters.
        let nodes = sqlx::query(sqlx::AssertSqlSafe(format!("{} select n.flow_run_id,n.id node_run_id,n.node_id,n.node_alias,n.node_type, case when ce.execution_id is not null then jsonb_build_object('execution_id',ce.execution_id,'node_id',ce.node_id,'parent_execution_id',ce.parent_execution_id,'round_path',ce.round_path) end execution_identity from scope_runs r join node_runs n on n.flow_run_id=r.id left join flow_container_executions ce on ce.node_run_id=n.id and ce.flow_run_id=n.flow_run_id order by n.started_at,n.id", scope_sql())))
            .bind(application_id).bind(flow_run_id).fetch_all(self.pool()).await?
            .into_iter().map(|row| Ok(WorkflowTrajectoryNode {
                execution_identity: decode_execution_identity(&row)?,
                flow_run_id:row.get("flow_run_id"), node_run_id:row.get("node_run_id"),
                node_id:row.get("node_id"), node_alias:row.get("node_alias"), node_type:row.get("node_type"),
            })).collect::<Result<Vec<_>>>()?;
        let extent = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{} select min(created_at) time_start,max(created_at) time_end from events",
            event_sql()
        )))
        .bind(application_id)
        .bind(flow_run_id)
        .fetch_one(self.pool())
        .await?;
        let overview = if query.index_only {
            // The full index excludes links, previews, bodies and locator payloads.
            let sql=format!("{} select event_id,source_id,event_sequence,event_type,created_at,category,flow_run_id,node_run_id,node_id,node_alias,task_run_id,parent_task_run_id,depth,is_round,execution_identity,metadata->>'trigger_request_id' trigger_request_id,jsonb_strip_nulls(jsonb_build_object('kind',metadata->'kind','purpose',metadata->'purpose','source',metadata->'source','status',metadata->'status','step_key',metadata->'step_key','flow_run_id',metadata->'flow_run_id','node_id',metadata->'node_id','node_run_id',metadata->'node_run_id','invocation_id',metadata->'invocation_id','provider_attempt_index',metadata->'provider_attempt_index','run_mode',metadata->'run_mode')) metadata from enriched e order by created_at,flow_run_id,source_rank,coalesce(event_sequence,0),event_id",event_sql());
            let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(application_id)
                .bind(flow_run_id)
                .fetch_all(self.pool())
                .await?;
            let mut index = Vec::new();
            let mut category_counts = std::collections::BTreeMap::new();
            for row in rows {
                let event_id: String = row.get("event_id");
                let category: String = row.get("category");
                let depth: i32 = row.get("depth");
                let is_round: bool = row.get("is_round");
                let event_time = row.get::<OffsetDateTime, _>("created_at");
                let event_node: Option<Uuid> = row.get("node_run_id");
                let event_request: Option<String> = row.get("trigger_request_id");
                if query.node_run_id.is_some_and(|id| event_node != Some(id))
                    || query
                        .request_id
                        .is_some_and(|id| event_request.as_deref() != Some(id.to_string().as_str()))
                    || from.is_some_and(|time| event_time < time)
                    || to.is_some_and(|time| event_time > time)
                {
                    continue;
                }
                *category_counts.entry(category.clone()).or_insert(0) += 1;
                if depth > 0 {
                    *category_counts.entry("agents".into()).or_insert(0) += 1;
                }
                if is_round && category != "rounds" {
                    *category_counts.entry("rounds".into()).or_insert(0) += 1;
                }
                if query.category.as_str() != "all"
                    && category != query.category.as_str()
                    && !(query.category.as_str() == "agents" && depth > 0)
                    && !(query.category.as_str() == "rounds" && is_round)
                {
                    continue;
                }
                let created_at = row
                    .get::<OffsetDateTime, _>("created_at")
                    .format(&Rfc3339)?;
                let native_step = if event_id.starts_with("native:") {
                    Some(ProviderTrajectoryStep {
                        event_id: row.get("source_id"),
                        event_sequence: row.get("event_sequence"),
                        event_type: row.get("event_type"),
                        created_at: created_at.clone(),
                        metadata: row.get("metadata"),
                        links: vec![],
                    })
                } else {
                    None
                };
                index.push(WorkflowTrajectoryIndexItem {
                    execution_identity: decode_execution_identity(&row)?,
                    node_id: row.get("node_id"),
                    event_id,
                    event_sequence: row.get("event_sequence"),
                    created_at,
                    category,
                    flow_run_id: row.get("flow_run_id"),
                    node_run_id: row.get("node_run_id"),
                    node_alias: row.get("node_alias"),
                    task_run_id: row.get("task_run_id"),
                    parent_task_run_id: row.get("parent_task_run_id"),
                    event_type: row.get("event_type"),
                    native_step,
                });
            }
            Some(TrajectoryOverview {
                request_roots: None,
                total_count: index.len() as i64,
                category_counts,
                time_start: formatted(extent.get("time_start"))?,
                time_end: formatted(extent.get("time_end"))?,
                items: index,
            })
        } else {
            None
        };
        Ok(WorkflowTrajectoryPage {
            previous_cursor,
            overview,
            items,
            next_cursor,
            nodes,
            time_start: formatted(extent.get("time_start"))?,
            time_end: formatted(extent.get("time_end"))?,
        })
    }

    pub(super) async fn read_workflow_trajectory_body(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        event_id: &str,
    ) -> Result<Option<WorkflowTrajectoryBody>> {
        // The locator must be present in precisely the same closed scope and allowlist as its summary.
        let locator=sqlx::query(sqlx::AssertSqlSafe(format!("{} select source_id,flow_run_id,node_run_id,event_type from events where event_id=$3 and metadata is null",event_sql())))
            .bind(application_id).bind(flow_run_id).bind(event_id).fetch_optional(self.pool()).await?;
        let Some(locator) = locator else {
            return Ok(None);
        };
        let source_id: Uuid = locator.get("source_id");
        let source_run: Uuid = locator.get("flow_run_id");
        let sections = if event_id.starts_with("round_started:")
            || event_id.starts_with("round_finished:")
        {
            let row = sqlx::query("select round_index,deadline_unix_ms,status,runtime_original_json(output_payload,raw_json_payloads,'output_payload') output_payload,runtime_original_json(variables,raw_json_payloads,'variables') variables,runtime_original_json(error_payload,raw_json_payloads,'error_payload') error_payload from flow_container_rounds where flow_run_id=$1 and execution_id=$2")
                .bind(source_run).bind(source_id).fetch_one(self.pool()).await?;
            let mut sections = vec![WorkflowEventSection {
                kind: "round".into(),
                value: serde_json::json!({"round_index":row.get::<i64,_>("round_index"),"deadline_unix_ms":row.get::<i64,_>("deadline_unix_ms"),"status":row.get::<String,_>("status")}),
            }];
            if event_id.starts_with("round_finished:") {
                sections.push(WorkflowEventSection { kind: "output".into(), value: serde_json::json!({"result":row.get::<Option<Value>,_>("output_payload"),"variables":row.get::<Option<Value>,_>("variables")}) });
                if let Some(error) = row.get::<Option<Value>, _>("error_payload") {
                    sections.push(WorkflowEventSection {
                        kind: "error".into(),
                        value: error,
                    });
                }
            }
            sections
        } else if event_id.starts_with("workflow:") || event_id.starts_with("runtime:") {
            let table = if event_id.starts_with("runtime:") {
                "runtime_events"
            } else {
                "flow_run_events"
            };
            let resolver = if table == "runtime_events" {
                "runtime_event_original_payload(payload,raw_json_payloads,flow_run_id)"
            } else {
                "runtime_original_json(payload,raw_json_payloads,'payload')"
            };
            let body_sql = format!("select {resolver} from {table} where id=$1 and flow_run_id=$2");
            let value: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(body_sql.as_str()))
                .bind(source_id)
                .bind(source_run)
                .fetch_one(self.pool())
                .await?;
            vec![WorkflowEventSection {
                kind: "payload".into(),
                value,
            }]
        } else {
            let finished = event_id.starts_with("node_finished:");
            let row=sqlx::query("select node_id,node_alias,node_type,status,runtime_original_json(input_payload,raw_json_payloads,'input_payload') input_payload,case when $3 then runtime_original_json(output_payload,raw_json_payloads,'output_payload') end output_payload,case when $3 then runtime_original_json(error_payload,raw_json_payloads,'error_payload') end error_payload from node_run_records where id=$1 and flow_run_id=$2")
                .bind(source_id).bind(source_run).bind(finished).fetch_one(self.pool()).await?;
            let mut sections = vec![WorkflowEventSection {
                kind: "node".into(),
                value: serde_json::json!({"node_run_id":source_id,"node_id":row.get::<String,_>("node_id"),"node_alias":row.get::<String,_>("node_alias"),"node_type":row.get::<String,_>("node_type"),"status":row.get::<String,_>("status")}),
            }];
            sections.push(WorkflowEventSection {
                kind: "input".into(),
                value: row.get("input_payload"),
            });
            if event_id.starts_with("node_finished:") {
                sections.push(WorkflowEventSection {
                    kind: "output".into(),
                    value: row.get("output_payload"),
                });
                if let Some(value) = row.get::<Option<Value>, _>("error_payload") {
                    sections.push(WorkflowEventSection {
                        kind: "error".into(),
                        value,
                    });
                }
                let failures = sqlx::query("select f.after_round_index,runtime_original_json(f.error_payload,f.raw_json_payloads,'error_payload') error_payload from flow_container_control_failures f join flow_container_executions e on e.flow_run_id=f.flow_run_id and e.execution_id=f.execution_id where e.flow_run_id=$1 and e.node_run_id=$2 order by f.after_round_index")
                    .bind(source_run).bind(source_id).fetch_all(self.pool()).await?;
                for failure in failures {
                    sections.push(WorkflowEventSection {
                        kind: "error".into(),
                        value: serde_json::json!({"phase":"break_condition","after_round_index":failure.get::<i64,_>("after_round_index"),"error_payload":failure.get::<Value,_>("error_payload")}),
                    });
                }
            }
            sections
        };
        Ok(Some(WorkflowTrajectoryBody {
            event_id: event_id.into(),
            sections,
        }))
    }
}

fn decode_execution_identity(
    row: &sqlx::postgres::PgRow,
) -> Result<Option<domain::NodeExecutionIdentity>> {
    row.get::<Option<serde_json::Value>, _>("execution_identity")
        .map(serde_json::from_value)
        .transpose()
        .map_err(Into::into)
}
