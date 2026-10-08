//! Cost-only repair of immutable imported facts; uses the existing ingest/delete lock order.
use super::super::*;
use control_plane_contracts::ports::{
    AgentLogPricingEvent, AgentLogPricingRecord, AgentLogUsage, AgentLogUsageBasis,
};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

pub(crate) fn total_cost<'a>(
    events: impl IntoIterator<Item = (&'a AgentLogUsage, bool, Option<&'a str>)>,
) -> Result<Option<String>> {
    let mut groups = BTreeMap::<&str, Vec<(&AgentLogUsage, Option<&str>)>>::new();
    for (usage, inherited, cost) in events {
        if inherited
            || (usage.basis == AgentLogUsageBasis::Cumulative
                && usage.response_id.as_deref().is_none_or(|id| id.is_empty()))
        {
            continue;
        }
        groups
            .entry(usage.response_id.as_deref().unwrap_or(""))
            .or_default()
            .push((usage, cost));
    }
    if groups.is_empty() {
        return Ok(None);
    }
    let mut total = Decimal::ZERO;
    for group in groups.values() {
        let has_delta = group
            .iter()
            .any(|(usage, _)| usage.basis == AgentLogUsageBasis::Delta);
        let selected: Vec<_> = if has_delta {
            group
                .iter()
                .filter(|(usage, _)| usage.basis == AgentLogUsageBasis::Delta)
                .collect()
        } else {
            group.last().into_iter().collect()
        };
        for (_, cost) in selected {
            let Some(cost) = cost else {
                return Ok(None);
            };
            total = total
                .checked_add(cost.parse::<Decimal>()?)
                .ok_or_else(|| anyhow!("agent_logs.cost_overflow"))?;
        }
    }
    Ok(Some(total.to_string()))
}

async fn read_events(
    connection: &mut sqlx::PgConnection,
    application_id: Uuid,
    record_id: Uuid,
) -> Result<Vec<(AgentLogPricingEvent, Option<String>)>> {
    let rows = sqlx::query(
        r#"select jsonb_build_object(
        'event_id',r.event_id, 'sequence',s.event_sequence,
        'occurred_at',c.content->>'occurred_at', 'model_id',c.content->>'model_id',
        'inherited',coalesce((c.content->>'inherited')::boolean,false), 'usage',c.content->'usage'
        ) as event, r.rated_cost::text as cost
        from application_log_upload_receipts r
        join client_trajectory_sections s on s.step_id=r.step_id and s.section='overview'
        join runtime_canonical_contents c on c.id=s.content_id and c.application_id=r.application_id
        where r.application_id=$1 and r.record_id=$2
        and c.content->'usage' is not null and c.content->'usage'<>'null'::jsonb
        order by s.event_sequence,r.event_id"#,
    )
    .bind(application_id)
    .bind(record_id)
    .fetch_all(connection)
    .await?;
    rows.into_iter()
        .map(|row| Ok((serde_json::from_value(row.get("event"))?, row.get("cost"))))
        .collect()
}

pub(crate) async fn next_record(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    scope_id: Uuid,
    after: Option<Uuid>,
) -> Result<Option<AgentLogPricingRecord>> {
    let mut connection = store.pool().acquire().await?;
    let supported: bool = sqlx::query_scalar("select exists(select 1 from applications where id=$1 and scope_id=$2 and application_type='agent_logs')")
        .bind(application_id).bind(scope_id).fetch_one(&mut *connection).await?;
    anyhow::ensure!(supported, "agent_logs.application_type");
    let id: Option<Uuid> = sqlx::query_scalar("select id from application_run_log_tasks where application_id=$1 and scope_id=$2 and source_kind='imported' and ($3::uuid is null or id>$3) order by id limit 1")
        .bind(application_id).bind(scope_id).bind(after).fetch_optional(&mut *connection).await?;
    let Some(record_id) = id else {
        return Ok(None);
    };
    let events = read_events(&mut connection, application_id, record_id)
        .await?
        .into_iter()
        .map(|(event, _)| event)
        .collect();
    Ok(Some(AgentLogPricingRecord { record_id, events }))
}

pub(crate) async fn apply(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    scope_id: Uuid,
    record_id: Uuid,
    costs: &[(String, Option<String>)],
) -> Result<u64> {
    let mut tx = store.pool().begin().await?;
    sqlx::query("select pg_advisory_xact_lock_shared(hashtextextended($1,0))")
        .bind(format!("agent-logs-application:{application_id}"))
        .execute(&mut *tx)
        .await?;
    let source: Option<String> = sqlx::query_scalar("select t.source_id from application_run_log_tasks t join applications a on a.id=t.application_id where t.application_id=$1 and t.scope_id=$2 and t.id=$3 and t.source_kind='imported' and a.application_type='agent_logs' and a.scope_id=$2")
        .bind(application_id).bind(scope_id).bind(record_id).fetch_optional(&mut *tx).await?;
    let source = source.ok_or_else(|| anyhow!("agent_logs.pricing_record_missing"))?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("agent-logs:{application_id}:{source}"))
        .execute(&mut *tx)
        .await?;
    let event_ids: Vec<_> = costs.iter().map(|(id, _)| id.clone()).collect();
    let rated_costs: Vec<_> = costs.iter().map(|(_, cost)| cost.clone()).collect();
    let present: i64 = sqlx::query_scalar("select count(*) from application_log_upload_receipts where application_id=$1 and record_id=$2 and event_id=any($3)")
        .bind(application_id).bind(record_id).bind(&event_ids).fetch_one(&mut *tx).await?;
    anyhow::ensure!(
        present as usize == event_ids.len(),
        "agent_logs.pricing_event_missing"
    );
    let changed = sqlx::query(
        r#"update application_log_upload_receipts r set rated_cost=p.cost::numeric
        from unnest($3::text[],$4::text[]) as p(event_id,cost)
        where r.application_id=$1 and r.record_id=$2 and r.event_id=p.event_id
        and r.rated_cost is distinct from p.cost::numeric"#,
    )
    .bind(application_id)
    .bind(record_id)
    .bind(event_ids)
    .bind(rated_costs)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    // Re-read under the source lock: any usage appended since the read snapshot already
    // carries the current ingestion rate and must participate in the aggregate.
    let events = read_events(&mut tx, application_id, record_id).await?;
    let total = total_cost(
        events
            .iter()
            .map(|(event, cost)| (&event.usage, event.inherited, cost.as_deref())),
    )?;
    sqlx::query("update application_run_log_tasks set total_cost=$3::numeric,cost_breakdown=jsonb_build_object('total_cost',$3::text) where application_id=$1 and id=$2 and (total_cost is distinct from $3::numeric or cost_breakdown is distinct from jsonb_build_object('total_cost',$3::text))")
        .bind(application_id).bind(record_id).bind(total).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(changed)
}
