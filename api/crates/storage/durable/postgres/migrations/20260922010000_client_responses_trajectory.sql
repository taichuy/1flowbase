-- Client protocol facts are independent of provider diagnostics. No historical backfill.
create table client_trajectory_captures (
    request_id uuid primary key,
    event_id uuid not null references runtime_events(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid,
    event_sequence bigint not null,
    status text not null,
    dropped_count bigint not null default 0,
    persist_failed_count bigint not null default 0
);
create index client_trajectory_capture_scope on client_trajectory_captures(flow_run_id,node_run_id);
create table client_trajectory_steps (
    id uuid primary key,
    event_id uuid not null references runtime_events(id) on delete cascade,
    request_id uuid not null references client_trajectory_captures(request_id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid,
    event_sequence bigint not null,
    metadata jsonb not null
);
create index client_trajectory_step_page on client_trajectory_steps(flow_run_id,event_sequence);
create index client_trajectory_step_node_page on client_trajectory_steps(flow_run_id,node_run_id,event_sequence);
create index client_trajectory_call on client_trajectory_steps(flow_run_id,(metadata->>'call_id'),event_sequence)
    where metadata->>'call_id' is not null;
create table client_trajectory_sections (
    event_id uuid primary key references runtime_events(id) on delete cascade,
    request_id uuid not null references client_trajectory_captures(request_id) on delete cascade,
    step_id uuid not null,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid,
    section text not null,
    event_sequence bigint not null
);
create index client_trajectory_section_page on client_trajectory_sections(flow_run_id,step_id,section,event_sequence);
-- Each section is its own body event; reading parameters cannot deserialize result/raw.
create function project_client_trajectory() returns trigger language plpgsql as $$
begin
    if new.event_type <> 'client_protocol_trajectory' then return new; end if;
    if new.payload->'fact'->>'kind' = 'integrity' then
        insert into client_trajectory_captures(request_id,event_id,flow_run_id,node_run_id,event_sequence,status,dropped_count,persist_failed_count)
        values ((new.payload->>'request_id')::uuid,new.id,new.flow_run_id,new.node_run_id,new.sequence,
            new.payload->'fact'->>'status',(new.payload->'fact'->>'dropped_count')::bigint,
            (new.payload->'fact'->>'persist_failed_count')::bigint)
        on conflict(request_id) do update set event_id=excluded.event_id,event_sequence=excluded.event_sequence,status=excluded.status,
            dropped_count=excluded.dropped_count,persist_failed_count=excluded.persist_failed_count
        where client_trajectory_captures.flow_run_id=excluded.flow_run_id
            and client_trajectory_captures.node_run_id is not distinct from excluded.node_run_id
            and client_trajectory_captures.event_sequence < excluded.event_sequence;
    elsif new.payload->'fact'->>'kind' = 'step' then
        insert into client_trajectory_steps(id,event_id,request_id,flow_run_id,node_run_id,event_sequence,metadata)
        values ((new.payload->'fact'->'step'->>'id')::uuid,new.id,(new.payload->>'request_id')::uuid,
            new.flow_run_id,new.node_run_id,new.sequence,new.payload->'fact'->'step')
        on conflict(id) do update set event_id=excluded.event_id,metadata=excluded.metadata
        where client_trajectory_steps.request_id=excluded.request_id
            and client_trajectory_steps.flow_run_id=excluded.flow_run_id;
    elsif new.payload->'fact'->>'kind' = 'section' then
        insert into client_trajectory_sections(event_id,request_id,step_id,flow_run_id,node_run_id,section,event_sequence)
        values (new.id,(new.payload->>'request_id')::uuid,(new.payload->'fact'->>'step_id')::uuid,
            new.flow_run_id,new.node_run_id,new.payload->'fact'->>'section',new.sequence);
    end if;
    return new;
end;
$$;
create trigger client_trajectory_projection after insert on runtime_events
    for each row execute function project_client_trajectory();
