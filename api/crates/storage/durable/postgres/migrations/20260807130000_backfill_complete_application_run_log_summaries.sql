with missing_runs as materialized (
    select runs.*
    from flow_runs runs
    where not exists (
        select 1
        from application_run_log_summaries summaries
        where summaries.flow_run_id = runs.id
    )
), usage_metrics as (
    select
        ledger.flow_run_id,
        sum(
            coalesce(
                total_tokens,
                coalesce(input_tokens, 0)
                + coalesce(output_tokens, 0)
                + coalesce(reasoning_output_tokens, 0)
            )
        )::bigint as total_tokens,
        sum(input_tokens)::bigint as input_tokens,
        sum(output_tokens)::bigint as output_tokens,
        sum(
            coalesce(
                input_cache_hit_tokens,
                cache_read_tokens,
                cached_input_tokens
            )
        )::bigint as input_cache_hit_tokens
    from runtime_usage_ledger ledger
    join missing_runs on missing_runs.id = ledger.flow_run_id
    group by ledger.flow_run_id
), node_metrics as (
    select
        node_runs.flow_run_id,
        sum(
            coalesce(
                case
                    when metrics_payload #>> '{usage,total_tokens}' ~ '^-?[0-9]+$'
                    then (metrics_payload #>> '{usage,total_tokens}')::bigint
                end,
                case
                    when metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                      or metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                      or metrics_payload #>> '{usage,reasoning_tokens}' ~ '^-?[0-9]+$'
                    then coalesce(
                        case
                            when metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                            then (metrics_payload #>> '{usage,input_tokens}')::bigint
                        end,
                        0
                    ) + coalesce(
                        case
                            when metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                            then (metrics_payload #>> '{usage,output_tokens}')::bigint
                        end,
                        0
                    ) + coalesce(
                        case
                            when metrics_payload #>> '{usage,reasoning_tokens}' ~ '^-?[0-9]+$'
                            then (metrics_payload #>> '{usage,reasoning_tokens}')::bigint
                        end,
                        0
                    )
                end,
                0
            )
        )::bigint as total_tokens,
        sum(
            case
                when metrics_payload #>> '{usage,input_tokens}' ~ '^-?[0-9]+$'
                then (metrics_payload #>> '{usage,input_tokens}')::bigint
            end
        )::bigint as input_tokens,
        sum(
            case
                when metrics_payload #>> '{usage,output_tokens}' ~ '^-?[0-9]+$'
                then (metrics_payload #>> '{usage,output_tokens}')::bigint
            end
        )::bigint as output_tokens,
        sum(
            coalesce(
                case
                    when metrics_payload #>> '{usage,input_cache_hit_tokens}' ~ '^-?[0-9]+$'
                    then (metrics_payload #>> '{usage,input_cache_hit_tokens}')::bigint
                end,
                case
                    when metrics_payload #>> '{usage,cache_read_tokens}' ~ '^-?[0-9]+$'
                    then (metrics_payload #>> '{usage,cache_read_tokens}')::bigint
                end,
                case
                    when metrics_payload #>> '{usage,cached_input_tokens}' ~ '^-?[0-9]+$'
                    then (metrics_payload #>> '{usage,cached_input_tokens}')::bigint
                end
            )
        )::bigint as input_cache_hit_tokens,
        count(distinct node_id)::bigint as unique_node_count
    from node_runs
    join missing_runs on missing_runs.id = node_runs.flow_run_id
    group by node_runs.flow_run_id
), callback_metrics as (
    select
        flow_run_callback_tasks.flow_run_id,
        sum(
            case
                when jsonb_typeof(request_payload -> 'tool_calls') = 'array'
                then jsonb_array_length(request_payload -> 'tool_calls')::bigint
                else 0
            end
        )::bigint as tool_callback_count
    from flow_run_callback_tasks
    join missing_runs on missing_runs.id = flow_run_callback_tasks.flow_run_id
    where callback_kind = 'llm_tool_calls'
    group by flow_run_id
), missing_summaries as (
    select
        runs.id as flow_run_id,
        runs.scope_id,
        runs.application_id,
        runs.run_mode,
        runs.status,
        runs.target_node_id,
        coalesce(
            nullif(left(btrim(runs.title), 255), ''),
            nullif(left(btrim(coalesce(
                runs.input_payload #>> '{node-start,query}',
                runs.input_payload #>> '{start,query}',
                runs.input_payload #>> '{query}',
                ''
            )), 255), ''),
            'Untitled run'
        ) as title,
        runs.external_user,
        users.account as authorized_account,
        runs.created_by,
        runs.api_key_id,
        api_keys.name as api_key_name_snapshot,
        runs.publication_version_id,
        runs.external_conversation_id,
        runs.external_trace_id,
        runs.compatibility_mode,
        runs.idempotency_key,
        coalesce(usage_metrics.total_tokens, node_metrics.total_tokens) as total_tokens,
        coalesce(usage_metrics.input_tokens, node_metrics.input_tokens) as input_tokens,
        coalesce(usage_metrics.output_tokens, node_metrics.output_tokens) as output_tokens,
        coalesce(
            usage_metrics.input_cache_hit_tokens,
            node_metrics.input_cache_hit_tokens
        ) as input_cache_hit_tokens,
        coalesce(node_metrics.unique_node_count, 0) as unique_node_count,
        coalesce(callback_metrics.tool_callback_count, 0) as tool_callback_count,
        runs.started_at,
        runs.finished_at,
        runs.created_at,
        runs.updated_at
    from missing_runs runs
    left join users on users.id = runs.created_by
    left join api_keys on api_keys.id = runs.api_key_id
    left join usage_metrics on usage_metrics.flow_run_id = runs.id
    left join node_metrics on node_metrics.flow_run_id = runs.id
    left join callback_metrics on callback_metrics.flow_run_id = runs.id
)
insert into application_run_log_summaries (
    flow_run_id,
    scope_id,
    application_id,
    run_mode,
    status,
    target_node_id,
    title,
    input_payload,
    external_user,
    created_by,
    authorized_account,
    api_key_id,
    api_key_name_snapshot,
    publication_version_id,
    external_conversation_id,
    external_trace_id,
    compatibility_mode,
    idempotency_key,
    total_tokens,
    input_tokens,
    output_tokens,
    input_cache_hit_tokens,
    input_cache_hit_rate,
    unique_node_count,
    tool_callback_count,
    started_at,
    finished_at,
    created_at,
    updated_at
)
select
    flow_run_id,
    scope_id,
    application_id,
    run_mode,
    status,
    target_node_id,
    title,
    '{}'::jsonb,
    external_user,
    created_by,
    authorized_account,
    api_key_id,
    api_key_name_snapshot,
    publication_version_id,
    external_conversation_id,
    external_trace_id,
    compatibility_mode,
    idempotency_key,
    total_tokens,
    input_tokens,
    output_tokens,
    input_cache_hit_tokens,
    case
        when input_cache_hit_tokens is not null
         and coalesce(input_tokens, 0) + input_cache_hit_tokens > 0
        then input_cache_hit_tokens::double precision
           / (coalesce(input_tokens, 0) + input_cache_hit_tokens)::double precision
        else null
    end,
    unique_node_count,
    tool_callback_count,
    started_at,
    finished_at,
    created_at,
    updated_at
from missing_summaries
on conflict (flow_run_id) do nothing;
