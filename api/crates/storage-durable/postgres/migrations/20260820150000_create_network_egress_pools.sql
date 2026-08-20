create table network_egress_pools (
    id uuid primary key,
    scope_id uuid not null,
    display_name text not null check (length(trim(display_name)) > 0),
    selection_strategy text not null check (selection_strategy in ('healthy_first')),
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check (scope_id = '00000000-0000-0000-0000-000000000000')
);

create unique index network_egress_pools_system_display_name_idx
    on network_egress_pools (scope_id, lower(display_name));

create table network_egress_pool_members (
    id uuid primary key,
    pool_id uuid not null references network_egress_pools(id) on delete cascade,
    -- Do not add a foreign key: a removed provider must remain an explicit invalid reference.
    provider_id uuid not null,
    provider_egress_key text not null check (length(trim(provider_egress_key)) > 0),
    enabled boolean not null default true,
    sequence integer not null check (sequence >= 0),
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (pool_id, provider_id, provider_egress_key)
);

create index network_egress_pool_members_pool_sequence_idx
    on network_egress_pool_members (pool_id, sequence asc, id asc);
