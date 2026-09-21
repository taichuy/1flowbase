-- Explicit many-to-many correlation: original client requests can traverse several LLM nodes.
-- Historical captures without Native event correlation keep their unknown node ownership.
create table client_trajectory_node_links (
    request_id uuid not null references client_trajectory_captures(request_id) on delete cascade,
    node_run_id uuid not null references node_runs(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    event_id uuid not null references runtime_events(id) on delete cascade,
    primary key (request_id, node_run_id)
);
create index client_trajectory_node_lookup on client_trajectory_node_links(flow_run_id,node_run_id,request_id);
create function project_client_trajectory_node_link() returns trigger language plpgsql as $$
begin
    if new.event_type='client_protocol_trajectory' and new.payload->'fact'->>'kind'='node_link' then
        insert into client_trajectory_node_links(request_id,node_run_id,flow_run_id,event_id)
        values ((new.payload->>'request_id')::uuid,(new.payload->'fact'->>'node_run_id')::uuid,new.flow_run_id,new.id)
        on conflict(request_id,node_run_id) do nothing;
    end if;
    return new;
end;
$$;
create trigger client_trajectory_node_link_projection after insert on runtime_events
    for each row execute function project_client_trajectory_node_link();
