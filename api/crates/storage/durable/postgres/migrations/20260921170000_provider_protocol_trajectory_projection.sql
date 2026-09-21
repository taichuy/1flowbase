-- Read-only UI projection. Protocol bodies remain in their sole runtime-event owner.
create table provider_protocol_trajectory_events (
    event_id uuid primary key references runtime_events(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid not null,
    event_sequence bigint not null,
    event_type text not null,
    metadata jsonb not null,
    created_at timestamptz not null
);
create index provider_protocol_trajectory_node_sequence
    on provider_protocol_trajectory_events(flow_run_id, node_run_id, event_sequence);

create function project_provider_protocol_trajectory() returns trigger language plpgsql as $$
begin
    if new.event_type in ('provider_protocol_observation', 'provider_protocol_integrity')
       and new.node_run_id is not null then
        insert into provider_protocol_trajectory_events
            (event_id, flow_run_id, node_run_id, event_sequence, event_type, metadata, created_at)
        values (new.id, new.flow_run_id, new.node_run_id, new.sequence,
                new.event_type, new.payload - 'body', new.created_at);
    end if;
    return new;
end;
$$;
create trigger provider_protocol_trajectory_insert after insert on runtime_events
    for each row execute function project_provider_protocol_trajectory();

insert into provider_protocol_trajectory_events
    (event_id, flow_run_id, node_run_id, event_sequence, event_type, metadata, created_at)
select id, flow_run_id, node_run_id, sequence, event_type, payload - 'body', created_at
from runtime_events
where event_type in ('provider_protocol_observation', 'provider_protocol_integrity')
    and node_run_id is not null;
