alter table flow_runs drop constraint flow_runs_run_mode_check;

alter table flow_runs
    add constraint flow_runs_run_mode_check
    check (run_mode in ('debug_node_preview', 'debug_flow_run'));

create table flow_run_callback_tasks (
    id uuid primary key,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid not null references node_runs(id) on delete cascade,
    callback_kind text not null,
    status text not null check (status in ('pending', 'completed', 'cancelled')),
    request_payload jsonb not null default '{}'::jsonb,
    response_payload jsonb,
    external_ref_payload jsonb,
    created_at timestamptz not null default now(),
    completed_at timestamptz
);

create index flow_run_callback_tasks_flow_created_idx
    on flow_run_callback_tasks (flow_run_id, created_at desc, id desc);
