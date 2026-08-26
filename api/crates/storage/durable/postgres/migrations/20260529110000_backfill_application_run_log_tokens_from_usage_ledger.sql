with usage_totals as (
    select
        flow_run_id,
        sum(total_tokens)::bigint as total_tokens
    from runtime_usage_ledger
    where total_tokens is not null
    group by flow_run_id
)
update application_run_log_summaries summaries
set total_tokens = usage_totals.total_tokens,
    log_updated_at = now()
from usage_totals
where summaries.flow_run_id = usage_totals.flow_run_id
  and summaries.total_tokens is distinct from usage_totals.total_tokens;
