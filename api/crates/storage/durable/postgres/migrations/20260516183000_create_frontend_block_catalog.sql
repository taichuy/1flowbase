create table if not exists frontend_block_catalog (
    id uuid primary key,
    installation_id uuid not null references plugin_installations(id) on delete cascade,
    provider_code text not null,
    plugin_id text not null,
    plugin_version text not null,
    contribution_code text not null,
    title text not null,
    runtime text not null,
    entry text not null,
    context_contract jsonb not null,
    permission_network text not null,
    permission_storage text not null,
    permission_secrets text not null,
    ui_capabilities jsonb not null default '[]'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint frontend_block_catalog_unique_code unique (installation_id, contribution_code),
    constraint frontend_block_catalog_runtime_check check (runtime in ('iframe')),
    constraint frontend_block_catalog_entry_not_empty check (length(trim(entry)) > 0),
    constraint frontend_block_catalog_permissions_check check (
        permission_network in ('none', 'outbound_only')
        and permission_storage = 'none'
        and permission_secrets = 'none'
    ),
    constraint frontend_block_catalog_context_shape_check check (jsonb_typeof(context_contract) = 'object'),
    constraint frontend_block_catalog_ui_capabilities_array_check check (jsonb_typeof(ui_capabilities) = 'array')
);

create index if not exists idx_frontend_block_catalog_installation
    on frontend_block_catalog (installation_id);

create index if not exists idx_frontend_block_catalog_provider_code
    on frontend_block_catalog (provider_code, contribution_code);
