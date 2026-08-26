create table capability_invocations (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    span_id uuid references runtime_spans(id) on delete set null,
    capability_id text not null,
    requested_by_span_id uuid references runtime_spans(id) on delete set null,
    requester_kind text not null,
    arguments_ref text,
    authorization_status text not null,
    authorization_reason text,
    result_ref text,
    normalized_result jsonb,
    started_at timestamptz,
    finished_at timestamptz,
    error_payload jsonb,
    created_at timestamptz not null default now()
);

create index capability_invocations_flow_created_idx
    on capability_invocations (flow_run_id, created_at asc, id asc);
