-- Identity metadata contains no authentication secret. Run deletion owns its lifecycle.
create table workflow_run_identity_snapshots (
    flow_run_id uuid primary key references flow_runs(id) on delete cascade,
    creator_identity jsonb not null check (jsonb_typeof(creator_identity) in ('object', 'null')),
    executor_identity jsonb not null check (jsonb_typeof(executor_identity) in ('object', 'null')),
    created_at timestamptz not null default now()
);
