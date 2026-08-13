alter table flow_run_checkpoints
    add constraint flow_run_checkpoints_id_flow_unique unique (id, flow_run_id);

alter table flow_run_callback_tasks
    add constraint flow_run_callback_tasks_id_flow_unique unique (id, flow_run_id);

alter table flow_run_resume_claims
    add constraint flow_run_resume_claims_checkpoint_owner_fk
        foreign key (checkpoint_id, flow_run_id)
        references flow_run_checkpoints(id, flow_run_id) on delete cascade,
    add constraint flow_run_resume_claims_callback_owner_fk
        foreign key (callback_task_id, flow_run_id)
        references flow_run_callback_tasks(id, flow_run_id) on delete cascade;

create function enforce_flow_run_recovery_transition()
returns trigger language plpgsql as $$
declare
    previous_state text;
begin
    if exists (
        select 1 from flow_run_recovery_history
         where flow_run_id = new.flow_run_id
           and idempotency_key = new.idempotency_key
    ) then
        return new;
    end if;
    select state_code into previous_state
      from flow_run_recovery_history
     where flow_run_id = new.flow_run_id
     order by sequence desc, id desc
     limit 1;

    if previous_state is null then
        return new;
    end if;

    if not (
        (previous_state = 'running' and new.state_code in (
            'waiting_callback', 'waiting_human', 'paused', 'retrying',
            'succeeded', 'failed', 'cancelled'
        ))
        or (previous_state in ('waiting_callback', 'waiting_human') and new.state_code in (
            'running', 'retrying', 'succeeded', 'failed', 'cancelled'
        ))
        or (previous_state = 'paused' and new.state_code in (
            'running', 'succeeded', 'failed', 'cancelled'
        ))
        or (previous_state = 'retrying' and new.state_code in (
            'running', 'waiting_callback', 'waiting_human',
            'succeeded', 'failed', 'cancelled'
        ))
    ) then
        raise exception 'illegal flow run recovery transition: % -> %',
            previous_state, new.state_code;
    end if;
    return new;
end;
$$;

create trigger flow_run_recovery_history_validate_transition
before insert on flow_run_recovery_history
for each row execute function enforce_flow_run_recovery_transition();
