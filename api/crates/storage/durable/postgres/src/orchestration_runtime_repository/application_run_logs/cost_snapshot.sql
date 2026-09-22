-- Internal credits: add every recorded amount, regardless of currency label.
update application_run_log_summaries
set total_cost = costs.total_cost
from (
    select case when count(normalized_cost) = count(*)
                then sum(normalized_cost) end as total_cost
    from runtime_cost_ledger where flow_run_id = $1
) costs
where flow_run_id = $1
