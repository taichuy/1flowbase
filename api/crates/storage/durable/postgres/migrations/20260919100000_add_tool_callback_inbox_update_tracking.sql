alter table flow_run_tool_callback_inbox
    add column updated_at timestamptz not null default now();

create index flow_run_tool_callback_inbox_scope_created_idx
    on flow_run_tool_callback_inbox (scope_id, created_at, id);
