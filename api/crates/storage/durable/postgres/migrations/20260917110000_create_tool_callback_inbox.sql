create table flow_run_tool_callback_inbox (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    callback_task_id uuid not null references flow_run_callback_tasks(id) on delete cascade,
    tool_call_id text not null,
    tool_ordinal integer not null check (tool_ordinal >= 0),
    status text not null default 'pending' check (status in ('pending', 'received')),
    result_payload jsonb,
    result_fingerprint text,
    created_at timestamptz not null default now(),
    received_at timestamptz,
    unique (scope_id, application_id, flow_run_id, callback_task_id, tool_call_id),
    unique (callback_task_id, tool_ordinal),
    check (
        (status = 'pending' and result_payload is null and result_fingerprint is null and received_at is null)
        or (status = 'received' and result_payload is not null and result_fingerprint is not null and received_at is not null)
    )
);

create index flow_run_tool_callback_inbox_round_status_idx
    on flow_run_tool_callback_inbox (callback_task_id, status, tool_ordinal);
