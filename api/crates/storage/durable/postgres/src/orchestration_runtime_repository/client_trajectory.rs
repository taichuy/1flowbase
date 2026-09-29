use super::*;
#[path = "client_trajectory/archive.rs"]
mod archive;
#[path = "client_trajectory/semantic.rs"]
mod semantic;
use control_plane_contracts::ports::{
    AppendClientTrajectoryArchiveInput, ClientTrajectoryArchiveFrame,
    ClientTrajectoryArchiveReceipt,
};
use control_plane_contracts::ports::{
    AppendClientTrajectoryInput, ClientTrajectoryFact, ClientTrajectoryPage,
    ClientTrajectorySection, ClientTrajectorySectionItem, ClientTrajectoryStep,
};

impl PgControlPlaneStore {
    pub(super) async fn append_client_trajectory_fact(
        &self,
        input: &AppendClientTrajectoryInput,
    ) -> Result<()> {
        if let ClientTrajectoryFact::Step { step } = &input.fact {
            anyhow::ensure!(
                step.flow_run_id == input.flow_run_id
                    && step.node_run_id == input.node_run_id
                    && step.request_id == input.request_id,
                "client trajectory scope mismatch"
            );
        }
        if let ClientTrajectoryFact::Section { section, value, .. } = &input.fact {
            if section == "raw" {
                // Legacy adapter callers use the same archive; no raw event is appended.
                return self.append_legacy_client_raw(input, value).await;
            }
        }
        let mut tx = self.pool().begin().await?;
        // Client delivery occurs after a business terminal commit. This dedicated
        // observational append does not reopen a run or relax the execution append fence.
        let exists: Option<Uuid> =
            sqlx::query_scalar("select id from flow_runs where id=$1 for no key update")
                .bind(input.flow_run_id)
                .fetch_optional(&mut *tx)
                .await?;
        anyhow::ensure!(exists.is_some(), "client trajectory flow missing");
        if let Some(node) = input.node_run_id {
            let valid: bool = sqlx::query_scalar(
                "select exists(select 1 from node_runs where id=$1 and flow_run_id=$2)",
            )
            .bind(node)
            .bind(input.flow_run_id)
            .fetch_one(&mut *tx)
            .await?;
            anyhow::ensure!(valid, "client trajectory node scope mismatch");
        }
        if let ClientTrajectoryFact::NodeLink { node_run_id } = &input.fact {
            let valid: bool = sqlx::query_scalar("select exists(select 1 from node_runs where id=$1 and flow_run_id=$2 and node_type='llm')")
                .bind(node_run_id).bind(input.flow_run_id).fetch_one(&mut *tx).await?;
            anyhow::ensure!(valid, "client trajectory node scope mismatch");
        }
        let scope = sqlx::query(
            "select flow_run_id,node_run_id from client_trajectory_captures where request_id=$1",
        )
        .bind(input.request_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(scope) = scope {
            anyhow::ensure!(
                scope.get::<Uuid, _>("flow_run_id") == input.flow_run_id
                    && scope.get::<Option<Uuid>, _>("node_run_id") == input.node_run_id,
                "client trajectory capture scope mismatch"
            );
        } else {
            anyhow::ensure!(
                matches!(input.fact, ClientTrajectoryFact::Integrity { .. }),
                "client trajectory capture missing"
            );
        }
        if let ClientTrajectoryFact::Step { step } = &input.fact {
            let owner: Option<Uuid> =
                sqlx::query_scalar("select request_id from client_trajectory_steps where id=$1")
                    .bind(step.id)
                    .fetch_optional(&mut *tx)
                    .await?;
            anyhow::ensure!(
                owner.is_none_or(|owner| owner == input.request_id),
                "client trajectory step scope mismatch"
            );
        }
        if let ClientTrajectoryFact::Section {
            step_id, section, ..
        } = &input.fact
        {
            anyhow::ensure!(
                [
                    "overview",
                    "parameters",
                    "result",
                    "schema",
                    "timing",
                    "usage",
                    "raw"
                ]
                .contains(&section.as_str()),
                "client trajectory section invalid"
            );
            // Raw wire can arrive before request JSON classification, but is always
            // bound to the real capture root. Other sections must own an indexed step.
            if section != "raw" || *step_id != input.request_id {
                let valid: bool = sqlx::query_scalar("select exists(select 1 from client_trajectory_steps where id=$1 and request_id=$2 and flow_run_id=$3)")
                    .bind(step_id).bind(input.request_id).bind(input.flow_run_id).fetch_one(&mut *tx).await?;
                anyhow::ensure!(valid, "client trajectory section scope mismatch");
            }
        }
        lock_flow_run_event_sequence(&mut tx, input.flow_run_id).await?;
        let sequence = next_runtime_event_sequence(&mut tx, input.flow_run_id).await?;
        match &input.fact {
            ClientTrajectoryFact::Step { step } => {
                semantic::write_step(&mut tx, input, step, sequence).await?;
            }
            ClientTrajectoryFact::Section {
                step_id,
                section,
                value,
            } => {
                semantic::write_section(&mut tx, input, *step_id, section, value, sequence, None)
                    .await?;
            }
            // Integrity, node correlation and response identity are real events.
            _ => {
                let payload = serde_json::to_value(input)?;
                sqlx::query("insert into runtime_events(id,flow_run_id,node_run_id,sequence,event_type,layer,source,trust_level,payload,raw_json_payloads,visibility,durability) values($1,$2,$3,$4,'client_protocol_trajectory','runtime_item','host','host_fact',($5::jsonb->0),jsonb_strip_nulls(jsonb_build_object('payload',($5::jsonb->1))),'internal','durable')")
                    .bind(Uuid::now_v7()).bind(input.flow_run_id).bind(input.node_run_id).bind(sequence)
                    .bind(lossless_json_parameter(&payload)).execute(&mut *tx).await?;
            }
        }
        // Binding moves the pre-bound durable capture into the run deletion scope.
        sqlx::query("update client_trajectory_archive_heads set flow_run_id=$2 where request_id=$1 and (flow_run_id is null or flow_run_id=$2)")
            .bind(input.request_id).bind(input.flow_run_id).execute(&mut *tx).await?;
        if matches!(&input.fact, ClientTrajectoryFact::Section { section, .. } if section == "result")
        {
            sqlx::query("update application_run_log_tasks set projection_output=projection_output where id=(select coalesce(log_task_run_id,flow_run_id) from application_run_log_summaries where flow_run_id=$1) and projection_settled_at is not null")
                .bind(input.flow_run_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn read_client_trajectory_page(
        &self,
        flow_run_id: Uuid,
        node_run_id: Option<Uuid>,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<ClientTrajectoryPage> {
        self.read_client_trajectory_selected_page(
            flow_run_id,
            node_run_id,
            cursor,
            limit,
            Default::default(),
        )
        .await
    }

    pub(super) async fn read_client_trajectory_selected_page(
        &self,
        flow_run_id: Uuid,
        node_run_id: Option<Uuid>,
        cursor: Option<i64>,
        limit: i64,
        selection: control_plane_contracts::ports::TrajectorySelection,
    ) -> Result<ClientTrajectoryPage> {
        let cursor = if cursor.is_none() {
            if let Some(target) = selection.target_id {
                let sequence: Option<i64> = sqlx::query_scalar("select event_sequence from client_trajectory_steps s where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2 or exists(select 1 from client_trajectory_node_links l where l.request_id=s.request_id and l.node_run_id=$2)) and id=$3 and ($4::uuid is null or request_id=$4)")
                    .bind(flow_run_id).bind(node_run_id).bind(target).bind(selection.request_id).fetch_optional(self.pool()).await?;
                Some(sequence.ok_or(control_plane_contracts::ports::TrajectoryTargetNotFound)? - 1)
            } else {
                None
            }
        } else {
            cursor
        };
        let limit = limit.clamp(1, 100);
        let integrity: String = sqlx::query_scalar("select case when count(*)=0 then 'not_recorded' when bool_or(status not in ('pending','complete') or dropped_count>0 or persist_failed_count>0) then 'incomplete' when bool_or(status='pending') then 'pending' else 'complete' end from client_trajectory_captures c where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2 or exists(select 1 from client_trajectory_node_links l where l.request_id=c.request_id and l.node_run_id=$2)) and ($3::uuid is null or c.request_id=$3)")
            .bind(flow_run_id).bind(node_run_id).bind(selection.request_id).fetch_one(self.pool()).await?;
        let rows = sqlx::query(r#"
            select client_trajectory_step_storage_body(s.id,s.flow_run_id) as metadata,s.semantic_metadata_restored,s.event_sequence,
                related.candidates as related_candidates,related.restored as related_restored
            from client_trajectory_steps s
            left join lateral (
                select array_agg(client_trajectory_step_storage_body(p.id,p.flow_run_id) order by p.event_sequence desc) as candidates,
                    array_agg(p.semantic_metadata_restored order by p.event_sequence desc) as restored from client_trajectory_steps p
                where p.flow_run_id=s.flow_run_id and p.node_run_id is not distinct from s.node_run_id and p.metadata->>'call_id'=s.metadata->>'call_id'
                    and (s.metadata->>'namespace' is null or p.metadata->>'namespace'=s.metadata->>'namespace')
                    and p.id<>s.id and p.event_sequence<s.event_sequence
                    and p.metadata->>'category'='tool_call' and p.metadata->>'origin'='emitted'
                    and p.metadata->'available_sections' ? 'parameters'
            ) related on s.metadata->>'origin'='submitted' and s.metadata->'available_sections' ? 'result'
            where s.flow_run_id=$1 and ($2::uuid is null or s.node_run_id=$2 or exists(select 1 from client_trajectory_node_links l where l.request_id=s.request_id and l.node_run_id=$2)) and s.event_sequence>$3 and ($5::uuid is null or s.request_id=$5)
            order by s.event_sequence limit $4
        "#).bind(flow_run_id).bind(node_run_id).bind(cursor.unwrap_or(0)).bind(limit+1).bind(selection.request_id).fetch_all(self.pool()).await?;
        let more = rows.len() > limit as usize;
        let mut items = Vec::new();
        for row in rows.into_iter().take(limit as usize) {
            let metadata: Value = row.get("metadata");
            let mut step: ClientTrajectoryStep = serde_json::from_value(semantic::step_metadata(
                metadata,
                row.get("semantic_metadata_restored"),
            )?)?;
            step.sequence = row.get("event_sequence");
            // Query projections only narrow candidates. Actual identifiers can
            // include NUL and must be compared after complete Rust restoration.
            if let Some(candidates) = row.get::<Option<Vec<Value>>, _>("related_candidates") {
                let restored: Vec<bool> = row.get("related_restored");
                for (candidate, restored) in candidates.into_iter().zip(restored) {
                    let candidate: ClientTrajectoryStep =
                        serde_json::from_value(semantic::step_metadata(candidate, restored)?)?;
                    if candidate.call_id == step.call_id
                        && step
                            .namespace
                            .as_ref()
                            .is_none_or(|namespace| candidate.namespace.as_ref() == Some(namespace))
                    {
                        step.related_step_id = Some(candidate.id);
                        if step.namespace.is_none() {
                            step.namespace = candidate.namespace;
                        }
                        break;
                    }
                }
            }
            items.push(step);
        }
        let next_cursor = if more {
            items.last().map(|s| s.sequence)
        } else {
            None
        };
        Ok(ClientTrajectoryPage {
            items,
            next_cursor,
            integrity,
        })
    }

    pub(super) async fn read_client_trajectory_section(
        &self,
        flow_run_id: Uuid,
        node_run_id: Option<Uuid>,
        step_id: Uuid,
        section: &str,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<Option<ClientTrajectorySection>> {
        if ![
            "overview",
            "parameters",
            "result",
            "schema",
            "timing",
            "usage",
            "raw",
        ]
        .contains(&section)
        {
            return Ok(None);
        }
        let step = sqlx::query("select request_id,client_trajectory_step_storage_body(s.id,s.flow_run_id) as metadata,semantic_metadata_restored from client_trajectory_steps s where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2 or exists(select 1 from client_trajectory_node_links l where l.request_id=s.request_id and l.node_run_id=$2)) and id=$3")
            .bind(flow_run_id).bind(node_run_id).bind(step_id).fetch_optional(self.pool()).await?;
        let Some(step) = step else { return Ok(None) };
        let metadata =
            semantic::step_metadata(step.get("metadata"), step.get("semantic_metadata_restored"))?;
        if !metadata["available_sections"]
            .as_array()
            .is_some_and(|v| v.iter().any(|v| v.as_str() == Some(section)))
        {
            return Ok(None);
        }
        let request_id: Uuid = step.get("request_id");
        let selected = if section == "raw" {
            request_id
        } else {
            step_id
        };
        let limit = limit.clamp(1, 32);
        if section == "raw" {
            return self
                .read_archived_client_raw(flow_run_id, request_id, step_id, cursor, limit)
                .await
                .map(Some);
        }
        let rows = semantic::section_rows(
            self.pool(),
            flow_run_id,
            request_id,
            selected,
            section,
            cursor.unwrap_or(0),
            limit + 1,
        )
        .await?;
        let more = rows.len() > limit as usize;
        let items: Vec<_> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(ClientTrajectorySectionItem {
                    sequence: row.get("event_sequence"),
                    value: semantic::restore_section(&row)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = if more {
            items.last().map(|item| item.sequence)
        } else {
            None
        };
        Ok(Some(ClientTrajectorySection {
            step_id,
            request_id,
            evidence_scope: if section == "raw" { "capture" } else { "step" }.into(),
            section: section.into(),
            items,
            next_cursor,
        }))
    }
}
