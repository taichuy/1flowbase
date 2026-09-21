-- Stable step identity/cursor and latest body locator are separate concerns.
alter table provider_semantic_trajectory_steps
    add column body_event_id uuid references runtime_events(id) on delete cascade,
    add column snapshot_count bigint not null default 1;
update provider_semantic_trajectory_steps set body_event_id=event_id;

create table native_trajectory_integrity (
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid not null,
    invocation_id text not null,
    provider_attempt_index bigint not null,
    event_id uuid not null references runtime_events(id) on delete cascade,
    event_sequence bigint not null,
    metadata jsonb not null,
    primary key(flow_run_id,node_run_id,invocation_id,provider_attempt_index)
);

create or replace function project_provider_semantic_trajectory() returns trigger language plpgsql as $$
begin
    if new.event_type='provider_semantic_step' and new.node_run_id is not null then
        insert into provider_semantic_trajectory_steps
            (event_id,flow_run_id,node_run_id,invocation_id,provider_attempt_index,step_key,event_sequence,metadata,created_at,body_event_id)
        values (new.id,new.flow_run_id,new.node_run_id,new.payload->>'invocation_id',
            (new.payload->>'provider_attempt_index')::bigint,new.payload->>'step_key',new.sequence,new.payload - 'body',new.created_at,new.id)
        on conflict(flow_run_id,node_run_id,invocation_id,provider_attempt_index,step_key)
        do update set metadata=excluded.metadata,body_event_id=excluded.body_event_id,
            snapshot_count=provider_semantic_trajectory_steps.snapshot_count+1;
    elsif new.event_type='native_trajectory_integrity' and new.node_run_id is not null then
        insert into native_trajectory_integrity
            (flow_run_id,node_run_id,invocation_id,provider_attempt_index,event_id,event_sequence,metadata)
        values (new.flow_run_id,new.node_run_id,new.payload->>'invocation_id',
            (new.payload->>'provider_attempt_index')::bigint,new.id,new.sequence,new.payload - 'body')
        on conflict(flow_run_id,node_run_id,invocation_id,provider_attempt_index)
        do update set event_id=excluded.event_id,event_sequence=excluded.event_sequence,metadata=excluded.metadata
        where native_trajectory_integrity.event_sequence < excluded.event_sequence;
    end if;
    return new;
end;
$$;
-- Existing semantic history remains supplier-derived; no synthetic Native backfill.
create index provider_protocol_trajectory_attempt_sequence on provider_protocol_trajectory_events
    (flow_run_id,node_run_id,(metadata->>'invocation_id'),((metadata->>'provider_attempt_index')::bigint),((metadata->>'sequence')::bigint))
    where event_type='provider_protocol_observation';
