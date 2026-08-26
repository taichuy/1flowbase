alter table node_runs
    add column debug_payload jsonb not null default '{}'::jsonb;
