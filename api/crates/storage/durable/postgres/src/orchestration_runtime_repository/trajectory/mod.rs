use super::*;
use control_plane_contracts::ports::{
    ProviderTrajectoryBody, ProviderTrajectoryPage, ProviderTrajectoryRepository,
    ProviderTrajectoryStep,
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
            with attempts as (
                select metadata->>'invocation_id', metadata->>'provider_attempt_index',
                    count(*) filter(where event_type='provider_protocol_observation') as observed,
                    count(*) filter(where event_type='provider_protocol_integrity') as integrity_count,
                    coalesce(max((metadata->>'observed_count')::bigint) filter(where event_type='provider_protocol_integrity'), 0) as expected,
                    coalesce(max((metadata->>'persist_failed_count')::bigint) filter(where event_type='provider_protocol_integrity'), 0) as failed,
                    bool_and(metadata->>'status'='complete') filter(where event_type='provider_protocol_integrity') as complete,
                    bool_and(metadata->>'status'='unavailable') filter(where event_type='provider_protocol_integrity') as unavailable
                from provider_protocol_trajectory_events where flow_run_id=$1 and node_run_id=$2
                group by metadata->>'invocation_id', metadata->>'provider_attempt_index'
            )
            select coalesce(sum(observed),0)::bigint as observation_count,
                coalesce(sum(failed),0)::bigint as persist_failed_count,
                coalesce(bool_and(coalesce(complete,false) and integrity_count > 0 and expected=observed and failed=0),false) as complete,
                coalesce(bool_and(observed=0 and coalesce(unavailable,false)),true) as unavailable
            from attempts
        "#).bind(flow_run_id).bind(node_run_id).fetch_one(self.pool()).await?;
        let observation_count: i64 = counts.get("observation_count");
        let persist_failed_count: i64 = counts.get("persist_failed_count");
        let complete: bool = counts.get("complete");
        let unavailable: bool = counts.get("unavailable");
        let integrity = if unavailable {
            "unavailable"
        } else if complete {
            "complete"
        } else {
            "incomplete"
        };
        let rows = sqlx::query(
            r#"
            select event_id,event_sequence,event_type,metadata,created_at
            from provider_protocol_trajectory_events
            where flow_run_id=$1 and node_run_id=$2 and event_sequence > $3
                and event_type='provider_protocol_observation'
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
    ) -> Result<Option<ProviderTrajectoryBody>> {
        let row = sqlx::query(
            r#"
            select runtime_original_json(e.payload, e.raw_json_payloads, 'payload') as payload
            from runtime_events e join provider_protocol_trajectory_events p on p.event_id=e.id
            where p.flow_run_id=$1 and p.node_run_id=$2 and p.event_id=$3
                and p.event_type='provider_protocol_observation'
        "#,
        )
        .bind(flow_run_id)
        .bind(node_run_id)
        .bind(event_id)
        .fetch_optional(self.pool())
        .await?;
        row.map(|row| {
            let payload: Value = row.get("payload");
            Ok(ProviderTrajectoryBody {
                event_id,
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
        .transpose()
    }
}
