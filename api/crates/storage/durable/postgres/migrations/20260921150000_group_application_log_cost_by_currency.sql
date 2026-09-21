-- Preserve existing snapshots without consulting mutable billing/request logs.
-- Currency-separated decimal strings keep their precision through JSON consumers.
alter table application_run_log_summaries rename column total_cost to cost_breakdown;
alter table application_run_log_summaries alter column cost_breakdown type jsonb using
    case when cost_breakdown is not null and currency_code is not null
         then jsonb_build_array(jsonb_build_object('currency_code', currency_code, 'total_cost', cost_breakdown)) end;
alter table application_run_log_tasks rename column total_cost to cost_breakdown;
alter table application_run_log_tasks alter column cost_breakdown type jsonb using
    case when cost_breakdown is not null and currency_code is not null
         then jsonb_build_array(jsonb_build_object('currency_code', currency_code, 'total_cost', cost_breakdown)) end;

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
    ), cost_totals as (
        select cost ->> 'currency_code' as currency_code,
               sum((cost ->> 'total_cost')::numeric)::text as total_cost
        from members cross join lateral jsonb_array_elements(cost_breakdown) cost
        group by cost ->> 'currency_code'
    ), cost_snapshot as (
        select case when (select bool_and(finished_at is not null and cost_breakdown is not null) from members)
                    then jsonb_agg(jsonb_build_object('currency_code', currency_code, 'total_cost', total_cost)
                                   order by currency_code) end as cost_breakdown
        from cost_totals
    ), aggregate as (
        select
            sum(invocation_count)::bigint as invocation_count,
            sum(compaction_count)::bigint as compaction_count,
            sum(total_tokens)::bigint as total_tokens,
            (select cost_breakdown from cost_snapshot) as cost_breakdown,
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
        invocation_count, compaction_count, cost_breakdown, total_tokens, input_tokens, output_tokens,
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
        g.invocation_count, g.compaction_count, g.cost_breakdown, g.total_tokens, g.input_tokens, g.output_tokens,
        g.input_cache_hit_tokens,
        case when coalesce(g.input_tokens,0) + g.input_cache_hit_tokens > 0
             then g.input_cache_hit_tokens::double precision / (coalesce(g.input_tokens,0) + g.input_cache_hit_tokens)::double precision end,
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
        compaction_count = excluded.compaction_count, cost_breakdown = excluded.cost_breakdown,
        total_tokens = excluded.total_tokens,
        input_tokens = excluded.input_tokens, output_tokens = excluded.output_tokens,
        input_cache_hit_tokens = excluded.input_cache_hit_tokens, input_cache_hit_rate = excluded.input_cache_hit_rate,
        unique_node_count = excluded.unique_node_count, tool_callback_count = excluded.tool_callback_count,
        started_at = excluded.started_at, finished_at = excluded.finished_at,
        created_at = excluded.created_at, updated_at = excluded.updated_at;
$$;

alter table application_run_log_summaries drop column currency_code;
alter table application_run_log_tasks drop column currency_code;

-- Change only the system-owned field contract; preserve the amount field's
-- existing title, description, exposure and display preferences.
update model_fields f set code = 'cost_breakdown', physical_column_name = 'cost_breakdown', field_kind = 'json'
from model_definitions d
where f.data_model_id = d.id and d.scope_kind = 'system'
    and d.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
    and d.code in ('application_run_log_summaries', 'application_run_log_tasks')
    and f.code = 'total_cost' and f.is_system;
delete from model_fields f using model_definitions d
where f.data_model_id = d.id and d.scope_kind = 'system'
    and d.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
    and d.code in ('application_run_log_summaries', 'application_run_log_tasks')
    and f.code = 'currency_code' and f.is_system;

-- Existing task members may each have a different, already recorded currency.
select application_run_log_task_refresh(anchor)
from (select distinct coalesce(log_task_run_id, flow_run_id) as anchor from application_run_log_summaries) anchors;
