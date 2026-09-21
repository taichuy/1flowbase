with currency_totals as (
    select settlement_currency as currency_code,
           sum(normalized_cost)::text as total_cost,
           count(normalized_cost) = count(*) and settlement_currency is not null as complete
    from runtime_cost_ledger where flow_run_id = $1
    group by settlement_currency
), costs as (
    select case when bool_and(complete)
                then jsonb_agg(jsonb_build_object('currency_code', currency_code, 'total_cost', total_cost)
                               order by currency_code) end as cost_breakdown
    from currency_totals
)
update application_run_log_summaries
set cost_breakdown = costs.cost_breakdown
from costs
where flow_run_id = $1
