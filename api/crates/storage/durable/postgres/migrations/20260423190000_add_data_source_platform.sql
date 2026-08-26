create table data_source_instances (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    installation_id uuid not null references plugin_installations(id) on delete cascade,
    source_code text not null,
    display_name text not null,
    status text not null,
    config_json jsonb not null default '{}'::jsonb,
    metadata_json jsonb not null default '{}'::jsonb,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index data_source_instances_workspace_source_idx
    on data_source_instances (workspace_id, source_code);

create table data_source_secrets (
    data_source_instance_id uuid primary key references data_source_instances(id) on delete cascade,
    encrypted_secret_json jsonb not null,
    secret_version integer not null,
    updated_at timestamptz not null default now()
);

create table data_source_catalog_caches (
    data_source_instance_id uuid primary key references data_source_instances(id) on delete cascade,
    refresh_status text not null,
    catalog_json jsonb not null default '[]'::jsonb,
    last_error_message text null,
    refreshed_at timestamptz null,
    updated_at timestamptz not null default now()
);

create table data_source_preview_sessions (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    actor_user_id uuid not null references users(id) on delete cascade,
    data_source_instance_id uuid null references data_source_instances(id) on delete cascade,
    config_fingerprint text not null,
    preview_json jsonb not null,
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);

create index data_source_preview_sessions_workspace_expires_idx
    on data_source_preview_sessions (workspace_id, expires_at desc, created_at desc);
