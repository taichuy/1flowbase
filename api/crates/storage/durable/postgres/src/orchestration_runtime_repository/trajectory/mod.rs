use super::*;
use control_plane_contracts::ports::{
    ProviderTrajectoryBody, ProviderTrajectoryEvidence, ProviderTrajectoryPage,
    ProviderTrajectoryRepository, ProviderTrajectoryStep,
};

#[async_trait]
impl ProviderTrajectoryRepository for PgControlPlaneStore {
    async fn provider_trajectory_page(
        &self,
        flow_run_id: Uuid,
        node_run_id: Uuid,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<ProviderTrajectoryPage> {
        let limit = limit.clamp(1, 100);
        let counts = sqlx::query(r#"
            with observations as (
                select metadata->>'invocation_id' as invocation_id,
                    metadata->>'provider_attempt_index' as attempt,
                    count(*) as observed
                from provider_protocol_trajectory_events
                where flow_run_id=$1 and node_run_id=$2 and event_type='provider_protocol_observation'
                group by 1,2
            ), latest_integrity as (
                select distinct on (metadata->>'invocation_id',metadata->>'provider_attempt_index')
                    metadata->>'invocation_id' as invocation_id,
                    metadata->>'provider_attempt_index' as attempt, metadata
                from provider_protocol_trajectory_events
                where flow_run_id=$1 and node_run_id=$2 and event_type='provider_protocol_integrity'
                order by metadata->>'invocation_id',metadata->>'provider_attempt_index',event_sequence desc
            ), attempts as (
                select coalesce(o.observed,0) as observed,
                    coalesce((i.metadata->>'observed_count')::bigint,0) as expected,
                    coalesce((i.metadata->>'persist_failed_count')::bigint,0) as failed,
                    coalesce((i.metadata->>'dropped_count')::bigint,0) as dropped,
                    coalesce(i.metadata->>'status'='complete',false) as complete,
                    coalesce(i.metadata->>'status'='unavailable',false) as unavailable
                from observations o full join latest_integrity i using(invocation_id,attempt)
            )
            select coalesce(sum(observed),0)::bigint as observation_count,
                coalesce(sum(failed),0)::bigint as persist_failed_count,
                coalesce(sum(dropped),0)::bigint as dropped_count,
                coalesce(bool_and(complete and expected=observed and failed=0 and dropped=0),false) as complete,
                coalesce(bool_and(observed=0 and unavailable),true) as unavailable
            from attempts
        "#).bind(flow_run_id).bind(node_run_id).fetch_one(self.pool()).await?;
        let observation_count: i64 = counts.get("observation_count");
        let persist_failed_count: i64 = counts.get("persist_failed_count");
        let complete: bool = counts.get("complete");
        let unavailable: bool = counts.get("unavailable");
        let dropped_count: i64 = counts.get("dropped_count");
        let integrity = if dropped_count > 0 || persist_failed_count > 0 {
            "incomplete"
        } else if unavailable && observation_count == 0 {
            "not_recorded"
        } else if complete {
            "complete"
        } else {
            "incomplete"
        };
        let rows = sqlx::query(
            r#"
            select event_id,event_sequence,'provider_semantic_step' as event_type,metadata,created_at
            from provider_semantic_trajectory_steps
            where flow_run_id=$1 and node_run_id=$2 and event_sequence > $3
            order by event_sequence asc limit $4
        "#,
        )
        .bind(flow_run_id)
        .bind(node_run_id)
        .bind(cursor.unwrap_or(0))
        .bind(limit + 1)
        .fetch_all(self.pool())
        .await?;
        let has_more = rows.len() > limit as usize;
        let items: Vec<ProviderTrajectoryStep> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(ProviderTrajectoryStep {
                    event_id: row.get("event_id"),
                    event_sequence: row.get("event_sequence"),
                    event_type: row.get("event_type"),
                    metadata: row.get("metadata"),
                    created_at: row
                        .get::<OffsetDateTime, _>("created_at")
                        .format(&time::format_description::well_known::Rfc3339)?,
                })
            })
            .collect::<Result<_>>()?;
        let next_cursor = if has_more {
            items.last().map(|item| item.event_sequence)
        } else {
            None
        };
        let semantic = sqlx::query("select count(*) as count, coalesce(bool_or(metadata->>'status' in ('incomplete','unavailable')),false) as incomplete from provider_semantic_trajectory_steps where flow_run_id=$1 and node_run_id=$2")
            .bind(flow_run_id).bind(node_run_id).fetch_one(self.pool()).await?;
        let integrity = if semantic.get::<bool, _>("incomplete") {
            "incomplete"
        } else if semantic.get::<i64, _>("count") == 0 && integrity == "complete" {
            "not_recorded"
        } else {
            integrity
        };
        Ok(ProviderTrajectoryPage {
            items,
            next_cursor,
            observation_count,
            persist_failed_count,
            integrity: integrity.into(),
        })
    }

    async fn provider_trajectory_body(
        &self,
        flow_run_id: Uuid,
        node_run_id: Uuid,
        event_id: Uuid,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<Option<ProviderTrajectoryBody>> {
        // Resolve an authorized semantic step to its exact attempt and evidence range.
        // Original event ids remain valid: history is retained without synthetic semantics.
        let scope = sqlx::query(
            r#"
            select invocation_id,provider_attempt_index,
                (metadata->>'raw_sequence_start')::bigint as first,
                (metadata->>'raw_sequence_end')::bigint as last
            from provider_semantic_trajectory_steps
            where flow_run_id=$1 and node_run_id=$2 and event_id=$3
            union all
            select metadata->>'invocation_id', (metadata->>'provider_attempt_index')::bigint,
                (metadata->>'sequence')::bigint,(metadata->>'sequence')::bigint
            from provider_protocol_trajectory_events
            where flow_run_id=$1 and node_run_id=$2 and event_id=$3
                and event_type='provider_protocol_observation'
            limit 1
        "#,
        )
        .bind(flow_run_id)
        .bind(node_run_id)
        .bind(event_id)
        .fetch_optional(self.pool())
        .await?;
        let Some(scope) = scope else { return Ok(None) };
        let limit = limit.clamp(1, 16);
        let rows = sqlx::query(
            r#"
            select p.event_id,(p.metadata->>'sequence')::bigint as sequence,
                runtime_original_json(e.payload,e.raw_json_payloads,'payload') as payload
            from provider_protocol_trajectory_events p join runtime_events e on e.id=p.event_id
            where p.flow_run_id=$1 and p.node_run_id=$2
                and p.event_type='provider_protocol_observation'
                and p.metadata->>'invocation_id'=$3
                and (p.metadata->>'provider_attempt_index')::bigint=$4
                and (p.metadata->>'sequence')::bigint between $5 and $6
                and (p.metadata->>'sequence')::bigint > $7
            order by (p.metadata->>'sequence')::bigint limit $8
        "#,
        )
        .bind(flow_run_id)
        .bind(node_run_id)
        .bind(scope.get::<String, _>("invocation_id"))
        .bind(scope.get::<i64, _>("provider_attempt_index"))
        .bind(scope.get::<i64, _>("first"))
        .bind(scope.get::<i64, _>("last"))
        .bind(cursor.unwrap_or(0))
        .bind(limit + 1)
        .fetch_all(self.pool())
        .await?;
        let has_more = rows.len() > limit as usize;
        let items = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let payload: Value = row.get("payload");
                Ok(ProviderTrajectoryEvidence {
                    event_id: row.get("event_id"),
                    sequence: row.get("sequence"),
                    body: payload
                        .get("body")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow!("protocol observation body missing"))?
                        .into(),
                    encoding: payload
                        .get("encoding")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow!("protocol observation encoding missing"))?
                        .into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = if has_more {
            items.last().map(|item| item.sequence)
        } else {
            None
        };
        Ok(Some(ProviderTrajectoryBody {
            event_id,
            items,
            next_cursor,
        }))
    }
}
