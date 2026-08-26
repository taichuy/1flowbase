create table node_contribution_registry (
    id uuid primary key,
    installation_id uuid not null references plugin_installations(id) on delete cascade,
    provider_code text not null,
    plugin_id text not null,
    plugin_version text not null,
    contribution_code text not null,
    node_shell text not null,
    category text not null,
    title text not null,
    description text not null,
    icon text not null,
    schema_ui jsonb not null default '{}'::jsonb,
    schema_version text not null,
    output_schema jsonb not null default '{}'::jsonb,
    required_auth jsonb not null default '[]'::jsonb,
    visibility text not null default 'public',
    experimental boolean not null default false,
    dependency_installation_kind text not null,
    dependency_plugin_version_range text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (installation_id, contribution_code)
);

create index node_contribution_registry_provider_idx
    on node_contribution_registry (provider_code, plugin_version, contribution_code);
