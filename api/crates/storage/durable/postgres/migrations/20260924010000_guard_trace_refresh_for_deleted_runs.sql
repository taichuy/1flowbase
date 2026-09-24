create or replace function enqueue_application_run_trace_refresh() returns trigger language plpgsql as $$
declare run_id uuid;
begin
 if TG_TABLE_NAME in ('flow_runs', 'application_run_log_tasks') then run_id := NEW.id;
 else run_id := NEW.flow_run_id;
 end if;
 -- Token deltas do not invalidate the structural tree. Completion/integrity does.
 if TG_TABLE_NAME = 'runtime_events' then
  if NEW.event_type not in ('provider_output_item_done','provider_protocol_integrity','visible_internal_llm_tool_completed','visible_internal_llm_tool_failed') then return NEW; end if;
 end if;
 -- Cascaded task updates can run after the parent flow_run has been deleted.
 insert into application_run_trace_refresh_queue(flow_run_id)
 select flow_runs.id from flow_runs where flow_runs.id = run_id
 on conflict(flow_run_id) do update set revision=application_run_trace_refresh_queue.revision+1;
 return NEW;
end $$;
