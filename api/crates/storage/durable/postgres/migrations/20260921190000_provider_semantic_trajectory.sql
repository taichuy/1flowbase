-- Semantic snapshots are projected only on writes. Preserve original protocol evidence.
create table provider_semantic_trajectory_steps (
    event_id uuid primary key references runtime_events(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid not null,
    invocation_id text not null,
    provider_attempt_index bigint not null,
    step_key text not null,
    event_sequence bigint not null,
    metadata jsonb not null,
    created_at timestamptz not null,
    unique(flow_run_id,node_run_id,invocation_id,provider_attempt_index,step_key)
);
create index provider_semantic_trajectory_node_sequence
    on provider_semantic_trajectory_steps(flow_run_id,node_run_id,event_sequence);
create function project_provider_semantic_trajectory() returns trigger language plpgsql as $$
begin
    if new.event_type='provider_semantic_step' and new.node_run_id is not null then
        insert into provider_semantic_trajectory_steps
            (event_id,flow_run_id,node_run_id,invocation_id,provider_attempt_index,step_key,event_sequence,metadata,created_at)
        values (new.id,new.flow_run_id,new.node_run_id,new.payload->>'invocation_id',
            (new.payload->>'provider_attempt_index')::bigint,new.payload->>'step_key',new.sequence,new.payload,new.created_at)
        on conflict(flow_run_id,node_run_id,invocation_id,provider_attempt_index,step_key)
        do update set metadata=excluded.metadata;
    end if;
    return new;
end;
$$;
create trigger provider_semantic_trajectory_insert after insert on runtime_events
    for each row execute function project_provider_semantic_trajectory();
-- Do not invent semantic records for older invocations. Raw evidence remains addressable.
