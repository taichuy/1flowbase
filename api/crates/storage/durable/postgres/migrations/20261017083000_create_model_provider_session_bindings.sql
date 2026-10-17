-- This is routing ownership, not response history. Keep the owner even if its
-- instance is removed so a subsequent request cannot silently select a new one.
create table model_provider_session_bindings (
    workspace_id uuid not null references workspaces(id) on delete cascade,
    scope_digest text not null check (scope_digest ~ '^[0-9a-f]{64}$'),
    provider_instance_id uuid not null,
    created_at timestamptz not null default now(),
    primary key (workspace_id, scope_digest)
);
