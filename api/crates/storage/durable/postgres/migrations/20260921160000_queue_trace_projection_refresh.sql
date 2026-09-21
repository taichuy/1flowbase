-- Projection work is scheduled by fact writers, never by a log GET.
create table application_run_trace_refresh_queue (
 flow_run_id uuid primary key references flow_runs(id) on delete cascade,
 revision bigint not null default 1,
 available_at timestamptz not null default now(),
 lease_until timestamptz,
 attempts integer not null default 0
);
create index application_run_trace_refresh_ready on application_run_trace_refresh_queue(available_at, flow_run_id);
create function enqueue_application_run_trace_refresh() returns trigger language plpgsql as $$
declare run_id uuid;
begin
 if TG_TABLE_NAME in ('flow_runs', 'application_run_log_tasks') then run_id := NEW.id;
 else run_id := NEW.flow_run_id;
 end if;
 -- Token deltas do not invalidate the structural tree. Completion/integrity does.
 if TG_TABLE_NAME = 'runtime_events' then
  if NEW.event_type not in ('provider_output_item_done','provider_protocol_integrity','visible_internal_llm_tool_completed','visible_internal_llm_tool_failed') then return NEW; end if;
 end if;
 insert into application_run_trace_refresh_queue(flow_run_id) values(run_id)
 on conflict(flow_run_id) do update set revision=application_run_trace_refresh_queue.revision+1;
 return NEW;
end $$;
create trigger trace_refresh_flow after insert or update on flow_runs for each row execute function enqueue_application_run_trace_refresh();
create trigger trace_refresh_node after insert or update on node_runs for each row execute function enqueue_application_run_trace_refresh();
create trigger trace_refresh_callback after insert or update on flow_run_callback_tasks for each row execute function enqueue_application_run_trace_refresh();
create trigger trace_refresh_task after insert or update on application_run_log_tasks for each row execute function enqueue_application_run_trace_refresh();
create trigger trace_refresh_runtime_event after insert on runtime_events for each row execute function enqueue_application_run_trace_refresh();
-- Existing facts are preserved and rebuilt asynchronously in bounded batches.
insert into application_run_trace_refresh_queue(flow_run_id) select id from flow_runs;
