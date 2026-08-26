create table flow_compiled_plans (
    id uuid primary key,
    flow_id uuid not null references flows(id) on delete cascade,
    flow_draft_id uuid not null unique references flow_drafts(id) on delete cascade,
    schema_version text not null,
    document_updated_at timestamptz not null,
    plan jsonb not null,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table flow_runs (
    id uuid primary key,
    application_id uuid not null references applications(id) on delete cascade,
    flow_id uuid not null references flows(id) on delete cascade,
    flow_draft_id uuid not null references flow_drafts(id) on delete cascade,
    compiled_plan_id uuid not null references flow_compiled_plans(id) on delete restrict,
    run_mode text not null check (run_mode in ('debug_node_preview')),
    target_node_id text,
    status text not null check (
        status in (
            'queued',
            'running',
            'waiting_callback',
            'waiting_human',
            'paused',
            'succeeded',
            'failed',
            'cancelled'
        )
    ),
    input_payload jsonb not null default '{}'::jsonb,
    output_payload jsonb not null default '{}'::jsonb,
    error_payload jsonb,
    created_by uuid not null references users(id),
    started_at timestamptz not null default now(),
    finished_at timestamptz,
    created_at timestamptz not null default now()
);

create index flow_runs_application_started_idx
    on flow_runs (application_id, started_at desc, id desc);

create table node_runs (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_id text not null,
    node_type text not null,
    node_alias text not null,
    status text not null check (
        status in (
            'pending',
            'ready',
            'running',
            'streaming',
            'waiting_tool',
            'waiting_callback',
            'waiting_human',
            'retrying',
            'succeeded',
            'failed',
            'skipped'
        )
    ),
    input_payload jsonb not null default '{}'::jsonb,
    output_payload jsonb not null default '{}'::jsonb,
    error_payload jsonb,
    metrics_payload jsonb not null default '{}'::jsonb,
    started_at timestamptz not null default now(),
    finished_at timestamptz
);

create index node_runs_flow_node_started_idx
    on node_runs (flow_run_id, node_id, started_at desc, id desc);

create table flow_run_checkpoints (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid references node_runs(id) on delete cascade,
    status text not null,
    reason text not null,
    locator_payload jsonb not null,
    variable_snapshot jsonb not null,
    external_ref_payload jsonb,
    created_at timestamptz not null default now()
);

create table flow_run_events (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid references node_runs(id) on delete cascade,
    sequence bigint not null,
    event_type text not null,
    payload jsonb not null,
    created_at timestamptz not null default now(),
    unique(flow_run_id, sequence)
);

create index flow_run_events_flow_sequence_idx
    on flow_run_events (flow_run_id, sequence asc);
