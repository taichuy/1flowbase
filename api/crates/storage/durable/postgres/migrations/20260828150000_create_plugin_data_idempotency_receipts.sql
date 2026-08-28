create table if not exists plugin_data_idempotency_receipts (
    owner_id text not null,
    workspace_id uuid not null,
    provider_instance_id text not null,
    idempotency_key text not null,
    request_hash text not null,
    response jsonb not null,
    created_at timestamptz not null default now(),
    primary key (owner_id, workspace_id, provider_instance_id, idempotency_key)
);
