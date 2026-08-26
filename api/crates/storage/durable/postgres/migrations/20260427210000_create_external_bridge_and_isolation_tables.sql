create table external_agent_sessions (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    flow_run_id uuid references flow_runs(id) on delete cascade,
    external_agent_kind text not null,
    external_session_id text not null,
    trust_level text not null,
    opaque_boundary_marked boolean not null default false,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    unique(workspace_id, external_agent_kind, external_session_id)
);

create index external_agent_sessions_workspace_created_idx
    on external_agent_sessions (workspace_id, created_at desc, id desc);

create index external_agent_sessions_flow_run_idx
    on external_agent_sessions (flow_run_id)
    where flow_run_id is not null;

create table external_agent_telemetry_events (
    id uuid primary key,
    external_agent_session_id uuid not null references external_agent_sessions(id) on delete cascade,
    runtime_event_id uuid references runtime_events(id) on delete set null,
    trust_level text not null,
    schema_version text not null,
    payload jsonb not null,
    signature_status text,
    created_at timestamptz not null default now()
);

create index external_agent_telemetry_events_session_created_idx
    on external_agent_telemetry_events (external_agent_session_id, created_at asc, id asc);

create index external_agent_telemetry_events_runtime_event_idx
    on external_agent_telemetry_events (runtime_event_id)
    where runtime_event_id is not null;
