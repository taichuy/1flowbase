use super::*;
use control_plane_contracts::ports::{
    InvalidWorkflowTrajectoryQuery, WorkflowEventSection, WorkflowTrajectoryBody,
    WorkflowTrajectoryEvent, WorkflowTrajectoryNode, WorkflowTrajectoryPage,
    WorkflowTrajectoryQuery,
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
        "node_started" => 0,
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
        let cursor: Option<(String, Uuid, i32, i64, String)> = query
            .cursor
            .as_deref()
            .map(|v| {
                serde_json::from_str(v)
                    .map_err(|_| InvalidWorkflowTrajectoryQuery("invalid trajectory cursor".into()))
            })
            .transpose()?;
        if let Some((_, _, rank, sequence, id)) = &cursor {
            let valid_id = id.split_once(':').is_some_and(|(prefix, id)| {
                matches!(
                    prefix,
                    "native" | "workflow" | "runtime" | "node_started" | "node_finished"
                ) && Uuid::parse_str(id).is_ok()
            });
            if !valid_id || *rank != source_rank(id) || *sequence < 0 {
                return Err(
                    InvalidWorkflowTrajectoryQuery("invalid trajectory cursor".into()).into(),
                );
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
        let sql = format!("{} select e.*,({links}) links from enriched e
            where ($3='all' or e.category=$3 or ($3='agents' and e.depth>0) or ($3='rounds' and e.is_round))
            and ($4::uuid is null or e.node_run_id=$4)
            and ($5::text is null or e.metadata->>'trigger_request_id'=$5)
            and ($6::timestamptz is null or e.created_at >= $6)
            and ($7::timestamptz is null or e.created_at <= $7)
            and ($8::timestamptz is null or (e.created_at,e.flow_run_id,e.source_rank,coalesce(e.event_sequence,0),e.event_id)>($8,$9::uuid,$10::integer,$11::bigint,$12::text))
            order by e.created_at,e.flow_run_id,e.source_rank,coalesce(e.event_sequence,0),e.event_id limit $13",event_sql());
        let rows = sqlx::query(&sql)
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
            .await?;
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
        let next_cursor = if more {
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
        let nodes=sqlx::query(&format!("{} select n.flow_run_id,n.id node_run_id,n.node_id,n.node_alias,n.node_type from scope_runs r join node_runs n on n.flow_run_id=r.id order by n.started_at,n.id",scope_sql()))
            .bind(application_id).bind(flow_run_id).fetch_all(self.pool()).await?
            .into_iter().map(|r|WorkflowTrajectoryNode {flow_run_id:r.get("flow_run_id"),node_run_id:r.get("node_run_id"),node_id:r.get("node_id"),node_alias:r.get("node_alias"),node_type:r.get("node_type")}).collect();
        let extent = sqlx::query(&format!(
            "{} select min(created_at) time_start,max(created_at) time_end from events",
            event_sql()
        ))
        .bind(application_id)
        .bind(flow_run_id)
        .fetch_one(self.pool())
        .await?;
        Ok(WorkflowTrajectoryPage {
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
        let locator=sqlx::query(&format!("{} select source_id,flow_run_id,node_run_id,event_type from events where event_id=$3 and metadata is null",event_sql()))
            .bind(application_id).bind(flow_run_id).bind(event_id).fetch_optional(self.pool()).await?;
        let Some(locator) = locator else {
            return Ok(None);
        };
        let source_id: Uuid = locator.get("source_id");
        let source_run: Uuid = locator.get("flow_run_id");
        let sections = if event_id.starts_with("workflow:") || event_id.starts_with("runtime:") {
            let table = if event_id.starts_with("runtime:") {
                "runtime_events"
            } else {
                "flow_run_events"
            };
            let body_sql = format!("select runtime_original_json(payload,raw_json_payloads,'payload') from {table} where id=$1 and flow_run_id=$2");
            let value: Value = sqlx::query_scalar(&body_sql)
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
            let row=sqlx::query("select node_id,node_alias,node_type,status,runtime_original_json(input_payload,raw_json_payloads,'input_payload') input_payload,case when $3 then runtime_original_json(output_payload,raw_json_payloads,'output_payload') end output_payload,case when $3 then runtime_original_json(error_payload,raw_json_payloads,'error_payload') end error_payload from node_runs where id=$1 and flow_run_id=$2")
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
            }
            sections
        };
        Ok(Some(WorkflowTrajectoryBody {
            event_id: event_id.into(),
            sections,
        }))
    }
}
