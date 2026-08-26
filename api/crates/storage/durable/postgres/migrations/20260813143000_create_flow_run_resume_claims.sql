create table flow_run_resume_claims (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    checkpoint_id uuid not null references flow_run_checkpoints(id) on delete cascade,
    callback_task_id uuid references flow_run_callback_tasks(id) on delete cascade,
    resume_kind text not null check (resume_kind in ('human', 'callback')),
    status text not null check (status in ('processing', 'succeeded', 'failed')),
    request_payload jsonb not null,
    claim_token uuid not null,
    generation bigint not null default 0 check (generation >= 0),
    lease_expires_at timestamptz not null,
    error_payload jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    completed_at timestamptz,
    check (
        (resume_kind = 'human' and callback_task_id is null)
        or (resume_kind = 'callback' and callback_task_id is not null)
    )
);

create unique index flow_run_resume_claims_human_target_unique_idx
    on flow_run_resume_claims (checkpoint_id)
    where resume_kind = 'human';

create unique index flow_run_resume_claims_callback_target_unique_idx
    on flow_run_resume_claims (callback_task_id)
    where resume_kind = 'callback';

create index flow_run_resume_claims_flow_status_idx
    on flow_run_resume_claims (flow_run_id, status, updated_at desc, id desc);
