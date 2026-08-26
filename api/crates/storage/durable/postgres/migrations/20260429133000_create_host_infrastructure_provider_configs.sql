create table host_infrastructure_provider_configs (
    id uuid primary key,
    installation_id uuid not null references plugin_installations(id) on delete cascade,
    extension_id text not null,
    provider_code text not null,
    config_ref text not null,
    enabled_contracts text[] not null default '{}',
    config_json jsonb not null default '{}'::jsonb,
    status text not null,
    updated_by uuid not null references users(id) on delete restrict,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint host_infra_provider_configs_status_check
        check (status in ('disabled', 'pending_restart', 'active')),
    constraint host_infra_provider_configs_config_ref_check
        check (config_ref like 'secret://system/%'),
    constraint host_infra_provider_configs_unique_provider
        unique (installation_id, provider_code)
);

create index host_infra_provider_configs_contracts_idx
    on host_infrastructure_provider_configs using gin (enabled_contracts);
