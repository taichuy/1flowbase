-- Preserve terminal snapshots without assuming every terminal writer already captured one.
create or replace function maintain_application_log_projection_settlement() returns trigger
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
        -- keep durable snapshots when present; missing snapshots use observed ledger facts.
        select case when count(cost)=count(*) then sum(cost) end into new.total_cost
        from (
            select case when s.finished_at is not null and s.total_cost is not null then s.total_cost
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

-- Repair only missing terminal snapshots backed by a complete retained ledger.
-- No flow/node/callback/billing writes; do not invent zero or overwrite known cost.
with repaired as (
    update application_run_log_summaries s set total_cost=ledger.total_cost
    from (
        select flow_run_id,sum(normalized_cost) as total_cost from runtime_cost_ledger
        group by flow_run_id having count(*)>0 and count(normalized_cost)=count(*)
    ) ledger
    where s.flow_run_id=ledger.flow_run_id and s.finished_at is not null and s.total_cost is null
    returning coalesce(s.log_task_run_id,s.flow_run_id) as anchor
)
select application_run_log_task_refresh(anchor) from (select distinct anchor from repaired) tasks;
