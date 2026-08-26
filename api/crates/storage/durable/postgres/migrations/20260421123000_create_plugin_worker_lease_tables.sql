create table plugin_worker_leases (
    id uuid primary key,
    installation_id uuid not null references plugin_installations(id) on delete cascade,
    worker_key text not null,
    status text not null,
    runtime_scope jsonb not null default '{}'::jsonb,
    last_heartbeat_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
