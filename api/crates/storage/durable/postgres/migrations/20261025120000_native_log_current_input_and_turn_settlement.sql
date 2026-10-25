-- A task input is the current request, never imported model history.
-- Ignore empty legacy projection placeholders before consulting retained input.
create or replace function application_run_log_task_user_input(run_id uuid) returns text
language sql stable as $$
    select coalesce(
        (select m.query from application_run_conversation_message_items m
         where m.flow_run_id=$1 and m.is_current and nullif(btrim(m.query),'') is not null
         order by m.display_sequence desc limit 1),
        (select m.content from application_run_conversation_message_items m
         where m.flow_run_id=$1 and m.is_current and m.role='user'
           and nullif(btrim(m.content),'') is not null
         order by m.display_sequence limit 1),
        case when jsonb_typeof(f.log_context #> '{prompt,content}')='string'
             and nullif(btrim(f.log_context #>> '{prompt,content}'),'') is not null
             then f.log_context #>> '{prompt,content}' end,
        (select string_agg(part->>'text', E'\n' order by position)
         from jsonb_array_elements(case
             when jsonb_typeof(f.log_context #> '{prompt,content}')='array'
             then f.log_context #> '{prompt,content}' else '[]'::jsonb end)
             with ordinality blocks(part,position)
         where jsonb_typeof(part->'text')='string'
         having nullif(btrim(string_agg(part->>'text', E'\n' order by position)),'') is not null),
        -- Same current-input keys used by the native run projection. A missing
        -- log prompt must not make us read system text or conversation history.
        (select value #>> '{}' from (
            select source, key_position, source_value->key as value
            from (values (0,f.input_payload),
                (1,coalesce(f.input_payload->'node-start',f.input_payload->'start')))
                inputs(source,source_value)
            cross join unnest(array['query','question','prompt','message','input','input_text'])
                with ordinality keys(key,key_position)
         ) candidates where jsonb_typeof(value)='string'
             and nullif(btrim(value #>> '{}'),'') is not null
         order by source,key_position limit 1)
    ) from flow_runs f where f.id=$1
$$;

-- Business-turn completion and individual execution/callback state are separate.
-- Only a proven older callback wait in the same declared client turn is excluded
-- from the status reduction. All source facts and all observed costs remain.
create or replace function application_run_log_task_refresh(anchor uuid) returns void
language sql as $$
    with members as (
        select s.*, f.log_context, f.status as source_status,
            f.started_at as source_started_at, f.finished_at as source_finished_at,
            f.api_key_id as source_api_key_id, f.external_user as source_external_user,
            coalesce(s.total_cost, (select sum(l.normalized_cost)
                from runtime_cost_ledger l where l.flow_run_id=s.flow_run_id)) as observed_cost
        from application_run_log_summaries s
        join flow_runs f on f.id = s.flow_run_id
        where s.flow_run_id = $1 or s.log_task_run_id = $1
    ), anchor_row as (
        select * from members where flow_run_id = $1
    ), ordered as (
        select array_agg(flow_run_id order by started_at, flow_run_id) as member_run_ids from members
    ), final_output as (
        select * from application_run_log_task_final_output((select member_run_ids from ordered))
    ), final_member as (
        select f.* from members f join final_output fo on fo.run_id=f.flow_run_id
        cross join anchor_row a
        where f.status='succeeded' and f.source_status='succeeded'
          and f.call_kind='generate' and f.source_finished_at is not null
          and nullif(btrim(fo.content),'') is not null
          and f.log_context->>'identity_status'='identified'
          and a.log_context->>'identity_status'='identified'
          and nullif(f.log_context->>'thread_id','') is not null
          and nullif(f.log_context->>'turn_id','') is not null
          and f.log_context->>'thread_id'=a.log_context->>'thread_id'
          and f.log_context->>'turn_id'=a.log_context->>'turn_id'
          and f.log_context->>'protocol'=a.log_context->>'protocol'
          and f.log_conversation_id=a.log_conversation_id
          and f.application_id=a.application_id and f.scope_id=a.scope_id
          and f.source_api_key_id is not distinct from a.source_api_key_id
          and f.source_external_user is not distinct from a.source_external_user
    ), effective_members as (
        select w.*, exists (
            select 1 from final_member f
            where w.status='waiting_callback' and w.source_status='waiting_callback'
              and w.call_kind='generate' and w.source_finished_at is null
              and w.source_started_at < f.source_started_at
              and w.log_context->>'identity_status'='identified'
              and w.log_context->>'thread_id'=f.log_context->>'thread_id'
              and w.log_context->>'turn_id'=f.log_context->>'turn_id'
              and w.log_context->>'protocol'=f.log_context->>'protocol'
              and w.log_conversation_id=f.log_conversation_id
              and w.application_id=f.application_id and w.scope_id=f.scope_id
              and w.source_api_key_id is not distinct from f.source_api_key_id
              and w.source_external_user is not distinct from f.source_external_user
              and exists (select 1 from flow_run_callback_tasks c
                  where c.flow_run_id=w.flow_run_id and c.status='pending')
              -- Completed earlier callback rounds are normal history. No
              -- pending round may have been received/resumed in the meantime.
              and not exists (select 1 from flow_run_callback_tasks c
                  where c.flow_run_id=w.flow_run_id and (
                      c.created_at >= f.source_started_at
                      or c.status not in ('pending','completed')
                      or (c.status='pending' and c.completed_at is not null)
                      or (c.status='completed' and
                          (c.completed_at is null or c.completed_at >= f.source_started_at))))
              and not exists (select 1 from flow_run_tool_callback_inbox i
                  join flow_run_callback_tasks c on c.id=i.callback_task_id
                  where i.flow_run_id=w.flow_run_id and (
                      i.created_at >= f.source_started_at or i.updated_at >= f.source_started_at
                      or i.received_at >= f.source_started_at
                      or (c.status='pending' and i.status<>'pending')
                      or (c.status='completed' and
                          (i.status<>'received' or i.received_at is null))))
              and not exists (select 1 from flow_run_callback_resume_attempts r
                  join flow_run_callback_tasks c on c.id=r.callback_task_id
                  where r.flow_run_id=w.flow_run_id and (
                      c.status='pending' or r.status in ('received','processing')
                      or r.completed_at is null
                      or greatest(r.created_at,r.updated_at,r.completed_at) >= f.source_started_at))
              and not exists (select 1 from flow_run_resume_claims r
                  left join flow_run_callback_tasks c on c.id=r.callback_task_id
                  where r.flow_run_id=w.flow_run_id and (
                      r.resume_kind<>'callback' or c.status='pending' or r.status='processing'
                      or r.completed_at is null
                      or greatest(r.created_at,r.updated_at,r.completed_at) >= f.source_started_at))
        ) as superseded_wait
        from members w
    ), aggregate as (
        select
            sum(invocation_count)::bigint as invocation_count,
            sum(compaction_count)::bigint as compaction_count,
            sum(total_tokens)::bigint as total_tokens,
            -- Terminal snapshots report known amounts across all attempts, including
            -- failed retries. All unknown stays NULL; active tasks stay unsettled.
            case when bool_and(finished_at is not null or superseded_wait)
                 then sum(observed_cost) end as total_cost,
            sum(input_tokens)::bigint as input_tokens,
            sum(output_tokens)::bigint as output_tokens,
            sum(input_cache_hit_tokens)::bigint as input_cache_hit_tokens,
            sum(unique_node_count)::bigint as unique_node_count,
            sum(tool_callback_count)::bigint as tool_callback_count,
            min(started_at) as started_at,
            min(created_at) as created_at,
            max(updated_at) as updated_at,
            case when bool_and(finished_at is not null or superseded_wait) then max(finished_at) end as finished_at,
            (array_agg(status order by case status
                when 'running' then 0 when 'waiting_callback' then 1
                when 'waiting_human' then 2 when 'paused' then 3
                when 'queued' then 4 else 5 end,
                -- Token counting or compaction must not mask the result of
                -- a generation attempt. Fall back naturally for standalone calls.
                case when call_kind='generate' then 0 else 1 end,
                started_at desc, flow_run_id desc) filter (where not superseded_wait))[1] as status,
            bool_or(not superseded_wait and status in ('queued','running','waiting_callback','waiting_human','paused')) as active
        from effective_members
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
        -- Any unproven or later active work still reopens the task. The
        -- successful same-turn final only dominates proven older callback waits.
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


-- Receipt/claim changes are real progress even before flow_runs moves to running.
-- Refresh only this derived owner, in the source transaction; never mutate the
-- source run, callback, delivery or accounting facts from this trigger.
create function application_run_log_callback_progress_refresh() returns trigger
language plpgsql as $$
declare anchor uuid;
begin
    select coalesce(log_task_run_id,flow_run_id) into anchor
    from application_run_log_summaries where flow_run_id=new.flow_run_id;
    if anchor is not null then
        perform application_run_log_task_refresh(anchor);
    end if;
    return new;
end $$;

create trigger application_run_log_callback_progress
    after insert or update of status,created_at,completed_at on flow_run_callback_tasks
    for each row execute function application_run_log_callback_progress_refresh();
create trigger application_run_log_inbox_progress
    after insert or update of status,created_at,received_at,updated_at on flow_run_tool_callback_inbox
    for each row execute function application_run_log_callback_progress_refresh();
create trigger application_run_log_resume_progress
    after insert or update of status,created_at,updated_at,completed_at on flow_run_callback_resume_attempts
    for each row execute function application_run_log_callback_progress_refresh();
create trigger application_run_log_claim_progress
    after insert or update of status,created_at,updated_at,completed_at on flow_run_resume_claims
    for each row execute function application_run_log_callback_progress_refresh();

-- Repair only derived native tasks from retained facts. A GET never writes.
select application_run_log_task_refresh(id) from application_run_log_tasks
where source_kind='native' and (
    nullif(btrim(user_input),'') is null
    or (status='waiting_callback' and final_output is not null));
