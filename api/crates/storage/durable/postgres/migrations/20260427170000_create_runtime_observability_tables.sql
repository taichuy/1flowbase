create table runtime_spans (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid references node_runs(id) on delete cascade,
    parent_span_id uuid references runtime_spans(id) on delete cascade,
    kind text not null,
    name text not null,
    status text not null,
    capability_id text,
    input_ref text,
    output_ref text,
    error_payload jsonb,
    metadata jsonb not null default '{}'::jsonb,
    started_at timestamptz not null,
    finished_at timestamptz
);

create index runtime_spans_flow_parent_started_idx
    on runtime_spans (flow_run_id, parent_span_id, started_at asc, id asc);

create table runtime_events (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid references node_runs(id) on delete cascade,
    span_id uuid references runtime_spans(id) on delete set null,
    parent_span_id uuid references runtime_spans(id) on delete set null,
    sequence bigint not null,
    event_type text not null,
    layer text not null,
    source text not null,
    trust_level text not null,
    item_id uuid,
    ledger_ref text,
    payload jsonb not null,
    visibility text not null,
    durability text not null,
    created_at timestamptz not null default now(),
    unique(flow_run_id, sequence)
);

create index runtime_events_flow_sequence_idx
    on runtime_events (flow_run_id, sequence asc);

create table runtime_items (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    span_id uuid references runtime_spans(id) on delete set null,
    kind text not null,
    status text not null,
    source_event_id uuid references runtime_events(id) on delete set null,
    input_ref text,
    output_ref text,
    usage_ledger_id uuid,
    trust_level text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table runtime_context_projections (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid references node_runs(id) on delete cascade,
    llm_turn_span_id uuid references runtime_spans(id) on delete set null,
    projection_kind text not null,
    merge_stage_ref text,
    source_transcript_ref text,
    source_item_refs jsonb not null default '[]'::jsonb,
    compaction_event_id uuid references runtime_events(id) on delete set null,
    summary_version text,
    model_input_ref text not null,
    model_input_hash text not null,
    compacted_summary_ref text,
    previous_projection_id uuid references runtime_context_projections(id) on delete set null,
    token_estimate bigint,
    provider_continuation_metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);

create index runtime_context_projections_flow_created_idx
    on runtime_context_projections (flow_run_id, created_at asc, id asc);

create table runtime_usage_ledger (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid references node_runs(id) on delete cascade,
    span_id uuid references runtime_spans(id) on delete set null,
    failover_attempt_id uuid,
    provider_instance_id uuid,
    gateway_route_id uuid,
    model_id text,
    upstream_model_id text,
    upstream_request_id text,
    input_tokens bigint,
    cached_input_tokens bigint,
    output_tokens bigint,
    reasoning_output_tokens bigint,
    total_tokens bigint,
    cache_read_tokens bigint,
    cache_write_tokens bigint,
    price_snapshot jsonb,
    cost_snapshot jsonb,
    usage_status text not null,
    raw_usage jsonb not null default '{}'::jsonb,
    normalized_usage jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);

create index runtime_usage_ledger_flow_created_idx
    on runtime_usage_ledger (flow_run_id, created_at asc, id asc);

create table runtime_artifacts (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    artifact_kind text not null,
    content_ref text not null,
    content_hash text not null,
    mime_type text,
    byte_size bigint,
    redaction_status text not null default 'none',
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);

create table runtime_audit_hashes (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    fact_table text not null,
    fact_id uuid not null,
    prev_hash text,
    row_hash text not null,
    created_at timestamptz not null default now()
);
