create table runtime_legacy_shadow_batches (
    id uuid primary key,
    execution_mode text not null check (execution_mode in ('apply')),
    status text not null check (status in ('running', 'completed')),
    requested_limit integer not null check (requested_limit > 0),
    lock_budget_ms integer not null check (lock_budget_ms > 0),
    start_cursor jsonb,
    next_cursor jsonb,
    statistics jsonb not null default '[]'::jsonb,
    difference_count bigint not null default 0 check (difference_count >= 0),
    started_at timestamptz not null default now(),
    completed_at timestamptz
);

create table runtime_legacy_shadow_rows (
    id uuid primary key,
    batch_id uuid not null references runtime_legacy_shadow_batches(id) on delete restrict,
    source_kind text not null check (
        source_kind in (
            'checkpoint_context',
            'callback_request',
            'callback_response',
            'run_event_history'
        )
    ),
    source_table text not null,
    source_column text not null,
    source_row_id uuid not null,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    run_classification text not null check (run_classification in ('pending', 'terminal')),
    source_hash text not null check (source_hash ~ '^sha256:[0-9a-f]{64}$'),
    source_byte_size bigint not null check (source_byte_size >= 0),
    canonical_content_id uuid not null references runtime_canonical_contents(id) on delete restrict,
    context_version_id uuid references runtime_context_projections(id) on delete restrict,
    created_at timestamptz not null default now(),
    unique (source_table, source_column, source_row_id),
    foreign key (flow_run_id, scope_id, application_id)
        references flow_runs(id, scope_id, application_id) on delete cascade,
    foreign key (canonical_content_id, scope_id, application_id)
        references runtime_canonical_contents(id, scope_id, application_id) on delete restrict,
    foreign key (context_version_id, scope_id, application_id, flow_run_id)
        references runtime_context_projections(id, scope_id, application_id, flow_run_id)
        on delete restrict
);

create index runtime_legacy_shadow_rows_application_run_idx
    on runtime_legacy_shadow_rows (scope_id, application_id, flow_run_id, source_kind, source_row_id);

create index runtime_legacy_shadow_rows_batch_idx
    on runtime_legacy_shadow_rows (batch_id, source_kind, source_row_id);
