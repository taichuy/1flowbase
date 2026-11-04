select case when count(input_tokens) + count(output_tokens) + count(total_tokens)
    + count(reasoning_output_tokens) + count(input_cache_hit_tokens)
    + count(input_cache_miss_tokens) + count(cache_read_tokens)
    + count(cached_input_tokens) + count(cache_write_tokens) = 0 then null
else jsonb_build_object(
    'prompt_tokens', sum(input_tokens),
    'completion_tokens', sum(output_tokens),
    'total_tokens', sum(coalesce(total_tokens, input_tokens + output_tokens)),
    'reasoning_tokens', sum(reasoning_output_tokens),
    'input_cache_hit_tokens', sum(input_cache_hit_tokens),
    'input_cache_miss_tokens', sum(input_cache_miss_tokens),
    'cache_read_tokens', sum(coalesce(cache_read_tokens, cached_input_tokens)),
    'cache_write_tokens', sum(cache_write_tokens)
) end
from runtime_usage_ledger
where flow_run_id = $1
