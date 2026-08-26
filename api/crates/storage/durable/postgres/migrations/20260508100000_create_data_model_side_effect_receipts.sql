create table if not exists data_model_side_effect_receipts (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    draft_id uuid not null references flow_drafts(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    node_run_id uuid not null references node_runs(id) on delete cascade,
    node_id text not null,
    action text not null,
    model_code text not null,
    record_id text,
    deleted_id text,
    affected_count bigint not null default 0,
    idempotency_key text not null,
    payload_hash text not null,
    actor_user_id uuid not null references users(id),
    scope_id uuid not null,
    status text not null,
    output_payload jsonb not null,
    created_at timestamptz not null default now(),
    constraint data_model_side_effect_receipts_action_check
        check (action in ('create', 'update', 'delete')),
    constraint data_model_side_effect_receipts_status_check
        check (status in ('pending', 'succeeded', 'failed')),
    constraint data_model_side_effect_receipts_payload_hash_check
        check (payload_hash <> '')
);

create unique index if not exists data_model_side_effect_receipts_workspace_key_idx
    on data_model_side_effect_receipts (workspace_id, idempotency_key);

create index if not exists data_model_side_effect_receipts_flow_run_idx
    on data_model_side_effect_receipts (flow_run_id, node_run_id);
