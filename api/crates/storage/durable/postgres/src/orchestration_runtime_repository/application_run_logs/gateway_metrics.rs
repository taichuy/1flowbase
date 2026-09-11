use super::*;

impl PgControlPlaneStore {
    pub(super) async fn gateway_metrics(
        &self,
        scope: &GatewayLogScope,
        kind: &str,
        id: Uuid,
    ) -> Result<(GatewayLogMetrics, Vec<String>)> {
        let predicate = match kind {
            "conversation" => "g.conversation_id=$4",
            "turn" => "g.turn_id=$4",
            _ => "f.id=$4",
        };
        let selected = format!(
            "select f.id,f.started_at,f.finished_at,f.status from flow_runs f join applications a on a.id=f.application_id left join gateway_log_invocations g on g.flow_run_id=f.id where a.workspace_id=$1 and f.application_id=$2 and ($3::uuid is null or f.api_key_id=$3) and {predicate}"
        );
        let cte = format!(
            "with selected as materialized ({selected}), attempts as materialized(select l.* from model_provider_request_logs l join selected s on s.id=l.flow_run_id where l.scope_id=$1), outputs as materialized(select o.* from gateway_log_output_items o join selected s on s.id=o.flow_run_id)"
        );
        let sql = format!(
            r#"{cte}
select jsonb_build_object(
 'invocation_count',(select count(*) from selected),
 'attempt_count',(select count(*) from attempts),
 'failed_attempt_count',(select count(*) from attempts where status='failed'),
 'input_tokens',(select case when count(input_tokens)=count(*) then sum(input_tokens)::bigint end from attempts),
 'output_tokens',(select case when count(output_tokens)=count(*) then sum(output_tokens)::bigint end from attempts),
 'total_tokens',(select case when count(total_tokens)=count(*) then sum(total_tokens)::bigint end from attempts),
 'elapsed_ms',(select case when count(finished_at)=count(*) then (extract(epoch from(max(finished_at)-min(started_at)))*1000)::bigint end from selected),
 'model_duration_ms',(select case when count(total_duration_ms)=count(*) then sum(total_duration_ms)::bigint end from attempts),
 'tool_result_wait_ms',null,
 'costs',coalesce((select jsonb_agg(jsonb_build_object('currency_code',currency_code,'amount',amount)) from(select currency_code,sum(total_cost)::text as amount from attempts where total_cost is not null and currency_code is not null group by currency_code order by currency_code) costs),'[]'::jsonb),
 'unknown_cost_attempts',(select count(*) from attempts where total_cost is null or currency_code is null)
) as metrics,
exists(select 1 from selected where status='failed') as failed,
exists(select 1 from selected where status in ('running','queued')) as active,
exists(select 1 from attempts where is_retry) as retried,
exists(select 1 from outputs where not conflicting and item->>'phase'='final_answer') as final_answer,
exists(select 1 from outputs o where item->>'type' in ('custom_tool_call','function_call') and not exists(select 1 from gateway_log_tool_results r where r.conversation_id=o.conversation_id and r.call_id=o.item->>'call_id' and not o.conflicting and not r.conflicting and r.result->>'type'=case o.item->>'type' when 'custom_tool_call' then 'custom_tool_call_output' else 'function_call_output' end)) as waiting_tool
"#
        );
        let row = sqlx::query(&sql)
            .bind(scope.scope_id)
            .bind(scope.application_id)
            .bind(scope.api_key_id)
            .bind(id)
            .fetch_one(self.pool())
            .await?;
        let mut metrics: GatewayLogMetrics = serde_json::from_value(row.try_get("metrics")?)?;
        let mut observations = Vec::new();
        for (column, label) in [
            ("failed", "invocation_failed"),
            ("active", "invocation_active"),
            ("retried", "retry_observed"),
            ("final_answer", "final_answer_observed"),
            ("waiting_tool", "waiting_for_tool_result"),
        ] {
            if row.try_get::<bool, _>(column)? {
                observations.push(label.into());
            }
        }
        if metrics.attempt_count == 0 && metrics.invocation_count > 0 {
            observations.push("attempts_pending_or_unavailable".into());
        }
        // Interval union stays in PostgreSQL: one scalar result, regardless of
        // conversation size. Missing or contradictory result times stay unknown.
        metrics.tool_result_wait_ms=sqlx::query_scalar(&format!(r#"
with selected as ({selected}), intervals as (
 select o.observed_at as start_at,r.received_at as end_at,
 max(r.received_at) over(order by o.observed_at,o.item_key rows between unbounded preceding and 1 preceding) as previous_end
 from gateway_log_output_items o join selected s on s.id=o.flow_run_id
 left join gateway_log_tool_results r on r.conversation_id=o.conversation_id
  and r.call_id=o.item->>'call_id' and not r.conflicting and not o.conflicting
  and r.result->>'type'=case o.item->>'type' when 'custom_tool_call' then 'custom_tool_call_output' else 'function_call_output' end
 where o.item->>'type' in ('custom_tool_call','function_call')
)
select case when count(*)=0 then 0
 when count(end_at)=count(*) and bool_and(end_at>=start_at)
 then sum(greatest(0,extract(epoch from(end_at-greatest(start_at,coalesce(previous_end,start_at))))*1000))::bigint end
from intervals
"#)).bind(scope.scope_id).bind(scope.application_id).bind(scope.api_key_id).bind(id).fetch_one(self.pool()).await?;
        Ok((metrics, observations))
    }
}
