-- Backfill application_run_log_summaries.total_tokens to include cache read/write tokens
-- New definition: total_tokens = input + output + reasoning + cache_read + cache_write

update application_run_log_summaries
set total_tokens = coalesce(
    (
        select sum(
            coalesce(runtime_usage_ledger.input_tokens, 0)
            + coalesce(runtime_usage_ledger.output_tokens, 0)
            + coalesce(runtime_usage_ledger.reasoning_output_tokens, 0)
            + coalesce(
                runtime_usage_ledger.input_cache_hit_tokens,
                runtime_usage_ledger.cache_read_tokens,
                runtime_usage_ledger.cached_input_tokens,
                0
            )
            + coalesce(runtime_usage_ledger.cache_write_tokens, 0)
        )::bigint
        from runtime_usage_ledger
        where runtime_usage_ledger.flow_run_id = application_run_log_summaries.flow_run_id
    ),
    (
        select sum(
            coalesce(
                case
                    when node_runs.metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                    then (node_runs.metrics_payload #>> '{usage,input_tokens}')::bigint
                end,
                0
            )
            + coalesce(
                case
                    when node_runs.metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                    then (node_runs.metrics_payload #>> '{usage,output_tokens}')::bigint
                end,
                0
            )
            + coalesce(
                case
                    when node_runs.metrics_payload #>> '{usage,reasoning_tokens}' ~ '^-?[0-9]+$'
                    then (node_runs.metrics_payload #>> '{usage,reasoning_tokens}')::bigint
                end,
                0
            )
            + coalesce(
                case
                    when node_runs.metrics_payload #>> '{usage,input_cache_hit_tokens}' ~ '^-?[0-9]+$'
                    then (node_runs.metrics_payload #>> '{usage,input_cache_hit_tokens}')::bigint
                end,
                case
                    when node_runs.metrics_payload #>> '{usage,cache_read_tokens}' ~ '^-?[0-9]+$'
                    then (node_runs.metrics_payload #>> '{usage,cache_read_tokens}')::bigint
                end,
                case
                    when node_runs.metrics_payload #>> '{usage,cached_input_tokens}' ~ '^-?[0-9]+$'
                    then (node_runs.metrics_payload #>> '{usage,cached_input_tokens}')::bigint
                end,
                0
            )
            + coalesce(
                case
                    when node_runs.metrics_payload #>> '{usage,cache_write_tokens}' ~ '^-?[0-9]+$'
                    then (node_runs.metrics_payload #>> '{usage,cache_write_tokens}')::bigint
                end,
                0
            )
        )::bigint
        from node_runs
        where node_runs.flow_run_id = application_run_log_summaries.flow_run_id
    )
)
where total_tokens is not null;
