-- Observation is asynchronous: Native facts may precede their client capture.
-- Keep the exact host identity in existing immutable facts; indexed read joins
-- resolve links once both arrive, without foreign-key races or historical guesses.
create index native_trajectory_trigger_page
    on provider_semantic_trajectory_steps(flow_run_id,(metadata->>'trigger_request_id'),event_sequence);
create index client_trajectory_response_origin
    on client_trajectory_steps(flow_run_id,(metadata->>'response_id'),request_id)
    where metadata->>'origin'='emitted';
create index client_trajectory_request_page
    on client_trajectory_steps(flow_run_id,request_id,event_sequence);

-- An empty prewarm can have no output/usage step, but still has a real response ID.
alter table client_trajectory_captures add column response_id text;
create index client_trajectory_capture_response on client_trajectory_captures(flow_run_id,response_id);
create function project_client_response_link() returns trigger language plpgsql as $$
begin
    if new.event_type='client_protocol_trajectory' and new.payload->'fact'->>'kind'='response_link' then
        update client_trajectory_captures set response_id=new.payload->'fact'->>'response_id'
        where request_id=(new.payload->>'request_id')::uuid and flow_run_id=new.flow_run_id
          and (response_id is null or response_id=new.payload->'fact'->>'response_id');
    end if;
    return new;
end;
$$;
create trigger client_response_link_projection after insert on runtime_events
    for each row execute function project_client_response_link();
