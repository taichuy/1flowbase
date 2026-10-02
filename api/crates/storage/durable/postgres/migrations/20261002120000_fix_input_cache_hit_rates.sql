-- Input cache rates use total input, including cached reads, and exclude output.
-- A reported cache miss count is authoritative for providers with exclusive input counts.
create or replace function application_run_log_usage_input_tokens(usage jsonb) returns bigint
language sql immutable as $$
    with counts as (
        select
            case when usage->>'input_tokens' ~ '^[0-9]+$' then (usage->>'input_tokens')::bigint end as input,
            case when usage->>'input_cache_miss_tokens' ~ '^[0-9]+$' then (usage->>'input_cache_miss_tokens')::bigint end as miss,
            coalesce(
                case when usage->>'input_cache_hit_tokens' ~ '^[0-9]+$' then (usage->>'input_cache_hit_tokens')::bigint end,
                case when usage->>'cache_read_tokens' ~ '^[0-9]+$' then (usage->>'cache_read_tokens')::bigint end,
                case when usage->>'cached_input_tokens' ~ '^[0-9]+$' then (usage->>'cached_input_tokens')::bigint end
            ) as hit,
            case when usage->>'cache_write_tokens' ~ '^[0-9]+$' then (usage->>'cache_write_tokens')::bigint end as writes
    )
    select case when miss is not null then miss + coalesce(hit,0) + coalesce(writes,0)
                else input end from counts;
$$;

create or replace function application_run_log_input_tokens(run_id uuid) returns bigint
language sql stable as $$
    select coalesce(
        (select sum(application_run_log_usage_input_tokens(jsonb_build_object(
            'input_tokens', input_tokens, 'input_cache_miss_tokens', input_cache_miss_tokens,
            'input_cache_hit_tokens', coalesce(input_cache_hit_tokens, cache_read_tokens, cached_input_tokens),
            'cache_write_tokens', cache_write_tokens
        )))::bigint from runtime_usage_ledger where flow_run_id = run_id),
        (select sum(application_run_log_usage_input_tokens(metrics_payload->'usage'))::bigint
         from node_run_records where flow_run_id = run_id)
    );
$$;

create or replace function application_run_log_task_refresh(anchor uuid) returns void
language sql as $$
    with members as (
        select s.*, f.log_context
        from application_run_log_summaries s
        join flow_runs f on f.id = s.flow_run_id
        where s.flow_run_id = $1 or s.log_task_run_id = $1
    ), anchor_row as (
        select * from members where flow_run_id = $1
    ), ordered as (
        select array_agg(flow_run_id order by started_at, flow_run_id) as member_run_ids from members
    ), aggregate as (
        select
            sum(invocation_count)::bigint as invocation_count,
            sum(compaction_count)::bigint as compaction_count,
            sum(total_tokens)::bigint as total_tokens,
            case when bool_and(finished_at is not null and total_cost is not null)
                 then sum(total_cost) end as total_cost,
            sum(input_tokens)::bigint as input_tokens,
            sum(output_tokens)::bigint as output_tokens,
            sum(input_cache_hit_tokens)::bigint as input_cache_hit_tokens,
            sum(unique_node_count)::bigint as unique_node_count,
            sum(tool_callback_count)::bigint as tool_callback_count,
            min(started_at) as started_at,
            min(created_at) as created_at,
            max(updated_at) as updated_at,
            case when bool_and(finished_at is not null) then max(finished_at) end as finished_at,
            (array_agg(status order by case status
                when 'running' then 0 when 'waiting_callback' then 1
                when 'waiting_human' then 2 when 'paused' then 3
                when 'queued' then 4 when 'failed' then 5
                when 'cancelled' then 6 when 'incomplete' then 7
                when 'succeeded' then 8 else 9 end,
                created_at desc, flow_run_id))[1] as status,
            bool_or(status in ('queued','running','waiting_callback','waiting_human','paused')) as active
        from members
    ), final_output as (
        select * from application_run_log_task_final_output((select member_run_ids from ordered))
    )
    insert into application_run_log_tasks (
        id, application_id, scope_id, member_run_ids, parent_task_run_id, log_conversation_id,
        client_thread_id, client_turn_id, subagent_kind, run_mode, status, outcome,
        user_input, final_output, final_output_run_id, target_node_id, title, external_user,
        created_by, authorized_account, api_key_id, api_key_name_snapshot, publication_version_id,
        external_conversation_id, external_trace_id, compatibility_mode, idempotency_key, call_kind,
        invocation_count, compaction_count, total_cost, total_tokens, input_tokens, output_tokens,
        input_cache_hit_tokens, input_cache_hit_rate, unique_node_count, tool_callback_count,
        started_at, finished_at, created_at, updated_at
    )
    select
        a.flow_run_id, a.application_id, a.scope_id, o.member_run_ids, a.parent_run_id, a.log_conversation_id,
        a.log_context ->> 'thread_id', a.log_context ->> 'turn_id', a.log_context ->> 'subagent_kind',
        a.run_mode, g.status,
        -- A later active call reopens the task even after an answer was observed;
        -- the observed answer stays on the row but does not claim completion.
        case when g.active then 'in_progress'
             when fo.content is not null then 'final_answer_observed'
             else 'no_final_answer' end,
        application_run_log_task_user_input(a.flow_run_id),
        fo.content, fo.run_id, a.target_node_id, a.title, a.external_user,
        a.created_by, a.authorized_account, a.api_key_id, a.api_key_name_snapshot, a.publication_version_id,
        a.external_conversation_id, a.external_trace_id, a.compatibility_mode, a.idempotency_key, a.call_kind,
        g.invocation_count, g.compaction_count, g.total_cost, g.total_tokens, g.input_tokens, g.output_tokens,
        g.input_cache_hit_tokens,
        case when g.input_tokens > 0
             then g.input_cache_hit_tokens::double precision / g.input_tokens::double precision end,
        g.unique_node_count, g.tool_callback_count,
        g.started_at, g.finished_at, g.created_at, g.updated_at
    from anchor_row a cross join ordered o cross join aggregate g left join final_output fo on true
    on conflict (id) do update set
        member_run_ids = excluded.member_run_ids, parent_task_run_id = excluded.parent_task_run_id,
        log_conversation_id = excluded.log_conversation_id, client_thread_id = excluded.client_thread_id,
        client_turn_id = excluded.client_turn_id, subagent_kind = excluded.subagent_kind,
        run_mode = excluded.run_mode, status = excluded.status, outcome = excluded.outcome,
        user_input = excluded.user_input, final_output = excluded.final_output,
        final_output_run_id = excluded.final_output_run_id, target_node_id = excluded.target_node_id,
        title = excluded.title, external_user = excluded.external_user, created_by = excluded.created_by,
        authorized_account = excluded.authorized_account, api_key_id = excluded.api_key_id,
        api_key_name_snapshot = excluded.api_key_name_snapshot, publication_version_id = excluded.publication_version_id,
        external_conversation_id = excluded.external_conversation_id, external_trace_id = excluded.external_trace_id,
        compatibility_mode = excluded.compatibility_mode, idempotency_key = excluded.idempotency_key,
        call_kind = excluded.call_kind, invocation_count = excluded.invocation_count,
        compaction_count = excluded.compaction_count, total_cost = excluded.total_cost,
        total_tokens = excluded.total_tokens,
        input_tokens = excluded.input_tokens, output_tokens = excluded.output_tokens,
        input_cache_hit_tokens = excluded.input_cache_hit_tokens, input_cache_hit_rate = excluded.input_cache_hit_rate,
        unique_node_count = excluded.unique_node_count, tool_callback_count = excluded.tool_callback_count,
        started_at = excluded.started_at, finished_at = excluded.finished_at,
        created_at = excluded.created_at, updated_at = excluded.updated_at;
$$;

-- Rebuild derived input counts only; raw usage and billing ledgers stay unchanged.
with corrected as (
    select flow_run_id, coalesce(application_run_log_input_tokens(flow_run_id), input_tokens) as input_tokens
    from application_run_log_summaries
)
update application_run_log_summaries s
set input_tokens = c.input_tokens,
    input_cache_hit_rate = case when c.input_tokens > 0
        then s.input_cache_hit_tokens::double precision / c.input_tokens::double precision end
from corrected c where c.flow_run_id = s.flow_run_id;

-- Task membership is already materialized; avoid rewriting status, output or cost snapshots.
with corrected as (
    select t.id, sum(s.input_tokens)::bigint as input_tokens
    from application_run_log_tasks t
    cross join lateral unnest(t.member_run_ids) member(run_id)
    join application_run_log_summaries s on s.flow_run_id = member.run_id
    group by t.id
)
update application_run_log_tasks t
set input_tokens = c.input_tokens,
    input_cache_hit_rate = case when c.input_tokens > 0
        then t.input_cache_hit_tokens::double precision / c.input_tokens::double precision end
from corrected c where c.id = t.id;

-- Use the original attempt's usage when present to retain exclusive-input provider semantics.
with corrected as (
    select l.id, coalesce(
        application_run_log_usage_input_tokens(
            coalesce(n.metrics_payload->'attempts'->(l.attempt_index - 1)->'usage',
                case when l.attempt_index = 1 then n.metrics_payload->'usage' end)
        ),
        l.input_tokens
    ) as input_tokens
    from model_provider_request_logs l
    left join node_run_records n on n.id = l.node_run_id
)
update model_provider_request_logs l
set input_cache_hit_rate = case when c.input_tokens > 0
    then round((l.input_cache_hit_tokens::numeric / c.input_tokens::numeric) * 10000) / 10000 end
from corrected c where c.id = l.id;
