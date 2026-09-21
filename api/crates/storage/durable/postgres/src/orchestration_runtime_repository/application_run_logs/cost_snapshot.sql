update application_run_log_summaries
set total_cost = costs.total_cost, currency_code = costs.currency_code
from (
    select case when count(*) > 0 and count(normalized_cost) = count(*)
                     and count(settlement_currency) = count(*)
                     and count(distinct settlement_currency) = 1
                then sum(normalized_cost)::text end as total_cost,
           case when count(*) > 0 and count(normalized_cost) = count(*)
                     and count(settlement_currency) = count(*)
                     and count(distinct settlement_currency) = 1
                then min(settlement_currency) end as currency_code
    from runtime_cost_ledger where flow_run_id = $1
) costs
where flow_run_id = $1
