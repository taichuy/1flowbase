create table application_run_log_summaries (
    flow_run_id uuid primary key references flow_runs(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    run_mode text not null,
    status text not null check (status in ('succeeded', 'failed', 'cancelled')),
    target_node_id text,
    title text not null,
    input_payload jsonb not null default '{}'::jsonb,
    external_user text,
    authorized_account text,
    api_key_id uuid,
    publication_version_id uuid,
    external_conversation_id text,
    external_trace_id text,
    compatibility_mode text,
    idempotency_key text,
    total_tokens bigint,
    unique_node_count bigint not null default 0,
    tool_callback_count bigint not null default 0,
    started_at timestamptz not null,
    finished_at timestamptz,
    created_at timestamptz not null,
    updated_at timestamptz not null,
    log_created_at timestamptz not null default now(),
    log_updated_at timestamptz not null default now()
);

create index application_run_log_summaries_created_idx
    on application_run_log_summaries (application_id, created_at desc, flow_run_id desc);

create index application_run_log_summaries_started_idx
    on application_run_log_summaries (application_id, started_at desc, flow_run_id desc);

create index application_run_log_summaries_finished_idx
    on application_run_log_summaries (application_id, finished_at desc, flow_run_id desc);

create index application_run_log_summaries_updated_idx
    on application_run_log_summaries (application_id, updated_at desc, flow_run_id desc);
