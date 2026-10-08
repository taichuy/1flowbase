-- Native records still obtain request snapshots from their anchor summary.
-- Imported records have no native summary; their canonical source facts own
-- model and reasoning fields, which must survive every insert/update.
create or replace function application_run_log_task_request_snapshot_sync()
returns trigger
language plpgsql as $$
begin
    if new.source_kind = 'native' then
        select summaries.requested_model_id, summaries.reasoning_effort
        into new.requested_model_id, new.reasoning_effort
        from application_run_log_summaries summaries
        where summaries.flow_run_id = new.id;
    end if;
    return new;
end
$$;
