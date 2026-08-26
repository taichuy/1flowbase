with usage_totals as (
    select
        flow_run_id,
        sum(coalesce(input_cache_hit_tokens, cache_read_tokens, cached_input_tokens))::bigint
            as input_cache_hit_tokens
    from runtime_usage_ledger
    where input_cache_hit_tokens is not null
       or cache_read_tokens is not null
       or cached_input_tokens is not null
    group by flow_run_id
)
update application_run_log_summaries summaries
set input_cache_hit_tokens = usage_totals.input_cache_hit_tokens,
    log_updated_at = now()
from usage_totals
where summaries.flow_run_id = usage_totals.flow_run_id
  and summaries.input_cache_hit_tokens is distinct from usage_totals.input_cache_hit_tokens;
