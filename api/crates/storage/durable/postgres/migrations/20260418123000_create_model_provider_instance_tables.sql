create table model_provider_instances (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    installation_id uuid not null references plugin_installations(id) on delete restrict,
    provider_code text not null,
    protocol text not null,
    display_name text not null,
    status text not null check (
        status in ('draft', 'ready', 'invalid', 'disabled')
    ),
    config_json jsonb not null default '{}'::jsonb,
    last_validated_at timestamptz,
    last_validation_status text check (
        last_validation_status in ('succeeded', 'failed')
    ),
    last_validation_message text,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index model_provider_instances_workspace_name_idx
    on model_provider_instances (workspace_id, provider_code, display_name);

create index model_provider_instances_workspace_status_idx
    on model_provider_instances (workspace_id, status, updated_at desc, id desc);

create table model_provider_instance_secrets (
    provider_instance_id uuid primary key references model_provider_instances(id) on delete cascade,
    encrypted_secret_json jsonb not null,
    secret_version integer not null,
    updated_at timestamptz not null default now()
);

create table provider_instance_model_catalog_cache (
    provider_instance_id uuid primary key references model_provider_instances(id) on delete cascade,
    model_discovery_mode text not null check (
        model_discovery_mode in ('static', 'dynamic', 'hybrid')
    ),
    refresh_status text not null check (
        refresh_status in ('idle', 'refreshing', 'ready', 'failed')
    ),
    source text not null check (
        source in ('static', 'dynamic', 'hybrid')
    ),
    models_json jsonb not null default '[]'::jsonb,
    last_error_message text,
    refreshed_at timestamptz,
    updated_at timestamptz not null default now()
);
