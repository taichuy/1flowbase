create table model_provider_catalog_sources (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    source_kind text not null,
    plugin_id text not null,
    provider_code text not null,
    display_name text not null,
    base_url_ref text,
    auth_secret_ref text,
    protocol text not null,
    status text not null,
    last_sync_run_id uuid,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table model_catalog_sync_runs (
    id uuid primary key,
    catalog_source_id uuid not null references model_provider_catalog_sources(id) on delete cascade,
    status text not null,
    error_message_ref text,
    discovered_count bigint not null default 0,
    imported_count bigint not null default 0,
    disabled_count bigint not null default 0,
    started_at timestamptz not null default now(),
    finished_at timestamptz
);

alter table model_provider_catalog_sources
    add constraint model_provider_catalog_sources_last_sync_run_fk
    foreign key (last_sync_run_id)
    references model_catalog_sync_runs(id)
    on delete set null;

create table model_provider_catalog_entries (
    id uuid primary key,
    provider_instance_id uuid references model_provider_instances(id) on delete set null,
    catalog_source_id uuid not null references model_provider_catalog_sources(id) on delete cascade,
    upstream_model_id text not null,
    display_label text not null,
    protocol text not null,
    capability_snapshot jsonb not null default '{}'::jsonb,
    parameter_schema_ref text,
    context_window bigint,
    max_output_tokens bigint,
    pricing_ref text,
    fetched_at timestamptz not null default now(),
    status text not null,
    unique(catalog_source_id, upstream_model_id, protocol)
);

create index model_provider_catalog_entries_source_idx
    on model_provider_catalog_entries (catalog_source_id, upstream_model_id asc, protocol asc);

create index model_provider_catalog_entries_provider_instance_idx
    on model_provider_catalog_entries (provider_instance_id, upstream_model_id asc, protocol asc);

create table model_failover_queue_templates (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    name text not null,
    version bigint not null,
    status text not null,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table model_failover_queue_items (
    id uuid primary key,
    queue_template_id uuid not null references model_failover_queue_templates(id) on delete cascade,
    sort_index integer not null,
    provider_instance_id uuid not null references model_provider_instances(id) on delete cascade,
    provider_code text not null,
    upstream_model_id text not null,
    protocol text not null,
    enabled boolean not null default true
);

create index model_failover_queue_items_template_sort_idx
    on model_failover_queue_items (queue_template_id, sort_index asc, id asc);

create table model_failover_queue_snapshots (
    id uuid primary key,
    queue_template_id uuid not null references model_failover_queue_templates(id) on delete restrict,
    version bigint not null,
    items jsonb not null,
    created_at timestamptz not null default now()
);

create index model_failover_queue_snapshots_template_created_idx
    on model_failover_queue_snapshots (queue_template_id, created_at desc, id desc);

create table model_failover_attempt_ledger (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid references node_runs(id) on delete cascade,
    llm_turn_span_id uuid references runtime_spans(id) on delete set null,
    queue_snapshot_id uuid references model_failover_queue_snapshots(id) on delete set null,
    attempt_index integer not null,
    provider_instance_id uuid,
    provider_code text not null,
    upstream_model_id text not null,
    protocol text not null,
    request_ref text,
    request_hash text,
    started_at timestamptz not null,
    first_token_at timestamptz,
    finished_at timestamptz,
    status text not null,
    failed_after_first_token boolean not null default false,
    upstream_request_id text,
    error_code text,
    error_message_ref text,
    usage_ledger_id uuid references runtime_usage_ledger(id) on delete set null,
    cost_ledger_id uuid,
    response_ref text
);

create index model_failover_attempt_ledger_flow_node_idx
    on model_failover_attempt_ledger (flow_run_id, node_run_id, attempt_index asc);

alter table runtime_usage_ledger
    add constraint runtime_usage_ledger_failover_attempt_fk
    foreign key (failover_attempt_id)
    references model_failover_attempt_ledger(id)
    on delete set null;
