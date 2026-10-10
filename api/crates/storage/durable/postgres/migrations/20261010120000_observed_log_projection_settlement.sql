-- Settled projections report observed amounts, not a completed billing claim.
-- Missing observations remain unknown; SUM retains known facts without inventing zero.
alter table application_run_log_tasks add column projection_output_source text;
create or replace function maintain_application_log_projection_settlement() returns trigger
language plpgsql as $$
begin
    if new.status <> 'waiting_callback' then
        new.projection_deadline_at := null;
        new.projection_settled_at := null;
        new.projection_output := null;
        new.projection_output_source := null;
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
        new.projection_output_source := null;
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
            nullif(btrim(new.final_output),''));
        new.projection_output_source := case when new.projection_output is null then 'timeout' else 'observed_output' end;
        new.projection_output := coalesce(new.projection_output, 'Timeout');
        -- Recompute, never add to the previous snapshot. Terminal member costs
        -- keep durable snapshots when present; missing snapshots use observed ledger facts.
        select sum(cost) into new.total_cost
        from (
            select case when s.finished_at is not null and s.total_cost is not null then s.total_cost
                else (select sum(normalized_cost)
                    from runtime_cost_ledger l where l.flow_run_id=s.flow_run_id) end as cost
            from application_run_log_summaries s where s.flow_run_id=any(new.member_run_ids)
        ) costs;
        select sum(input_tokens),
            sum(output_tokens),
            sum(total_tokens)
        into new.input_tokens,new.output_tokens,new.total_tokens
        from (
            select case when usage.n>0 then usage.input_tokens else s.input_tokens end as input_tokens,
                case when usage.n>0 then usage.output_tokens else s.output_tokens end as output_tokens,
                case when usage.n>0 then usage.total_tokens else s.total_tokens end as total_tokens
            from application_run_log_summaries s
            cross join lateral (
                select count(*) as n,
                    sum(input_tokens) as input_tokens,
                    sum(output_tokens) as output_tokens,
                    sum(total_tokens) as total_tokens
                from runtime_usage_ledger u where u.flow_run_id=s.flow_run_id
            ) usage
            where s.flow_run_id=any(new.member_run_ids)
        ) observed_usage;
    end if;
    return new;
end $$;

-- Recompute previously settled read models from retained facts. No execution,
-- callback, usage or cost ledger row is mutated; the existing deadline is retained.
update application_run_log_tasks set projection_settled_at=projection_settled_at
where status='waiting_callback' and projection_settled_at is not null;
