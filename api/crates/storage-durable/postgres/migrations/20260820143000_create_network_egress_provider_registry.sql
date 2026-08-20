create table network_egress_providers (
    id uuid primary key,
    scope_id uuid not null,
    installation_id uuid not null unique references plugin_installations(id) on delete restrict,
    provider_code text not null,
    display_name text not null,
    secret_ref text not null check (secret_ref like 'secret://%'),
    lifecycle text not null check (lifecycle in ('draft', 'active', 'disabled')),
    health_status text not null check (health_status in ('unknown', 'healthy', 'unhealthy')),
    last_sync_error text,
    last_synced_at timestamptz,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (scope_id = '00000000-0000-0000-0000-000000000000')
);

create table network_egress_projections (
    provider_id uuid not null references network_egress_providers(id) on delete cascade,
    provider_egress_key text not null,
    display_name text not null,
    region text,
    tags text[] not null default '{}',
    availability text not null check (availability in ('available', 'unavailable')),
    synced_at timestamptz not null,
    primary key (provider_id, provider_egress_key)
);

create index network_egress_providers_health_idx
    on network_egress_providers (lifecycle, health_status, updated_at desc, id asc);
