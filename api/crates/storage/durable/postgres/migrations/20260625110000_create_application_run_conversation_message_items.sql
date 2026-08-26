create table application_run_conversation_message_items (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    display_sequence bigint not null check (display_sequence >= 0),
    source_kind text not null check (source_kind in ('imported_context', 'current_run')),
    role text,
    content text,
    query text,
    model text,
    answer text,
    detail_run_id uuid references flow_runs(id) on delete set null,
    can_open_detail boolean not null,
    is_current boolean not null,
    status text not null,
    started_at timestamptz not null,
    finished_at timestamptz,
    projection_version integer not null default 1 check (projection_version >= 1),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (flow_run_id, projection_version, display_sequence)
);

create index application_run_conversation_message_items_run_sequence_idx
    on application_run_conversation_message_items (
        application_id,
        flow_run_id,
        projection_version,
        display_sequence asc,
        id asc
    );
