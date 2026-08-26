create table if not exists file_storages (
    id uuid primary key,
    code text not null unique,
    title text not null,
    driver_type text not null,
    enabled boolean not null default true,
    is_default boolean not null default false,
    config_json jsonb not null default '{}'::jsonb,
    rule_json jsonb not null default '{}'::jsonb,
    health_status text not null default 'unknown',
    last_health_error text null,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists file_storages_single_default_idx
    on file_storages ((is_default))
    where is_default = true;

create table if not exists file_tables (
    id uuid primary key,
    code text not null unique,
    title text not null,
    scope_kind text not null,
    scope_id uuid not null,
    model_definition_id uuid not null references model_definitions(id),
    bound_storage_id uuid not null references file_storages(id),
    is_builtin boolean not null default false,
    is_default boolean not null default false,
    status text not null default 'active',
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists file_tables_scope_idx
    on file_tables (scope_kind, scope_id, created_at desc);
