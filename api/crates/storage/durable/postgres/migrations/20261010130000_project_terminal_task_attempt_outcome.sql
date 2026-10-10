-- Task status describes the latest business attempt after all work is terminal.
-- Failed attempts remain unchanged in the source runs and visible in the trace.
create or replace function application_run_log_task_refresh(anchor uuid) returns void
language sql as $$
    with members as (
        select s.*, f.log_context,
            coalesce(s.total_cost, (select sum(l.normalized_cost)
                from runtime_cost_ledger l where l.flow_run_id=s.flow_run_id)) as observed_cost
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
            -- Terminal snapshots report known amounts across all attempts, including
            -- failed retries. All unknown stays NULL; active tasks stay unsettled.
            case when bool_and(finished_at is not null)
                 then sum(observed_cost) end as total_cost,
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
                when 'queued' then 4 else 5 end,
                -- Token counting or compaction must not mask the result of
                -- a generation attempt. Fall back naturally for standalone calls.
                case when call_kind='generate' then 0 else 1 end,
                started_at desc, flow_run_id desc))[1] as status,
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

-- Repair native terminal task projections only. No source run, callback,
-- usage/cost ledger, imported record or active task is rewritten.
select application_run_log_task_refresh(id) from application_run_log_tasks
where source_kind='native' and finished_at is not null;
