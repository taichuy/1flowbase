create table if not exists mcp_client_credentials (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    instance_record_id uuid not null references mcp_instances(id) on delete cascade,
    encrypted_secret_json jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (user_id, workspace_id, instance_record_id)
);

create index if not exists idx_mcp_client_credentials_workspace_instance
    on mcp_client_credentials (workspace_id, instance_record_id);
