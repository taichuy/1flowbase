create or replace function enqueue_application_run_trace_refresh() returns trigger language plpgsql as $$
declare
 run_id uuid;
 previous_flow flow_runs%ROWTYPE;
begin
 if TG_TABLE_NAME = 'flow_runs' and TG_OP = 'UPDATE' then
  -- Sequence allocation is durable bookkeeping, not a change to the trace.
  -- Compare the typed row so new source columns automatically invalidate it.
  previous_flow := OLD;
  previous_flow.runtime_event_sequence_high_water := NEW.runtime_event_sequence_high_water;
  if NEW is not distinct from previous_flow then return NEW; end if;
 end if;
 if TG_TABLE_NAME in ('flow_runs', 'application_run_log_tasks') then run_id := NEW.id;
 else run_id := NEW.flow_run_id;
 end if;
 -- Every visible tool state enriches the tree, including started/waiting.
 -- starts_with is literal: underscores must not act as LIKE wildcards.
 if TG_TABLE_NAME = 'runtime_events' then
  if NEW.event_type not in ('provider_output_item_done','provider_protocol_integrity')
     and not starts_with(NEW.event_type, 'visible_internal_llm_tool_') then return NEW; end if;
 end if;
 -- Cascaded task updates can run after the parent flow_run has been deleted.
 insert into application_run_trace_refresh_queue(flow_run_id)
 select flow_runs.id from flow_runs where flow_runs.id = run_id
 on conflict(flow_run_id) do update set revision=application_run_trace_refresh_queue.revision+1;
 return NEW;
end $$;
