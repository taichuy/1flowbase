-- The live owner commits canonical items through the runtime event persister.
-- Replace only the unused index introduced by this feature, never retained facts.
drop index gateway_log_output_facts;
create index gateway_log_output_facts on runtime_events(flow_run_id,sequence)
    where event_type='provider_output_item_done';
