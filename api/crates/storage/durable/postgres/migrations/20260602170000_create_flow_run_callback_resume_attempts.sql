create table flow_run_callback_resume_attempts (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    callback_task_id uuid not null references flow_run_callback_tasks(id) on delete cascade,
    source text not null,
    status text not null check (status in ('received', 'processing', 'succeeded', 'failed', 'cancelled')),
    response_payload jsonb not null default '{}'::jsonb,
    idempotency_key text not null,
    error_payload jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    completed_at timestamptz
);

create unique index flow_run_callback_resume_attempts_callback_task_unique_idx
    on flow_run_callback_resume_attempts (callback_task_id);

create unique index flow_run_callback_resume_attempts_idempotency_key_unique_idx
    on flow_run_callback_resume_attempts (idempotency_key);

create index flow_run_callback_resume_attempts_flow_created_idx
    on flow_run_callback_resume_attempts (flow_run_id, created_at desc, id desc);

create index flow_run_callback_resume_attempts_status_idx
    on flow_run_callback_resume_attempts (status, created_at desc, id desc);
