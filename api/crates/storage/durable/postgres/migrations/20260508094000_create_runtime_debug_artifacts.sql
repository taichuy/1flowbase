create table runtime_debug_artifacts (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    flow_run_id uuid references flow_runs(id) on delete cascade,
    node_run_id uuid references node_runs(id) on delete set null,
    run_event_id uuid references flow_run_events(id) on delete set null,
    artifact_kind text not null,
    content_type text not null,
    original_size_bytes bigint not null check (original_size_bytes >= 0),
    preview_size_bytes bigint not null check (preview_size_bytes >= 0),
    storage_id uuid not null references file_storages(id) on delete restrict,
    storage_ref text not null,
    retention_state text not null default 'active' check (
        retention_state in ('active', 'pending_delete', 'deleted')
    ),
    created_at timestamptz not null default now()
);

create index runtime_debug_artifacts_application_created_idx
    on runtime_debug_artifacts (application_id, created_at desc, id desc);

create index runtime_debug_artifacts_flow_run_idx
    on runtime_debug_artifacts (flow_run_id, created_at desc, id desc);

create index runtime_debug_artifacts_node_run_idx
    on runtime_debug_artifacts (node_run_id, created_at desc, id desc)
    where node_run_id is not null;

create index runtime_debug_artifacts_retention_idx
    on runtime_debug_artifacts (retention_state, created_at asc);
