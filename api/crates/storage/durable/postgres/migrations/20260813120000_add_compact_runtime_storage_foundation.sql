alter table applications
    add constraint applications_id_scope_unique unique (id, scope_id);

alter table flow_runs
    add constraint flow_runs_id_scope_application_unique unique (id, scope_id, application_id);

alter table runtime_spans
    add constraint runtime_spans_id_flow_unique unique (id, flow_run_id);

alter table node_runs
    add constraint node_runs_id_flow_unique unique (id, flow_run_id);

create table runtime_canonical_contents (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    content_hash text not null check (content_hash ~ '^sha256:[0-9a-f]{64}$'),
    content jsonb not null,
    byte_size bigint not null check (byte_size >= 0),
    created_at timestamptz not null default now(),
    unique (application_id, content_hash),
    unique (id, scope_id, application_id),
    foreign key (application_id, scope_id) references applications(id, scope_id) on delete cascade
);

create index runtime_canonical_contents_scope_application_created_idx
    on runtime_canonical_contents (scope_id, application_id, created_at, id);

create function reject_runtime_canonical_content_update()
returns trigger language plpgsql as $$
begin
    raise exception 'runtime canonical content is immutable';
end;
$$;

create trigger runtime_canonical_contents_reject_update
before update on runtime_canonical_contents
for each row execute function reject_runtime_canonical_content_update();

alter table runtime_context_projections
    add column application_id uuid references applications(id) on delete cascade,
    add column context_sequence bigint check (context_sequence >= 0),
    add column transition_kind text check (
        transition_kind in ('initial', 'append', 'callback', 'retry', 'declared_compaction')
    ),
    add column transition_actor text check (transition_actor in ('host', 'client', 'provider')),
    add column declared_compaction_provenance jsonb,
    add column actual_content_id uuid references runtime_canonical_contents(id) on delete restrict;

create unique index runtime_context_projections_flow_context_sequence_idx
    on runtime_context_projections (flow_run_id, context_sequence)
    where context_sequence is not null;

alter table runtime_context_projections
    add constraint runtime_context_projections_owned_identity_unique
        unique (id, scope_id, application_id, flow_run_id),
    add constraint runtime_context_projections_owned_run_fk
        foreign key (flow_run_id, scope_id, application_id)
        references flow_runs(id, scope_id, application_id) on delete cascade,
    add constraint runtime_context_projections_owned_content_fk
        foreign key (actual_content_id, scope_id, application_id)
        references runtime_canonical_contents(id, scope_id, application_id) on delete restrict,
    add constraint runtime_context_projections_owned_parent_fk
        foreign key (previous_projection_id, scope_id, application_id, flow_run_id)
        references runtime_context_projections(id, scope_id, application_id, flow_run_id)
        on delete restrict;

create table runtime_invocation_context_bindings (
    invocation_span_id uuid primary key references runtime_spans(id) on delete cascade,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    context_version_id uuid not null references runtime_context_projections(id) on delete restrict,
    created_at timestamptz not null default now(),
    foreign key (invocation_span_id, flow_run_id)
        references runtime_spans(id, flow_run_id) on delete cascade,
    foreign key (context_version_id, scope_id, application_id, flow_run_id)
        references runtime_context_projections(id, scope_id, application_id, flow_run_id)
        on delete restrict,
    foreign key (flow_run_id, scope_id, application_id)
        references flow_runs(id, scope_id, application_id) on delete cascade
);

create index runtime_invocation_context_bindings_scope_run_created_idx
    on runtime_invocation_context_bindings (scope_id, application_id, flow_run_id, created_at, invocation_span_id);

create table flow_run_recovery_history (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid references node_runs(id) on delete cascade,
    sequence bigint not null check (sequence >= 0),
    state_code text not null check (
        state_code in ('running', 'waiting_callback', 'waiting_human', 'paused', 'retrying', 'succeeded', 'failed', 'cancelled')
    ),
    node_sequence bigint not null check (node_sequence >= 0),
    iteration_index bigint not null check (iteration_index >= 0),
    attempt_index integer not null check (attempt_index >= 0),
    resume_sequence bigint not null check (resume_sequence >= 0),
    event_sequence bigint not null check (event_sequence >= 0),
    context_version_id uuid not null references runtime_context_projections(id) on delete restrict,
    recovery_content_id uuid references runtime_canonical_contents(id) on delete restrict,
    idempotency_key text not null check (idempotency_key <> ''),
    created_at timestamptz not null default now(),
    unique (flow_run_id, sequence),
    unique (flow_run_id, idempotency_key),
    foreign key (flow_run_id, scope_id, application_id)
        references flow_runs(id, scope_id, application_id) on delete cascade,
    foreign key (node_run_id, flow_run_id)
        references node_runs(id, flow_run_id) on delete cascade,
    foreign key (context_version_id, scope_id, application_id, flow_run_id)
        references runtime_context_projections(id, scope_id, application_id, flow_run_id)
        on delete restrict,
    foreign key (recovery_content_id, scope_id, application_id)
        references runtime_canonical_contents(id, scope_id, application_id) on delete restrict
);

create index flow_run_recovery_history_scope_run_sequence_idx
    on flow_run_recovery_history (scope_id, application_id, flow_run_id, sequence, id);

create function reject_flow_run_recovery_history_update()
returns trigger language plpgsql as $$
begin
    raise exception 'flow run recovery history is append-only';
end;
$$;

create trigger flow_run_recovery_history_reject_update
before update on flow_run_recovery_history
for each row execute function reject_flow_run_recovery_history_update();
