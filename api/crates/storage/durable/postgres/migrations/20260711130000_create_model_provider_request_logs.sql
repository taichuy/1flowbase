create table if not exists model_provider_request_logs (
    id uuid primary key,
    scope_id uuid not null,
    attempt_id uuid not null unique,
    flow_run_id uuid not null,
    application_name text not null,
    attempt_index integer not null,
    provider_instance_id uuid,
    provider_instance_display_name text,
    provider_code text not null,
    protocol text not null,
    upstream_model_id text not null,
    reasoning_effort text,
    status text not null,
    error_code text,
    failed_after_first_token boolean not null default false,
    input_tokens bigint,
    output_tokens bigint,
    total_tokens bigint,
    started_at timestamptz not null,
    first_token_at timestamptz,
    finished_at timestamptz,
    time_to_first_token_ms bigint,
    total_duration_ms bigint,
    created_at timestamptz not null default now()
);

create index if not exists model_provider_request_logs_scope_started_idx
    on model_provider_request_logs (scope_id, started_at desc, id desc);
create index if not exists model_provider_request_logs_scope_provider_started_idx
    on model_provider_request_logs (scope_id, provider_instance_id, started_at desc);
create index if not exists model_provider_request_logs_scope_status_started_idx
    on model_provider_request_logs (scope_id, status, started_at desc);
