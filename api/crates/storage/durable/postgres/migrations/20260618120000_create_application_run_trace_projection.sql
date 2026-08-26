create table application_run_trace_projection_statuses (
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    projection_version integer not null check (projection_version > 0),
    status text not null check (status in ('pending', 'running', 'succeeded', 'failed', 'stale', 'partial')),
    source_watermark text not null,
    attempt_count integer not null default 0 check (attempt_count >= 0),
    last_attempt_at timestamptz,
    last_success_at timestamptz,
    last_error_code text,
    last_error_stage text,
    last_error_source_kind text,
    last_error_source_locator text,
    last_error_message text,
    last_error_ref text,
    retriable boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (flow_run_id, projection_version)
);

create index application_run_trace_projection_statuses_status_idx
    on application_run_trace_projection_statuses (status, updated_at desc);

create table application_run_trace_nodes (
    trace_node_id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    parent_trace_node_id uuid references application_run_trace_nodes(trace_node_id)
        on delete cascade deferrable initially deferred,
    stable_locator text not null,
    node_kind text not null,
    owner_kind text,
    owner_id text,
    order_key text not null,
    node_id text,
    node_type text,
    node_alias text not null,
    status text not null,
    started_at timestamptz not null,
    finished_at timestamptz,
    duration_ms bigint,
    metrics_payload jsonb not null default '{}'::jsonb,
    has_children boolean not null default false,
    child_count bigint not null default 0 check (child_count >= 0),
    has_content boolean not null default false,
    content_ref text,
    projection_version integer not null check (projection_version > 0),
    source_watermark text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index application_run_trace_nodes_stable_locator_idx
    on application_run_trace_nodes (flow_run_id, stable_locator);

create index application_run_trace_nodes_children_idx
    on application_run_trace_nodes (flow_run_id, parent_trace_node_id, order_key, trace_node_id);

create index application_run_trace_nodes_owner_idx
    on application_run_trace_nodes (flow_run_id, owner_kind, owner_id);

create index application_run_trace_nodes_projection_idx
    on application_run_trace_nodes (flow_run_id, projection_version, source_watermark);

create table application_run_trace_node_contents (
    trace_node_id uuid primary key references application_run_trace_nodes(trace_node_id) on delete cascade,
    content_kind text not null,
    payload jsonb not null default '{}'::jsonb,
    source_refs jsonb not null default '[]'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
