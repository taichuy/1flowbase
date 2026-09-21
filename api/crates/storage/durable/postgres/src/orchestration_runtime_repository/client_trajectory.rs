use super::*;
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
        let payload = serde_json::to_value(input)?;
        anyhow::ensure!(
            serde_json::to_vec(&payload)?.len() <= 2 * 1024 * 1024,
            "client trajectory record capacity"
        );
        let mut tx = self.pool().begin().await?;
        // Client delivery occurs after a business terminal commit. This dedicated
        // observational append does not reopen a run or relax the execution append fence.
        let exists: Option<Uuid> =
            sqlx::query_scalar("select id from flow_runs where id=$1 for update")
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
        sqlx::query("insert into runtime_events(id,flow_run_id,node_run_id,sequence,event_type,layer,source,trust_level,payload,raw_json_payloads,visibility,durability) values($1,$2,$3,$4,'client_protocol_trajectory','runtime_item','host','host_fact',($5::jsonb->0),jsonb_strip_nulls(jsonb_build_object('payload',($5::jsonb->1))),'internal','durable')")
            .bind(Uuid::now_v7()).bind(input.flow_run_id).bind(input.node_run_id).bind(sequence)
            .bind(lossless_json_parameter(&payload)).execute(&mut *tx).await?;
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
        let limit = limit.clamp(1, 100);
        let integrity: String = sqlx::query_scalar("select case when count(*)=0 then 'not_recorded' when bool_and(status='complete' and dropped_count=0 and persist_failed_count=0) then 'complete' else 'incomplete' end from client_trajectory_captures where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2)")
            .bind(flow_run_id).bind(node_run_id).fetch_one(self.pool()).await?;
        let rows = sqlx::query(r#"
            select s.metadata,s.event_sequence,related.id as related_step_id,related.namespace as related_namespace
            from client_trajectory_steps s
            left join lateral (
                select p.id,p.metadata->>'namespace' as namespace from client_trajectory_steps p
                where p.flow_run_id=s.flow_run_id and p.node_run_id is not distinct from s.node_run_id and p.metadata->>'call_id'=s.metadata->>'call_id'
                    and (s.metadata->>'namespace' is null or p.metadata->>'namespace'=s.metadata->>'namespace')
                    and p.id<>s.id and p.event_sequence<s.event_sequence
                    and p.metadata->>'category'='tool_call' and p.metadata->>'origin'='emitted'
                    and p.metadata->'available_sections' ? 'parameters'
                order by p.event_sequence desc limit 1
            ) related on s.metadata->>'origin'='submitted' and s.metadata->'available_sections' ? 'result'
            where s.flow_run_id=$1 and ($2::uuid is null or s.node_run_id=$2) and s.event_sequence>$3
            order by s.event_sequence limit $4
        "#).bind(flow_run_id).bind(node_run_id).bind(cursor.unwrap_or(0)).bind(limit+1).fetch_all(self.pool()).await?;
        let more = rows.len() > limit as usize;
        let mut items = Vec::new();
        for row in rows.into_iter().take(limit as usize) {
            let mut step: ClientTrajectoryStep = serde_json::from_value(row.get("metadata"))?;
            step.sequence = row.get("event_sequence");
            step.related_step_id = row
                .get::<Option<Uuid>, _>("related_step_id")
                .or(step.related_step_id);
            if step.namespace.is_none() {
                step.namespace = row.get::<Option<String>, _>("related_namespace");
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
        let step = sqlx::query("select request_id,metadata from client_trajectory_steps where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2) and id=$3")
            .bind(flow_run_id).bind(node_run_id).bind(step_id).fetch_optional(self.pool()).await?;
        let Some(step) = step else { return Ok(None) };
        let metadata: Value = step.get("metadata");
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
        let rows=sqlx::query(r#"
            select p.event_sequence,runtime_original_json(e.payload,e.raw_json_payloads,'payload') as payload
            from client_trajectory_sections p join runtime_events e on e.id=p.event_id
            where p.flow_run_id=$1 and p.request_id=$2 and p.step_id=$3 and p.section=$4
                and p.event_sequence>$5
            order by p.event_sequence limit $6
        "#).bind(flow_run_id).bind(request_id).bind(selected).bind(section).bind(cursor.unwrap_or(0)).bind(limit+1).fetch_all(self.pool()).await?;
        let more = rows.len() > limit as usize;
        let items: Vec<_> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let payload: Value = row.get("payload");
                ClientTrajectorySectionItem {
                    sequence: row.get("event_sequence"),
                    value: payload["fact"]["value"].clone(),
                }
            })
            .collect();
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
