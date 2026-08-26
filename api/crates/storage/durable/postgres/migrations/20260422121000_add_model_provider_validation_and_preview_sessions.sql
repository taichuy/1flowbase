alter table model_provider_instances
    add column validation_model_id text;

create table model_provider_preview_sessions (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    actor_user_id uuid not null references users(id) on delete cascade,
    installation_id uuid references plugin_installations(id) on delete cascade,
    instance_id uuid references model_provider_instances(id) on delete cascade,
    config_fingerprint text not null,
    models_json jsonb not null default '[]'::jsonb,
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);

create index model_provider_preview_sessions_workspace_expires_idx
    on model_provider_preview_sessions (workspace_id, expires_at desc, created_at desc);
