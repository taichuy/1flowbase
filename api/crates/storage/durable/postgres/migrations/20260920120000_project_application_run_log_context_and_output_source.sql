-- #2090: the run log read model distinguishes where the visible output came
-- from, which layer contributed a system/developer context entry, and which
-- run watermark the current rows were projected from.
--
-- All three are derived facts about the same original events and payloads. No
-- original event, request, answer or application configuration is rewritten by
-- this migration, and no historical model call is replayed.
alter table application_run_conversation_message_items
    add column output_source text
        check (output_source in ('provider_output_item','persisted_answer','error','none')),
    add column context_source text
        check (context_source in ('client_request','application_config','effective_prompt')),
    add column source_revision text;

-- Existing rows keep a null revision, so the first read of a run reprojects it
-- under the new writer instead of serving a partially described projection.
create index application_run_conversation_message_items_revision_idx
    on application_run_conversation_message_items (
        application_id,
        flow_run_id,
        projection_version,
        source_revision
    );

-- One watermark per run covers every fact the run projection derives from:
-- member status and payloads, retained request context, retained tool results,
-- formal provider output items and the persisted canonical answer.
--
-- Streaming deltas (`text_delta`, `tool_call_delta`) are deliberately excluded.
-- They never change the projection, so counting them would force a rebuild on
-- every token instead of on every completed fact.
create function application_run_message_projection_watermark(run_id uuid) returns text
language sql stable as $$
    with anchor as (
        select id, application_id, status, updated_at, finished_at,
               log_context, input_payload, output_payload
        from flow_runs
        where id = $1
    ), members as (
        select id, status, updated_at, finished_at, log_context, input_payload, output_payload
        from anchor
        union
        select f.id, f.status, f.updated_at, f.finished_at,
               f.log_context, f.input_payload, f.output_payload
        from anchor a
        join application_run_log_conversation_runs(
            a.application_id, (a.log_context->>'log_conversation_id')::uuid) member on true
        join flow_runs f on f.id = member.run_id
    ), facts as (
        select
            m.id::text
            || ':' || m.status
            || ':' || extract(epoch from m.updated_at)::text
            || ':' || coalesce(extract(epoch from m.finished_at)::text, '-')
            || ':' || md5(coalesce(m.output_payload->>'answer', '-'))
            || ':' || md5(coalesce((m.log_context->'tool_results')::text, '-'))
            || ':' || coalesce(m.log_context->>'log_task_run_id', '-')
            || ':' || md5(coalesce((m.log_context->'prompt')::text, '-'))
            || ':' || md5(coalesce((m.input_payload->'__native_model_prompt_context')::text, '-'))
            || ':' || md5(coalesce(m.input_payload->>'system', '-'))
            || ':' || md5(coalesce(m.input_payload#>>'{node-start,system}', '-'))
            || ':' || md5(coalesce(m.input_payload#>>'{start,system}', '-'))
            || ':' || md5(coalesce(m.input_payload->>'query', '-'))
            || ':' || md5(coalesce(m.input_payload#>>'{node-start,query}', '-'))
            || ':' || md5(coalesce(m.input_payload#>>'{start,query}', '-'))
            as part
        from members m
        union all
        select
            e.flow_run_id::text
            || ':output_item_done:'
            || count(*)::text
            || ':' || coalesce(max(e.sequence), 0)::text
        from runtime_events e
        join members m on m.id = e.flow_run_id
        where e.event_type = 'provider_output_item_done'
        group by e.flow_run_id
    )
    select case
        when exists (select 1 from anchor)
        then md5(string_agg(part, '|' order by part))
    end
    from facts
$$;

-- A task's user input must not depend on the run projection being built yet:
-- a call that is still running already retains its request. The retained client
-- prompt is a structured message whose content may be a string or content
-- parts, so both shapes are flattened here.
create or replace function application_run_log_task_user_input(run_id uuid) returns text
language sql stable as $$
    select coalesce(
        (
            select m.content
            from application_run_conversation_message_items m
            where m.flow_run_id = run_id
              and m.role = 'user'
            order by m.display_sequence
            limit 1
        ),
        (
            select m.query
            from application_run_conversation_message_items m
            where m.flow_run_id = run_id
              and m.query is not null
            order by m.display_sequence
            limit 1
        ),
        case
            when jsonb_typeof(f.log_context #> '{prompt,content}') = 'string'
            then nullif(btrim(f.log_context #>> '{prompt,content}'), '')
        end,
        case
            when jsonb_typeof(f.log_context #> '{prompt,content}') = 'array'
            then nullif(btrim((
                select string_agg(
                    coalesce(part->>'text', part->>'content', ''), '' order by ordinality)
                from jsonb_array_elements(f.log_context #> '{prompt,content}')
                    with ordinality element(part, ordinality)
            )), '')
        end
    )
    from flow_runs f
    where f.id = run_id
$$;

-- The final answer of a task is the last assistant message projected for any
-- member call that the client marked as a final answer, the persisted answer a
-- call stored without projecting a provider message, or the run answer of a
-- call without native output. Reading it never rebuilds projections.
create or replace function application_run_log_task_final_output(members uuid[])
returns table(run_id uuid, content text)
language sql stable as $$
    select m.flow_run_id, coalesce(m.content, m.answer)
    from application_run_conversation_message_items m
    join unnest($1) with ordinality member(run_id, position) on member.run_id = m.flow_run_id
    where (m.role = 'assistant' and m.native_message #>> '{_source_item,phase}' = 'final_answer')
       or m.output_source = 'persisted_answer'
       or (m.role is null and m.answer is not null and m.native_message is null)
    order by member.position desc, m.display_sequence desc
    limit 1
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
        invocation_count, compaction_count, total_tokens, input_tokens, output_tokens,
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
        g.invocation_count, g.compaction_count, g.total_tokens, g.input_tokens, g.output_tokens,
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
        compaction_count = excluded.compaction_count, total_tokens = excluded.total_tokens,
        input_tokens = excluded.input_tokens, output_tokens = excluded.output_tokens,
        input_cache_hit_tokens = excluded.input_cache_hit_tokens, input_cache_hit_rate = excluded.input_cache_hit_rate,
        unique_node_count = excluded.unique_node_count, tool_callback_count = excluded.tool_callback_count,
        started_at = excluded.started_at, finished_at = excluded.finished_at,
        created_at = excluded.created_at, updated_at = excluded.updated_at;
$$;

-- Recompute every existing task row with the retained-prompt fallback so a run
-- that is still in flight already reports what the user asked.
select application_run_log_task_refresh(anchor)
from (select distinct coalesce(log_task_run_id, flow_run_id) as anchor from application_run_log_summaries) anchors;
