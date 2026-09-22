-- A read-model deadline, deliberately independent of workflow/callback lifecycle.
alter table application_run_log_tasks
    add column projection_deadline_at timestamptz,
    add column projection_settled_at timestamptz,
    add column projection_output text;
create index application_run_log_tasks_projection_due
    on application_run_log_tasks(projection_deadline_at, id)
    where status='waiting_callback' and projection_settled_at is null;

-- Only text actually emitted to the client is eligible. Tool/reasoning records
-- and client-submitted history must never become an assistant reply.
create function application_run_log_task_last_client_text(members uuid[]) returns text
language sql stable as $$
    select body.content
    from unnest(members) with ordinality member(run_id, position)
    join client_trajectory_steps s on s.flow_run_id=member.run_id
        and s.metadata->>'category'='assistant' and s.metadata->>'origin'='emitted'
    join client_trajectory_sections c on c.flow_run_id=s.flow_run_id
        and c.step_id=s.id and c.section='result'
    join runtime_events e on e.id=c.event_id
    cross join lateral (select runtime_original_json(e.payload,e.raw_json_payloads,'payload')
        #> '{fact,value}' as value) raw
    cross join lateral (select case json_typeof(raw.value)
        when 'string' then raw.value #>> '{}'
        when 'array' then (select string_agg(case part->>'type'
                when 'output_text' then part->>'text'
                when 'text' then part->>'text'
                when 'refusal' then part->>'refusal' end, '' order by position)
            from json_array_elements(raw.value) with ordinality p(part,position))
        end as content) body
    where nullif(btrim(body.content),'') is not null
    order by member.position desc,c.event_sequence desc limit 1
$$;

create function maintain_application_log_projection_settlement() returns trigger
language plpgsql as $$
begin
    if new.status <> 'waiting_callback' then
        new.projection_deadline_at := null;
        new.projection_settled_at := null;
        new.projection_output := null;
        return new;
    end if;
    if tg_op='INSERT' then
        new.projection_deadline_at := clock_timestamp()+interval '5 minutes';
    elsif old.status <> 'waiting_callback'
        or new.member_run_ids is distinct from old.member_run_ids
        or new.invocation_count is distinct from old.invocation_count
        or new.tool_callback_count is distinct from old.tool_callback_count then
        -- Actual execution progress starts another waiting episode. A repeated
        -- projector write / usage correction / page read cannot extend it.
        new.projection_deadline_at := clock_timestamp()+interval '5 minutes';
        new.projection_settled_at := null;
        new.projection_output := null;
    end if;
    if new.projection_settled_at is not null then
        new.projection_output := coalesce(application_run_log_task_last_client_text(new.member_run_ids),
            (select nullif(btrim(m.content),'') from application_run_conversation_message_items m
                join unnest(new.member_run_ids) with ordinality member(run_id,position) on member.run_id=m.flow_run_id
                where m.role='assistant' and m.output_source='provider_output_item'
                    and m.native_message #>> '{_source_item,type}'='message'
                    and m.native_message->>'_log_conflicting' is distinct from 'true'
                    and nullif(btrim(m.content),'') is not null
                order by member.position desc,m.display_sequence desc limit 1),
            nullif(btrim(new.final_output),''), 'Timeout');
        -- Recompute, never add to the previous snapshot. Terminal member costs
        -- already have durable snapshots; active members use observed ledger facts.
        select case when count(cost)=count(*) then sum(cost) end into new.total_cost
        from (
            select case when s.finished_at is not null then s.total_cost
                else (select case when count(normalized_cost)=count(*) then sum(normalized_cost) end
                    from runtime_cost_ledger l where l.flow_run_id=s.flow_run_id) end as cost
            from application_run_log_summaries s where s.flow_run_id=any(new.member_run_ids)
        ) costs;
        select case when count(input_tokens)=count(*) then sum(input_tokens) end,
            case when count(output_tokens)=count(*) then sum(output_tokens) end,
            case when count(total_tokens)=count(*) then sum(total_tokens) end
        into new.input_tokens,new.output_tokens,new.total_tokens
        from (
            select case when usage.n>0 then usage.input_tokens else s.input_tokens end as input_tokens,
                case when usage.n>0 then usage.output_tokens else s.output_tokens end as output_tokens,
                case when usage.n>0 then usage.total_tokens else s.total_tokens end as total_tokens
            from application_run_log_summaries s
            cross join lateral (
                select count(*) as n,
                    case when count(input_tokens)=count(*) then sum(input_tokens) end as input_tokens,
                    case when count(output_tokens)=count(*) then sum(output_tokens) end as output_tokens,
                    case when count(total_tokens)=count(*) then sum(total_tokens) end as total_tokens
                from runtime_usage_ledger u where u.flow_run_id=s.flow_run_id
            ) usage
            where s.flow_run_id=any(new.member_run_ids)
        ) observed_usage;
    end if;
    return new;
end $$;
create trigger application_log_projection_settlement
    before insert or update on application_run_log_tasks
    for each row execute function maintain_application_log_projection_settlement();

-- Existing waiting rows retain their last recorded progress time, not migration time.
update application_run_log_tasks set projection_deadline_at=updated_at+interval '5 minutes'
    where status='waiting_callback';

create function settle_next_application_log_projection() returns boolean
language plpgsql as $$
declare task_id uuid;
begin
    select id into task_id from application_run_log_tasks
    where status='waiting_callback' and projection_settled_at is null
        and projection_deadline_at <= now()
    order by projection_deadline_at,id for update skip locked limit 1;
    if task_id is null then return false; end if;
    -- Lock only the projection row. Never write flow/node/callback/ledger tables.
    update application_run_log_tasks set projection_settled_at=clock_timestamp()
        where id=task_id;
    return true;
end $$;
